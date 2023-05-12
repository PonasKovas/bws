use std::mem::size_of;

use crate::{FromBytes, ToBytes};

macro_rules! implement_for_primitive {
    ( $( $primitive:ty ),+ )  => {
    	$(
			impl ToBytes for $primitive {
	            fn write_to<W: std::io::Write>(&self, write: &mut W) -> std::io::Result<usize> {
	                let buf = self.to_be_bytes();
	                write.write_all(buf.as_slice())?;

	                Ok(buf.len())
	            }
	        }
	        impl FromBytes for $primitive {
    			fn read_from<R: std::io::Read>(read: &mut R) -> std::io::Result<Self>
			    where
			        Self: Sized,
			    {
			        let mut buf = [0u8; size_of::<Self>()];
			        read.read_exact(&mut buf)?;

			        Ok(Self::from_be_bytes(buf))
			    }
			}

    	)+

    };
}

implement_for_primitive! { u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64 }
