use crate::{FromBytes, ToBytes};
use serde_json::Value as JsonValue;

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub enum SBStatus {
    StatusRequest,
    PingRequest(PingRequest),
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub enum CBStatus {
    StatusResponse(StatusResponse),
    PingResponse(PingResponse),
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct PingRequest {
    pub payload: i64,
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct PingResponse {
    pub payload: i64,
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct StatusResponse {
    pub json: JsonValue,
}
