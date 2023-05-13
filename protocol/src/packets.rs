pub mod handshake;
mod legacy_ping;
pub mod status;

pub use handshake::SBHandshake;
pub use legacy_ping::{LegacyPing, LegacyPingResponse};
pub use status::{CBStatus, SBStatus};

#[derive(Debug, PartialEq, Clone)]
pub enum ServerBound {
    Handshake(SBHandshake),
    Status(SBStatus),
    Login,
    Play,
    LegacyPing(LegacyPing),
}

#[derive(Debug, PartialEq, Clone)]
pub enum ClientBound {
    Status(CBStatus),
    Login,
    Play,
    LegacyPingResponse(LegacyPingResponse),
}
