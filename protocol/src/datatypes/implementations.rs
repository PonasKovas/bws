use super::*;
use super::{Deserializable, Serializable};
use std::borrow::Cow;
use std::cmp::max;
use std::convert::TryInto;
use std::io::{self, Cursor, ErrorKind, Read, Result, Write};

impl Serializable for ChunkSections {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        // first we need the size of the whole structure IN BYTES
        // (good job mojang)
        let mut size = 0i32;
        for chunk_section in &self.0 {
            size += 3; // 2 bytes for block count, 1 byte for "bits per block"
            match &chunk_section.palette {
                Palette::Indirect(palette) => {
                    size += VarInt(palette.len() as i32).size() as i32;
                    for block in palette {
                        size += block.size() as i32;
                    }
                }
                Palette::Direct => {}
            }

            size += VarInt(chunk_section.data.len() as i32).size() as i32;
            size += 8 * chunk_section.data.len() as i32; // i64s
        }

        VarInt(size).to_writer(&mut *output)?;
        for section in &self.0 {
            section.block_count.to_writer(&mut *output)?;
            match &section.palette {
                Palette::Indirect(mappings) => {
                    let bits_per_block = max(
                        4,
                        32u8 - max(mappings.len() as u32 - 1, 1).leading_zeros() as u8,
                    );
                    bits_per_block.to_writer(&mut *output)?;
                    mappings.to_writer(&mut *output)?;
                }
                Palette::Direct => {
                    15u8.to_writer(&mut *output)?;
                }
            }
            section.data.to_writer(&mut *output)?;
        }

        Ok(())
    }
}
impl Deserializable for ChunkSections {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        todo!()
    }
}

impl<'a> Serializable for CommandNode<'a> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        match self {
            CommandNode::Root { children } => {
                let mut flags = 0u8; // todo bitflags
                flags |= 0; // root type
                flags.to_writer(&mut *output)?;
                children.to_writer(&mut *output)?;
            }
            CommandNode::Literal {
                executable,
                children,
                redirect,
                name,
            } => {
                let mut flags = 0u8;
                flags |= 1; // literal type
                if *executable {
                    flags |= 0x04;
                }
                if let Some(_) = redirect {
                    flags |= 0x08;
                }
                flags.to_writer(&mut *output)?;
                children.to_writer(&mut *output)?;
                if let Some(r) = redirect {
                    r.to_writer(&mut *output)?;
                }
                name.to_writer(&mut *output)?;
            }
            CommandNode::Argument {
                executable,
                children,
                redirect,
                name,
                parser,
                suggestions,
            } => {
                let mut flags = 0u8;
                flags |= 2; // argument type
                if *executable {
                    flags |= 0x04;
                }
                if let Some(_) = redirect {
                    flags |= 0x08;
                }
                if let Some(_) = suggestions {
                    flags |= 0x10;
                }
                flags.to_writer(&mut *output)?;
                children.to_writer(&mut *output)?;
                if let Some(r) = redirect {
                    r.to_writer(&mut *output)?;
                }
                name.to_writer(&mut *output)?;
                parser.to_writer(&mut *output)?;
                if let Some(suggestions) = suggestions {
                    suggestions.to_writer(&mut *output)?;
                }
            }
        }

        Ok(())
    }
}
impl<'a> Deserializable for CommandNode<'a> {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        todo!()
    }
}

impl Serializable for Parser {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        match self {
            Parser::String(properties) => {
                "brigadier:string".to_writer(&mut *output)?;
                properties.to_writer(&mut *output)?;
            }
        }
        Ok(())
    }
}

impl<'a> Serializable for StatusResponse<'a> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        serde_json::to_string(&self.json)
            .unwrap()
            .to_writer(&mut *output)
    }
}
impl<'a> Deserializable for StatusResponse<'a> {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        Ok(Self {
            json: serde_json::from_str(&String::from_reader(input)?)?,
        })
    }
}

impl<'a> Serializable for Chat<'a> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        serde_json::to_string(self)?.to_writer(output)
    }
}
impl<'a> Deserializable for Chat<'a> {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        Ok(serde_json::from_str(&String::from_reader(input)?)?)
    }
}

impl Serializable for Nbt {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        quartz_nbt::write::write_nbt_uncompressed(output, "", &self.0)?;

        Ok(())
    }
}
impl Deserializable for Nbt {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        Ok(Self(quartz_nbt::read::read_nbt_uncompressed(input)?.0))
    }
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

impl Serializable for Position {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        let encoded: u64 = ((self.x as u64 & 0x3FFFFFF) << 38)
            | ((self.z as u64 & 0x3FFFFFF) << 12)
            | (self.y as u64 & 0xFFF);

        encoded.to_writer(output)
    }
}
impl Deserializable for Position {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self>
    where
        Self: Sized,
    {
        let encoded = u64::from_reader(input)?;

        let mut x = (encoded >> 38) as i32;
        let mut y = (encoded & 0xFFF) as i32;
        let mut z = (encoded << 26 >> 38) as i32;
        if x >= 2i32.pow(25) {
            x -= 2i32.pow(26);
        }
        if y >= 2i32.pow(11) {
            y -= 2i32.pow(12);
        }
        if z >= 2i32.pow(25) {
            z -= 2i32.pow(26);
        }

        Ok(Self { x, y, z })
    }
}

impl<'a, T: Serializable + ToOwned + ?Sized> Serializable for Cow<'a, T> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        (**self).to_writer(output)
    }
}
impl<'a, T: ToOwned + ?Sized> Deserializable for Cow<'a, T>
where
    <T as ToOwned>::Owned: Deserializable,
{
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        Ok(Cow::Owned(<T as ToOwned>::Owned::from_reader(input)?))
    }
}

impl Serializable for str {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        self.as_bytes().to_writer(output)
    }
}
impl Serializable for String {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        self.as_bytes().to_writer(output)
    }
}
impl Deserializable for String {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let result = String::from_utf8(Vec::from_reader(input)?)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(result)
    }
}

impl<T: Serializable> Serializable for Vec<T> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        (&self[..]).to_writer(output)
    }
}
impl<T: Deserializable> Deserializable for Vec<T> {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let len = VarInt::from_reader(&mut *input)?.0 as usize;

        let mut res = Vec::with_capacity(len);

        for _ in 0..len {
            res.push(T::from_reader(&mut *input)?);
        }

        Ok(res)
    }
}

impl<T: Serializable> Serializable for [T] {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        VarInt(self.len() as i32).to_writer(&mut *output)?;

        for e in self {
            e.to_writer(&mut *output)?;
        }

        Ok(())
    }
}

// Box<[T]> are like Vec<T> except that there's no length prefix and you just read to end
impl<T: Serializable> Serializable for Box<[T]> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        for e in &**self {
            e.to_writer(&mut *output)?;
        }

        Ok(())
    }
}
impl<T: Deserializable> Deserializable for Box<[T]> {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let bytes_per_element = std::mem::size_of::<T>();

        let mut buf: Vec<u8> = vec![0; bytes_per_element];
        let mut res = Vec::new();

        'outer: loop {
            for i in 0..bytes_per_element {
                if let Err(e) = input.read_exact(&mut buf[i..=i]) {
                    if e.kind() == ErrorKind::UnexpectedEof {
                        // check if we were reading the first byte, because that would be valid
                        if i == 0 {
                            break 'outer;
                        }
                    }
                    // this means we got an EOF or another error mid-reading an element
                    return Err(e);
                }
            }

            let e = T::from_reader(&mut Cursor::new(&buf))?;

            res.push(e);
        }

        Ok(res.into())
    }
}

impl<T: Serializable, const N: usize> Serializable for [T; N] {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        for e in self {
            e.to_writer(&mut *output)?;
        }

        Ok(())
    }
}
impl<T: Deserializable, const N: usize> Deserializable for [T; N] {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut result = [(); N].map(|_| None); // Option<T> is not Copy

        for i in 0..N {
            result[i] = Some(T::from_reader(&mut *input)?);
        }

        Ok(result.map(|e| e.unwrap()))
    }
}

impl<T: Serializable, L: Serializable + TryFrom<usize>, const N: usize> Serializable
    for ArrWithLen<T, L, N>
{
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        match L::try_from(N) {
            Ok(s) => s.to_writer(&mut *output)?,
            Err(_) => {
                return Err(std::io::Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed to serialize the size ({}) of an ArrWithLen as a {}",
                        N,
                        std::any::type_name::<L>()
                    ),
                ));
            }
        }

        self.0.to_writer(&mut *output)
    }
}
impl<T: Deserializable, L: Deserializable + TryInto<usize>, const N: usize> Deserializable
    for ArrWithLen<T, L, N>
{
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let len: usize = match L::from_reader(&mut *input)?.try_into() {
            Ok(s) => s,
            Err(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "Failed to convert size ({}) of an ArrWithLen from {} to usize",
                        N,
                        std::any::type_name::<L>()
                    ),
                ));
            }
        };
        if len != N {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Tried reading ArrWithLen but the size was not correct"),
            ))
        } else {
            Ok(Self::new(<[T; N]>::from_reader(&mut *input)?))
        }
    }
}

impl<'a, T: Serializable> Serializable for MaybeStatic<'a, T> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        match self {
            MaybeStatic::Static(bytes) => {
                output.write_all(bytes)?;
            }
            MaybeStatic::Owned(item) => {
                item.to_writer(output)?;
            }
        }
        Ok(())
    }
}
impl<'a, T: Deserializable> Deserializable for MaybeStatic<'a, T> {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        Ok(MaybeStatic::Owned(T::from_reader(input)?))
    }
}

impl<T: Serializable> Serializable for Option<T> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        match self {
            Some(val) => {
                true.to_writer(output)?;
                val.to_writer(output)
            }
            None => false.to_writer(output),
        }
    }
}
impl<T: Deserializable> Deserializable for Option<T> {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        if bool::from_reader(input)? {
            Ok(Some(T::from_reader(input)?))
        } else {
            Ok(None)
        }
    }
}

impl TryFrom<usize> for VarInt {
    type Error = std::num::TryFromIntError;

    fn try_from(value: usize) -> std::result::Result<Self, Self::Error> {
        Ok(Self(value.try_into()?))
    }
}
impl TryFrom<VarInt> for usize {
    type Error = std::num::TryFromIntError;

    fn try_from(value: VarInt) -> std::result::Result<Self, Self::Error> {
        value.0.try_into()
    }
}

// primitives:

impl Serializable for f64 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for f64 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 8];

        input.read_exact(&mut bytes)?;

        Ok(f64::from_be_bytes(bytes))
    }
}

impl Serializable for f32 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for f32 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 4];

        input.read_exact(&mut bytes)?;

        Ok(f32::from_be_bytes(bytes))
    }
}

impl Serializable for u8 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for u8 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 1];

        input.read_exact(&mut bytes)?;

        Ok(u8::from_be_bytes(bytes))
    }
}

impl Serializable for i8 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for i8 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 1];

        input.read_exact(&mut bytes)?;

        Ok(i8::from_be_bytes(bytes))
    }
}

impl Serializable for u16 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for u16 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 2];

        input.read_exact(&mut bytes)?;

        Ok(u16::from_be_bytes(bytes))
    }
}

impl Serializable for i16 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for i16 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 2];

        input.read_exact(&mut bytes)?;

        Ok(i16::from_be_bytes(bytes))
    }
}

impl Serializable for u32 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for u32 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 4];

        input.read_exact(&mut bytes)?;

        Ok(u32::from_be_bytes(bytes))
    }
}

impl Serializable for i32 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for i32 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 4];

        input.read_exact(&mut bytes)?;

        Ok(i32::from_be_bytes(bytes))
    }
}

impl Serializable for u64 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for u64 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 8];

        input.read_exact(&mut bytes)?;

        Ok(u64::from_be_bytes(bytes))
    }
}

impl Serializable for i64 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for i64 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 8];

        input.read_exact(&mut bytes)?;

        Ok(i64::from_be_bytes(bytes))
    }
}

impl Serializable for u128 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for u128 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 16];

        input.read_exact(&mut bytes)?;

        Ok(u128::from_be_bytes(bytes))
    }
}

impl Serializable for i128 {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&self.to_be_bytes())
    }
}
impl Deserializable for i128 {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 16];

        input.read_exact(&mut bytes)?;

        Ok(i128::from_be_bytes(bytes))
    }
}

impl Serializable for bool {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        output.write_all(&[*self as u8])
    }
}
impl Deserializable for bool {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 1];

        input.read_exact(&mut bytes)?;

        Ok(bytes[0] != 0)
    }
}

impl<T1: Serializable, T2: Serializable> Serializable for (T1, T2) {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<()> {
        self.0.to_writer(output)?;
        self.1.to_writer(output)?;
        Ok(())
    }
}
impl<T1: Deserializable, T2: Deserializable> Deserializable for (T1, T2) {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        Ok((T1::from_reader(input)?, T2::from_reader(input)?))
    }
}
