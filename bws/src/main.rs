#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unused_imports)]

mod linear_search;
mod plugins;
mod shutdown;
mod vtable;

use anyhow::{bail, Context, Result};
pub use linear_search::LinearSearch;
use log::{debug, error, info, trace, warn};
use once_cell::sync::{Lazy, OnceCell};
pub use shutdown::{shutdown, shutdown_started, wait_for_shutdown};
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use std::{sync::atomic::AtomicU32, time::Duration};
use tokio::sync::{broadcast, mpsc};

fn main() -> Result<()> {
    // Parse the env vars and args that need to be parsed before loading plugins
    let log_use_timestamps = std::env::var_os("BWS_DISABLE_TIMESTAMPS").is_none();

    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(if log_use_timestamps {
            Some(Default::default())
        } else {
            None
        })
        .parse_default_env()
        .init();

    // Attempt to load plugins
    info!("Loading plugins...");
    let plugins = plugins::load_plugins().context("Error loading plugins")?;

    // Start the plugins
    plugins::start_plugins(&plugins).context("Couldn't start plugins")?;

    Ok(())
}
