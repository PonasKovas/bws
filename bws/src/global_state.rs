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

pub type CStream = Arc<Mutex<ClientStream>>;

pub type GlobalState = Arc<InnerGlobalState>;

pub struct InnerGlobalState {
    pub compression_treshold: i32,
    pub port: u16,
    pub clients: RwLock<Slab<Client>>,
    pub plugins: HashMap<String, RwLock<crate::plugins::Plugin>>,
}

pub struct Client {
    // Behind an Arc and a Mutex so worlds dont have to lock
    // the whole `players` Slab with write access just to send packets to players
    // they will just clone the arc and store it themselves.
    pub stream: CStream,
    pub address: SocketAddr,
    /// Some if the client is in Play state
    pub play_data: Option<PlayData>,
}

pub struct PlayData {
    pub username: String,
    pub uuid: u128,
    pub properties: Vec<PlayerInfoAddPlayerProperty<'static>>,
    pub ping: f32, // in milliseconds
    pub settings: Option<ClientSettings<'static>>,
}

pub struct ClientStream {
    pub sender: ic::SHInputSender,
    pub receiver: ic::SHOutputReceiver,
    // If None, that means the client has already disconnected
    pub disconnect: Option<oneshot::Sender<()>>,
}

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("client already disconnected")]
    StreamError,
}

impl ClientStream {
    pub fn send(&mut self, packet: ClientBound<'static>) -> Result<(), StreamError> {
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
