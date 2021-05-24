use crate::packets::{ClientBound, ServerBound};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub type SHSender = UnboundedSender<SHBound>;
pub type WSender = UnboundedSender<WBound>;
pub type SHReceiver = UnboundedReceiver<SHBound>;
pub type WReceiver = UnboundedReceiver<WBound>;

// StreamHandlerBound - all messages that are sent from GlobalState or Worlds to individual
// players' stream handler threads
pub enum SHBound {
    Disconnect,
    SendPacket(ClientBound),
}

// WorldBound - all messages that are sent from individual players' stream handlers to
// their respective worlds.
pub enum WBound {
    Disconnect,
    Packet(ServerBound),
}
