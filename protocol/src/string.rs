use crate::{FromBytes, ToBytes, VarInt};
use std::io::{Read, Write};
use tracing::debug;

impl ToBytes for String {
    fn write_to<W: Write>(&self, write: &mut W) -> std::io::Result<usize> {
        let mut written = 0;

        if self.len() as u64 > i32::MAX as u64 {
            // Since the string is prefixed by the length in the form of VarInt
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "String is too long",
            ));
        }

        written += VarInt(self.len() as i32).write_to(write)?;

        write.write_all(self.as_bytes())?;
        written += self.len();

        Ok(written)
    }
}

impl FromBytes for String {
    fn read_from<R: Read>(read: &mut R) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let length = VarInt::read_from(read)?;

        let mut buffer = vec![0u8; length.0 as usize];

        read.read_exact(&mut buffer[..])?;

        let string = String::from_utf8(buffer.to_vec()).map_err(|e| {
            debug!("String not valid UTF-8: {}", e);
            std::io::Error::new(std::io::ErrorKind::InvalidData, "String not valid UTF-8")
        })?;

        Ok(string)
    }
}
