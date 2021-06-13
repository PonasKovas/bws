use super::{deserializable, serializable};
use super::{Deserializable, Serializable};
use std::io::{Read, Result, Write};

pub struct VarInt(pub i32);

#[deserializable]
#[serializable]
pub struct Something {
    pub first: VarInt,
    pub second: VarInt,
}

impl Serializable for VarInt {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        let mut number = self.0 as u32;

        loop {
            let mut byte: u8 = number as u8 & 0b01111111;

            number = number >> 7;
            if number != 0 {
                byte = byte | 0b10000000;
            }

            output.write_all(&[byte])?;

            if number == 0 {
                break;
            }
        }

        Ok(())
    }
}

impl Deserializable for VarInt {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self>
    where
        Self: Sized,
    {
        let mut i = 0;
        let mut result: i32 = 0;

        loop {
            let mut number = [0];
            input.read_exact(&mut number)?;

            let value = (number[0] & 0b01111111) as i32;
            result = result | (value << (7 * i));

            if (number[0] & 0b10000000) == 0 {
                break;
            }
            i += 1;
        }

        Ok(Self(result))
    }
}
