pub mod datatypes;

pub use protocol_derive::{deserializable, serializable};
use std::io::{Read, Result, Write};

pub trait Serializable {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()>;
}

pub trait Deserializable {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self>
    where
        Self: Sized;
}
