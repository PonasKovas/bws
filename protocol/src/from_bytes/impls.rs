use crate::{newtypes::VarLong, FromBytes, VarInt};
use serde_json::Value;
use std::io::{ErrorKind, Read, Result};
use uuid::Uuid;

impl<T: FromBytes> FromBytes for Box<T> {
    fn read_from<R: Read>(read: &mut R) -> Result<Self> {
        Ok(Box::new(T::read_from(read)?))
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

impl FromBytes for Value {
    fn read_from<R: Read>(read: &mut R) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(serde_json::from_reader(read)?)
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

impl FromBytes for String {
    fn read_from<R: Read>(read: &mut R) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let length = VarInt::read_from(read)?;

        let mut buffer = vec![0u8; length.0 as usize];

        read.read_exact(&mut buffer[..])?;

        let string = String::from_utf8(buffer.to_vec()).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "String not valid UTF-8")
        })?;

        Ok(string)
    }
}

impl FromBytes for Uuid {
    fn read_from<R: Read>(read: &mut R) -> std::io::Result<Uuid> {
        Ok(Uuid::from_u128(u128::read_from(read)?))
    }
}

impl FromBytes for VarInt {
    fn read_from<R: std::io::Read>(read: &mut R) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut num_read = 0; // Count of bytes that have been read
        let mut result = 0i32; // The VarInt being constructed

        loop {
            // VarInts are at most 5 bytes long.
            if num_read == 5 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "VarInt is too big",
                ));
            }

            // Read a byte
            let mut byte = 0u8;
            read.read_exact(std::slice::from_mut(&mut byte))?;

            // Extract the 7 lower bits (the data bits) and cast to i32
            let value = (byte & 0b0111_1111) as i32;

            // Shift the data bits to the correct position and add them to the result
            result |= value << (7 * num_read);

            num_read += 1;

            // If the high bit is not set, this was the last byte in the VarInt
            if (byte & 0b1000_0000) == 0 {
                break;
            }
        }

        Ok(Self(result))
    }
}

impl FromBytes for VarLong {
    fn read_from<R: std::io::Read>(read: &mut R) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut num_read = 0; // Count of bytes that have been read
        let mut result = 0i64; // The VarInt being constructed

        loop {
            // VarLongs are at most 10 bytes long.
            if num_read > 10 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "VarLong is too big",
                ));
            }

            // Read a byte
            let mut byte = 0u8;
            read.read_exact(std::slice::from_mut(&mut byte))?;

            // Extract the 7 lower bits (the data bits) and cast to i32
            let value = (byte & 0b0111_1111) as i64;

            // Shift the data bits to the correct position and add them to the result
            result |= value << (7 * num_read);

            num_read += 1;

            // If the high bit is not set, this was the last byte in the VarInt
            if (byte & 0b1000_0000) == 0 {
                break;
            }
        }

        Ok(Self(result))
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

impl FromBytes for bool {
    fn read_from<R: Read>(read: &mut R) -> Result<bool> {
        let mut buf = [0u8; 1];
        read.read_exact(&mut buf)?;

        Ok(buf[0] != 0)
    }
}

macro_rules! impl_from_bytes {
    ( $( $primitive:ty ),+ )  => {
        $(
            impl FromBytes for $primitive {
                fn read_from<R: Read>(read: &mut R) -> Result<Self> {
                    let mut buf = [0u8; std::mem::size_of::<Self>()];
                    read.read_exact(&mut buf)?;

                    Ok(Self::from_be_bytes(buf))
                }
            }

        )+

    };
}

impl_from_bytes! { u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64 }
