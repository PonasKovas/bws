use crate::{newtypes::NextState, BString, FromBytes, ToBytes, VarInt};

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub enum SBHandshake {
    Handshake(Handshake),
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct Handshake {
    pub protocol_version: VarInt,
    pub server_address: BString<255>,
    pub server_port: u16,
    pub next_state: NextState,
}
