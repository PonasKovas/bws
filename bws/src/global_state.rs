use crate::internal_communication as ic;
use crate::internal_communication::SHSender;
use slab::Slab;
use std::net::SocketAddr;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{Mutex, RwLock};

pub struct Player {
    pub username: String,
    pub address: SocketAddr,
    pub sh_sender: SHSender,
    pub view_distance: Option<i8>,
}

// This is the global state that will be available on all threads
// By itself it holds no mutable data so it can be cloned freely
pub struct GlobalState {
    pub description: Mutex<serde_json::Value>,
    pub favicon: Mutex<String>,
    pub player_sample: Mutex<serde_json::Value>,
    pub max_players: Mutex<i32>,
    pub players: Mutex<Slab<Player>>,
    pub w_login: ic::WSender,
    pub w_lobby: ic::WSender,
    pub compression_treshold: i32,
    pub port: u16,
}
