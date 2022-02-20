use super::{NbtCompound, NbtTag};
use crate::Serializable;
use std::io::{Result, Write};

impl Serializable for NbtCompound {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut written = 0;

        // NbtCompound tag
        written += 0xAu8.to_writer(output)?;
        // Root name (len as u16 + string)
        // In our case ""
        written += 0x0u16.to_writer(output)?;

        // the compound
        written += serialize_compound(output, &self)?;

        Ok(written)
    }
}

fn tag_id(tag: &NbtTag) -> u8 {
    match tag {
        NbtTag::Byte(_) => 1,
        NbtTag::Short(_) => 2,
        NbtTag::Int(_) => 3,
        NbtTag::Long(_) => 4,
        NbtTag::Float(_) => 5,
        NbtTag::Double(_) => 6,
        NbtTag::ByteArray(_) => 7,
        NbtTag::String(_) => 8,
        NbtTag::List(_) => 9,
        NbtTag::Compound(_) => 10,
        NbtTag::IntArray(_) => 11,
        NbtTag::LongArray(_) => 12,
    }
}

fn serialize_tag_id<W: Write>(output: &mut W, tag: &NbtTag) -> Result<usize> {
    let mut written = 0;

    let id: u8 = tag_id(tag);

    written += id.to_writer(output)?;

    Ok(written)
}

fn serialize_compound<W: Write>(output: &mut W, compound: &NbtCompound) -> Result<usize> {
    let mut written = 0;

    for t in &compound.0 {
        // Element id
        written += serialize_tag_id(output, &t.1)?;

        // Element name (len as u16 + string)
        written += (t.0.len() as u16).to_writer(output)?;
        for byte in t.0.as_bytes() {
            written += byte.to_writer(output)?;
        }

        // Element
        written += serialize_tag(output, &t.1)?;
    }

    // The Tag_END byte so signify end of compound
    written += 0x0u8.to_writer(output)?;

    Ok(written)
}

fn serialize_tag<W: Write>(output: &mut W, tag: &NbtTag) -> Result<usize> {
    let mut written = 0;

    match tag {
        NbtTag::Byte(byte) => {
            written += byte.to_writer(output)?;
        }
        NbtTag::Short(short) => {
            written += short.to_writer(output)?;
        }
        NbtTag::Int(int) => {
            written += int.to_writer(output)?;
        }
        NbtTag::Long(long) => {
            written += long.to_writer(output)?;
        }
        NbtTag::Float(float) => {
            written += float.to_writer(output)?;
        }
        NbtTag::Double(double) => {
            written += double.to_writer(output)?;
        }
        NbtTag::ByteArray(array) => {
            // Length as i32
            written += (array.len() as i32).to_writer(output)?;
            for byte in array {
                written += byte.to_writer(output)?;
            }
        }
        NbtTag::String(string) => {
            // Length as u16
            written += (string.len() as u16).to_writer(output)?;
            for byte in string.as_bytes() {
                written += byte.to_writer(output)?;
            }
        }
        NbtTag::List(list) => {
            // Type id
            if list.len() == 0 {
                // Tag_END works, since there are no elements
                written += 0u8.to_writer(output)?;
            } else {
                // Otherwise the tag of the first element
                written += serialize_tag_id(output, &list[0])?;

                let tag = tag_id(&list[0]);

                // Number of elements as i32
                written += (list.len() as i32).to_writer(output)?;

                for element in &list.0 {
                    // all elements must be of the same type
                    if tag_id(element) != tag {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Elements in NbtList must be of the same type",
                        ));
                    }

                    written += serialize_tag(output, element)?;
                }
            }
        }
        NbtTag::Compound(compound) => {
            written += serialize_compound(output, compound)?;
        }
        NbtTag::IntArray(array) => {
            // length as i32
            written += (array.len() as i32).to_writer(output)?;
            // elements
            for element in array.as_slice() {
                written += element.to_writer(output)?;
            }
        }
        NbtTag::LongArray(array) => {
            // length as i32
            written += (array.len() as i32).to_writer(output)?;
            // elements
            for element in array.as_slice() {
                written += element.to_writer(output)?;
            }
        }
    }

    Ok(written)
}
