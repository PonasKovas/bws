#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unused_imports)]

mod linear_search;
mod plugins;
mod vtable;

use anyhow::{bail, Context, Result};
use clap::Command;
pub use linear_search::LinearSearch;
use log::{debug, error, info, trace, warn};
use once_cell::sync::{Lazy, OnceCell};
use std::io::BufRead;
use std::io::Write;
use std::ptr::null;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::Condvar;
use std::sync::Mutex;
use std::{sync::atomic::AtomicU32, time::Duration};
use tokio::sync::{broadcast, mpsc};

static END_PROGRAM: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

fn main() -> Result<()> {
    // Parse the env vars and args that need to be parsed before loading plugins
    // false if BWS_DISABLE_TIMESTAMPS set to anything other than 0 and false
    let log_use_timestamps =
        !std::env::var_os("BWS_DISABLE_TIMESTAMPS").map_or(false, |s| s != "0" && s != "false");

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

    // block the thread until notification on END_PROGRAM is received
    let mut end_program = END_PROGRAM.0.lock().unwrap();
    while !*end_program {
        end_program = END_PROGRAM.1.wait(end_program).unwrap();
    }

    Ok(())
}
