use ironties::TypeInfo;

#[derive(TypeInfo, Debug, Clone, PartialEq)]
pub struct BString<const MAX: usize>(String);
