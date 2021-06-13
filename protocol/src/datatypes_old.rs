use bitflags::bitflags;
use serde::ser::SerializeSeq;
use serde::{ser, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{from_reader, from_str, to_string, to_writer};
use shrinkwraprs::Shrinkwrap;
use std::{
    fmt::Display,
    io::{self, Cursor, Read, Write},
};
use thiserror::Error;
use tokio::{io::BufReader, net::TcpStream};

// Serialize and Deserialize implemented manually
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VarInt(pub i32);

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

// Used in DeclareCommands packet
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

bitflags! {
    pub struct PlayerAbilities: u8 {
        const INVULNERABLE = 0x01;
        const FLYING = 0x02;
        const ALLOW_FLYING = 0x04;
        const INSTANT_BREAK = 0x08;
    }
}
bitflags! {
    pub struct PositionAndLookFlags: u8 {
        const RELATIVE_X = 0x01;
        const RELATIVE_Y = 0x02;
        const RELATIVE_Z = 0x04;
        const RELATIVE_YAW = 0x08; // i have possibly mixed up yaw and pitch here
        const RELATIVE_PITCH = 0x10;
    }
}

bitflags! {
    #[derive(Serialize)]
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

#[derive(Serialize, Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum Parser {
    String(StringParserType),
}

#[derive(Serialize, Debug, Clone)]
pub enum StringParserType {
    SingleWord = 0,
    QuotablePhrase,
    GreedyPhrase,
}

#[derive(Serialize, Debug, Clone)]
pub enum NextState {
    Status = 1,
    Login,
}

#[derive(Shrinkwrap, Debug, Clone)]
#[shrinkwrap(mutable)]
pub struct Arr<T: Serialize, const N: usize>(pub [T; N]);

/// Array with it's length prefixed as a VarInt
#[derive(Shrinkwrap, Debug, Clone)]
#[shrinkwrap(mutable)]
pub struct ArrWithLen<T: Serialize, const N: usize>(pub [T; N]);

#[derive(Shrinkwrap, Debug, Clone)]
#[shrinkwrap(mutable)]
pub struct Nbt(pub nbt::Blob);

#[derive(Debug, Clone)]
pub struct StatusResponse {
    pub json: StatusResponseJson,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusResponseJson {
    pub version: StatusVersion,
    pub players: StatusPlayers,
    pub description: Chat,
    pub favicon: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusVersion {
    pub name: String,
    pub protocol: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusPlayers {
    pub max: i32,
    pub online: i32,
    pub sample: Vec<StatusPlayerSampleEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusPlayerSampleEntry {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum Difficulty {
    Peaceful = 0,
    Easy,
    Normal,
    Hard,
}

#[derive(Debug, Clone, Serialize)]
pub enum ClientStatusAction {
    PerformRespawn = 0,
    RequestStats,
}

#[derive(Debug, Clone, Serialize)]
pub enum ChatMode {
    Enabled = 0,
    CommandsOnly,
    Hidden,
}

#[derive(Debug, Clone, Serialize)]
pub enum MainHand {
    Left = 0,
    Right,
}

#[derive(Debug, Clone, Serialize)]
pub enum ChatPosition {
    Chat = 0,
    System,
    AboveHotbar,
}

#[derive(Serialize, Debug, Clone)]
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

#[derive(Serialize, Debug, Clone)]
pub struct Tags {
    name: String,
    entries: Vec<VarInt>,
}

#[derive(Serialize, Debug, Clone)]
pub enum Gamemode {
    Survival = 0,
    Creative,
    Adventure,
    Spectator,
}

#[derive(Serialize, Debug, Clone)]
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

impl StatusPlayerSampleEntry {
    pub fn new(name: String) -> Self {
        Self {
            name,
            id: "00000000-0000-0000-0000-000000000000".to_string(),
        }
    }
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

impl VarInt {
    pub fn size(&self) -> u8 {
        // the inner +6 is so that dividing by 7 would always round up
        std::cmp::max((32 - (self.0 as u32).leading_zeros() + 6) / 7, 1) as u8
    }
    pub async fn read(input: &mut BufReader<TcpStream>) -> io::Result<Self> {
        use tokio::io::AsyncReadExt;

        let mut i = 0;
        let mut result: i32 = 0;

        loop {
            let number = input.read_u8().await?;

            let value = (number & 0b01111111) as i32;
            result = result | (value << (7 * i));

            if (number & 0b10000000) == 0 {
                break;
            }
            i += 1;
        }

        Ok(Self(result))
    }
    pub async fn write(&self, output: &mut BufReader<TcpStream>) -> io::Result<()> {
        use tokio::io::AsyncWriteExt;

        let mut number = self.0 as u32;

        loop {
            let mut byte: u8 = number as u8 & 0b01111111;

            number = number >> 7;
            if number != 0 {
                byte = byte | 0b10000000;
            }

            output.write_u8(byte).await?;

            if number == 0 {
                break;
            }
        }

        Ok(())
    }
}

impl Serialize for StatusResponse {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&serde_json::to_string(&self.json).unwrap())
    }
}

impl Serialize for PlayerAbilities {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.bits())
    }
}
impl Serialize for PositionAndLookFlags {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.bits())
    }
}

impl Serialize for Nbt {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // this is definetely not the most efficient way but whatever
        let mut buffer = Vec::with_capacity(self.0.len_bytes());
        self.to_writer(&mut buffer).unwrap();

        serializer.serialize_bytes(&buffer)
    }
}

impl<T: Serialize, const N: usize> Serialize for Arr<T, N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_seq(None)?;

        for i in 0..N {
            s.serialize_element(&self[i])?;
        }

        s.end()
    }
}

impl<T: Serialize, const N: usize> Serialize for ArrWithLen<T, N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_seq(Some(N))?;

        for i in 0..N {
            s.serialize_element(&self[i])?;
        }

        s.end()
    }
}

impl Serialize for CommandNode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_seq(None)?;

        match self {
            CommandNode::Root { children } => {
                let mut flags = 0;
                flags |= 0; // root type
                s.serialize_element(&flags)?;
                s.serialize_element(&children)?;
            }
            CommandNode::Literal {
                executable,
                children,
                redirect,
                name,
            } => {
                let mut flags = 0;
                flags |= 1; // literal type
                if *executable {
                    flags |= 0x04;
                }
                if let Some(_) = redirect {
                    flags |= 0x08;
                }
                s.serialize_element(&flags)?;
                s.serialize_element(&children)?;
                if let Some(r) = redirect {
                    s.serialize_element(&r)?;
                }
                s.serialize_element(&name)?;
            }
            CommandNode::Argument {
                executable,
                children,
                redirect,
                name,
                parser,
                suggestions,
            } => {
                let mut flags = 0;
                flags |= 2; // argument type
                if *executable {
                    flags |= 0x04;
                }
                if let Some(_) = redirect {
                    flags |= 0x08;
                }
                if let Some(_) = suggestions {
                    flags |= 0x10;
                }
                s.serialize_element(&flags)?;
                s.serialize_element(&children)?;
                if let Some(r) = redirect {
                    s.serialize_element(&r)?;
                }
                s.serialize_element(&name)?;
                s.serialize_element(parser)?;
                if let Some(suggestions) = suggestions {
                    s.serialize_element(&suggestions)?;
                }
            }
        }

        s.end()
    }
}

impl Serialize for Parser {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_seq(None)?;

        match self {
            Parser::String(properties) => {
                s.serialize_element("brigadier:string")?;
                s.serialize_element(properties)?;
            }
        }

        s.end()
    }
}

impl Serialize for ChunkSections {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // first we need the size of the following "byte array"
        // (good job mojang)
        let mut size = 0i32;
        for chunk_section in &self.0 {
            size += 3; // 2 bytes for block count, 1 byte for "bits per block"
            match &chunk_section.palette {
                Palette::Indirect(palette) => {
                    size += VarInt(palette.len() as i32).size() as i32;
                    for block in palette {
                        size += block.size() as i32;
                    }
                }
                Palette::Direct => {}
            }

            size += VarInt(chunk_section.data.len() as i32).size() as i32;
            size += 8 * chunk_section.data.len() as i32; // i64s
        }

        let mut s = serializer.serialize_seq(None)?;

        s.serialize_element(&size)?;
        for section in &self.0 {
            s.serialize_element(&section.block_count)?;
            match &section.palette {
                Palette::Indirect(mappings) => {
                    let bits_per_block = std::cmp::max(
                        4,
                        32u8 - std::cmp::max(mappings.len() as u32 - 1, 1).leading_zeros() as u8,
                    );
                    s.serialize_element(&bits_per_block)?;
                    s.serialize_element(&mappings)?;
                }
                Palette::Direct => {
                    s.serialize_element(&15u8)?;
                }
            }
            s.serialize_element(&section.data)?;
        }

        s.end()
    }
}

impl Serialize for VarInt {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut number = self.0 as u32;

        let mut s = serializer.serialize_seq(None)?;

        loop {
            let mut byte: u8 = number as u8 & 0b01111111;

            number = number >> 7;
            if number != 0 {
                byte = byte | 0b10000000;
            }

            s.serialize_element(&byte)?;

            if number == 0 {
                break;
            }
        }

        s.end()
    }
}
