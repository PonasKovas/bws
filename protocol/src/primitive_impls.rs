use crate::{FromBytes, ToBytes};
use std::io::{Read, Result, Write};
use std::mem::size_of;

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
	        impl FromBytes for $primitive {
    			fn read_from<R: Read>(read: &mut R) -> Result<Self> {
			        let mut buf = [0u8; size_of::<Self>()];
			        read.read_exact(&mut buf)?;

			        Ok(Self::from_be_bytes(buf))
			    }
			}

    	)+

    };
}

implement_for_primitive! { u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64 }

impl ToBytes for bool {
    fn write_to<W: Write>(&self, write: &mut W) -> Result<usize> {
        write.write_all(&[if *self { 0x01 } else { 0x00 }])?;

        Ok(1)
    }
}
impl FromBytes for bool {
    fn read_from<R: Read>(read: &mut R) -> Result<bool> {
        let mut buf = [0u8; 1];
        read.read_exact(&mut buf)?;

        Ok(buf[0] != 0)
    }
}
