pub(crate) mod legacy_ping;

use crate::Server;
use protocol::packets::{ClientBound, ServerBound};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpStream,
    sync::{
        broadcast::{Receiver, Sender},
        mpsc::{UnboundedReceiver, UnboundedSender},
    },
};

pub struct Conn {
    pub(crate) addr: SocketAddr,
    pub(crate) input: Receiver<ServerBound>,
    pub(crate) output: UnboundedSender<ClientBound>,
}

pub(crate) struct ConnCtx {
    pub id: usize,
    pub stream: BufReader<TcpStream>,
    pub addr: SocketAddr,
    pub input: Sender<ServerBound>,
    pub output: UnboundedReceiver<ClientBound>,
    pub buf: Vec<u8>,
}

pub(crate) async fn handle_new_conn(
    server: Arc<Server>,
    mut ctx: ConnCtx,
) -> Result<(), Box<dyn std::error::Error>> {
    // Handle legacy ping
    legacy_ping::handle(server, &mut ctx).await?;

    Ok(())
}
