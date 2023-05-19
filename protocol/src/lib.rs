extern crate self as protocol;

mod r#box;
mod bstring;
mod json;
pub mod newtypes;
mod option;
pub mod packets;
mod primitive_impls;
mod string;
mod uuid;
mod varint;
mod vec;

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
