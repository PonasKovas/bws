use std::net::SocketAddr;

use protocol::packets::{PlayClientBound, PlayServerBound};
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};

pub type SHInputSender = UnboundedSender<PlayClientBound<'static>>;
// just happens that this one isn't used anywhere because its only used from stream_handler.rs
// where its not passed around and the type is implicit
// but we will keep the type definition for consistency
#[allow(dead_code)]
pub type SHInputReceiver = UnboundedReceiver<PlayClientBound<'static>>;
pub type SHOutputSender = Sender<PlayServerBound<'static>>;
pub type SHOutputReceiver = Receiver<PlayServerBound<'static>>;
pub type WSender = UnboundedSender<WBound>;
pub type WReceiver = UnboundedReceiver<WBound>;

/// WorldBound - general messages for worlds. Can be sent both from other worlds, and from stream handler tasks
/// basically from anywhere
#[derive(Debug)]
pub enum WBound {
    AddPlayer {
        id: usize,
    },
    /// this is the only CORRECT way to move a player across worlds.
    /// Send this to world that the player currently is in.
    /// Then that world must remove the player from itself
    /// and send the above AddPlayer packet to the given new world sender
    MovePlayer {
        id: usize,
        new_world: WSender,
    },
    /// Exits the world gracefully, saving the data
    Exit,
}
