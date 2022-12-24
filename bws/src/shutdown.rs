use once_cell::sync::{Lazy, OnceCell};

// A broadcast channel that will go off when the program needs to shutdown
// And can be used to initiate a shutdown
pub static SHUTDOWN: Lazy<(flume::Sender<()>, flume::Receiver<()>)> =
    Lazy::new(|| flume::bounded(1));

/// Call to iniate a clean shutdown of the program
pub fn shutdown() {
    // if this fails, that means the shutdown is on its way already,
    // so we dont care
    let _ = SHUTDOWN.0.try_send(());
}

/// Blocks the thread until a shutdown signal is received
pub fn wait_for_shutdown() {
    // can't fail, since there's always at least one sender in the same static
    SHUTDOWN.1.recv().unwrap();
}

/// Future completes when a shutdown signal is received
pub async fn wait_for_shutdown_async() {
    // can't fail, since there's always at least one sender in the same static
    SHUTDOWN.1.recv_async().await.unwrap();
}

/// Returns true if shutdown already initiated
pub fn shutdown_started() -> bool {
    SHUTDOWN.1.try_recv().is_ok()
}
