#![feature(array_map)]
#![feature(never_type)]
// deal with this later
#![allow(clippy::large_enum_variant)]

/// Used by the [`command!`] macro internally but can be used manually too
pub mod commands_builder;
/// Data types uesd in the Minecraft protocol
pub mod datatypes;
/// All packets of the Minecraft protocol
pub mod packets;
#[macro_use]
mod macros;

pub use protocol_derive::{Deserializable, Serializable};
use std::io::{Read, Result, Write};

/// A trait for types that can be serialized into bytes for use in the Minecraft protocol
pub trait Serializable {
    /// Returns how many bytes were written.
    ///
    /// You can feed it a no-op writer and just calculate the size of a packet efficiently.
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize>;
}

/// A trait for types that can be deserialized from bytes in the Minecraft protocol
pub trait Deserializable {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self>
    where
        Self: Sized;
}

// this is used from the derive macros, not intended for any other use
#[doc(hidden)]
pub struct PeekedStream<D: Serializable, R: Read> {
    pub peeked: Option<D>,
    pub stream: R,
}

impl<D: Serializable, R: Read> Read for PeekedStream<D, R> {
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize> {
        if let Some(peeked) = self.peeked.take() {
            let initial_len = buf.len();

            peeked.to_writer(&mut buf)?;

            Ok(initial_len - buf.len())
        } else {
            self.stream.read(buf)
        }
    }
}
