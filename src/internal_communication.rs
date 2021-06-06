use std::net::SocketAddr;

use crate::packets::{ClientBound, ServerBound};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub type SHSender = UnboundedSender<SHBound>;
pub type WSender = UnboundedSender<WBound>;
pub type SHReceiver = UnboundedReceiver<SHBound>;
pub type WReceiver = UnboundedReceiver<WBound>;

// StreamHandlerBound - all messages that are sent from GlobalState or Worlds to individual
// players' stream handler threads
#[derive(Debug)]
pub enum SHBound {
    AssignId(usize), // the stream handler thread receives this packet when the player joins any world, it contains the player ID inside that world
    Packet(ClientBound),
    Disconnect, // only
    ChangeWorld(WSender),
}

// WorldBound - all messages that are sent from individual players' stream handlers to
// their respective worlds.
#[derive(Debug)]
pub enum WBound {
    AddPlayer(String, SHSender, SocketAddr), // The player username, sender to the connection task and the address of client
    // Only removes the player from the given world, the actual client may still be connected and may join other worlds
    RemovePlayer(usize),        // id of the player
    Packet(usize, ServerBound), // id of the player and the packet
}
