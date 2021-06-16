use crate::internal_communication as ic;
use anyhow::bail;
use anyhow::{Context, Result};
use futures::FutureExt;
use log::debug;
use protocol::datatypes::*;
use protocol::packets::*;
use slab::Slab;
use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use tokio::sync::{Mutex, RwLock};
use tokio::task::unconstrained;

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
    pub view_distance: Option<i8>,
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
