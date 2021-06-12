use super::datatypes::*;
use crate::world;
use nbt::Blob as Nbt;
use serde::{Deserialize, Serialize};
use std::{
    env::Vars,
    hash,
    io::{self, Cursor, Write},
};

// Sent from the server to the client
#[derive(Debug, Clone)]
pub enum ClientBound {
    HandShake, // No packets are sent from the server in the HandShake state
    Status(StatusClientBound),
    Login(LoginClientBound),
    Play(PlayClientBound),
}

#[derive(Serialize, Debug, Clone)]
pub enum StatusClientBound {
    Response(StatusResponse),
    Pong(i64),
}

#[derive(Serialize, Debug, Clone)]
pub enum LoginClientBound {
    Disconnect(Chat),
    EncryptionRequest {
        // Up to 20 characters
        server_id: String,
        public_key: Vec<u8>,
        verify_token: Vec<u8>,
    },
    LoginSuccess {
        uuid: u128,
        username: String,
    },
    SetCompression {
        treshold: VarInt,
    },
    PluginRequest {
        message_id: VarInt,
        channel: String,
        // the client figures out the length based on the packet size
        data: Box<[u8]>,
    },
}

#[derive(Serialize, Debug, Clone)]
pub enum PlayClientBound {
    SpawnEntity {
        entity_id: VarInt,
        object_uuid: u128,
        entity_type: VarInt,
        x: f64,
        y: f64,
        z: f64,
        pitch: f32,
        yaw: f32,
        data: i32,
        velocity_x: i16,
        velocity_y: i16,
        velocity_z: i16,
    },
}

impl StatusClientBound {
    pub fn cb(self) -> ClientBound {
        ClientBound::Status(self)
    }
}
impl LoginClientBound {
    pub fn cb(self) -> ClientBound {
        ClientBound::Login(self)
    }
}
impl PlayClientBound {
    pub fn cb(self) -> ClientBound {
        ClientBound::Play(self)
    }
}

impl Serialize for ClientBound {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::HandShake => ().serialize(serializer),
            Self::Status(packet) => packet.serialize(serializer),
            Self::Login(packet) => packet.serialize(serializer),
            Self::Play(packet) => packet.serialize(serializer),
        }
    }
}
