use abi_stable::external_types::RRwLock;
use abi_stable::std_types::{RArc, RVec};
use anyhow::{bail, Context, Result};
use bws_plugin_interface::global_state::GlobalState;
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

pub async fn wait_for_shutdown() {
    // can't fail, since there's always at least one sender in the same static
    SHUTDOWN.1.recv_async().await.unwrap();
}

/// Returns true if shutdown already initiated
pub fn shutdown_started() -> bool {
    SHUTDOWN.1.try_recv().is_ok()
}
