use super::{NbtCompound, NbtList, NbtTag};
use crate::Deserializable;
use std::io::{Read, Result};

#[cfg(feature = "ffi_safe")]
use super::{String, Vec};

impl Deserializable for NbtCompound {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let id = u8::from_reader(input)?;
        if id != 0xA {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Root tag must be compound",
            ));
        }

        // just discard the root name, we don't need it
        let root_name_len = u16::from_reader(input)?;
        for _ in 0..root_name_len {
            u8::from_reader(input)?;
        }

        Ok(deserialize_compound(input)?)
    }
}

fn deserialize_compound<R: Read>(input: &mut R) -> Result<NbtCompound> {
    let mut compound = NbtCompound::new();

    loop {
        // element tag
        let tag = u8::from_reader(input)?;

        if tag == 0x0 {
            // Tag_End
            break;
        }

        let name = deserialize_tag(input, 8)?; // string
        let value = deserialize_tag(input, tag)?;

        if let NbtTag::String(s) = name {
            // infallible ^
            compound.insert(s, value);
        }
    }

    Ok(compound)
}

fn deserialize_tag<R: Read>(input: &mut R, id: u8) -> Result<NbtTag> {
    Ok(match id {
        1 => NbtTag::Byte(i8::from_reader(input)?),
        2 => NbtTag::Short(i16::from_reader(input)?),
        3 => NbtTag::Int(i32::from_reader(input)?),
        4 => NbtTag::Long(i64::from_reader(input)?),
        5 => NbtTag::Float(f32::from_reader(input)?),
        6 => NbtTag::Double(f64::from_reader(input)?),
        7 => {
            let len = i32::from_reader(input)?;

            let mut array = Vec::new();
            for _ in 0..len {
                array.push(i8::from_reader(input)?)
            }

            NbtTag::ByteArray(array)
        }
        8 => {
            let len = u16::from_reader(input)?;

            let mut array = Vec::new();
            for _ in 0..len {
                array.push(u8::from_reader(input)?)
            }

            let string = match String::from_utf8(array) {
                Ok(s) => s,
                Err(_) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "String must be valid utf-8",
                    ))
                }
            };

            NbtTag::String(string)
        }
        9 => {
            let element_tag_id = u8::from_reader(input)?;
            let len = i32::from_reader(input)?;

            if element_tag_id == 0 && len != 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "List Tag Id cant be Tag_END for non-empty lists",
                ));
            }

            let mut list = Vec::new();
            for _ in 0..len {
                list.push(deserialize_tag(input, element_tag_id)?)
            }

            NbtTag::List(NbtList(list))
        }
        10 => NbtTag::Compound(deserialize_compound(input)?),
        11 => {
            let len = i32::from_reader(input)?;

            let mut array = Vec::new();
            for _ in 0..len {
                array.push(i32::from_reader(input)?)
            }

            NbtTag::IntArray(array)
        }
        12 => {
            let len = i32::from_reader(input)?;

            let mut array = Vec::new();
            for _ in 0..len {
                array.push(i64::from_reader(input)?)
            }

            NbtTag::LongArray(array)
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid Tag id",
            ));
        }
    })
}
