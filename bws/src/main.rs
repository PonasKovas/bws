pub mod cli;
pub mod graceful_shutdown;
mod linear_search;
pub mod logging;
pub mod server;

use anyhow::{Context, Result};
pub use linear_search::LinearSearch;
use once_cell::sync::Lazy;
use tokio::runtime::Builder;
use tracing::info;

use crate::server::ServerBase;

fn main() -> Result<()> {
    human_panic::setup_panic!();

    // Parse env and command line args
    Lazy::force(&cli::OPT);

    logging::init();

    ctrlc::set_handler(move || {
        graceful_shutdown::shutdown();
    })?;

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(cli::OPT.net_workers)
        .thread_name("bws-net-worker")
        .build()
        .unwrap();

    let rt_handle = runtime.handle().clone();

    std::thread::spawn(move || {
        struct MyServer {
            serverbase_store: server::ServerBaseStore,
        }
        impl server::ServerBase for MyServer {
            fn store(&self) -> &server::ServerBaseStore {
                &self.serverbase_store
            }
            fn store_mut(&mut self) -> &mut server::ServerBaseStore {
                &mut self.serverbase_store
            }
        }
        let my_server = MyServer {
            serverbase_store: server::ServerBaseStore::new(),
        };
        my_server.run(rt_handle, 25565).unwrap();
    });

    graceful_shutdown::wait_for_shutdown();
    info!("Shutting down...");
    graceful_shutdown::wait_for_guards();

    Ok(())
}
