use crate::{BString, FromBytes, ToBytes, VarInt};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub enum SBLogin {
    LoginStart(LoginStart),
    EncryptionResponse(EncryptionResponse),
    PluginResponse(PluginResponse),
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub enum CBLogin {
    Disconnect(Disconnect),
    EncryptionRequest(EncryptionRequest),
    LoginSuccess(LoginSuccess),
    SetCompression(SetCompression),
    PluginRequest(PluginRequest),
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct LoginStart {
    pub name: BString<16>,
    pub uuid: Option<Uuid>,
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct EncryptionResponse {
    pub shared_secret: Vec<u8>,
    pub verify_token: Vec<u8>,
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct PluginResponse {
    pub message_id: VarInt,
    pub data: Option<Box<[u8]>>,
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct Disconnect {
    pub reason: JsonValue, // todo chat object newtype
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct EncryptionRequest {
    pub server_id: BString<20>,
    pub public_key: Vec<u8>,
    pub verify_token: Vec<u8>,
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct LoginSuccess {
    pub uuid: Uuid,
    pub username: BString<16>,
    pub properties: Vec<Property>,
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct Property {
    pub name: BString<32767>,
    pub value: BString<32767>,
    pub signature: Option<BString<32767>>,
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct SetCompression {
    pub threshold: VarInt,
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct PluginRequest {
    pub message_id: VarInt,
    pub channel: String,
    pub data: Box<[u8]>,
}
