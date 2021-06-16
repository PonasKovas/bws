use protocol::{datatypes::VarInt, Deserializable, Serializable};
use protocol_derive::{deserializable, serializable};

#[serializable]
#[deserializable]
pub enum Enum<'a> {
    First,
    Second(&'a str),
}

#[serializable]
#[deserializable]
pub struct Struct<'a> {
    first: &'a u32,
    second: &'a mut i16,
}

#[test]
fn lifetimes() {}
