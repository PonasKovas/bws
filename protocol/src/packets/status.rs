use crate::{BString, FromBytes, ToBytes};

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
    payload: i64,
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct PingResponse {
    payload: i64,
}

#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct StatusResponse {
    json_response: BString<32767>,
}
