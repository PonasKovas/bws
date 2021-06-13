use super::datatypes::*;
use crate::world;
use serde::{Deserialize, Serialize};
use std::{
    env::Vars,
    hash,
    io::{self, Cursor, Write},
};

// Sent from the server to the client
#[derive(Debug, Clone)]
pub enum ClientBound {
    Handshake(HandshakeClientBound),
    Status(StatusClientBound),
    Login(LoginClientBound),
    Play(PlayClientBound),
}

// No packets are sent from the server in the HandShake state
#[derive(Serialize, Debug, Clone)]
pub enum HandshakeClientBound {}

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
    SpawnExperienceOrb,       // todo
    SpawnLivingEntity,        // todo
    SpawnPainting,            // todo
    SpawnPlayer,              // todo
    EntityAnimation,          // todo
    Statistics,               // todo
    AcknowledgePlayerDigging, // todo
    BlockBreakAnimation,      // todo
    BlockEntityData,          // todo
    BlockAction,              // todo
    BlockChange,              // todo
    BossBar,                  // todo
    ServerDifficulty {
        difficulty: Difficulty,
        locked: bool,
    },
    ChatMessage {
        message: Chat,
        position: ChatPosition,
        sender: u128,
    },
    TabComplete, // todo
    DeclareCommands {
        nodes: Vec<CommandNode>,
        root: VarInt,
    },
    WindowConfirmation, // todo
    CloseWindow,        // todo
    WindowItems,        // todo
    WindowProperty,     // todo
    SetSlot,            // todo
    SetCooldown,        // todo
    PluginMessage {
        channel: String,
        data: Box<[u8]>,
    },
    NamedSoundEffect, // todo
    Disconnect(Chat),
    EntityStatus,    // todo
    Explosion,       // todo
    UnloadChunk,     // todo
    ChangeGameState, // todo
    OpenHorseWindow, // todo
    KeepAlive(i64),
    ChunkData {
        chunk_x: i32,
        chunk_z: i32,
        // bits 0-15, if 1 then the chunk section will be sent in this packet
        primary_bitmask: VarInt,
        heightmaps: Nbt,
        // 4x4x4 sections in the entire chunk (16x256x16),
        biomes: ArrWithLen<VarInt, 1024>,
        sections: ChunkSections,
        block_entities: Vec<Nbt>,
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
        world_names: Vec<String>,
        dimension_codec: Nbt,
        dimension: Nbt,
        world_name: String,
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
    PlayerInfo,  // todo
    FacePlayer,  // todo
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
        world_name: String,
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
    Title(TitleAction),
    EntitySoundEffect {
        sound_id: VarInt,
        category: SoundCategory,
        entity_id: VarInt,
        volume: f32,
        pitch: f32,
    },
    SoundEffect,               // todo
    StopSound,                 // todo
    PlayerListHeaderAndFooter, // todo
    NbtQueryResponse,          // todo
    CollectItem,               // todo
    EntityTeleport,            // todo
    Advancements,              // todo
    EntityProperties,          // todo
    EntityEffect,              // todo
    DeclareRecipes,            // todo
    Tags {
        blocks: Vec<Tags>,
        items: Vec<Tags>,
        fluids: Vec<Tags>,
        entities: Vec<Tags>,
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
            Self::Handshake(packet) => packet.serialize(serializer),
            Self::Status(packet) => packet.serialize(serializer),
            Self::Login(packet) => packet.serialize(serializer),
            Self::Play(packet) => packet.serialize(serializer),
        }
    }
}
