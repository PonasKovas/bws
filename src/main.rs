mod chat_parse;
mod clone_all;
mod datatypes;
mod global_state;
mod internal_communication;
mod packets;
mod stream_handler;

pub use chat_parse::parse as chat_parse;
use global_state::GlobalState;
use serde_json::json;
use slab::Slab;
use std::path::PathBuf;
use std::sync::Arc;
use structopt::StructOpt;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

const SUPPOERTED_PROTOCOL_VERSIONS: &[u32] = &[753];
const VERSION_NAME: &str = "1.16.3 BWS";

#[derive(Debug, StructOpt, Clone)]
#[structopt(name = "bws", about = "Hello this is the description!")]
pub struct Opt {
    /// The port on which to start the server
    #[structopt(short, long, default_value = "25565", env = "PORT")]
    pub port: u16,

    /// The favicon of the server to display in the server list
    #[structopt(short, long, default_value = "assets/favicon.png", env = "FAVICON")]
    pub favicon: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();

    let favicon = std::fs::read(&opt.favicon)?;

    // TODO: read the initial description and player sample from a config or as an argument/env var

    let global_state = GlobalState {
        description: Arc::new(Mutex::new(chat_parse(
            "§l                   §4✕ §cNO JAVA §4✕\n                  §2✓ §a100% Rust §2✓"
                .to_string(),
        ))),
        favicon: Arc::new(Mutex::new(format!(
            "data:image/png;base64,{}",
            base64::encode(favicon)
        ))),
        player_sample: Arc::new(Mutex::new(json!([
            {
                "name": "§2§lMade from scratch with §a§l100% Rust",
                "id": "00000000-0000-0000-0000-000000000000",
            },
            {
                "name": "",
                "id": "00000000-0000-0000-0000-000000000000",
            },
            {
                "name": "By §6§lPonas §b© §d2021",
                "id": "00000000-0000-0000-0000-000000000000",
            },
            {
                "name": "",
                "id": "00000000-0000-0000-0000-000000000000",
            },
        ]))),
        max_players: Arc::new(Mutex::new(0)),
        players: Arc::new(Mutex::new(Slab::new())),
    };

    // // a broadcast channel for sending instructions to network tasks
    // // the format is (client_id, instruction)
    // // where client id is assigned on connect
    // let (n_sender, n_receiver) =
    //     broadcast::channel::<(ClientId, stream_handler::Instruction)>(4096);

    // // a broadcast channel for sending instructions to worlds
    // // the format is (world_id, instruction)
    // // where world id is assigned on world creation (0 is lobby)
    // let (w_sender, w_receiver) = broadcast::channel::<(WorldId, world::Instruction)>(1024);

    let listener = TcpListener::bind(("0.0.0.0", opt.port)).await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();

        // // we can just assign incremental IDs, and we shouldnt ever run out
        // let id = next_id;
        // next_id = next_id.wrapping_add(1);

        tokio::spawn(stream_handler::handle_stream(socket, global_state.clone()));
    }
}
