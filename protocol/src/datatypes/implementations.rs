use super::*;
use super::{Deserializable, Serializable};
use std::borrow::Cow;
use std::cmp::max;
use std::convert::TryInto;
use std::io::{self, Cursor, ErrorKind, Read, Result, Write};

impl Serializable for ChunkSections {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
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

        let mut sum = 0;

        sum += VarInt(size).to_writer(&mut *output)?;
        for section in &self.0 {
            sum += section.block_count.to_writer(&mut *output)?;
            match &section.palette {
                Palette::Indirect(mappings) => {
                    let bits_per_block = max(
                        4,
                        32u8 - max(mappings.len() as u32 - 1, 1).leading_zeros() as u8,
                    );
                    sum += bits_per_block.to_writer(&mut *output)?;
                    sum += mappings.to_writer(&mut *output)?;
                }
                Palette::Direct => {
                    sum += 15u8.to_writer(&mut *output)?;
                }
            }
            sum += section.data.to_writer(&mut *output)?;
        }

        Ok(sum)
    }
}
impl Deserializable for ChunkSections {
    fn from_reader<R: Read>(_input: &mut R) -> Result<Self> {
        todo!()
    }
}

impl<'a> Serializable for CommandNode<'a> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut sum = 0;

        match self {
            CommandNode::Root { children } => {
                let mut flags = 0u8;
                flags |= 0; // root type
                sum += flags.to_writer(&mut *output)?;
                sum += children.to_writer(&mut *output)?;
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
                sum += flags.to_writer(&mut *output)?;
                sum += children.to_writer(&mut *output)?;
                if let Some(r) = redirect {
                    sum += r.to_writer(&mut *output)?;
                }
                sum += name.to_writer(&mut *output)?;
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
                sum += flags.to_writer(&mut *output)?;
                sum += children.to_writer(&mut *output)?;
                if let Some(r) = redirect {
                    sum += r.to_writer(&mut *output)?;
                }
                sum += name.to_writer(&mut *output)?;
                sum += parser.to_writer(&mut *output)?;
                if let Some(suggestions) = suggestions {
                    sum += suggestions.to_writer(&mut *output)?;
                }
            }
        }

        Ok(sum)
    }
}
impl<'a> Deserializable for CommandNode<'a> {
    fn from_reader<R: Read>(_input: &mut R) -> Result<Self> {
        todo!()
    }
}

impl Serializable for SuggestionsType {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut written = 0;

        match self {
            Self::AskServer => {
                written += "minecraft:ask_server".to_writer(output)?;
            }
            SuggestionsType::AllRecipes => {
                written += "minecraft:all_recipes".to_writer(output)?;
            }
            SuggestionsType::AvailableSounds => {
                written += "minecraft:available_sounds".to_writer(output)?;
            }
            SuggestionsType::SummonableEntities => {
                written += "minecraft:summonable_entities".to_writer(output)?;
            }
        }

        Ok(written)
    }
}
impl Deserializable for SuggestionsType {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let string = String::from_reader(input)?;

        match string.as_str() {
            "minecraft:ask_server" => Ok(Self::AskServer),
            "minecraft:all_recipes" => Ok(Self::AllRecipes),
            "minecraft:available_sounds" => Ok(Self::AvailableSounds),
            "minecraft:summonable_entities" => Ok(Self::SummonableEntities),
            _ => Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "Invalid suggestion type",
            )),
        }
    }
}

impl Serializable for Parser {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut sum = 0;

        match self {
            Parser::String(properties) => {
                sum += "brigadier:string".to_writer(output)?;
                sum += properties.to_writer(output)?;
            }
            Parser::Integer(options) => {
                sum += "brigadier:integer".to_writer(output)?;
                sum += options.to_writer(output)?;
            }
            Parser::Bool => {
                sum += "brigadier:bool".to_writer(output)?;
            }
        }
        Ok(sum)
    }
}

impl Serializable for IntegerParserOptions {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut flags = 0u8;
        if self.min.is_some() {
            flags |= 0x01;
        }
        if self.max.is_some() {
            flags |= 0x02;
        }

        let mut written = 0;

        written += flags.to_writer(output)?;
        if let Some(val) = self.min {
            written += val.to_writer(output)?;
        }
        if let Some(val) = self.max {
            written += val.to_writer(output)?;
        }

        Ok(written)
    }
}
impl Deserializable for IntegerParserOptions {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let flags = u8::from_reader(input)?;

        let min = if flags & 0x01 == 0 {
            None
        } else {
            Some(i32::from_reader(input)?)
        };

        let max = if flags & 0x02 == 0 {
            None
        } else {
            Some(i32::from_reader(input)?)
        };

        Ok(Self { min, max })
    }
}

impl<'a> Serializable for StatusResponse<'a> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        serde_json::to_string(self)?.to_writer(output)
    }
}
impl<'a> Deserializable for Chat<'a> {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        Ok(serde_json::from_str(&String::from_reader(input)?)?)
    }
}

impl<'a> Serializable for EntityMetadata<'a> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut sum = 0;

        for item in &self.0 {
            sum += item.to_writer(output)?;
        }

        sum += 0xFFu8.to_writer(output)?;

        Ok(sum)
    }
}
impl<'a> Deserializable for EntityMetadata<'a> {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let mut res = Vec::new();

        loop {
            let index = u8::from_reader(input)?;
            if index == 0xFF {
                break;
            }

            res.push((index, EntityMetadataEntry::from_reader(input)?));
        }

        Ok(Self(res))
    }
}

impl Serializable for Nbt {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        // there's a problem: the quart_nbt library doesn't return the number of bytes written when writing.
        // so we'll have to make a wrapper around the given Write stream that will count the bytes as they are written
        // and then use that value.
        struct ByteCounter<W: Write> {
            counter: usize,
            stream: W,
        }
        impl<W: Write> Write for ByteCounter<W> {
            fn write(&mut self, buf: &[u8]) -> Result<usize> {
                let written = self.stream.write(buf)?;
                self.counter += written;

                Ok(written)
            }

            fn flush(&mut self) -> Result<()> {
                self.stream.flush()
            }
        }
        let mut output_wrapper = ByteCounter {
            counter: 0,
            stream: output,
        };

        quartz_nbt::write::write_nbt_uncompressed(&mut output_wrapper, "", &self.0)?;

        Ok(output_wrapper.counter)
    }
}
impl Deserializable for Nbt {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        Ok(Self(quartz_nbt::read::read_nbt_uncompressed(input)?.0))
    }
}

impl Serializable for OptionalNbt {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        match &self.0 {
            None => {
                // the TAG_END is 0 so just write a 0
                Ok(0u8.to_writer(output)?)
            }
            Some(nbt) => nbt.to_writer(output),
        }
    }
}
impl Deserializable for OptionalNbt {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        let first_byte = u8::from_reader(input)?;

        if first_byte == 0 {
            Ok(Self(None))
        } else {
            // a wrapper hack so I could peek at the first byte and then reuse it
            struct Wrapper<R>(Option<u8>, R);
            impl<R: Read> Read for Wrapper<R> {
                fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
                    if buf.len() == 0 {
                        return Ok(0);
                    }
                    if let Some(b) = self.0.take() {
                        buf[0] = b;
                        Ok(1)
                    } else {
                        self.1.read(buf)
                    }
                }
            }

            let mut reader = Wrapper(Some(first_byte), input);
            Ok(Self(Some(Nbt::from_reader(&mut reader)?)))
        }
    }
}

impl Serializable for VarInt {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut sum = 0;

        let mut number = self.0 as u32;

        loop {
            let mut byte: u8 = number as u8 & 0b01111111;

            number = number >> 7;
            if number != 0 {
                byte = byte | 0b10000000;
            }

            output.write_all(&[byte])?;
            sum += 1;

            if number == 0 {
                break;
            }
        }

        Ok(sum)
    }
}
impl Deserializable for VarInt {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self>
    where
        Self: Sized,
    {
        let mut i = 0;
        let mut result: i64 = 0;

        loop {
            let mut number = [0];
            input.read_exact(&mut number)?;

            let value = (number[0] & 0b01111111) as i64;
            result = result | (value << (7 * i));

            if (number[0] & 0b10000000) == 0 || i == 4 {
                break;
            }
            i += 1;
        }

        Ok(Self(result as i32))
    }
}

impl Serializable for Position {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        self.as_bytes().to_writer(output)
    }
}
impl Serializable for String {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut sum = 0;

        sum += VarInt(self.len() as i32).to_writer(&mut *output)?;

        for e in self {
            sum += e.to_writer(&mut *output)?;
        }

        Ok(sum)
    }
}

// Box<[T]> are like Vec<T> except that there's no length prefix and you just read to end
impl<T: Serializable> Serializable for Box<[T]> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut sum = 0;

        for e in &**self {
            sum += e.to_writer(&mut *output)?;
        }

        Ok(sum)
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut sum = 0;

        for e in self {
            sum += e.to_writer(&mut *output)?;
        }

        Ok(sum)
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut sum = 0;

        match L::try_from(N) {
            Ok(s) => {
                sum += s.to_writer(&mut *output)?;
            }
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

        sum += self.0.to_writer(&mut *output)?;

        Ok(sum)
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        match self {
            MaybeStatic::Static(bytes) => {
                output.write_all(bytes)?;
                Ok(bytes.len())
            }
            MaybeStatic::Owned(item) => Ok(item.to_writer(output)?),
        }
    }
}
impl<'a, T: Deserializable> Deserializable for MaybeStatic<'a, T> {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        Ok(MaybeStatic::Owned(T::from_reader(input)?))
    }
}

impl<T: Serializable> Serializable for Option<T> {
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut sum = 0;

        match self {
            Some(val) => {
                sum += true.to_writer(output)?;
                sum += val.to_writer(output)?;
            }
            None => sum += false.to_writer(output)?,
        }

        Ok(sum)
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<f64>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<f32>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<u8>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<i8>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<u16>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<i16>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<u32>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<i32>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<u64>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<i64>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<u128>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&self.to_be_bytes())?;
        Ok(std::mem::size_of::<i128>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        output.write_all(&[*self as u8])?;
        Ok(std::mem::size_of::<bool>())
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
    fn to_writer<W: Write>(&self, output: &mut W) -> Result<usize> {
        let mut sum = 0;
        sum += self.0.to_writer(output)?;
        sum += self.1.to_writer(output)?;
        Ok(sum)
    }
}
impl<T1: Deserializable, T2: Deserializable> Deserializable for (T1, T2) {
    fn from_reader<R: Read>(input: &mut R) -> Result<Self> {
        Ok((T1::from_reader(input)?, T2::from_reader(input)?))
    }
}

impl Serializable for () {
    fn to_writer<W: Write>(&self, _output: &mut W) -> Result<usize> {
        Ok(std::mem::size_of::<()>())
    }
}
impl Deserializable for () {
    fn from_reader<R: Read>(_input: &mut R) -> Result<Self> {
        Ok(())
    }
}
