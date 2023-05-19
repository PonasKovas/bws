use crate::{FromBytes, ToBytes, VarInt};
use std::io::{Read, Write};
use tracing::debug;

impl<T: ToBytes> ToBytes for Option<T> {
    fn write_to<W: Write>(&self, write: &mut W) -> std::io::Result<usize> {
        let mut written = 0;

        match self {
            Some(inner) => {
                written += true.write_to(write)?;
                written += inner.write_to(write)?;
            }
            None => {
                written += false.write_to(write)?;
            }
        }

        Ok(written)
    }
}

impl<T: FromBytes> FromBytes for Option<T> {
    fn read_from<R: Read>(read: &mut R) -> std::io::Result<Self> {
        let tag = bool::read_from(read)?;

        if tag {
            let value = T::read_from(read)?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }
}
