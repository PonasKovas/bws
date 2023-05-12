extern crate self as protocol;

mod bstring;
pub mod newtypes;
mod primitive_impls;
mod string;
mod varint;

use newtypes::NextState;
use std::io::{Read, Result, Write};

pub use bstring::BString;
pub use protocol_derive::{FromBytes, ToBytes};
pub use varint::VarInt;

pub trait FromBytes {
    fn read_from<R: Read>(read: &mut R) -> Result<Self>
    where
        Self: Sized;
}

pub trait ToBytes {
    fn write_to<W: Write>(&self, write: &mut W) -> Result<usize>;
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct Handshake {
    pub protocol_version: VarInt,
    pub server_address: BString<255>,
    pub server_port: u16,
    pub next_state: NextState,
}
