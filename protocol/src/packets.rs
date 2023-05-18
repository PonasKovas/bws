pub mod handshake;
pub mod status;

pub use handshake::SBHandshake;
pub use status::{CBStatus, SBStatus};

#[derive(Debug, PartialEq, Clone)]
pub enum ServerBound {
    Handshake(SBHandshake),
    Status(SBStatus),
    Login,
    Play,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ClientBound {
    Status(CBStatus),
    Login,
    Play,
}
