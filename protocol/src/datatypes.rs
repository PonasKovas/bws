pub mod chat_parse;
mod implementations;

use super::{deserializable, serializable};
use super::{Deserializable, Serializable};
use bitflags::bitflags;
use shrinkwraprs::Shrinkwrap;
use std::io::{Read, Result, Write};

#[derive(Debug, Clone)]
pub struct VarInt(pub i32);

impl VarInt {
    pub fn size(&self) -> u8 {
        // the inner +6 is so that dividing by 7 would always round up
        std::cmp::max((32 - (self.0 as u32).leading_zeros() + 6) / 7, 1) as u8
    }
}

// A newtype around an array except that when serializing/deserializing it has the fixed length as a prefix
#[derive(Shrinkwrap, Debug, Clone)]
#[shrinkwrap(mutable)]
pub struct ArrWithLen<T, const N: usize>(pub [T; N]);

#[derive(Shrinkwrap, Debug, Clone)]
#[shrinkwrap(mutable)]
pub struct Nbt(pub nbt::Blob);

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub enum WorldBorderAction {
    SetSize {
        diameter: f64,
    },
    LerpSize {
        old_diameter: f64,
        new_diameter: f64,
        speed: VarInt, // real time milliseconds, not ticks
    },
    SetCenter {
        x: f64,
        z: f64,
    },
    Initialize {
        x: f64,
        z: f64,
        old_diameter: f64,
        new_diameter: f64,
        speed: VarInt,
        portal_teleport_boundary: VarInt,
        warning_blocks: VarInt,
        warning_time: VarInt, // in seconds
    },
    SetWarningTime(VarInt),
    SetWarningBlocks(VarInt),
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub enum TitleAction {
    SetTitle(Chat),
    SetSubtitle(Chat),
    SetActionBar(Chat),
    SetDisplayTime {
        // time in ticks
        fade_in: i32,
        display: i32,
        fade_out: i32,
    },
    Hide,
    Reset,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub struct Tags {
    name: String,
    entries: Vec<VarInt>,
}

#[derive(Debug, Clone)]
pub struct ChunkSections(Vec<ChunkSection>);

#[derive(Debug, Clone)]
pub struct ChunkSection {
    // number of non-air blocks in the chuck section, for lighting purposes.
    pub block_count: i16,
    pub palette: Palette,
    pub data: Vec<i64>,
}

#[derive(Debug, Clone)]
pub enum Palette {
    Indirect(Vec<VarInt>),
    Direct,
}

#[derive(Debug, Clone)]
pub enum CommandNode {
    Root {
        // indices of the children
        children: Vec<VarInt>,
    },
    Literal {
        executable: bool,
        children: Vec<VarInt>,
        redirect: Option<VarInt>,
        name: String,
    },
    Argument {
        executable: bool,
        children: Vec<VarInt>,
        redirect: Option<VarInt>,
        name: String,
        parser: Parser,
        suggestions: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum Parser {
    String(StringParserType),
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone, Copy)]
pub enum StringParserType {
    SingleWord = 0,
    QuotablePhrase,
    GreedyPhrase,
}

bitflags! {
    #[deserializable]
    #[serializable]
    pub struct PlayerAbilities: u8 {
        const INVULNERABLE = 0x01;
        const FLYING = 0x02;
        const ALLOW_FLYING = 0x04;
        const INSTANT_BREAK = 0x08;
    }
}
bitflags! {
    #[deserializable]
    #[serializable]
    pub struct PositionAndLookFlags: u8 {
        const RELATIVE_X = 0x01;
        const RELATIVE_Y = 0x02;
        const RELATIVE_Z = 0x04;
        const RELATIVE_YAW = 0x08; // i have possibly mixed up yaw and pitch here
        const RELATIVE_PITCH = 0x10;
    }
}

bitflags! {
    #[deserializable]
    #[serializable]
    pub struct SkinParts: u8 {
        const CAPE = 0x01;
        const JACKET = 0x02;
        const LEFT_SLEEVE = 0x04;
        const RIGHT_SLEEVE = 0x08;
        const LEFT_PANTS = 0x10;
        const RIGHT_PANTS = 0x20;
        const HAT = 0x40;
    }
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone, Copy)]
pub enum NextState {
    Status = 1,
    Login,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone, Copy)]
pub enum Difficulty {
    Peaceful = 0,
    Easy,
    Normal,
    Hard,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone, Copy)]
pub enum ClientStatusAction {
    PerformRespawn = 0,
    RequestStats,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone, Copy)]
pub enum ChatMode {
    Enabled = 0,
    CommandsOnly,
    Hidden,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone, Copy)]
pub enum MainHand {
    Left = 0,
    Right,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone, Copy)]
pub enum ChatPosition {
    Chat = 0,
    System,
    AboveHotbar,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone, Copy)]
pub enum Gamemode {
    Survival = 0,
    Creative,
    Adventure,
    Spectator,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone, Copy)]
pub enum SoundCategory {
    Master = 0,
    Music,
    Records,
    Weather,
    Blocks,
    Hostile,
    Neutral,
    Players,
    Ambient,
    Voice,
}

#[derive(Debug, Clone)]
pub struct StatusResponse {
    pub json: StatusResponseJson,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusResponseJson {
    pub version: StatusVersion,
    pub players: StatusPlayers,
    pub description: Chat,
    pub favicon: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusVersion {
    pub name: String,
    pub protocol: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusPlayers {
    pub max: i32,
    pub online: i32,
    pub sample: Vec<StatusPlayerSampleEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusPlayerSampleEntry {
    pub name: String,
    pub id: String,
}

impl StatusPlayerSampleEntry {
    pub fn new(name: String) -> Self {
        Self {
            name,
            id: "00000000-0000-0000-0000-000000000000".to_string(),
        }
    }
}

// chat objects are represented in JSON so we use serde
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Chat {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underlined: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfuscated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Chat>,
}

impl Chat {
    pub fn new() -> Self {
        Self {
            text: "".to_string(),
            bold: None,
            italic: None,
            underlined: None,
            strikethrough: None,
            obfuscated: None,
            color: None,
            extra: Vec::new(),
        }
    }
}
