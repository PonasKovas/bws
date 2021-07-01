use protocol::{Deserializable, Serializable};

#[derive(Serializable, Deserializable)]
#[discriminant_as(u16)]
pub enum BaseEnum {
    First,
    Second(String),
}

#[derive(Serializable, Deserializable)]
#[discriminant_as(i64)]
pub enum Child {
    #[inline_enum]
    Base(BaseEnum),
    #[discriminant(3)]
    Fourth(i16),
    Fifth {
        x: f32,
        y: f32,
        z: f32,
    },
}

#[test]
fn enum_inlining() {}
