#![feature(array_map)]
#![feature(never_type)]

pub mod commands_builder;
pub mod datatypes;
#[macro_use]
pub mod macros;
pub mod packets;

pub use protocol_derive::{Deserializable, Serializable};
use std::io::{Read, Result, Write};

pub trait Serializable {
    /// Returns how many bytes were written.
    ///
    /// You can feed it a no-op writer and just calculate the size of a packet efficiently.
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize>;
}

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
