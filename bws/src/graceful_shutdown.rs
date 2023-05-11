use crate::cli;
use once_cell::sync::OnceCell;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Condvar, Mutex,
    },
    time::Duration,
};
use tracing::warn;

pub static SHUTDOWN: OnceCell<()> = OnceCell::new();

// Count of guards currently alive
//
// When the shutdown channel is fired, the program will attempt to wait until the counter reaches 0,
// unless it takes too long
static WAIT_FOR: (Mutex<u64>, Condvar) = (Mutex::new(0), Condvar::new());

pub struct GracefulShutdownGuard {
    _private: (),
}

impl GracefulShutdownGuard {
    pub fn new() -> Self {
        // increase WAIT_FOR counter
        *WAIT_FOR.0.lock().unwrap() += 1;

        Self { _private: () }
    }
}

impl Drop for GracefulShutdownGuard {
    fn drop(&mut self) {
        // decrease WAIT_FOR counter
        let mut counter_lock = WAIT_FOR.0.lock().unwrap();
        *counter_lock -= 1;

        // notify the waiting thread if the counter has reached 0
        if *counter_lock == 0 {
            WAIT_FOR.1.notify_all();
        }
    }
}

/// Creates a guard that prevents shutdown while it's in scope, until shutdown timer ends
pub fn guard() -> GracefulShutdownGuard {
    GracefulShutdownGuard::new()
}

/// Initiates a shutdown
pub fn shutdown() {
    // discarding result, since it doesn't matter if a shutdown has already been issued,
    let _ = SHUTDOWN.set(());
}

/// Blocks the thread until a shutdown is initiated
pub fn wait_for_shutdown() {
    SHUTDOWN.wait();
}

/// Waits until all guards dropped or timeout
pub fn wait_for_guards() {
    static TIMED_OUT: AtomicBool = AtomicBool::new(false);

    let timeout = cli::OPT.shutdown_timeout;

    if timeout >= 0 {
        let timed_out_ref = &TIMED_OUT;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(timeout as u64));

            timed_out_ref.store(true, Ordering::SeqCst);

            warn!("Graceful shutdown timed out. Shutting down forcefully.");

            WAIT_FOR.1.notify_all();
        });
    }

    // block the thread until counter reaches 0 or timeout occurs
    let mut counter = WAIT_FOR.0.lock().unwrap();
    while *counter > 0 && !TIMED_OUT.load(Ordering::SeqCst) {
        counter = WAIT_FOR.1.wait(counter).unwrap();
    }
}
