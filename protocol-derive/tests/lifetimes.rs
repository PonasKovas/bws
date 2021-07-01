use std::borrow::Cow;

use protocol::Serializable;

#[derive(Serializable)]
pub enum Enum<'a> {
    First,
    Second(Cow<'a, str>),
}

#[derive(Serializable)]
pub struct Struct<'a> {
    first: Cow<'a, str>,
    second: Cow<'a, str>,
}

#[test]
fn lifetimes() {}
