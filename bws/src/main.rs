#![feature(array_map)]
#![deny(unused_must_use)]
// while developing (TODO remove)
#![allow(unused_imports)]

#[macro_use]
mod incl_macro;
mod chat_parse;
mod clone_all;
#[allow(dead_code)]
mod datatypes;
mod global_state;
mod internal_communication;
#[allow(dead_code)]
mod packets;
mod stream_handler;
mod world;

use anyhow::{Context, Result};
pub use chat_parse::parse as chat_parse;
use futures::select;
use futures::FutureExt;
use global_state::GlobalState;
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use packets::ClientBound;
use serde_json::json;
use slab::Slab;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use structopt::StructOpt;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::datatypes::StatusPlayerSampleEntry;
use crate::datatypes::StatusPlayers;
use crate::datatypes::StatusResponse;
use crate::datatypes::StatusVersion;

const SUPPORTED_PROTOCOL_VERSIONS: &[i32] = &[753, 754]; // 1.16.3+
const VERSION_NAME: &str = "1.16 BWS";

#[derive(Debug, StructOpt, Clone)]
#[structopt(name = "bws", about = "Hello this is the description!")]
pub struct Opt {
    /// The port on which to start the server
    #[structopt(short, long, default_value = "25565", env = "PORT")]
    pub port: u16,

    /// The favicon of the server to display in the server list
    #[structopt(short, long, default_value = "assets/favicon.png", env = "FAVICON")]
    pub favicon: PathBuf,

    /// The server description shown in the server list
    #[structopt(short, long, default_value = "§aA BWS server", env = "DESCRIPTION")]
    pub description: String,

    /// The player sample shown in the server list
    #[structopt(long, default_value = "§c1.16\n§aBWS", env = "PLAYER_SAMPLE")]
    pub player_sample: String,

    /// The maximum number of players allowed on the server, if zero or negative, no limit is enforced
    #[structopt(short, long, default_value = "0", env = "MAX_PLAYERS")]
    pub max_players: i32,

    /// The maximum number of bytes before the packet is compressed. Negative means no compression.
    #[structopt(long, default_value = "256", env = "COMPRESSION_TRESHOLD")]
    pub compression_treshold: i32,
}

lazy_static! {
    static ref GLOBAL_STATE: GlobalState = {
        let opt = Opt::from_args();

        let favicon = match std::fs::read(&opt.favicon) {
            Ok(f) => f,
            Err(e) => {
                error!("Couldn't load the favicon ({:?})! {}", opt.favicon, e);
                std::process::exit(1);
            }
        };

        // parse the player sample to the format minecraft requires
        let mut player_sample = Vec::new();
        for line in opt.player_sample.lines() {
            player_sample.push(StatusPlayerSampleEntry{
                name: line.to_string(),
                id: "00000000-0000-0000-0000-000000000000".to_string(),
            });
        }
        GlobalState {
            description: Mutex::new(chat_parse::parse(opt.description)),
            favicon: Mutex::new(format!(
                "data:image/png;base64,{}",
                base64::encode(favicon)
            )),
            player_sample: Mutex::new(player_sample),
            max_players: Mutex::new(opt.max_players),
            players: RwLock::new(Slab::new()),
            w_login: match world::login::start() {
                Ok(w) => w,
                Err(e) => {
                    error!("Error creating login world: {}", e);
                    std::process::exit(1);
                },
            },
            w_lobby: world::lobby::start(),
            compression_treshold: opt.compression_treshold,
            port: opt.port,
        }
    };
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    lazy_static::initialize(&GLOBAL_STATE);

    let join_handles = Arc::new(std::sync::Mutex::new(Vec::new()));

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            shutdown(&join_handles.clone()).await;
            Ok(())
        },
        _ = run(&join_handles) => {
            Ok(())
        },
    }
}

async fn run(join_handles: &std::sync::Mutex<Vec<JoinHandle<()>>>) -> Result<()> {
    info!("Listening on port {}", GLOBAL_STATE.port);

    let listener = TcpListener::bind(("0.0.0.0", GLOBAL_STATE.port))
        .await
        .context("Couldn't bind the listener")?;

    loop {
        let (socket, _) = listener.accept().await.unwrap();

        let mut lock = join_handles.lock().unwrap();
        // cleanup already finished handles
        for handle in (0..lock.len()).rev() {
            if let Some(_) = tokio::task::unconstrained(&mut lock[handle]).now_or_never() {
                lock.remove(handle);
            }
        }
        lock.push(tokio::spawn(stream_handler::handle_stream(socket)));
    }
}

async fn shutdown(sh_handles: &std::sync::Mutex<Vec<JoinHandle<()>>>) -> ! {
    let message = chat_parse("§4§lThe server has shutdown.");
    for player in &*GLOBAL_STATE.players.read().await {
        let mut stream = (player.1).stream.lock().await;
        let _ = stream.send(ClientBound::PlayDisconnect(message.clone()));
        stream.disconnect();
    }

    // todo also shutdown worlds

    info!("Exiting...");

    // wait for all stream handler threads with a timeout of 1 second
    tokio::select! {
        _ = async move {
            for handle in &mut *sh_handles.lock().unwrap() {
                let _ = handle.await;
            }
        } => {},
        _ = tokio::time::sleep(Duration::from_secs(1)) => {}
    }

    std::process::exit(0);
}