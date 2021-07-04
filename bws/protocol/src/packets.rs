use super::datatypes::*;
use super::{Deserializable, Serializable};
use std::borrow::Cow;
use std::io::{self, Write};

use crate as protocol;

/// Sent from the server to the client
#[derive(Debug, Clone, PartialEq, strum::ToString)]
pub enum ClientBound<'a> {
    Status(StatusClientBound<'a>),
    Login(LoginClientBound<'a>),
    Play(PlayClientBound<'a>),
}

/// Sent from the client to the server
#[derive(Debug, Clone, PartialEq, strum::ToString)]
pub enum ServerBound<'a> {
    Handshake(HandshakeServerBound<'a>),
    Status(StatusServerBound),
    Login(LoginServerBound<'a>),
    Play(PlayServerBound<'a>),
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq, strum::ToString)]
pub enum HandshakeServerBound<'a> {
    Handshake {
        protocol: VarInt,
        server_address: Cow<'a, str>,
        server_port: u16,
        next_state: NextState,
    },
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq, strum::ToString)]
pub enum StatusClientBound<'a> {
    Response(StatusResponse<'a>),
    Pong(i64),
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq, strum::ToString)]
pub enum StatusServerBound {
    Request,
    Ping(i64),
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq, strum::ToString)]
pub enum LoginClientBound<'a> {
    Disconnect(Chat<'a>),
    EncryptionRequest {
        // Up to 20 characters
        server_id: Cow<'a, str>,
        public_key: Vec<u8>,
        verify_token: Vec<u8>,
    },
    LoginSuccess {
        uuid: u128,
        username: Cow<'a, str>,
    },
    SetCompression {
        treshold: VarInt,
    },
    PluginRequest {
        message_id: VarInt,
        channel: Cow<'a, str>,
        // the client figures out the length based on the packet size
        data: Box<[u8]>,
    },
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq, strum::ToString)]
pub enum LoginServerBound<'a> {
    LoginStart {
        username: Cow<'a, str>,
    },
    EncryptionResponse {
        shared_secret: Vec<u8>,
        verify_token: Vec<u8>,
    },
    PluginResponse {
        message_id: VarInt,
        successful: bool,
        // the server figures out the length based on the packet size
        data: Box<[u8]>,
    },
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq, strum::ToString)]
pub enum PlayClientBound<'a> {
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
    SpawnExperienceOrb, // todo
    SpawnLivingEntity,  // todo
    SpawnPainting,      // todo
    SpawnPlayer {
        entity_id: VarInt,
        uuid: u128,
        x: f64,
        y: f64,
        z: f64,
        yaw: Angle,
        pitch: Angle,
    },
    EntityAnimation {
        entity_id: VarInt,
        animation: EntityAnimation,
    },
    Statistics,               // todo
    AcknowledgePlayerDigging, // todo
    BlockBreakAnimation,      // todo
    BlockEntityData,          // todo
    BlockAction,              // todo
    BlockChange {
        location: Position,
        new_block_id: VarInt, // in the global palette
    },
    BossBar, // todo
    ServerDifficulty {
        difficulty: Difficulty,
        locked: bool,
    },
    ChatMessage {
        message: Chat<'a>,
        position: ChatPosition,
        sender: u128,
    },
    TabComplete, // todo
    DeclareCommands {
        nodes: Vec<CommandNode<'a>>,
        root: VarInt,
    },
    WindowConfirmation, // todo
    CloseWindow,        // todo
    WindowItems {
        window_id: u8,
        slots: ArrWithLen<Slot, u16, 46>,
    },
    WindowProperty, // todo
    SetSlot {
        window_id: i8,
        slot: i16,
        slot_data: Slot,
    },
    SetCooldown, // todo
    PluginMessage {
        channel: Cow<'a, str>,
        data: Box<[u8]>,
    },
    NamedSoundEffect, // todo
    Disconnect(Chat<'a>),
    EntityStatus {
        // good job on making the protocol so consistent, mojang
        entity_id: i32,
        /// see https://wiki.vg/Entity_statuses
        status: i8,
    },
    Explosion,       // todo
    UnloadChunk,     // todo
    ChangeGameState, // todo
    OpenHorseWindow, // todo
    KeepAlive(i64),
    ChunkData {
        chunk_x: i32,
        chunk_z: i32,
        chunk: Chunk,
    },
    Effect,      // todo
    Particle,    // todo
    UpdateLight, // todo
    JoinGame {
        /// entity ID, global on the server
        eid: i32,
        hardcore: bool,
        gamemode: Gamemode,
        previous_gamemode: Gamemode,
        world_names: Vec<Cow<'a, str>>,
        dimension_codec: MaybeStatic<'a, Nbt>,
        dimension: Nbt,
        world_name: Cow<'a, str>,
        hashed_seed: i64,
        /// doesn't do anything
        max_players: VarInt,
        view_distance: VarInt,
        /// shows less on the F3 debug screen
        reduced_debug_info: bool,
        enable_respawn_screen: bool,
        /// debug worlds cannot be modified and have predefined blocks
        debug_mode: bool,
        /// flat worlds have horizon at y=0 instead of y=63 and different void fog
        flat: bool,
    },
    MapData,   // todo
    TradeList, // todo
    EntityPosition {
        entity_id: VarInt,
        delta_x: i16, // newX * 32 - prevX * 32) * 128
        delta_y: i16, // newY * 32 - prevY * 32) * 128
        delta_z: i16, // newZ * 32 - prevZ * 32) * 128
        on_ground: bool,
    },
    EntityPositionAndRotation {
        entity_id: VarInt,
        delta_x: i16, // (newX * 32 - prevX * 32) * 128
        delta_y: i16, // (newY * 32 - prevY * 32) * 128
        delta_z: i16, // (newZ * 32 - prevZ * 32) * 128
        yaw: Angle,   // absolute, not delta
        pitch: Angle, // absolute, not delta
        on_ground: bool,
    },
    EntityRotation {
        entity_id: VarInt,
        yaw: Angle,   // absolute, not delta
        pitch: Angle, // absolute, not delta
        on_ground: bool,
    },
    EntityMovement {
        entity_id: VarInt,
    },
    VehicleMovement,     // todo
    OpenBook,            // tood
    OpenWindow,          // todo
    OpenSignEditor,      // todo
    CraftRecipeResponse, // todo
    PlayerAbilities {
        abilities: PlayerAbilities,
        flying_speed: f32,
        field_of_view: f32,
    },
    CombatEvent, // todo
    PlayerInfo(PlayerInfo<'a>),
    FacePlayer, // todo
    PlayerPositionAndLook {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        flags: PositionAndLookFlags,
        id: VarInt,
    },
    UnlockRecipes,                // todo
    DestroyEntities(Vec<VarInt>), // entity IDs
    RemoveEntityEffect,           // todo
    ResourcePackSend,             // todo
    Respawn {
        dimension: Nbt,
        world_name: Cow<'a, str>,
        hashed_seed: i64,
        gamemode: Gamemode,
        previous_gamemode: Gamemode,
        debug: bool,
        flat: bool,
        copy_metadata: bool,
    },
    EntityHeadLook {
        entity_id: VarInt,
        head_yaw: Angle,
    },
    MultiBlockChange,     // todo
    SelectAdvancementTab, // todo
    WorldBorder(WorldBorderAction),
    Camera,         // todo
    HeldItemChange, // todo
    UpdateViewPosition {
        chunk_x: VarInt,
        chunk_z: VarInt,
    },
    UpdateViewDistance(VarInt),
    SpawnPosition,     // todo
    DisplayScoreboard, // todo
    EntityMetadata {
        entity_id: VarInt,
        metadata: EntityMetadata<'a>,
    },
    AttachEntity,    // todo
    EntityVelocity,  // todo
    EntityEquipment, // todo
    SetExperience {
        bar: f32, // between 0 and 1
        level: VarInt,
        exp: VarInt,
    },
    UpdateHealth {
        health: f32, // 0 - dead, 20 - full
        food: VarInt,
        saturation: f32,
    },
    ScoreboardObjective, // todo
    SetPassengers,       // todo
    Teams,               // todo
    UpdateScore,         // todo
    TimeUpdate {
        world_age: i64,
        time: i64,
    },
    Title(TitleAction<'a>),
    EntitySoundEffect {
        sound_id: VarInt,
        category: SoundCategory,
        entity_id: VarInt,
        volume: f32,
        pitch: f32,
    },
    SoundEffect, // todo
    StopSound,   // todo
    PlayerListHeaderAndFooter {
        header: Chat<'a>,
        footer: Chat<'a>,
    },
    NbtQueryResponse, // todo
    CollectItem,      // todo
    EntityTeleport {
        entity_id: VarInt,
        x: f64, // all absolute here
        y: f64,
        z: f64,
        yaw: Angle,
        pitch: Angle,
        on_ground: bool,
    },
    Advancements,     // todo
    EntityProperties, // todo
    EntityEffect,     // todo
    DeclareRecipes,   // todo
    Tags {
        blocks: MaybeStatic<'a, Vec<Tags<'a>>>,
        items: MaybeStatic<'a, Vec<Tags<'a>>>,
        fluids: MaybeStatic<'a, Vec<Tags<'a>>>,
        entities: MaybeStatic<'a, Vec<Tags<'a>>>,
    },
}

#[derive(Serializable, Deserializable, Debug, Clone, PartialEq, strum::ToString)]
pub enum PlayServerBound<'a> {
    TeleportConfirm {
        teleport_id: VarInt,
    },
    QueryBlockNbt, // todo
    SetDifficulty(Difficulty),
    ChatMessage(Cow<'a, str>),
    ClientStatus(ClientStatusAction),
    ClientSettings(ClientSettings<'a>),
    TabComplete {
        transaction_id: VarInt,
        text: Cow<'a, str>,
    },
    WindowConfirmation {
        window_id: i8,
        action_number: i16,
        accepted: bool,
    },
    ClickWindowButton {
        window_id: i8,
        button_id: i8,
    },
    ClickWindow, // todo
    CloseWindow {
        window_id: i8,
    },
    PluginMessage {
        channel: Cow<'a, str>,
        data: Box<[u8]>,
    },
    EditBook,          // todo
    QueryEntityNbt,    // todo
    InteractEntity,    // todo
    GenerateStructure, // todo
    KeepAlive(i64),
    LockDifficulty, // todo
    PlayerPosition {
        x: f64,
        feet_y: f64,
        z: f64,
        on_ground: bool,
    },
    PlayerPositionAndRotation {
        x: f64,
        feet_y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    PlayerRotation {
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    PlayerMovement {
        on_ground: bool,
    },
    VehicleMove,        // todo
    SteerBoat,          // todo
    PickItem,           // todo
    CraftRecipeRequest, // todo
    PlayerAbilites {
        flags: PlayerAbilities, // but the client changes only FLYING
    },
    PlayerDigging {
        status: PlayerDiggingStatus,
        location: Position,
        face: Direction,
    },
    EntityAction {
        entity_id: VarInt,
        action: EntityAction,
        jump_boost: VarInt, // used only when Action is jump with horse, otherwise its 0
    },
    SteerVehicle,       // todo
    SetRecipeBookState, // todo
    SetDisplayedRecipe, // todo
    NameItem,           // todo
    ResourcePackStatus, // todo
    AdvancementTab,     // todo
    SelectTrade,        // todo
    SetBeaconEffect,    // todo
    HeldItemChange {
        slot: i16,
    },
    UpdateCommandBlock,         // todo
    UpdateCommandBlockMinecart, // todo
    CreativeInventoryAction {
        slot: i16,
        item: Slot,
    },
    UpdateJigsawBlock,    // todo
    UpdateStructureBlock, // todo
    UpdateSign,           // todo
    Animation {
        hand: Hand,
    },
    Spectate, // todo
    PlayerBlockPlacement {
        hand: Hand,
        location: Position,
        face: Direction,
        cursor_position_x: f32,
        cursor_position_y: f32,
        cursor_position_z: f32,
        inside_block: bool,
    },
    UseItem, // todo
}

impl<'a> StatusClientBound<'a> {
    pub fn cb(self) -> ClientBound<'a> {
        ClientBound::Status(self)
    }
}
impl<'a> LoginClientBound<'a> {
    pub fn cb(self) -> ClientBound<'a> {
        ClientBound::Login(self)
    }
}
impl<'a> PlayClientBound<'a> {
    pub fn cb(self) -> ClientBound<'a> {
        ClientBound::Play(self)
    }
}

impl<'a> Serializable for ClientBound<'a> {
    fn to_writer<W: Write>(&self, output: &mut W) -> io::Result<usize> {
        match self {
            Self::Status(packet) => packet.to_writer(output),
            Self::Login(packet) => packet.to_writer(output),
            Self::Play(packet) => packet.to_writer(output),
        }
    }
}

impl<'a> HandshakeServerBound<'a> {
    pub fn sb(self) -> ServerBound<'a> {
        ServerBound::Handshake(self)
    }
}
impl StatusServerBound {
    pub fn sb(self) -> ServerBound<'static> {
        ServerBound::Status(self)
    }
}
impl<'a> LoginServerBound<'a> {
    pub fn sb(self) -> ServerBound<'a> {
        ServerBound::Login(self)
    }
}
impl<'a> PlayServerBound<'a> {
    pub fn sb(self) -> ServerBound<'a> {
        ServerBound::Play(self)
    }
}

impl<'a> Serializable for ServerBound<'a> {
    fn to_writer<W: Write>(&self, output: &mut W) -> io::Result<usize> {
        match self {
            Self::Handshake(packet) => packet.to_writer(output),
            Self::Status(packet) => packet.to_writer(output),
            Self::Login(packet) => packet.to_writer(output),
            Self::Play(packet) => packet.to_writer(output),
        }
    }
}
