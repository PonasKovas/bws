pub mod chat_parse;
mod implementations;

use super::{Deserializable, Serializable};
use bitflags::bitflags;
use shrinkwraprs::Shrinkwrap;
use std::borrow::Cow;
use std::convert::TryFrom;
use std::io::Cursor;
use std::marker::PhantomData;

use crate as protocol;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarInt(pub i32);

impl TryFrom<i32> for VarInt {
    type Error = !;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(Self(value))
    }
}

impl TryFrom<VarInt> for i32 {
    type Error = !;

    fn try_from(value: VarInt) -> Result<Self, Self::Error> {
        Ok(value.0)
    }
}

impl VarInt {
    pub fn size(&self) -> u8 {
        // the inner +6 is so that dividing by 7 would always round up
        std::cmp::max((32 - (self.0 as u32).leading_zeros() + 6) / 7, 1) as u8
    }
}

// A newtype around an array except that when serializing/deserializing it has the fixed length as a prefix
#[derive(Shrinkwrap, Debug, Clone, PartialEq)]
#[shrinkwrap(mutable)]
pub struct ArrWithLen<T, L, const N: usize>(#[shrinkwrap(main_field)] pub [T; N], PhantomData<L>);

impl<T, L, const N: usize> ArrWithLen<T, L, N> {
    pub fn new(arr: [T; N]) -> Self {
        Self(arr, PhantomData)
    }
}

#[derive(Shrinkwrap, Debug, Clone, PartialEq)]
#[shrinkwrap(mutable)]
pub struct Nbt(pub quartz_nbt::NbtCompound);

/// the same as normal Nbt, except that it allows for it to be just a single TAG_END byte, without any actual data.
#[derive(Shrinkwrap, Debug, Clone, PartialEq)]
#[shrinkwrap(mutable)]
pub struct OptionalNbt(pub Option<quartz_nbt::NbtCompound>);

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct Angle(pub u8);

impl Angle {
    pub fn from_degrees(degrees: f32) -> Self {
        Self(((degrees / 360.0).rem_euclid(1.0) * 256.0) as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Maybe static. Helps save resources when sending the same fixed data to many clients,
/// because you don't have to clone the data for each one of them, you just serialize a byte slice
/// Note that the static variant contains ALREADY SERIALIZED bytes
/// Use with caution, nothing's going to stop you from sending invalid datatypes.
#[derive(Debug, Clone, PartialEq)]
pub enum MaybeStatic<'a, T> {
    Static(&'a [u8]),
    Owned(T),
}

impl<'a, T: Deserializable> MaybeStatic<'a, T> {
    pub fn into_owned(self: MaybeStatic<'a, T>) -> T {
        match self {
            MaybeStatic::Static(bytes) => {
                T::from_reader(&mut Cursor::<Vec<u8>>::new(bytes.into())).unwrap()
            }
            MaybeStatic::Owned(item) => item,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityMetadata<'a>(pub Vec<EntityMetadataEntry<'a>>);

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub enum EntityMetadataEntry<'a> {
    Byte(i8),
    VarInt(VarInt),
    Float(f32),
    String(Cow<'a, str>),
    Chat(Chat<'a>),
    OptChat(Option<Chat<'a>>),
    Slot(Slot),
    Boolean(bool),
    Rotation {
        x: f32,
        y: f32,
        z: f32,
    },
    Position(Position),
    OptPosition(Option<Position>),
    Direction(Direction),
    OptUuid(Option<u128>),
    OptBlockId(VarInt),
    Nbt(Nbt),
    Particle(), // todo!
    VillagerData {
        villager_type: VarInt,
        profession: VarInt,
        level: VarInt,
    },
    OptVarInt(VarInt),
    Pose(Pose),
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub enum PlayerInfo<'a> {
    AddPlayer(Vec<(u128, PlayerInfoAddPlayer<'a>)>),
    UpdateGamemode(Vec<(u128, PlayerInfoUpdateGamemode)>),
    UpdateLatency(Vec<(u128, PlayerInfoUpdateLatency)>),
    UpdateDisplayName(Vec<(u128, PlayerInfoUpdateDisplayName<'a>)>),
    RemovePlayer(Vec<u128>),
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct PlayerInfoAddPlayer<'a> {
    pub name: Cow<'a, str>,
    pub properties: Vec<PlayerInfoAddPlayerProperty<'a>>,
    pub gamemode: Gamemode,
    pub ping: VarInt,
    pub display_name: Option<Chat<'a>>,
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct PlayerInfoAddPlayerProperty<'a> {
    pub name: Cow<'a, str>,
    pub value: Cow<'a, str>,
    pub signature: Option<Cow<'a, str>>,
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct PlayerInfoUpdateGamemode {
    pub gamemode: Gamemode,
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct PlayerInfoUpdateLatency {
    /// In milliseconds
    pub ping: VarInt,
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct PlayerInfoUpdateDisplayName<'a> {
    pub display_name: Option<Chat<'a>>,
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
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
#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
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
        biomes: ArrWithLen<VarInt, VarInt, 1024>,
        sections: ChunkSections,
        block_entities: Vec<Nbt>,
    },
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub enum TitleAction<'a> {
    SetTitle(Chat<'a>),
    SetSubtitle(Chat<'a>),
    SetActionBar(Chat<'a>),
    SetDisplayTime {
        // time in ticks
        fade_in: i32,
        display: i32,
        fade_out: i32,
    },
    Hide,
    Reset,
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct Tags<'a> {
    pub name: Cow<'a, str>,
    pub entries: Vec<VarInt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkSections(pub Vec<ChunkSection>);

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkSection {
    // number of non-air blocks in the chuck section, for lighting purposes.
    pub block_count: i16,
    pub palette: Palette,
    pub data: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Palette {
    Indirect(Vec<VarInt>),
    Direct,
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct Slot(pub Option<InnerSlot>);

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq)]
pub struct InnerSlot {
    pub item_id: VarInt,
    pub item_count: i8,
    pub nbt: OptionalNbt,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandNode<'a> {
    Root {
        // indices of the children
        children: Vec<VarInt>,
    },
    Literal {
        executable: bool,
        children: Vec<VarInt>,
        redirect: Option<VarInt>,
        name: Cow<'a, str>,
    },
    Argument {
        executable: bool,
        children: Vec<VarInt>,
        redirect: Option<VarInt>,
        name: Cow<'a, str>,
        parser: Parser,
        suggestions: Option<Cow<'a, str>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Parser {
    String(StringParserType),
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum StringParserType {
    SingleWord,
    QuotablePhrase,
    GreedyPhrase,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum EntityAction {
    StartSneaking,
    StopSneaking,
    LeaveBed,
    StartSprinting,
    StopSprinting,
    StartJumpWithHorse,
    StopJumpWithHorse,
    OpenHorseInventory,
    StartFlyingWithElytra,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum Pose {
    Standing,
    FallFlying,
    Sleeping,
    Swimming,
    SpinAttack,
    Sneaking,
    Dying,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
// this one should actually be serialized as an unsigned byte
// but there's no difference until we have more than 127 variants,
// which I think we will never do, so this works
pub enum EntityAnimation {
    SwingMainArm,
    TakeDamage,
    LeaveBed,
    SwingOffhand,
    CriticalEffect,
    MagicCriticalEffect,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum PlayerDiggingStatus {
    StartedDigging,
    CancelledDigging,
    FinishedDigging,
    DropItemStack,
    DropItem,
    ShootArrowOrFinishEating,
    SwapItemInHand,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

bitflags! {
    #[derive(Serializable, Deserializable)]
    pub struct PlayerAbilities: u8 {
        const INVULNERABLE = 0x01;
        const FLYING = 0x02;
        const ALLOW_FLYING = 0x04;
        const INSTANT_BREAK = 0x08;
    }
}
bitflags! {
    #[derive(Serializable, Deserializable)]
    pub struct PositionAndLookFlags: u8 {
        const RELATIVE_X = 0x01;
        const RELATIVE_Y = 0x02;
        const RELATIVE_Z = 0x04;
        const RELATIVE_YAW = 0x08; // i have possibly mixed up yaw and pitch here
        const RELATIVE_PITCH = 0x10;
    }
}

bitflags! {
    #[derive(Serializable, Deserializable)]
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

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum NextState {
    #[discriminant(1)]
    Status,
    Login,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum ClientStatusAction {
    PerformRespawn,
    RequestStats,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum ChatMode {
    Enabled,
    CommandsOnly,
    Hidden,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum Hand {
    Left,
    Right,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum ChatPosition {
    Chat,
    System,
    AboveHotbar,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum Gamemode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

#[derive(Serializable, Deserializable, Debug, Clone, Copy, PartialEq)]
pub enum SoundCategory {
    Master,
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

#[derive(Debug, Clone, PartialEq)]
pub struct StatusResponse<'a> {
    pub json: StatusResponseJson<'a>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct StatusResponseJson<'a> {
    pub version: StatusVersion<'a>,
    pub players: StatusPlayers<'a>,
    pub description: Chat<'a>,
    pub favicon: Cow<'a, str>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct StatusVersion<'a> {
    pub name: Cow<'a, str>,
    pub protocol: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct StatusPlayers<'a> {
    pub max: i32,
    pub online: i32,
    pub sample: Vec<StatusPlayerSampleEntry<'a>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct StatusPlayerSampleEntry<'a> {
    pub name: Cow<'a, str>,
    pub id: Cow<'a, str>,
}

impl<'a> StatusPlayerSampleEntry<'a> {
    pub fn new(name: Cow<'a, str>) -> Self {
        Self {
            name,
            id: "00000000-0000-0000-0000-000000000000".into(),
        }
    }
}

// chat objects are represented in JSON so we use serde
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Chat<'a> {
    pub text: Cow<'a, str>,
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
    pub color: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Chat<'a>>,
}

impl<'a> Chat<'a> {
    pub fn new() -> Self {
        Self {
            text: "".into(),
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
