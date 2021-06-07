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
use global_state::GlobalState;
use internal_communication::SHBound;
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use packets::ClientBound;
use serde_json::json;
use slab::Slab;
use std::path::PathBuf;
use std::sync::Arc;
use structopt::StructOpt;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

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
        let mut player_sample = json!([]);
        for line in opt.player_sample.lines() {
            player_sample.as_array_mut().unwrap().push(json!({
                "name": line.to_string(),
                "id": "00000000-0000-0000-0000-000000000000",
            }));
        }
        GlobalState {
            description: Arc::new(Mutex::new(chat_parse::parse_json(opt.description))),
            favicon: Arc::new(Mutex::new(format!(
                "data:image/png;base64,{}",
                base64::encode(favicon)
            ))),
            player_sample: Arc::new(Mutex::new(player_sample)),
            max_players: Arc::new(Mutex::new(opt.max_players)),
            players: Arc::new(Mutex::new(Slab::new())),
            w_login: world::start(match world::login::new() {
                Ok(w) => w,
                Err(e) => {
                    error!("Error creating login world: {}", e);
                    std::process::exit(1);
                },
            }),
            w_lobby: world::start(match world::lobby::new() {
                Ok(w) => w,
                Err(e) => {
                    error!("Error creating lobby world: {}", e);
                    std::process::exit(1);
                },
            }),
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

    // properly shutdown in case of SIGINT
    ctrlc::set_handler({
        clone_all!(join_handles);
        move || {
            shutdown(join_handles.clone());
        }
    })
    .context("Error setting SIGINT handler")?;

    info!("Listening on port {}", GLOBAL_STATE.port);

    let listener = TcpListener::bind(("0.0.0.0", GLOBAL_STATE.port))
        .await
        .context("Couldn't bind the listener")?;

    loop {
        let (socket, _) = listener.accept().await.unwrap();

        join_handles
            .lock()
            .unwrap()
            .push(tokio::spawn(stream_handler::handle_stream(socket)));
    }
}

fn shutdown(sh_handles: Arc<std::sync::Mutex<Vec<JoinHandle<()>>>>) {
    for player in &*futures::executor::block_on(GLOBAL_STATE.players.lock()) {
        let _ = (player.1)
            .sh_sender
            .send(SHBound::Packet(ClientBound::PlayDisconnect(chat_parse(
                "§4§lThe server has shutdown.",
            ))));
        let _ = (player.1).sh_sender.send(SHBound::Disconnect);
    }

    // todo also shutdown worlds

    info!("Exiting...");

    for handle in &mut *sh_handles.lock().unwrap() {
        let _ = futures::executor::block_on(handle);
    }

    std::process::exit(0);
}
