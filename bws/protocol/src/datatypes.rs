pub mod chat_parse;
mod implementations;

use super::{deserializable, serializable};
use super::{Deserializable, Serializable};
use bitflags::bitflags;
use shrinkwraprs::Shrinkwrap;
use std::borrow::Cow;
use std::io::Cursor;

#[derive(Debug, Clone, Copy)]
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
pub struct Nbt(pub quartz_nbt::NbtCompound);

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub struct Angle(pub u8);

impl Angle {
    pub fn from_degrees(degrees: f32) -> Self {
        Self((degrees / 256.0).rem_euclid(1.0) as u8)
    }
}

/// Maybe static. Helps save resources when sending the same fixed data to many clients,
/// because you don't have to clone the data for each one of them, you just serialize a static byte slice
/// Note that the static variant contains ALREADY SERIALIZED bytes
/// Use with caution, nothing's going to stop you from sending invalid datatypes.
#[derive(Debug, Clone)]
pub enum MaybeStatic<T> {
    Static(&'static [u8]),
    Owned(T),
}

impl<T: Deserializable> MaybeStatic<T> {
    pub fn into_owned(self: MaybeStatic<T>) -> T {
        match self {
            MaybeStatic::Static(bytes) => {
                T::from_reader(&mut Cursor::<Vec<u8>>::new(bytes.into())).unwrap()
            }
            MaybeStatic::Owned(item) => item,
        }
    }
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub enum PlayerInfo {
    AddPlayer(Vec<(u128, PlayerInfoAddPlayer)>),
    UpdateGamemode(Vec<(u128, PlayerInfoUpdateGamemode)>),
    UpdateLatency(Vec<(u128, PlayerInfoUpdateLatency)>),
    UpdateDisplayName(Vec<(u128, PlayerInfoUpdateDisplayName)>),
    RemovePlayer(Vec<u128>),
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub struct PlayerInfoAddPlayer {
    pub name: Cow<'static, str>,
    pub properties: Vec<PlayerInfoAddPlayerProperty>,
    pub gamemode: Gamemode,
    pub ping: VarInt,
    pub display_name: Option<Chat>,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub struct PlayerInfoAddPlayerProperty {
    pub name: Cow<'static, str>,
    pub value: Cow<'static, str>,
    pub signature: Option<Cow<'static, str>>,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub struct PlayerInfoUpdateGamemode {
    pub gamemode: Gamemode,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub struct PlayerInfoUpdateLatency {
    /// In milliseconds
    pub ping: VarInt,
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub struct PlayerInfoUpdateDisplayName {
    pub display_name: Option<Chat>,
}

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

// no need for manual Serialization and Deserialization implementation since the first field
// `full_chunk` is a bool and the VarInt enum equivalent of 0 is false, 1 is true so this works
#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub enum Chunk {
    Partial {
        // bits 0-15, if 1 then the chunk section will be sent in this packet
        primary_bitmask: VarInt,
        heightmaps: Nbt,
        sections: ChunkSections,
        block_entities: Vec<Nbt>,
    },
    Full {
        // bits 0-15, if 1 then the chunk section will be sent in this packet
        primary_bitmask: VarInt,
        heightmaps: Nbt,
        // 4x4x4 sections in the entire chunk (16x256x16),
        biomes: ArrWithLen<VarInt, 1024>,
        sections: ChunkSections,
        block_entities: Vec<Nbt>,
    },
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
    pub name: Cow<'static, str>,
    pub entries: Vec<VarInt>,
}

#[derive(Debug, Clone)]
pub struct ChunkSections(pub Vec<ChunkSection>);

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
        name: Cow<'static, str>,
    },
    Argument {
        executable: bool,
        children: Vec<VarInt>,
        redirect: Option<VarInt>,
        name: Cow<'static, str>,
        parser: Parser,
        suggestions: Option<Cow<'static, str>>,
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
    pub favicon: Cow<'static, str>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatusVersion {
    pub name: Cow<'static, str>,
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
    pub name: Cow<'static, str>,
    pub id: Cow<'static, str>,
}

impl StatusPlayerSampleEntry {
    pub fn new(name: Cow<'static, str>) -> Self {
        Self {
            name,
            id: "00000000-0000-0000-0000-000000000000".into(),
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
