#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unused_imports)]

mod linear_search;
mod plugins;
mod vtable;

use anyhow::{bail, Context};
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

fn main() -> Result<(), ()> {
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
    let plugins = match plugins::load_plugins() {
        Ok(p) => p,
        Err(e) => {
            error!("Error loading plugins: {e}");
            return Err(());
        }
    };
    vtable::plugin_api::PLUGINS.set(plugins).unwrap();

    // Initialize the plugins
    if let Err(e) = plugins::init_plugins() {
        error!("Couldn't initialize plugins: {e}");
        return Err(());
    }

    // Now parse env vars and args
    let matches = vtable::cmd::CLAP_COMMAND_BUILDER
        .lock()
        .expect("Couldn't lock the Clap command builder mutex after initializing plugins")
        .as_mut()
        .unwrap()
        .get_matches_mut();
    vtable::cmd::CLAP_MATCHES.set(matches).unwrap();

    // Fire the "start" event
    let start_event_id = vtable::get_event_id("start".into());
    if !vtable::fire_event(start_event_id, null()) {
        error!("Couldn't start BWS");
        return Err(());
    }

    // block the thread until notification on END_PROGRAM is received
    let mut end_program = END_PROGRAM.0.lock().unwrap();
    while !*end_program {
        end_program = END_PROGRAM.1.wait(end_program).unwrap();
    }

    Ok(())
}
