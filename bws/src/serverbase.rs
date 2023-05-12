use std::net::SocketAddr;

use crate::graceful_shutdown::ShutdownSystem;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    runtime::Handle,
};
use tracing::{info, instrument};

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

/// Represents basic server capabilities, such as listening on a TCP port and handling connections, managing worlds
pub trait ServerBase: Sized {
    fn store(&self) -> &ServerBaseStore;
    fn store_mut(&mut self) -> &mut ServerBaseStore;

    fn run(self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        Handle::current().block_on(async {
            let shutdown_system = self.store().shutdown.clone();
            let _shutdown_guard = shutdown_system.guard();

            let listener = TcpListener::bind(("127.0.0.1", port)).await?;

            tokio::select! {
                _ = shutdown_system.wait_for_shutdown() => {},
                _ = serve(self, listener) => {},
            }

            Ok(())
        })
    }
}

async fn serve<S: ServerBase>(server: S, listener: TcpListener) -> Result<(), tokio::io::Error> {
    loop {
        let (socket, addr) = listener.accept().await?;
        let shutdown_system = server.store().shutdown.clone();

        tokio::spawn(handle_conn(shutdown_system, socket, addr));
    }
}

#[instrument]
async fn handle_conn(
    shutdown_system: ShutdownSystem,
    mut socket: TcpStream,
    addr: SocketAddr,
) -> Result<(), tokio::io::Error> {
    let _shutdown_guard = shutdown_system.guard();

    info!("Connection!");

    loop {
        tokio::select! {
            _ = socket.read_u8() => {
                socket.write_all(b"kas skaitys tas gaidys\n").await?;
            },
            _ = shutdown_system.wait_for_shutdown() => {
                socket.write_all(b"Sorry gotta go!..\n").await?;
                break;
            },
        }
    }

    Ok(())
}
