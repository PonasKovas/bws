use std::net::SocketAddr;

use crate::packets::{ClientBound, ServerBound};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub type SHInputSender = UnboundedSender<ClientBound>;
pub type SHInputReceiver = UnboundedReceiver<ClientBound>;
pub type SHOutputSender = UnboundedSender<ServerBound>;
pub type SHOutputReceiver = UnboundedReceiver<ServerBound>;
pub type WSender = UnboundedSender<WBound>;
pub type WReceiver = UnboundedReceiver<WBound>;

// WorldBound - general messages for worlds. Can be sent both from other worlds, and from stream handler tasks
// basically from anywhere
#[derive(Debug)]
pub enum WBound {
    AddPlayer(usize), // id of the player
}
