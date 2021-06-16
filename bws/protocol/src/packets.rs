use super::datatypes::*;
use super::{deserializable, serializable, Deserializable, Serializable};
use std::borrow::Cow;
use std::io::{self, Write};

/// Sent from the server to the client
#[derive(Debug, Clone)]
pub enum ClientBound<'a> {
    Status(StatusClientBound<'a>),
    Login(LoginClientBound<'a>),
    Play(PlayClientBound<'a>),
}

/// Sent from the client to the server
#[derive(Debug, Clone)]
pub enum ServerBound<'a> {
    Handshake(HandshakeServerBound<'a>),
    Status(StatusServerBound),
    Login(LoginServerBound<'a>),
    Play(PlayServerBound<'a>),
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub enum HandshakeServerBound<'a> {
    Handshake {
        protocol: VarInt,
        server_address: Cow<'a, str>,
        server_port: u16,
        next_state: NextState,
    },
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub enum StatusClientBound<'a> {
    Response(StatusResponse<'a>),
    Pong(i64),
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub enum StatusServerBound {
    Request,
    Ping(i64),
}

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
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

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
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

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
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
    EntityAnimation,          // todo
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
    WindowItems,        // todo
    WindowProperty,     // todo
    SetSlot,            // todo
    SetCooldown,        // todo
    PluginMessage {
        channel: Cow<'a, str>,
        data: Box<[u8]>,
    },
    NamedSoundEffect, // todo
    Disconnect(Chat<'a>),
    EntityStatus,    // todo
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
        // entity ID, global on the server
        eid: i32,
        hardcore: bool,
        gamemode: Gamemode,
        previous_gamemode: Gamemode,
        world_names: Vec<Cow<'a, str>>,
        dimension_codec: MaybeStatic<'a, Nbt>,
        dimension: Nbt,
        world_name: Cow<'a, str>,
        hashed_seed: i64,
        // doesn't do anything
        max_players: VarInt,
        view_distance: VarInt,
        // shows less on the F3 debug screen
        reduced_debug_info: bool,
        enable_respawn_screen: bool,
        // debug worlds cannot be modified and have predefined blocks
        debug_mode: bool,
        // flat worlds have horizon at y=0 instead of y=63 and different void fog
        flat: bool,
    },
    MapData,                   // todo
    TradeList,                 // todo
    EntityPosition,            // todo
    EntityPositionAndRotation, // todo
    EntityRotation,            // todo
    EntityMovement,            // todo
    VehicleMovement,           // todo
    OpenBook,                  // tood
    OpenWindow,                // todo
    OpenSignEditor,            // todo
    CraftRecipeResponse,       // todo
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
    UnlockRecipes,      // todo
    DestroyEntities,    // todo
    RemoveEntityEffect, // todo
    ResourcePackSend,   // todo
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
    EntityHeadLook,       // todo
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
    EntityMetadata,    // todo
    AttachEntity,      // todo
    EntityVelocity,    // todo
    EntityEquipment,   // todo
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
    EntityTeleport,   // todo
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

#[deserializable]
#[serializable]
#[derive(Debug, Clone)]
pub enum PlayServerBound<'a> {
    TeleportConfirm {
        teleport_id: VarInt,
    },
    QueryBlockNbt, // todo
    SetDifficulty(Difficulty),
    ChatMessage(Cow<'a, str>),
    ClientStatus(ClientStatusAction),
    ClientSettings {
        locale: Cow<'a, str>,
        view_distance: i8,
        chat_mode: ChatMode,
        chat_colors: bool,
        displayed_skin_parts: SkinParts,
        main_hand: MainHand,
    },
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
        face: Face,
    },
    EntityAction,               // todo
    SteerVehicle,               // todo
    SetRecipeBookState,         // todo
    SetDisplayedRecipe,         // todo
    NameItem,                   // todo
    ResourcePackStatus,         // todo
    AdvancementTab,             // todo
    SelectTrade,                // todo
    SetBeaconEffect,            // todo
    HeldItemChange,             // todo
    UpdateCommandBlock,         // todo
    UpdateCommandBlockMinecart, // todo
    CreativeInventoryAction,    // todo
    UpdateJigsawBlock,          // todo
    UpdateStructureBlock,       // todo
    UpdateSign,                 // todo
    Animation,                  // todo
    Spectate,                   // todo
    PlayerBlockPlacement {
        hand: MainHand,
        location: Position,
        face: Face,
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
    fn to_writer<W: Write>(&self, output: &mut W) -> io::Result<()> {
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
    fn to_writer<W: Write>(&self, output: &mut W) -> io::Result<()> {
        match self {
            Self::Handshake(packet) => packet.to_writer(output),
            Self::Status(packet) => packet.to_writer(output),
            Self::Login(packet) => packet.to_writer(output),
            Self::Play(packet) => packet.to_writer(output),
        }
    }
}
