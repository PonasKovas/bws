use crate::{FromBytes, ToBytes, VarInt};
use std::io::{Read, Write};
use tracing::debug;

impl<T: ToBytes> ToBytes for Vec<T> {
    fn write_to<W: Write>(&self, write: &mut W) -> std::io::Result<usize> {
        let mut written = 0;

        if self.len() as u64 > i32::MAX as u64 {
            // Since the data is prefixed by the length in the form of VarInt
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Vec is too big",
            ));
        }

        written += VarInt(self.len() as i32).write_to(write)?;

        for e in self {
            written += e.write_to(write)?;
        }

        Ok(written)
    }
}

impl<T: FromBytes> FromBytes for Vec<T> {
    fn read_from<R: Read>(read: &mut R) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let length = VarInt::read_from(read)?;

        let mut buffer = Vec::with_capacity(length.0 as usize);

        for _ in 0..length.0 {
            buffer.push(T::read_from(read)?);
        }

        Ok(buffer)
    }
}
