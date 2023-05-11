use ironties::TypeInfo;

mod bstring;
mod varint;

pub use bstring::BString;
pub use varint::VarInt;

#[derive(TypeInfo, Debug, Clone, PartialEq)]
pub struct Handshake {
    pub protocol_version: VarInt,
    pub server_address: BString<255>,
}
