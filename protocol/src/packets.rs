pub mod handshake;
pub mod login;
pub mod status;

use crate::{FromBytes, ToBytes};
pub use handshake::SBHandshake;
pub use login::{CBLogin, SBLogin};
pub use status::{CBStatus, SBStatus};

#[derive(FromBytes, ToBytes)]
struct NoTag;
impl From<i32> for NoTag {
    fn from(_: i32) -> Self {
        Self
    }
}

#[derive(ToBytes, Debug, PartialEq, Clone)]
#[discriminant_as(NoTag)]
pub enum ServerBound {
    Handshake(SBHandshake),
    Status(SBStatus),
    Login(SBLogin),
    Play,
}

#[derive(ToBytes, Debug, PartialEq, Clone)]
#[discriminant_as(NoTag)]
pub enum ClientBound {
    Status(CBStatus),
    Login(CBLogin),
    Play,
}

impl From<SBHandshake> for ServerBound {
    fn from(value: SBHandshake) -> Self {
        Self::Handshake(value)
    }
}

impl From<SBStatus> for ServerBound {
    fn from(value: SBStatus) -> Self {
        Self::Status(value)
    }
}

impl From<CBStatus> for ClientBound {
    fn from(value: CBStatus) -> Self {
        Self::Status(value)
    }
}

impl From<SBLogin> for ServerBound {
    fn from(value: SBLogin) -> Self {
        Self::Login(value)
    }
}

impl From<CBLogin> for ClientBound {
    fn from(value: CBLogin) -> Self {
        Self::Login(value)
    }
}
