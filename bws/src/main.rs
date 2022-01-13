#![feature(backtrace)]
#![deny(unsafe_op_in_unsafe_fn)]

mod linear_search;
mod plugins;

use std::time::Duration;

pub use linear_search::LinearSearch;

use anyhow::{bail, Context, Result};
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use structopt::StructOpt;

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
    static ref OPT: Opt = Opt::from_args();
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
                Ok(())
            },
            _ = tokio::spawn(async_main()) => {
                Ok(())
            }
        }
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

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            Ok(())
        },
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
        } => {
            Ok(())
        },
        // _ = run(state.clone()) => {
        //     Ok(())
        // },
    }
}
