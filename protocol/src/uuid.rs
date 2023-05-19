use crate::{FromBytes, ToBytes};
use std::io::{Read, Write};
use uuid::Uuid;

impl ToBytes for Uuid {
    fn write_to<W: Write>(&self, write: &mut W) -> std::io::Result<usize> {
        Ok(self.as_u128().write_to(write)?)
    }
}

impl FromBytes for Uuid {
    fn read_from<R: Read>(read: &mut R) -> std::io::Result<Uuid> {
        Ok(Uuid::from_u128(u128::read_from(read)?))
    }
}
