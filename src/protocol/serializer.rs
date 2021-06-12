use super::datatypes::VarInt;
use serde::{ser, Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt::Display,
    io::{self, Cursor, Read, Write},
};
use thiserror::Error;
use tokio::{io::BufReader, net::TcpStream};

#[derive(Error, Debug)]
pub struct Error(io::Error);

pub struct MinecraftProtocolSerializer<W: Write> {
    output: W,
}

pub fn to_writer<T: Serialize, W: Write>(output: W, value: &T) -> Result<(), Error> {
    let mut serializer = MinecraftProtocolSerializer { output };

    value.serialize(&mut serializer)
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Self(io::Error::new(io::ErrorKind::Other, format!("{}", msg)))
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self(e)
    }
}

impl<'a, W: Write> Serializer for &'a mut MinecraftProtocolSerializer<W> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<(), Error> {
        Ok(self.output.write_all(&[v as u8])?)
    }
    fn serialize_i8(self, v: i8) -> Result<(), Error> {
        Ok(self.output.write_all(&v.to_be_bytes())?)
    }
    fn serialize_i16(self, v: i16) -> Result<(), Error> {
        Ok(self.output.write_all(&v.to_be_bytes())?)
    }
    fn serialize_i32(self, v: i32) -> Result<(), Error> {
        Ok(self.output.write_all(&v.to_be_bytes())?)
    }
    fn serialize_i64(self, v: i64) -> Result<(), Error> {
        Ok(self.output.write_all(&v.to_be_bytes())?)
    }
    fn serialize_u8(self, v: u8) -> Result<(), Error> {
        Ok(self.output.write_all(&v.to_be_bytes())?)
    }
    fn serialize_u16(self, v: u16) -> Result<(), Error> {
        Ok(self.output.write_all(&v.to_be_bytes())?)
    }
    fn serialize_u32(self, v: u32) -> Result<(), Error> {
        Ok(self.output.write_all(&v.to_be_bytes())?)
    }
    fn serialize_u64(self, v: u64) -> Result<(), Error> {
        Ok(self.output.write_all(&v.to_be_bytes())?)
    }
    fn serialize_f32(self, v: f32) -> Result<(), Error> {
        Ok(self.output.write_all(&v.to_be_bytes())?)
    }
    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        Ok(self.output.write_all(&v.to_be_bytes())?)
    }
    fn serialize_char(self, _v: char) -> Result<(), Error> {
        // This format does not support chars
        Err(io::Error::new(
            io::ErrorKind::Other,
            "Minecraft Protocol does not support serializing chars.",
        ))?
    }
    fn serialize_str(self, v: &str) -> Result<(), Error> {
        // length of the string as a varint
        VarInt(v.len() as i32).serialize(&mut *self)?;
        // the actual string bytes
        Ok(self.output.write_all(v.as_bytes())?)
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        Ok(self.output.write_all(v)?)
    }
    fn serialize_none(self) -> Result<(), Error> {
        Ok(())
    }
    fn serialize_some<T>(self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Error> {
        Ok(())
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        Ok(())
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
    ) -> Result<(), Error> {
        VarInt(variant_index as i32).serialize(self)
    }
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        VarInt(variant_index as i32).serialize(&mut *self)?;
        value.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        // Most sized sequences are prefixed by a VarInt
        if let Some(len) = len {
            VarInt(len as i32).serialize(&mut *self)?;
        }
        // if the size isn't known then its probably implicit
        Ok(self)
    }
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        // if it's a tuple, that means the size is known on both ends
        self.serialize_seq(None)
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        // Same as above
        self.serialize_seq(None)
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        VarInt(variant_index as i32).serialize(&mut *self)?;
        Ok(self)
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        unimplemented!()
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Ok(self)
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        VarInt(variant_index as i32).serialize(&mut *self)?;
        Ok(self)
    }
}

impl<'a, W: Write> ser::SerializeSeq for &'a mut MinecraftProtocolSerializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }
    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, W: Write> ser::SerializeTuple for &'a mut MinecraftProtocolSerializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, W: Write> ser::SerializeTupleStruct for &'a mut MinecraftProtocolSerializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, W: Write> ser::SerializeTupleVariant for &'a mut MinecraftProtocolSerializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, W: Write> ser::SerializeMap for &'a mut MinecraftProtocolSerializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, _key: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        unimplemented!()
    }

    fn serialize_value<T>(&mut self, _value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        unimplemented!()
    }

    fn end(self) -> Result<(), Error> {
        unimplemented!()
    }
}

impl<'a, W: Write> ser::SerializeStruct for &'a mut MinecraftProtocolSerializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, W: Write> ser::SerializeStructVariant for &'a mut MinecraftProtocolSerializer<W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}
