use crate::{FromBytes, ToBytes};
use std::io::{ErrorKind, Read, Result, Write};

impl<T: ToBytes> ToBytes for Box<T> {
    fn write_to<W: Write>(&self, write: &mut W) -> Result<usize> {
        Ok((**self).write_to(write)?)
    }
}

impl<T: FromBytes> FromBytes for Box<T> {
    fn read_from<R: Read>(read: &mut R) -> Result<Self> {
        Ok(Box::new(T::read_from(read)?))
    }
}

impl<T: ToBytes> ToBytes for Box<[T]> {
    fn write_to<W: Write>(&self, write: &mut W) -> Result<usize> {
        let mut written = 0;

        for e in &**self {
            written += e.write_to(write)?;
        }

        Ok(written)
    }
}

impl<T: FromBytes> FromBytes for Box<[T]> {
    fn read_from<R: Read>(read: &mut R) -> Result<Self> {
        // Todo maybe error if EOF mid-element
        let mut res = Vec::new();

        loop {
            match T::read_from(read) {
                Ok(element) => res.push(element),
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(res.into_boxed_slice()),
                Err(e) => return Err(e),
            }
        }
    }
}
