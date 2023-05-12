mod bstring;
mod varint;

use std::io::{Read, Result, Write};

pub use bstring::BString;
pub use varint::VarInt;

pub trait FromBytes {
    fn read_from<R: Read>(read: &mut R) -> Result<Self>
    where
        Self: Sized;
}

pub trait ToBytes {
    fn write_to<W: Write>(&self, write: &mut W) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Handshake {
    pub protocol_version: VarInt,
    pub server_address: BString<255>,
}
