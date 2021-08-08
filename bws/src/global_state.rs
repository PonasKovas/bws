use crate::internal_communication as ic;
use anyhow::bail;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures::FutureExt;
use log::info;
use log::{debug, error, warn};
use protocol::commands_builder::CommandsBuilder;
use protocol::packets::*;
use protocol::{command, datatypes::*};
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
use tokio::task::{unconstrained, JoinHandle};

const BANNED_IPS_FILE: &str = "banned.addresses";
const PLAYER_DATA_FILE: &str = "player_data.ron";

pub type PStream = Arc<Mutex<PlayerStream>>;

pub type GlobalState = Arc<InnerGlobalState>;

pub struct InnerGlobalState {
    // fields that shouldn't be mutated at runtime
    pub compression_treshold: i32,
    pub port: u16,
    // fields that can be mutated at runtime freely
    pub description: Mutex<Chat<'static>>,
    pub favicon: Mutex<String>,
    pub player_sample: Mutex<Vec<StatusPlayerSampleEntry<'static>>>,
    pub max_players: Mutex<i32>,
    pub players: RwLock<Slab<Player>>,
    pub player_data: RwLock<HashMap<String, PlayerData>>,
    pub banned_addresses: RwLock<HashSet<IpAddr>>,
    pub plugins: crate::plugins::Plugins,
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
    pub logged_in: bool,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct PlayerData {
    #[serde(default)]
    pub permissions: PlayerPermissions,
    // until when, reason, and the issuer of the ban, if any
    #[serde(default)]
    pub banned: Option<(DateTime<Utc>, String, Option<String>)>,
    // groups: PlayerGroups,
    // score, statistics...
}

fn is_false(arg: &bool) -> bool {
    !arg
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct PlayerPermissions {
    /// Has a fat OWNER prefix and can execute the most powerful server commands
    /// Can set the admin permission for anyone
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub owner: bool,
    /// Has a badass prefix and is very scary
    /// can set any permissions except for "owner" and "admin" for anyone (again, except for owners and admins) (add/remove)
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub admin: bool,
    /// Can use the /editmode command in the lobby
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub edit_lobby: bool,
    /// Can ban (and unban) usernames, permanently or for a specific duration
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub ban_usernames: bool,
    /// Can ban (and unban) IPs
    #[serde(default)]
    #[serde(skip_serializing_if = "is_false")]
    pub ban_ips: bool,
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

impl PlayerPermissions {
    pub fn extend_permission_commands(&self, commands: &mut CommandsBuilder) {
        if self.ban_ips {
            commands.extend(command!(
                ("banip", literal => [
                    (X "ip address", argument (String: SingleWord) => []),
                ]),
                ("unbanip", literal => [
                    (X "ip address", argument (String: SingleWord) => []),
                ]),
            ));
        }
        if self.ban_usernames {
            commands.extend(command!(
                ("ban", literal => [
                    ("username", argument (String: SingleWord) suggestions=AskServer => [
                        (X "duration in minutes", argument (Integer: Some(0), None) => [
                            (X "reason", argument (String: GreedyPhrase) => [])
                        ])
                    ]),
                ]),
                ("unban", literal => [
                    (X "username", argument (String: SingleWord) suggestions=AskServer => []),
                ]),
            ));
        }
        if self.admin {
            commands.extend(command!(
                ("setperm", literal => [
                    ("username", argument (String: SingleWord) suggestions=AskServer => [
                        ("permission", argument (String: SingleWord) suggestions=AskServer => [
                            (X "value", argument (Bool) => [])
                        ])
                    ]),
                ]),
                (X "perms", literal => [
                    (X "username", argument (String: SingleWord) suggestions=AskServer => [])
                ]),
            ));
        }
    }
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
            if disconnect.send(()).is_err() {
                debug!("trying to disconnect already disconnected player.");
            }
        }
    }
}

impl InnerGlobalState {
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
