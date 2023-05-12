use std::net::SocketAddr;

use crate::graceful_shutdown::ShutdownSystem;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    runtime::Handle,
};
use tracing::{info, instrument};

/// Represents basic server capabilities, such as listening on a TCP port and handling connections
pub trait ServerBase: Sized + Send + Sync {
    fn store(&self) -> &ServerBaseStore;
    fn store_mut(&mut self) -> &mut ServerBaseStore;

    fn run(self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        Handle::current().block_on(async {
            let _shutdown_guard = self.store().shutdown.guard();

            let listener = TcpListener::bind(("127.0.0.1", port)).await?;

            tokio::select! {
                _ = self.store().shutdown.wait_for_shutdown() => {},
                _ = serve(listener) => {},
            }

            Ok(())
        })
    }
}

async fn serve(listener: TcpListener) -> Result<(), tokio::io::Error> {
    loop {
        let (mut socket, _addr) = listener.accept().await?;
        socket.write_all(b"kas skaitys tas gaidys\n").await?;
        info!("Connection!");
    }
}

/// Data storage required to operate a base server
#[derive(Debug)]
pub struct ServerBaseStore {
    pub shutdown: ShutdownSystem,
}

impl ServerBaseStore {
    pub fn new() -> Self {
        Self {
            shutdown: ShutdownSystem::new(),
        }
    }
}
