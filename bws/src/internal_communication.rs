use crate::packets::{ClientBound, ServerBound};

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
