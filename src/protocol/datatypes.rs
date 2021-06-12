use serde::ser::SerializeSeq;
use serde::{ser, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{from_reader, from_str, to_string, to_writer};
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
// todo make these enum structs
#[derive(Debug, Clone)]
pub enum CommandNode {
    // child node indices
    Root(Vec<VarInt>),
    // executable, child nodes indices, redirect, name
    Literal(bool, Vec<VarInt>, Option<VarInt>, String),
    // executable, child nodes indices, redirect, name, parser, whether has suggestions
    Argument(bool, Vec<VarInt>, Option<VarInt>, String, Parser, bool),
}

// todo enum here
#[derive(Debug, Clone)]
pub enum Parser {
    String(VarInt), // type, 0 - SINGLE_WORD, 1 - QUOTABLE_PHRASE, 2 - GREEDY_PHRASE
}

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
