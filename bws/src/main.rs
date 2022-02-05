#![feature(backtrace)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unused_imports)]

mod linear_search;
mod opt;
mod plugins;
mod shutdown;

use abi_stable::external_types::RRwLock;
use abi_stable::std_types::{RArc, RVec};
use anyhow::{bail, Context, Result};
use bws_plugin_interface::global_state::GlobalState;
pub use linear_search::LinearSearch;
use log::{debug, error, info, warn};
use once_cell::sync::{Lazy, OnceCell};
pub use shutdown::{shutdown, shutdown_started, wait_for_shutdown};
use std::future::pending;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use std::{sync::atomic::AtomicU32, time::Duration};
use tokio::sync::{broadcast, mpsc};

fn main() -> Result<()> {
    let opt = opt::collect_opt();

    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(if opt.is_present("disable_timestamps") {
            None
        } else {
            Some(Default::default())
        })
        .parse_default_env()
        .init();

    // Start the tokio runtime, for handling network IO
    let rt = tokio::runtime::Runtime::new().context("Couldn't start tokio runtime")?;

    // When any task panics, exit the whole app
    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::capture();
        println!("{info}\n{bt}");

        shutdown();
    }));

    info!("Loading plugins...");
    let plugins = plugins::load_plugins().context("Error loading plugins")?;

    // Construct the global state, which may be needed when starting plugins already
    let global_state: RArc<RRwLock<GlobalState>> = RArc::new(RRwLock::new(GlobalState {
        plugins: plugins.into_iter().map(|p| RArc::new(p)).collect(),
    }));

    plugins::start_plugins(&global_state).context("Couldn't start plugins")?;
    info!("All plugins loaded.");
    info!("Starting TCP listener");

    rt.block_on(async move {
        tokio::spawn(net());

        tokio::select! {
            _ = wait_for_shutdown() => {},
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

        shutdown();

        Ok(())
    })
}

async fn net() -> Result<()> {
    tokio::select! {
        _ = wait_for_shutdown() => {},
        _ = async move {
            // do work here
        } => {},
    }

    Ok(())
}
