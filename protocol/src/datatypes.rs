pub mod chat_parse;
super::cfg_ser! {
    // Implementations of Serializable and Deserializable
    mod implementations;

    use super::{Deserializable, Serializable};

    // needed for proc macros
    use crate as protocol;
}

use bitflags::bitflags;
use std::borrow::Cow;
use std::convert::TryFrom;
use std::io::Cursor;
use std::marker::PhantomData;

pub use super::nbt::NbtCompound as Nbt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarInt(pub i32);

impl TryFrom<i32> for VarInt {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(Self(value))
    }
}

impl TryFrom<VarInt> for i32 {
    type Error = ();

    fn try_from(value: VarInt) -> Result<Self, Self::Error> {
        Ok(value.0)
    }
}

super::cfg_ser! {
    impl VarInt {
        pub fn size(&self) -> usize {
            struct Size;
            impl std::io::Write for Size {
                fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                    Ok(buf.len())
                }

                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }

            self.to_writer(&mut Size).unwrap()
        }
    }
}

/// A newtype around an array except that when serializing/deserializing it has the fixed length as a prefix
#[derive(Debug, Clone, PartialEq)]
pub struct ArrWithLen<T, L, const N: usize>(pub [T; N], PhantomData<L>);

impl<T, L, const N: usize> ArrWithLen<T, L, N> {
    pub fn new(arr: [T; N]) -> Self {
        Self(arr, PhantomData)
    }
}

/// the same as normal Nbt, except that it allows for it to be just a single TAG_END byte, without any actual data.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionalNbt(pub Option<Nbt>);

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub struct Angle(pub u8);

impl Angle {
    pub fn from_degrees(degrees: f32) -> Self {
        Self(((degrees / 360.0).rem_euclid(1.0) * 256.0) as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Maybe static. Helps save resources when sending the same fixed data to many clients,
/// because you don't have to clone the data for each one of them, you just serialize a byte slice
///
/// Note that the static variant contains **ALREADY SERIALIZED** bytes
///
/// **Use with caution**, nothing's going to stop you from sending invalid datatypes.
#[derive(Debug, Clone, PartialEq)]
pub enum MaybeStatic<'a, T> {
    Static(&'a [u8]),
    Owned(T),
}

super::cfg_ser! {
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityMetadata<'a>(pub Vec<(u8, EntityMetadataEntry<'a>)>);

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum EntityMetadataEntry<'a> {
    Byte(u8),
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

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum Pose {
    Standing,
    FallFlying,
    Sleeping,
    Swimming,
    SpinAttack,
    Sneaking,
    Dying,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
#[cfg_attr(feature = "ser", discriminant_as(u8))]
pub enum GameStateChangeReason {
    NoRespawnBlockAvailable,
    EndRaining,
    BeginRaining,
    ChangeGamemode,
    WinGame,
    DemoEvent,
    ArrowHitPlayer,
    RainLevelChange,
    ThunderLevelChange,
    PlayPufferfishStingSound,
    PlayElderGuardianMobAppearance,
    EnableRespawnScreen,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum PlayerInfo<'a> {
    AddPlayer(Vec<(u128, PlayerInfoAddPlayer<'a>)>),
    UpdateGamemode(Vec<(u128, PlayerInfoUpdateGamemode)>),
    UpdateLatency(Vec<(u128, PlayerInfoUpdateLatency)>),
    UpdateDisplayName(Vec<(u128, PlayerInfoUpdateDisplayName<'a>)>),
    RemovePlayer(Vec<u128>),
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub struct PlayerInfoAddPlayer<'a> {
    pub name: Cow<'a, str>,
    pub properties: Vec<PlayerInfoAddPlayerProperty<'a>>,
    pub gamemode: Gamemode,
    pub ping: VarInt,
    pub display_name: Option<Chat<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub struct PlayerInfoAddPlayerProperty<'a> {
    pub name: Cow<'a, str>,
    pub value: Cow<'a, str>,
    pub signature: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub struct ClientSettings<'a> {
    pub locale: Cow<'a, str>,
    pub view_distance: i8,
    pub chat_mode: ChatMode,
    pub chat_colors: bool,
    pub displayed_skin_parts: SkinParts,
    pub main_hand: Hand,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub struct PlayerInfoUpdateGamemode {
    pub gamemode: Gamemode,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub struct PlayerInfoUpdateLatency {
    /// In milliseconds
    pub ping: VarInt,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub struct PlayerInfoUpdateDisplayName<'a> {
    pub display_name: Option<Chat<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
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
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
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

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
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

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
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

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub struct Slot(pub Option<InnerSlot>);

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
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
        suggestions: Option<SuggestionsType>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionsType {
    AskServer,
    AllRecipes,
    AvailableSounds,
    SummonableEntities,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Parser {
    String(StringParserType),
    Integer(IntegerParserOptions),
    Float(FloatParserOptions),
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntegerParserOptions {
    pub min: Option<i32>,
    pub max: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatParserOptions {
    pub min: Option<f32>,
    pub max: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum StringParserType {
    SingleWord,
    QuotablePhrase,
    GreedyPhrase,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
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

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
#[cfg_attr(feature = "ser", discriminant_as(u8))]
pub enum EntityAnimation {
    SwingMainArm,
    TakeDamage,
    LeaveBed,
    SwingOffhand,
    CriticalEffect,
    MagicCriticalEffect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum PlayerDiggingStatus {
    StartedDigging,
    CancelledDigging,
    FinishedDigging,
    DropItemStack,
    DropItem,
    ShootArrowOrFinishEating,
    SwapItemInHand,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

bitflags! {
    #[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
    pub struct PlayerAbilities: u8 {
        const INVULNERABLE = 0x01;
        const FLYING = 0x02;
        const ALLOW_FLYING = 0x04;
        const INSTANT_BREAK = 0x08;
    }
}
bitflags! {
    #[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
    pub struct PositionAndLookFlags: u8 {
        const RELATIVE_X = 0x01;
        const RELATIVE_Y = 0x02;
        const RELATIVE_Z = 0x04;
        const RELATIVE_YAW = 0x08; // i have possibly mixed up yaw and pitch here
        const RELATIVE_PITCH = 0x10;
    }
}

bitflags! {
    #[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
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

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum NextState {
    #[cfg_attr(feature = "ser", discriminant(1))]
    Status,
    Login,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum ClientStatusAction {
    PerformRespawn,
    RequestStats,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum ChatMode {
    Enabled,
    CommandsOnly,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum Hand {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum ChatPosition {
    Chat,
    System,
    AboveHotbar,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
pub enum Gamemode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ser", derive(Serializable, Deserializable))]
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

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(serde::Serialize, serde::Deserialize))]
pub struct StatusResponseJson<'a> {
    pub version: StatusVersion<'a>,
    pub players: StatusPlayers<'a>,
    pub description: Chat<'a>,
    pub favicon: Cow<'a, str>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(serde::Serialize, serde::Deserialize))]
pub struct StatusVersion<'a> {
    pub name: Cow<'a, str>,
    pub protocol: i32,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(serde::Serialize, serde::Deserialize))]
pub struct StatusPlayers<'a> {
    pub max: i32,
    pub online: i32,
    pub sample: Vec<StatusPlayerSampleEntry<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(serde::Serialize, serde::Deserialize))]
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
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ser", derive(serde::Serialize, serde::Deserialize))]
pub struct Chat<'a> {
    pub text: Cow<'a, str>,
    #[cfg_attr(feature = "ser", serde(default))]
    #[cfg_attr(feature = "ser", serde(skip_serializing_if = "Option::is_none"))]
    pub bold: Option<bool>,
    #[cfg_attr(feature = "ser", serde(default))]
    #[cfg_attr(feature = "ser", serde(skip_serializing_if = "Option::is_none"))]
    pub italic: Option<bool>,
    #[cfg_attr(feature = "ser", serde(default))]
    #[cfg_attr(feature = "ser", serde(skip_serializing_if = "Option::is_none"))]
    pub underlined: Option<bool>,
    #[cfg_attr(feature = "ser", serde(default))]
    #[cfg_attr(feature = "ser", serde(skip_serializing_if = "Option::is_none"))]
    pub strikethrough: Option<bool>,
    #[cfg_attr(feature = "ser", serde(default))]
    #[cfg_attr(feature = "ser", serde(skip_serializing_if = "Option::is_none"))]
    pub obfuscated: Option<bool>,
    #[cfg_attr(feature = "ser", serde(default))]
    #[cfg_attr(feature = "ser", serde(skip_serializing_if = "Option::is_none"))]
    pub color: Option<Cow<'a, str>>,
    #[cfg_attr(feature = "ser", serde(default))]
    #[cfg_attr(feature = "ser", serde(skip_serializing_if = "Vec::is_empty"))]
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

impl<'a> Default for Chat<'a> {
    fn default() -> Self {
        Self::new()
    }
}
