#![feature(array_map)]

#[macro_use]
mod incl_macro;
mod chat_parse;
mod clone_all;
mod datatypes;
mod global_state;
mod internal_communication;
mod packets;
mod stream_handler;
mod world;

pub use chat_parse::parse as chat_parse;
use global_state::GlobalState;
use serde_json::json;
use slab::Slab;
use std::path::PathBuf;
use std::sync::Arc;
use structopt::StructOpt;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

const SUPPORTED_PROTOCOL_VERSIONS: &[i64] = &[753, 754]; // 1.16.3+
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();

    let favicon = std::fs::read(&opt.favicon)?;

    // parse the player sample to the format minecraft requires
    let mut player_sample = json!([]);
    for line in opt.player_sample.lines() {
        player_sample.as_array_mut().unwrap().push(json!({
            "name": line.to_string(),
            "id": "00000000-0000-0000-0000-000000000000",
        }));
    }

    let global_state = GlobalState {
        description: Arc::new(Mutex::new(chat_parse(opt.description))),
        favicon: Arc::new(Mutex::new(format!(
            "data:image/png;base64,{}",
            base64::encode(favicon)
        ))),
        player_sample: Arc::new(Mutex::new(player_sample)),
        max_players: Arc::new(Mutex::new(opt.max_players)),
        players: Arc::new(Mutex::new(Slab::new())),
        w_login: Arc::new(Mutex::new(world::start(world::login::new()))),
    };

    let listener = TcpListener::bind(("0.0.0.0", opt.port)).await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();

        tokio::spawn(stream_handler::handle_stream(socket, global_state.clone()));
    }
}
