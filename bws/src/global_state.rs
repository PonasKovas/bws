use crate::internal_communication as ic;
use anyhow::bail;
use anyhow::{Context, Result};
use futures::FutureExt;
use log::debug;
use protocol::datatypes::*;
use protocol::packets::*;
use serde::{Deserialize, Serialize};
use slab::Slab;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
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
    permissions: PlayerPermissions,
    // banned: Option<UntilWhatDate>,
    // groups: PlayerGroups,
    // score, statistics...
}

#[derive(Serialize, Deserialize)]
pub struct PlayerPermissions {
    owner: bool,
    edit_lobby: bool,
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
        // create the file
        File::create(PLAYER_DATA_FILE)?;

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
        // create the file
        File::create(BANNED_IPS_FILE)?;
    }

    Ok(addresses)
}
