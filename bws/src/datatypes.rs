use std::io::{self, Cursor, Read, Write};
use tokio::{io::BufReader, net::TcpStream};

// A data type that is used in the minecraft protocol
// all info available on https://wiki.vg/index.php?title=Protocol
pub trait DataType {
    fn serialize(self, output: &mut Vec<u8>);
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self>
    where
        Self: Sized;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VarInt(pub i64);

#[derive(Clone, Debug)]
pub struct MString(pub String);

#[derive(Clone, Debug, Copy)]
pub enum Slot {
    Present(VarInt, i8), // TODO: this has NBT data too
    NotPresent,
}

impl VarInt {
    pub fn size(&self) -> u8 {
        let mut bytes = 0;
        let mut temp = self.0;
        loop {
            bytes += 1;
            temp = temp >> 7;
            if temp == 0 {
                break;
            }
        }

        bytes
    }
    pub async fn read(input: &mut BufReader<TcpStream>) -> io::Result<Self> {
        use tokio::io::AsyncReadExt;

        let mut i = 0;
        let mut result: i64 = 0;

        loop {
            let number = input.read_u8().await?;

            let value = (number & 0b01111111) as i64;
            result = result | (value << (7 * i));

            if (number & 0b10000000) == 0 {
                break;
            }
            i += 1;
        }

        Ok(Self(result))
    }
    pub async fn write(self, output: &mut BufReader<TcpStream>) -> io::Result<()> {
        use tokio::io::AsyncWriteExt;

        let mut number = (self.0 as u64) as i64;

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
    fn serialize(self, output: &mut Vec<u8>) {
        let mut number = (self.0 as u64) as i64;

        loop {
            let mut byte: u8 = number as u8 & 0b01111111;

            number = number >> 7;
            if number != 0 {
                byte = byte | 0b10000000;
            }

            output.push(byte);

            if number == 0 {
                break;
            }
        }
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let mut i = 0;
        let mut result: i64 = 0;

        let mut number = [0];
        loop {
            input.read_exact(&mut number)?;

            let value = (number[0] & 0b01111111) as i64;
            result = result | (value << (7 * i));

            if (number[0] & 0b10000000) == 0 {
                break;
            }
            i += 1;
        }

        Ok(Self(result))
    }
}

impl DataType for MString {
    fn serialize(self, output: &mut Vec<u8>) {
        // string length as VarInt
        VarInt(self.0.len() as i64).serialize(output);
        // the actual string bytes
        output.write_all(self.0.as_bytes()).unwrap();
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let string_length = VarInt::deserialize(input)?;

        let mut string = vec![0; string_length.0 as usize];
        input.read_exact(&mut string[..])?;
        let string = String::from_utf8_lossy(&string).into_owned();

        Ok(MString(string))
    }
}

impl DataType for Slot {
    fn serialize(self, output: &mut Vec<u8>) {
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
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
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
    fn serialize(self, output: &mut Vec<u8>) {
        // vec length as VarInt
        let size = self.len();
        VarInt(size as i64).serialize(output);

        // the actual data
        for item in self {
            item.serialize(output);
        }
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let vec_size = VarInt::deserialize(input)?;

        let mut data = Vec::with_capacity(vec_size.0 as usize);
        for _ in 0..vec_size.0 {
            data.push(T::deserialize(input)?);
        }

        Ok(data)
    }
}

impl DataType for u16 {
    fn serialize(self, output: &mut Vec<u8>) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let mut bytes = [0u8; 2];

        input.read_exact(&mut bytes)?;

        Ok(u16::from_be_bytes(bytes))
    }
}

impl DataType for i32 {
    fn serialize(self, output: &mut Vec<u8>) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let mut bytes = [0u8; 4];

        input.read_exact(&mut bytes)?;

        Ok(i32::from_be_bytes(bytes))
    }
}

impl DataType for i16 {
    fn serialize(self, output: &mut Vec<u8>) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let mut bytes = [0u8; 2];

        input.read_exact(&mut bytes)?;

        Ok(i16::from_be_bytes(bytes))
    }
}

impl DataType for i8 {
    fn serialize(self, output: &mut Vec<u8>) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let mut bytes = [0u8; 1];

        input.read_exact(&mut bytes)?;

        Ok(i8::from_be_bytes(bytes))
    }
}

impl DataType for i64 {
    fn serialize(self, output: &mut Vec<u8>) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let mut bytes = [0u8; 8];

        input.read_exact(&mut bytes)?;

        Ok(i64::from_be_bytes(bytes))
    }
}

impl DataType for f32 {
    fn serialize(self, output: &mut Vec<u8>) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let mut bytes = [0u8; 4];

        input.read_exact(&mut bytes)?;

        Ok(f32::from_be_bytes(bytes))
    }
}

impl DataType for f64 {
    fn serialize(self, output: &mut Vec<u8>) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let mut bytes = [0u8; 8];

        input.read_exact(&mut bytes)?;

        Ok(f64::from_be_bytes(bytes))
    }
}

impl DataType for u8 {
    fn serialize(self, output: &mut Vec<u8>) {
        output.write_all(&mut self.to_be_bytes()).unwrap();
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let mut bytes = [0u8; 1];

        input.read_exact(&mut bytes)?;

        Ok(u8::from_be_bytes(bytes))
    }
}

impl DataType for bool {
    fn serialize(self, output: &mut Vec<u8>) {
        output.write_all(&[self as u8]).unwrap();
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let mut x = [0];
        input.read_exact(&mut x)?;

        Ok(x[0] == 1)
    }
}

impl DataType for u128 {
    fn serialize(self, output: &mut Vec<u8>) {
        // nice format, mojang
        output
            .write_all(&mut ((self >> 64) as u64).to_be_bytes())
            .unwrap();
        output.write_all(&mut (self as u64).to_be_bytes()).unwrap();
    }
    fn deserialize(input: &mut Cursor<&Vec<u8>>) -> io::Result<Self> {
        let mut bytes = [0u8; 8];
        input.read_exact(&mut bytes)?;
        let mut number = (u64::from_be_bytes(bytes) as u128) << 64;

        let mut bytes = [0u8; 8];
        input.read_exact(&mut bytes)?;
        number |= u64::from_be_bytes(bytes) as u128;

        Ok(number)
    }
}
