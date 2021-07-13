use crate::internal_communication as ic;
use anyhow::bail;
use anyhow::{Context, Result};
use futures::FutureExt;
use log::info;
use log::{debug, error, warn};
use protocol::datatypes::*;
use protocol::packets::*;
use serde::{Deserialize, Serialize};
use slab::Slab;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use tokio::sync::{Mutex, RwLock};
use tokio::task::unconstrained;

const BANNED_IPS_FILE: &'static str = "banned.addresses";
const PLAYER_DATA_FILE: &'static str = "player_data.ron";

pub type PStream = Arc<Mutex<PlayerStream>>;

// its gonna be a static
pub struct GlobalState {
    // immutable fields
    pub compression_treshold: i32,
    pub port: u16,
    // mutable
    pub description: Mutex<Chat<'static>>,
    pub favicon: Mutex<String>,
    pub player_sample: Mutex<Vec<StatusPlayerSampleEntry<'static>>>,
    pub max_players: Mutex<i32>,
    pub w_login: ic::WSender,
    pub w_lobby: ic::WSender,
    pub players: RwLock<Slab<Player>>,
    pub player_data: RwLock<HashMap<String, PlayerData>>,
    pub banned_addresses: RwLock<HashSet<IpAddr>>,
}

pub struct Player {
    // Behind an Arc and a Mutex so worlds dont have to lock
    // the whole `players` Slab with write access just to send packets to players
    // they will just clone the arc and store it themselves.
    pub stream: PStream,
    pub username: String,
    pub address: SocketAddr,
    pub uuid: u128,
    pub properties: Vec<PlayerInfoAddPlayerProperty<'static>>,
    pub ping: f32, // in milliseconds
    pub settings: Option<ClientSettings<'static>>,
}

#[derive(Serialize, Deserialize)]
pub struct PlayerData {
    pub permissions: PlayerPermissions,
    // banned: Option<UntilWhatDate>,
    // groups: PlayerGroups,
    // score, statistics...
}

fn is_false(arg: &bool) -> bool {
    !arg
}

#[derive(Serialize, Deserialize)]
pub struct PlayerPermissions {
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub owner: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub edit_lobby: bool,
}

pub struct PlayerStream {
    pub sender: ic::SHInputSender,
    pub receiver: ic::SHOutputReceiver,
    // If None, that means the player has already been disconnected
    pub disconnect: Option<oneshot::Sender<()>>,
}

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("client already disconnected")]
    StreamError,
}

impl PlayerStream {
    pub fn send(&mut self, packet: PlayClientBound<'static>) -> Result<(), StreamError> {
        self.sender
            .send(packet)
            .map_err(|_| StreamError::StreamError)
    }
    /// Returns Err if the player has disconnected
    /// And None, if the player is connected, but no packets in queue
    pub fn try_recv(&mut self) -> Result<Option<PlayServerBound<'static>>, StreamError> {
        // Tries executing the recv() exactly once. If there's a message in the queue it will return it
        // If not, it will also immediatelly return with a None
        let message = match unconstrained(self.receiver.recv()).now_or_never() {
            Some(m) => m,
            None => return Ok(None),
        };

        match message {
            Some(m) => Ok(Some(m)),
            None => Err(StreamError::StreamError),
        }
    }
    pub fn disconnect(&mut self) {
        if let Some(disconnect) = self.disconnect.take() {
            // this returns a Result, with Err meaning that the receiver has already dropped
            // but we don't care, since that just means that the player is already disconnected
            if let Err(_) = disconnect.send(()) {
                debug!("trying to disconnect already disconnect player.");
            }
        }
    }
}

impl GlobalState {
    pub async fn save_player_data(&self) {
        if let Err(e) = self.inner_save_player_data().await {
            error!("Error saving player data: {}", e);
            warn!("Make sure to save it before closing the server, otherwise data might be lost.");
        }
    }
    pub async fn save_banned_ips(&self) {
        if let Err(e) = self.inner_save_banned_ips().await {
            error!("Error saving banned IPs: {}", e);
            warn!("Make sure to save them before closing the server, otherwise changes might be lost.");
        }
    }
    async fn inner_save_player_data(&self) -> Result<()> {
        use tokio::fs::File;
        use tokio::io::AsyncWriteExt;

        let data = ron::ser::to_string_pretty(&*self.player_data.read().await, Default::default())?;

        let mut f = File::create(PLAYER_DATA_FILE)
            .await
            .context(format!("Failed to create {}.", PLAYER_DATA_FILE))?;

        f.write_all(data.as_bytes())
            .await
            .context(format!("Couldn't write to {}", PLAYER_DATA_FILE))?;

        Ok(())
    }
    async fn inner_save_banned_ips(&self) -> Result<()> {
        use tokio::fs::File;
        use tokio::io::AsyncWriteExt;

        let mut f = File::create(BANNED_IPS_FILE)
            .await
            .context(format!("Failed to create {}.", BANNED_IPS_FILE))?;

        let addresses = self.banned_addresses.read().await;
        for ip in addresses.iter() {
            f.write_all(format!("{}\n", &ip).as_bytes())
                .await
                .context(format!("Error writing to {}", BANNED_IPS_FILE))?;
        }

        Ok(())
    }
}

pub fn read_player_data() -> Result<HashMap<String, PlayerData>> {
    if Path::new(PLAYER_DATA_FILE).exists() {
        // read the data
        let mut f = File::open(PLAYER_DATA_FILE)
            .context(format!("Failed to open {}.", PLAYER_DATA_FILE))?;

        let mut data = String::new();

        f.read_to_string(&mut data)
            .context(format!("Error reading {}.", PLAYER_DATA_FILE))?;

        Ok(ron::from_str(&data).context(format!(
            "Error deserializing {}. (bad format)",
            PLAYER_DATA_FILE
        ))?)
    } else {
        info!("Creating {}", PLAYER_DATA_FILE);
        // create the file
        File::create(PLAYER_DATA_FILE)?.write_all("{}".as_bytes())?;

        Ok(HashMap::new())
    }
}

pub fn read_banned_ips() -> Result<HashSet<IpAddr>> {
    let mut addresses = HashSet::new();
    if Path::new(BANNED_IPS_FILE).exists() {
        // read the data
        let f =
            File::open(BANNED_IPS_FILE).context(format!("Failed to open {}.", BANNED_IPS_FILE))?;

        let lines = BufReader::new(f).lines();
        for line in lines {
            let line = line.context(format!("Error reading {}.", BANNED_IPS_FILE))?;

            let ip = IpAddr::from_str(&line).context("Couldn't parse address")?;

            addresses.insert(ip);
        }
    } else {
        info!("Creating {}", BANNED_IPS_FILE);
        // create the file
        File::create(BANNED_IPS_FILE)?;
    }

    Ok(addresses)
}
