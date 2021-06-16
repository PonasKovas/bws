use std::net::SocketAddr;

use protocol::packets::{PlayClientBound, PlayServerBound};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub type SHInputSender = UnboundedSender<PlayClientBound<'static>>;
pub type SHInputReceiver = UnboundedReceiver<PlayClientBound<'static>>;
pub type SHOutputSender = UnboundedSender<PlayServerBound<'static>>;
pub type SHOutputReceiver = UnboundedReceiver<PlayServerBound<'static>>;
pub type WSender = UnboundedSender<WBound>;
pub type WReceiver = UnboundedReceiver<WBound>;

// WorldBound - general messages for worlds. Can be sent both from other worlds, and from stream handler tasks
// basically from anywhere
#[derive(Debug)]
pub enum WBound {
    AddPlayer { id: usize },
    // this is the only CORRECT way to move a player across worlds.
    // Send this to world that the player currently is in.
    // Then that world must remove the player from itself
    // and send the above AddPlayer packet to the given new world sender
    MovePlayer { id: usize, new_world: WSender },
}