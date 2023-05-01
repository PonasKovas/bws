#![deny(unsafe_op_in_unsafe_fn)]

mod linear_search;
mod plugins;
mod vtable;

use anyhow::{Context, Result};
pub use linear_search::LinearSearch;
use once_cell::sync::OnceCell;
use std::sync::{
    mpsc::{self, SyncSender},
    Condvar, Mutex,
};

static END_PROGRAM: OnceCell<SyncSender<()>> = OnceCell::new();
// static END_PROGRAM: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

fn main() -> Result<()> {
    // true if BWS_DISABLE_TIMESTAMPS set to anything other than 0
    let log_disable_timestamps =
        std::env::var_os("BWS_DISABLE_TIMESTAMPS").map_or(false, |s| s != "0");

    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(if log_disable_timestamps {
            None
        } else {
            Some(Default::default())
        })
        .parse_default_env()
        .init();

    // Attempt to load plugins
    let plugins = plugins::load_plugins().context("Error loading plugins")?;
    plugins::PLUGINS.set(plugins).unwrap();

    // Initialize the plugins
    plugins::init_plugins().context("Couldn't initialize plugins")?;

    // Now parse env vars and args
    let matches = vtable::cmd::CLAP_COMMAND_BUILDER
        .lock()
        .expect("Couldn't lock the Clap command builder mutex after initializing plugins")
        .as_mut()
        .unwrap()
        .get_matches_mut();
    vtable::cmd::CLAP_MATCHES.set(matches).unwrap();

    // Start the plugins
    plugins::start_plugins().context("Couldn't start plugins")?;

    let (sender, receiver) = mpsc::sync_channel(0);
    END_PROGRAM.set(sender);

    // block the thread until notification on END_PROGRAM is received
    let _ = receiver.recv();

    Ok(())
}
