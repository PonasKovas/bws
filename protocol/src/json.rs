use std::io::{Read, Result, Write};

use crate::{FromBytes, ToBytes};
use serde_json::Value;

impl FromBytes for Value {
    fn read_from<R: Read>(read: &mut R) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(serde_json::from_reader(read)?)
    }
}

impl ToBytes for Value {
    fn write_to<W: Write>(&self, write: &mut W) -> Result<usize> {
        serde_json::to_string(self)?.write_to(write)
    }
}
