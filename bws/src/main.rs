#![feature(backtrace)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unused_imports)]

mod linear_search;
mod opt;
mod plugins;

use anyhow::{bail, Context, Result};
pub use linear_search::LinearSearch;
use log::{debug, error, info, warn};
use once_cell::sync::{Lazy, OnceCell};
use std::future::pending;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use std::{sync::atomic::AtomicU32, time::Duration};
use tokio::sync::{broadcast, mpsc};

// An broadcast channel that will go off when the program needs to shutdown
// And can be used to initiate a shutdown
pub static SHUTDOWN: Lazy<(flume::Sender<()>, flume::Receiver<()>)> =
    Lazy::new(|| flume::bounded(1));

/// Call to iniate a clean shutdown of the program
pub fn shutdown() {
    // if this fails, that means the shutdown is on its way already,
    // so we dont care
    let _ = SHUTDOWN.0.try_send(());
}

pub async fn wait_for_shutdown_async() {
    // can't fail, since there's always at least one sender in the same static
    SHUTDOWN.1.recv_async().await.unwrap();
}

pub fn wait_for_shutdown() {
    // can't fail, since there's always at least one sender in the same static
    SHUTDOWN.1.recv().unwrap();
}

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
    let mut plugins = plugins::load_plugins().context("Error loading plugins")?;

    rt.block_on(async move {
        tokio::spawn(net());

        tokio::select! {
            _ = wait_for_shutdown_async() => {},
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
        _ = wait_for_shutdown_async() => {},
        _ = async move {
            // do work here
        } => {},
    }

    Ok(())
}
