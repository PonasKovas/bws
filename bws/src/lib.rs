#[cfg(feature = "application")]
pub mod application;

pub mod graceful_shutdown;
mod linear_search;
pub mod serverbase;

use std::sync::Arc;

pub use linear_search::LinearSearch;
use serverbase::serve;
use serverbase::ServerBase;
use tokio::{net::TcpListener, runtime::Handle};

pub fn run<S: ServerBase>(server: S, port: u16) -> std::io::Result<()> {
    Handle::current().block_on(async {
        let server = Arc::new(server);

        let _shutdown_guard = server.store().shutdown.guard();

        let listener = TcpListener::bind(("127.0.0.1", port)).await?;

        tokio::select! {
            _ = server.store().shutdown.wait_for_shutdown() => {},
            _ = serve(server.clone(), listener) => {},
        }

        Ok(())
    })
}
