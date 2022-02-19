use protocol::{Deserializable, Serializable};

#[derive(Serializable, Deserializable)]
#[discriminant_as(u16)]
pub enum Foo {
    First,
    Second(String),
    Third { x: f32, y: bool },
    Fourth,
}

#[derive(Serializable, Deserializable)]
pub enum Bar {
    #[discriminant(3)]
    First(i64),
    #[discriminant(-12)]
    Second,
    Third {
        x: f64,
    },
    Fourth,
    Fifth,
    Sixth(i8),
    Seventh {
        x: f32,
        y: f32,
        z: f32,
    },
}

#[derive(Serializable, Deserializable)]
pub enum Boo {}

#[derive(Serializable, Deserializable)]
pub struct Baz {
    first: f32,
    second: bool,
}

#[derive(Serializable, Deserializable)]
pub struct Bazz(pub bool, pub u128);

#[derive(Serializable, Deserializable)]
pub struct Bazzz;

#[test]
fn simple() {}
