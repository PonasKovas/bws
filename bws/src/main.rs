#![feature(array_map)]
#![feature(box_syntax)]
#![feature(bench_black_box)]
#![feature(backtrace)]
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
use global_state::{GlobalState, InnerGlobalState};
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

fn main() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;

    // When any task panics, exit the whole app
    let (panic_sender, mut panic_receiver) = tokio::sync::mpsc::unbounded_channel();
    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::capture();
        println!("{}\n{}", info, bt);
        let _ = panic_sender.send(());
    }));

    rt.block_on(async move {
        tokio::select! {
            _ = panic_receiver.recv() => {
                // some task panicked
                shutdown().await;
                Ok(())
            },
            _ = tokio::spawn(async_main()) => {
                Ok(())
            }
        }
    })
}

async fn async_main() -> Result<()> {
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

    let state = Arc::new(InnerGlobalState {
        clients: RwLock::new(Slab::new()),
        compression_treshold: OPT.compression_treshold,
        port: OPT.port,
        plugins: plugins::load_plugins()
            .await
            .context("Error loading plugins")?
            .into_iter()
            .map(|(k, v)| (k, RwLock::new(v)))
            .collect(),
    });

    plugins::start_plugins(&state)
        .await
        .context("Error starting plugins")?;

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
