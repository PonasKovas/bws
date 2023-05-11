use anyhow::Result;
use tokio::{io::AsyncWriteExt, net::TcpListener, runtime::Runtime};

/// Represents basic server capabilities, such as listening on a TCP port and handling connections
pub trait ServerBase: Sized {
    fn store(&self) -> &ServerBaseStore;
    fn store_mut(&mut self) -> &mut ServerBaseStore;

    fn run(self, rt: &Runtime, port: u16) -> Result<()> {
        rt.block_on(async {
            let listener = TcpListener::bind(("127.0.0.1", port)).await?;

            loop {
                let (mut socket, _addr) = listener.accept().await?;
                socket.write_all(b"kas skaitys tas gaidys\n").await?;
            }
        })
    }
}

pub struct ServerBaseStore {}

impl ServerBaseStore {
    pub fn new() -> Self {
        Self {}
    }
}
