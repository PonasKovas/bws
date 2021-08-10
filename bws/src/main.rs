#![feature(array_map)]
#![feature(box_syntax)]
#![feature(bench_black_box)]
#![deny(unused_must_use)]
// while developing (TODO remove)
#![allow(unused_imports)]

#[macro_use]
mod incl_macro;
mod collision;
mod data;
mod global_state;
mod internal_communication;
// mod map;
mod plugins;
mod shared;
// mod stream_handler;
// mod world;

use anyhow::{Context, Result};
use futures::select;
use futures::FutureExt;
use global_state::{read_banned_ips, read_player_data, GlobalState, InnerGlobalState};
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use protocol::datatypes::chat_parse::parse as chat_parse;
use protocol::datatypes::StatusPlayerSampleEntry;
use protocol::packets::PlayClientBound;
use serde_json::json;
use slab::Slab;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use structopt::StructOpt;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

const SUPPORTED_PROTOCOL_VERSIONS: &[i32] = &[753, 754]; // 1.16.3+
const VERSION_NAME: &str = "1.16.5 BWS";
const ABI_VERSION: u32 = 0;

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

    /// If set, the application will not log timestamps. Useful when using with systemd, because it logs timestamps itself.
    #[structopt(long, env = "DISABLE_TIMESTAMPS")]
    pub disable_timestamps: bool,
}

lazy_static! {
    static ref OPT: Opt = Opt::from_args();
}

// #[tokio::main]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(if OPT.disable_timestamps {
            None
        } else {
            Some(Default::default())
        })
        .parse_default_env()
        .init();

    info!("Initializing...");

    let state = Arc::new({
        let favicon = match std::fs::read(&OPT.favicon) {
            Ok(f) => f,
            Err(e) => {
                error!("Couldn't load the favicon ({:?})! {}", OPT.favicon, e);
                warn!("Falling back to the default embedded favicon!");

                incl!("assets/favicon.png").to_vec()
            }
        };

        // parse the player sample to the format minecraft requires
        let mut player_sample = Vec::new();
        for line in OPT.player_sample.lines() {
            player_sample.push(StatusPlayerSampleEntry::new(line.to_owned().into()));
        }
        InnerGlobalState {
            description: Mutex::new(chat_parse(OPT.description.clone())),
            favicon: Mutex::new(format!("data:image/png;base64,{}", base64::encode(favicon))),
            player_sample: Mutex::new(player_sample),
            max_players: Mutex::new(OPT.max_players),
            players: RwLock::new(Slab::new()),
            compression_treshold: OPT.compression_treshold,
            port: OPT.port,
            player_data: RwLock::new(read_player_data().context("Error reading player data")?),
            banned_addresses: RwLock::new(
                read_banned_ips().context("Error reading banned addresses")?,
            ),
            plugins: plugins::load_plugins()
                .await
                .context("Error loading plugins")?,
        }
    });
    lazy_static::initialize(&data::ITEMS_TO_BLOCKS);

    tokio::select! {
        _ = signal::ctrl_c() => {
            shutdown().await;
            Ok(())
        },
        // On Unixes, handle SIGTERM too
        _ = async move {
            #[cfg(unix)]
            {
                let mut sig = signal::unix::signal(signal::unix::SignalKind::terminate()).unwrap();
                sig.recv().await
            }
            #[cfg(not(unix))]
            {
                futures::future::pending().await
            }
        } => {
            shutdown().await;
            Ok(())
        },
        _ = run(state.clone()) => {
            Ok(())
        },
    }
}

async fn run(state: GlobalState) -> Result<()> {
    info!("Listening on port {}", state.port);

    let listener = TcpListener::bind(("0.0.0.0", state.port))
        .await
        .context("Couldn't bind the listener")?;

    loop {
        let (socket, _) = listener.accept().await.unwrap();

        // tokio::spawn(stream_handler::handle_stream(socket));
    }
}

async fn shutdown() -> ! {
    info!("Exiting...");

    std::process::exit(0);
}
