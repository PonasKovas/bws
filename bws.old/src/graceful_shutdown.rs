use once_cell::sync::OnceCell;
use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};
use tokio::sync::Notify;
use tracing::warn;

/// A graceful shutdown system. Cloning creates a new handle to the same system.
#[derive(Debug, Clone)]
pub struct ShutdownSystem {
    initiate: Arc<(OnceCell<()>, Notify)>,
    active_guards: Arc<(Mutex<u64>, Condvar)>,
}

/// Prevents a shutdown while in scope, unless forcuful shutdown timeout is reached
pub struct GracefulShutdownGuard<'a> {
    system: &'a ShutdownSystem,
}

impl ShutdownSystem {
    /// Constructs a new [`ShutdownSystem`]
    pub fn new() -> Self {
        Self {
            initiate: Arc::new((OnceCell::new(), Notify::new())),
            active_guards: Arc::new((Mutex::new(0), Condvar::new())),
        }
    }
    /// Creates a guard that prevents shutdown while it's in scope, or until a timeout is reached
    pub fn guard(&self) -> GracefulShutdownGuard {
        // increase counter
        *self.active_guards.0.lock().unwrap() += 1;

        GracefulShutdownGuard { system: self }
    }
    /// Initiates a shutdown
    pub fn shutdown(&self) {
        // discard because we don't care if shutdown already initiated
        let _ = self.initiate.0.set(());
        // Notify any async waiters
        self.initiate.1.notify_waiters();
    }
    /// Blocks the thread until a shutdown is initiated
    pub fn blocking_wait_for_shutdown(&self) {
        self.initiate.0.wait();
    }
    /// Waits for the shutdown initiation asynchronously
    pub async fn wait_for_shutdown(&self) {
        self.initiate.1.notified().await;
    }
    /// Blocks the thread until there's no more active guards or the timeout is reached
    ///
    /// This is meant to be called after the shutdown is initiated, to give some time to clean up
    /// and then terminate the program.
    ///
    /// `timeout` is in milliseconds.
    pub fn blocking_wait_for_guards(&self, timeout: Option<u64>) {
        let counter = self.active_guards.0.lock().unwrap();
        if let Some(timeout) = timeout {
            let (_counter, timed_out) = self
                .active_guards
                .1
                .wait_timeout_while(counter, Duration::from_millis(timeout), |counter| {
                    *counter > 0
                })
                .unwrap();

            if timed_out.timed_out() {
                warn!("Graceful shutdown timed out. Shutting down forcefully.");
            }
        } else {
            let _counter = self
                .active_guards
                .1
                .wait_while(counter, |counter| *counter > 0)
                .unwrap();
        }
    }
}

impl<'a> Drop for GracefulShutdownGuard<'a> {
    fn drop(&mut self) {
        // decrease guards counter
        let mut counter_lock = self.system.active_guards.0.lock().unwrap();
        *counter_lock -= 1;

        // notify anyone waiting if the counter has reached 0
        if *counter_lock == 0 {
            self.system.active_guards.1.notify_all();
        }
    }
}
