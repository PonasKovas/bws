use ironties::TypeInfo;

#[derive(TypeInfo, Debug, Clone, Copy, PartialEq)]
pub struct VarInt(pub i32);
