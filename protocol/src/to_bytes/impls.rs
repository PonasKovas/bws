use crate::{newtypes::VarLong, ToBytes, VarInt};
use serde_json::Value;
use std::io::{Result, Write};
use uuid::Uuid;

impl<T: ToBytes> ToBytes for Box<T> {
    fn write_to<W: Write>(&self, write: &mut W) -> Result<usize> {
        Ok((**self).write_to(write)?)
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

impl ToBytes for Value {
    fn write_to<W: Write>(&self, write: &mut W) -> Result<usize> {
        serde_json::to_string(self)?.write_to(write)
    }
}

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

impl ToBytes for Uuid {
    fn write_to<W: Write>(&self, write: &mut W) -> std::io::Result<usize> {
        Ok(self.as_u128().write_to(write)?)
    }
}

impl ToBytes for VarInt {
    fn write_to<W: std::io::Write>(&self, write: &mut W) -> std::io::Result<usize> {
        let mut i = 0;
        let mut value = self.0;
        loop {
            // Take the 7 lower bits of the value
            let mut temp = (value & 0b0111_1111) as u8;

            // Shift the value 7 bits to the right.
            value = ((value as u32) >> 7) as i32;

            // If there is more data to write, set the high bit
            if value != 0 {
                temp |= 0b1000_0000;
            }

            write.write_all(&[temp])?;
            i += 1;

            // If there is no more data to write, exit the loop
            if value == 0 {
                break;
            }
        }

        Ok(i)
    }
}

impl ToBytes for VarLong {
    fn write_to<W: std::io::Write>(&self, write: &mut W) -> std::io::Result<usize> {
        let mut i = 0;

        let mut value = self.0;
        loop {
            // Take the 7 lower bits of the value
            let mut temp = (value & 0b0111_1111) as u8;

            // Shift the value 7 bits to the right.
            value = ((value as u64) >> 7) as i64;

            // If there is more data to write, set the high bit
            if value != 0 {
                temp |= 0b1000_0000;
            }

            write.write_all(&[temp])?;
            i += 1;

            // If there is no more data to write, exit the loop
            if value == 0 {
                break;
            }
        }

        Ok(i)
    }
}

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

impl ToBytes for bool {
    fn write_to<W: Write>(&self, write: &mut W) -> Result<usize> {
        write.write_all(&[if *self { 0x01 } else { 0x00 }])?;

        Ok(1)
    }
}

macro_rules! implement_for_primitive {
    ( $( $primitive:ty ),+ )  => {
        $(
            impl ToBytes for $primitive {
                fn write_to<W: Write>(&self, write: &mut W) -> Result<usize> {
                    let buf = self.to_be_bytes();
                    write.write_all(buf.as_slice())?;

                    Ok(buf.len())
                }
            }
        )+

    };
}

implement_for_primitive! { u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64 }
