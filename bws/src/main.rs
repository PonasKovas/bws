#![feature(backtrace)]
#![deny(unsafe_op_in_unsafe_fn)]

mod linear_search;
mod plugins;

use anyhow::{bail, Context, Result};
use lazy_static::lazy_static;
pub use linear_search::LinearSearch;
use log::{debug, error, info, warn};
use std::future::pending;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use std::{sync::atomic::AtomicU32, time::Duration};
use structopt::StructOpt;
use tokio::sync::{broadcast, mpsc};

#[derive(Debug, StructOpt, Clone)]
#[structopt(name = "bws", about = "A light-weight and modular minecraft server")]
pub struct Opt {
    /// The port on which to start the server
    #[structopt(short, long, default_value = "25565", env = "PORT")]
    pub port: u16,

    /// The maximum number of bytes before the packet is compressed. Negative means no compression.
    #[structopt(long, default_value = "256", env = "COMPRESSION_TRESHOLD")]
    pub compression_treshold: i32,

    /// If set, the application will not log timestamps. Useful when using with systemd, because it logs timestamps by itself.
    #[structopt(long, env = "DISABLE_TIMESTAMPS")]
    pub disable_timestamps: bool,
}

lazy_static! {
    pub static ref OPT: Opt = Opt::from_args();
}

lazy_static! {
    // a broadcast channel that will go off on a shutdown to let tasks gracefully exit
    static ref GRACEFUL_EXIT_SENDER: broadcast::Sender<()> = broadcast::channel(1).0;
}

lazy_static! {
    // An mpsc that will let anyone from anywhere exit the whole program gracefully
    pub static ref SHUTDOWN: (mpsc::Sender<()>, Mutex<Option<mpsc::Receiver<()>>>) = {
        let (sender, receiver) = mpsc::channel(1);
        (sender, Mutex::new(Some(receiver)))
    };
}

// The number for which to wait on a shutdown before forcefully exiting all remaining tasks
static NUMBER_TO_WAIT_ON_SHUTDOWN: AtomicU32 = AtomicU32::new(0);
// All gracefully exiting tasks will increase this after exiting on a shutdown
static GRACEFULLY_EXITED: AtomicU32 = AtomicU32::new(0);

/// Call to make sure the program doesnt shutdown without waiting for you to exit
/// returns a receiver that goes off on a shutdown so you can exit gracefully
/// and a reference to an atomic integer for you to increase once you're finished
pub fn register_graceful_shutdown() -> (broadcast::Receiver<()>, &'static AtomicU32) {
    NUMBER_TO_WAIT_ON_SHUTDOWN.fetch_add(1, Ordering::SeqCst);

    (GRACEFUL_EXIT_SENDER.subscribe(), &GRACEFULLY_EXITED)
}

/// Call to iniate a clean shutdown of the program
pub fn shutdown() {
    let _ = SHUTDOWN.0.send(());
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(if OPT.disable_timestamps {
            None
        } else {
            Some(Default::default())
        })
        .parse_default_env()
        .init();

    let rt = tokio::runtime::Runtime::new()?;

    // When any task panics, exit the whole app
    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::capture();
        println!("{info}\n{bt}");
        // if this fails, that means the shutdown is on its way already,
        // so we dont care
        let _ = SHUTDOWN.0.send(());
    }));

    let mut shutdown_receiver = SHUTDOWN.1.lock().unwrap().take().unwrap();

    rt.block_on(async move {
        tokio::spawn(async_main());

        tokio::select! {
            _ = shutdown_receiver.recv() => {},
            _ = tokio::signal::ctrl_c() => {},
            // On Unixes, handle SIGTERM too
            _ = async move {
                #[cfg(unix)]
                {
                    let mut sig = tokio::signal::unix::signal(
                        tokio::signal::unix::SignalKind::terminate()
                    ).unwrap();
                    sig.recv().await
                }
                #[cfg(not(unix))]
                {
                    futures::future::pending().await
                }
            } => {},
        }
        // gracefully shutdown

        // tell that we're shutting down to all tasks that need it
        let _ = GRACEFUL_EXIT_SENDER.send(());

        // wait for them
        while GRACEFULLY_EXITED.load(Ordering::SeqCst)
            < NUMBER_TO_WAIT_ON_SHUTDOWN.load(Ordering::SeqCst)
        {
            std::thread::sleep(Duration::from_millis(10));
        }

        std::process::exit(0)
    })
}

async fn async_main() -> Result<()> {
    info!("Loading plugins...");
    let mut plugins = plugins::load_plugins()
        .await
        .context("Error loading plugins")?;

    plugins::start_plugins(&mut plugins)
        .await
        .context("Error starting plugins")?;

    pending().await
}
