use crate::{FromBytes, ToBytes};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use uuid::Uuid;

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

/// [`StatusResponseBuilder`] for building
#[derive(FromBytes, ToBytes, Debug, Clone, PartialEq)]
pub struct StatusResponse {
    pub json: JsonValue,
}

pub struct StatusResponseBuilder {
    json: JsonValue,
}

impl StatusResponseBuilder {
    pub fn new(version: String, protocol: i32) -> Self {
        Self {
            json: json!({
                "version": {
                    "name": version,
                    "protocol": protocol,
                },
            }),
        }
    }
    pub fn players(mut self, online: i32, max: i32, sample: Vec<PlayerSample>) -> Self {
        self.json["players"] = json!({
            "max": max,
            "online": online,
            "sample": sample,
        });

        self
    }
    pub fn enforces_secure_chat(mut self, enforces: bool) -> Self {
        self.json["enforcesSecureChat"] = json!(enforces);

        self
    }
    pub fn description_raw(mut self, description: JsonValue) -> Self {
        self.json["description"] = description;

        self
    }
    pub fn description(mut self, description: String) -> Self {
        self.json["description"] = json!(description);

        self
    }
    pub fn favicon(mut self, png: Vec<u8>) -> Self {
        let png = base64::engine::general_purpose::STANDARD.encode(png);
        let favicon = format!("data:image/png;base64,{png}");
        self.json["favicon"] = json!(favicon);

        self
    }
    pub fn build(self) -> StatusResponse {
        StatusResponse { json: self.json }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct PlayerSample {
    json: JsonValue,
}

impl PlayerSample {
    pub fn new(username: String, uuid: Uuid) -> Self {
        Self {
            json: json!({
                "name": username,
                "id": uuid,
            }),
        }
    }
    pub fn from_text(text: String) -> Self {
        Self {
            json: json!({
                "name": text,
                "id": "00000000-0000-0000-0000-000000000000",
            }),
        }
    }
}
