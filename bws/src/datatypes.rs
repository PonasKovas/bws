use std::io::{self, Cursor, Read, Write};
use tokio::{io::BufReader, net::TcpStream};

// A data type that is used in the minecraft protocol
// all info available on https://wiki.vg/index.php?title=Protocol
pub trait DataType {
    fn serialize<W: Write>(&self, output: &mut W);
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self>
    where
        Self: Sized;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VarInt(pub i32);

#[derive(Clone, Debug, Copy)]
pub enum Slot {
    Present(VarInt, i8), // TODO: this has NBT data too
    NotPresent,
}

#[derive(Debug, Clone)]
pub struct ChunkSection {
    // number of non-air blocks in the chuck section, for lighting purposes.
    pub block_count: i16,
    pub palette: Palette,
    pub data: Vec<i64>,
}

#[derive(Debug, Clone)]
pub enum Palette {
    Indirect(Vec<VarInt>),
    Direct,
}

#[derive(Debug, Clone)]
pub struct Chat(pub String);

// Used in DeclareCommands packet
#[derive(Debug, Clone)]
pub enum CommandNode {
    Root(Vec<VarInt>),                                  // child nodes indices
    Literal(bool, Vec<VarInt>, Option<VarInt>, String), // executable, child nodes indices, redirect, name
    Argument(bool, Vec<VarInt>, Option<VarInt>, String, Parser, bool), // executable, child nodes indices, redirect, name, parser, whether has suggestions
}

#[derive(Debug, Clone)]
pub enum Parser {
    String(VarInt), // type, 0 - SINGLE_WORD, 1 - QUOTABLE_PHRASE, 2 - GREEDY_PHRASE
}

impl VarInt {
    pub fn size(&self) -> u8 {
        // the inner +6 is so that dividing by 7 would always round up
        std::cmp::max((32 - (self.0 as u32).leading_zeros() + 6) / 7, 1) as u8
    }
    pub async fn read(input: &mut BufReader<TcpStream>) -> io::Result<Self> {
        use tokio::io::AsyncReadExt;

        let mut i = 0;
        let mut result: i32 = 0;

        loop {
            let number = input.read_u8().await?;

            let value = (number & 0b01111111) as i32;
            result = result | (value << (7 * i));

            if (number & 0b10000000) == 0 {
                break;
            }
            i += 1;
        }

        Ok(Self(result))
    }
    pub async fn write(&self, output: &mut BufReader<TcpStream>) -> io::Result<()> {
        use tokio::io::AsyncWriteExt;

        let mut number = self.0 as u32;

        loop {
            let mut byte: u8 = number as u8 & 0b01111111;

            number = number >> 7;
            if number != 0 {
                byte = byte | 0b10000000;
            }

            output.write_u8(byte).await?;

            if number == 0 {
                break;
            }
        }

        Ok(())
    }
}

// DataType implementations //
//////////////////////////////

impl DataType for VarInt {
    fn serialize<W: Write>(&self, output: &mut W) {
        let mut number = self.0 as u32;

        loop {
            let mut byte: u8 = number as u8 & 0b01111111;

            number = number >> 7;
            if number != 0 {
                byte = byte | 0b10000000;
            }

            output.write_all(&[byte]).unwrap();

            if number == 0 {
                break;
            }
        }
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut i = 0;
        let mut result: i32 = 0;

        let mut number = [0];
        loop {
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

impl DataType for String {
    fn serialize<W: Write>(&self, output: &mut W) {
        // string length as VarInt
        VarInt(self.len() as i32).serialize(output);
        // the actual string bytes
        output.write_all(self.as_bytes()).unwrap();
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let string_length = VarInt::deserialize(input)?;

        let mut string = vec![0; string_length.0 as usize];
        input.read_exact(&mut string[..])?;
        let string = String::from_utf8_lossy(&string).into_owned();

        Ok(string)
    }
}

impl DataType for Chat {
    fn serialize<W: Write>(&self, output: &mut W) {
        // since its just a newtype for string
        self.0.serialize(output);
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        Ok(Self(String::deserialize(input)?))
    }
}

impl DataType for Palette {
    fn serialize<W: Write>(&self, output: &mut W) {
        match self {
            Palette::Indirect(palette) => {
                let bits_per_block = std::cmp::max(
                    4,
                    32u8 - std::cmp::max(palette.len() as u32 - 1, 1).leading_zeros() as u8,
                );
                bits_per_block.serialize(output);
                palette.serialize(output);
            }
            Palette::Direct => {
                (15u8).serialize(output);
            }
        }
    }
    fn deserialize<R: Read>(_input: &mut R) -> io::Result<Self> {
        // not sure if the client ever sends palettes :/
        unimplemented!();
    }
}

impl DataType for CommandNode {
    fn serialize<W: Write>(&self, output: &mut W) {
        let mut flags = 0u8;
        match self {
            Self::Root(children) => {
                flags.serialize(output);
                children.serialize(output);
            }
            Self::Literal(executable, children, redirect, name) => {
                flags = flags | 0x01;
                if *executable {
                    flags = flags | 0x04;
                }
                if let Some(_) = redirect {
                    flags = flags | 0x08;
                }
                flags.serialize(output);
                children.serialize(output);
                if let Some(redirect) = redirect {
                    redirect.serialize(output);
                }
                name.serialize(output);
            }
            Self::Argument(executable, children, redirect, name, parser, has_suggestions) => {
                flags = flags | 0x02;
                if *executable {
                    flags = flags | 0x04;
                }
                if let Some(_) = redirect {
                    flags = flags | 0x08;
                }
                if *has_suggestions {
                    flags = flags | 0x10;
                }
                flags.serialize(output);
                children.serialize(output);
                if let Some(redirect) = redirect {
                    redirect.serialize(output);
                }
                name.serialize(output);
                parser.serialize(output);
                if *has_suggestions {
                    "minecraft:ask_server".to_string().serialize(output); // always ask server
                }
            }
        }
    }
    fn deserialize<R: Read>(_input: &mut R) -> io::Result<Self> {
        unimplemented!();
    }
}

impl DataType for Parser {
    fn serialize<W: Write>(&self, output: &mut W) {
        match self {
            Parser::String(properties) => {
                "brigadier:string".to_string().serialize(output);

                properties.serialize(output);
            }
        }
    }
    fn deserialize<R: Read>(_input: &mut R) -> io::Result<Self> {
        unimplemented!();
    }
}

impl DataType for ChunkSection {
    fn serialize<W: Write>(&self, output: &mut W) {
        self.block_count.serialize(output);

        self.palette.serialize(output);

        self.data.serialize(output);
    }
    fn deserialize<R: Read>(_input: &mut R) -> io::Result<Self> {
        // not sure if the client ever sends chunk sections either
        unimplemented!();
    }
}

impl DataType for Slot {
    fn serialize<W: Write>(&self, output: &mut W) {
        match self {
            Slot::Present(id, number) => {
                true.serialize(output);

                id.serialize(output);
                number.serialize(output);
            }
            Slot::NotPresent => {
                false.serialize(output);
            }
        }
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        if bool::deserialize(input)? {
            Ok(Self::Present(
                VarInt::deserialize(input)?,
                i8::deserialize(input)?,
            ))
        } else {
            Ok(Self::NotPresent)
        }
    }
}

impl<T: DataType> DataType for Vec<T> {
    fn serialize<W: Write>(&self, output: &mut W) {
        // vec length as VarInt
        let size = self.len();
        VarInt(size as i32).serialize(output);

        // the actual data
        for item in self {
            item.serialize(output);
        }
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let vec_size = VarInt::deserialize(input)?;

        let mut data = Vec::with_capacity(vec_size.0 as usize);
        for _ in 0..vec_size.0 {
            data.push(T::deserialize(input)?);
        }

        Ok(data)
    }
}

impl<T: DataType, F: DataType> DataType for (T, F) {
    fn serialize<W: Write>(&self, output: &mut W) {
        self.0.serialize(output);
        self.1.serialize(output);
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        Ok((T::deserialize(input)?, F::deserialize(input)?))
    }
}

impl DataType for u16 {
    fn serialize<W: Write>(&self, output: &mut W) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 2];

        input.read_exact(&mut bytes)?;

        Ok(u16::from_be_bytes(bytes))
    }
}

impl DataType for i32 {
    fn serialize<W: Write>(&self, output: &mut W) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 4];

        input.read_exact(&mut bytes)?;

        Ok(i32::from_be_bytes(bytes))
    }
}

impl DataType for i16 {
    fn serialize<W: Write>(&self, output: &mut W) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 2];

        input.read_exact(&mut bytes)?;

        Ok(i16::from_be_bytes(bytes))
    }
}

impl DataType for i8 {
    fn serialize<W: Write>(&self, output: &mut W) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 1];

        input.read_exact(&mut bytes)?;

        Ok(i8::from_be_bytes(bytes))
    }
}

impl DataType for i64 {
    fn serialize<W: Write>(&self, output: &mut W) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 8];

        input.read_exact(&mut bytes)?;

        Ok(i64::from_be_bytes(bytes))
    }
}

impl DataType for f32 {
    fn serialize<W: Write>(&self, output: &mut W) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 4];

        input.read_exact(&mut bytes)?;

        Ok(f32::from_be_bytes(bytes))
    }
}

impl DataType for f64 {
    fn serialize<W: Write>(&self, output: &mut W) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 8];

        input.read_exact(&mut bytes)?;

        Ok(f64::from_be_bytes(bytes))
    }
}

impl DataType for u8 {
    fn serialize<W: Write>(&self, output: &mut W) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 1];

        input.read_exact(&mut bytes)?;

        Ok(u8::from_be_bytes(bytes))
    }
}

impl DataType for bool {
    fn serialize<W: Write>(&self, output: &mut W) {
        output.write_all(&[*self as u8]).unwrap();
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut x = [0];
        input.read_exact(&mut x)?;

        Ok(x[0] == 1)
    }
}

impl DataType for u128 {
    fn serialize<W: Write>(&self, output: &mut W) {
        // nice format, mojang
        output
            .write_all(&mut ((self >> 64) as u64).to_be_bytes())
            .unwrap();
        output.write_all(&mut (*self as u64).to_be_bytes()).unwrap();
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut bytes = [0u8; 8];
        input.read_exact(&mut bytes)?;
        let mut number = (u64::from_be_bytes(bytes) as u128) << 64;

        let mut bytes = [0u8; 8];
        input.read_exact(&mut bytes)?;
        number |= u64::from_be_bytes(bytes) as u128;

        Ok(number)
    }
}

// this is epic. thank you rust, very cool.
impl<T: DataType, const N: usize> DataType for [T; N] {
    fn serialize<W: Write>(&self, output: &mut W) {
        for i in 0..N {
            self[i].serialize(output);
        }
    }
    fn deserialize<R: Read>(input: &mut R) -> io::Result<Self> {
        let mut result = [(); N].map(|_| None); // Option<T> is not Copy :/

        for i in 0..N {
            result[i] = Some(T::deserialize(input)?);
        }

        Ok(result.map(|e| e.unwrap()))
    }
}
