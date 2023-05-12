use crate::graceful_shutdown::ShutdownSystem;
use tokio::{io::AsyncWriteExt, net::TcpListener, runtime::Handle};
use tracing::instrument;

/// Represents basic server capabilities, such as listening on a TCP port and handling connections
pub trait ServerBase: Sized + Send + Sync {
    fn store(&self) -> &ServerBaseStore;
    fn store_mut(&mut self) -> &mut ServerBaseStore;

    #[instrument(skip(self))]
    fn run(self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        Handle::current().block_on(async {
            let listener = TcpListener::bind(("127.0.0.1", port)).await?;

            loop {
                let (mut socket, _addr) = listener.accept().await?;
                socket.write_all(b"kas skaitys tas gaidys\n").await?;
            }
        })
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
