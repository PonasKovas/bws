use std::sync::Arc;

use tokio::runtime::Builder;
use tracing::info;

use crate::serverbase::ServerBase;

mod logging;

/// Convenience function for applications to run a single server
///
/// Takes care of logging, a tokio runtime, handling ctrl-c, and shutdown
pub fn run_app<S: ServerBase + Send + 'static>(
    server: S,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("test");
    logging::init();

    let shutdown_system = server.store().shutdown.clone();
    ctrlc::set_handler(move || {
        shutdown_system.shutdown();
    })?;

    let runtime = Arc::new(
        Builder::new_multi_thread()
            .enable_all()
            .thread_name("bws-net-worker")
            .build()
            .unwrap(),
    );

    let rt = Arc::clone(&runtime);

    info!("Start up...");

    let shutdown_system = server.store().shutdown.clone();
    let port = config.port;
    std::thread::spawn(move || {
        let _rt = rt.enter();

        crate::run(server, port).unwrap();
    });

    shutdown_system.blocking_wait_for_shutdown();
    info!("Shutting down...");
    shutdown_system.blocking_wait_for_guards(config.shutdown_timeout);

    Ok(())
}

/// Simple configuration for the server
#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub shutdown_timeout: Option<u64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 25565,
            shutdown_timeout: Some(3000),
        }
    }
}
