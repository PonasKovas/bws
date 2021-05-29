use crate::datatypes::*;
use nbt::Blob as Nbt;
use std::{
    env::Vars,
    io::{self, Cursor, Write},
};

// Sent from the client to the server
#[derive(Debug, Clone)]
pub enum ServerBound {
    Handshake(VarInt, String, u16, VarInt), // protocol, address, port, next state
    StatusRequest,
    StatusPing(i64),    // random number
    LoginStart(String), // username
    KeepAlive(i64),
    ChatMessage(String), // the raw message
    // ChatMessage(String), // the raw message, up to 256 characters
    // ClientStatus(VarInt), // 0 - respawn, 1 - request statistics
    // InteractEntity(VarInt, VarInt, bool), // entity id, [0 - interact, 1 - attack, 2 - interact at (not supported)], whether sneaking
    // PlayerPositionAndRotation(f64, f64, f64, f32, f32, bool), // x, y, z, yaw, pitch, whether on ground
    // Animation(VarInt),                                        // 0 - main hand, 1 - off hand
    // TeleportConfirm(VarInt),                                  // teleport id
    // EntityAction(VarInt, VarInt, VarInt), // player's entity id, action (see https://wiki.vg/index.php?title=Protocol&oldid=16091#Entity_Action), jump boost (only for jumping with horse)
    // HeldItemChange(i16),                  // slot id 0-8
    // UseItem(VarInt),                      // 0 - main hand, 1 - off hand
    // PlayerDigging(VarInt, i64, i8),       // action [0-6], position, face
    Unknown(VarInt), // the packet id of the unknown packet
}

// Sent from the server to the client
#[derive(Debug, Clone)]
pub enum ClientBound {
    StatusResponse(String),
    StatusPong(i64), // the same random number
    LoginDisconnect(Chat),
    KeepAlive(i64),
    SetCompression(VarInt),     // treshold
    LoginSuccess(u128, String), // UUID and Username
    JoinGame(
        i32,
        bool,
        u8,
        i8,
        Vec<String>,
        Nbt,
        String,
        i64,
        VarInt,
        VarInt,
        bool,
        bool,
        bool,
        bool,
    ), // entity id, is_hardcore, gamemode, previous gamemode, worlds [name], dimension, identifier, hashed seed, max_players, view_distance, reduced_debug_info, enable_respawn_screen, is_debug, is_flat
    TimeUpdate(i64, i64), // world age and region time.
    Title(TitleAction),
    PlayerPositionAndLook(f64, f64, f64, f32, f32, u8, VarInt), // x, y, z, yaw, pitch, flags, tp id
    SetBrand(String),                                           // name
    DeclareCommands(Vec<CommandNode>, VarInt), // all the nodes, and the index of the root node
    ChatMessage(Chat, u8), // json chat data, position (0 chat, 1 system, 2 above hotbar)
    ChunkData(
        i32,
        i32,
        VarInt,
        Nbt,
        Vec<VarInt>,
        Vec<ChunkSection>,
        Vec<Nbt>,
    ), // chunk X, chunk Z, primary bit mask, heightmaps, biomes, data, block entities
    PlayDisconnect(Chat),
    // UpdateHealth(f32, VarInt, f32), // health, food, saturation
    //
    // SpawnLivingEntity(
    //     VarInt,
    //     u128,
    //     VarInt,
    //     f64,
    //     f64,
    //     f64,
    //     u8,
    //     u8,
    //     u8,
    //     i16,
    //     i16,
    //     i16,
    // ), // entity id, uuid, type, x, y, z, yaw, pitch, head pitch, velocity: x, y, z
    // EntityTeleport(VarInt, f64, f64, f64, u8, u8, bool), // entity id, x, y, z, yaw, pitch, whether on ground
    // EntityPosition(VarInt, i16, i16, i16, bool), // entity id, delta x, y ,z, whether on ground
    // DestroyEntities(Vec<VarInt>),                // Array of entity IDs to destroy
    //
    // SetSlot(i8, i16, Slot), // window id, slot id, slot data
    // Statistics(Vec<(VarInt, VarInt, VarInt)>), // Category, id, value
    //
}

#[derive(Debug, Clone)]
pub enum TitleAction {
    SetTitle(Chat),
    SetSubtitle(Chat),
    SetActionBar(Chat),
    SetDisplayTime(i32, i32, i32), // fade in, dislay, fade out - all in ticks
    Hide,
    Reset,
}

impl ServerBound {
    pub fn deserialize(input: &mut Cursor<&Vec<u8>>, state: i32) -> io::Result<Self> {
        let packet_id = VarInt::deserialize(input)?.0;

        let result = match state {
            0 => {
                // Handshake
                match packet_id {
                    0x00 => Ok(Self::Handshake(
                        VarInt::deserialize(input)?,
                        String::deserialize(input)?,
                        u16::deserialize(input)?,
                        VarInt::deserialize(input)?,
                    )),
                    _ => Ok(Self::Unknown(VarInt(packet_id))),
                }
            }
            1 => {
                // Status
                match packet_id {
                    0x00 => Ok(Self::StatusRequest),
                    0x01 => Ok(Self::StatusPing(i64::deserialize(input)?)),
                    _ => Ok(Self::Unknown(VarInt(packet_id))),
                }
            }
            2 => {
                // Login
                match packet_id {
                    0x00 => Ok(Self::LoginStart(String::deserialize(input)?)),
                    _ => Ok(Self::Unknown(VarInt(packet_id))),
                }
            }
            3 => {
                // Play
                match packet_id {
                    0x10 => Ok(Self::KeepAlive(i64::deserialize(input)?)),
                    0x03 => Ok(Self::ChatMessage(String::deserialize(input)?)),
                    _ => Ok(Self::Unknown(VarInt(packet_id))),
                }
            }
            _ => Ok(Self::Unknown(VarInt(packet_id))),
        };

        result
    }
}

impl ClientBound {
    pub fn serialize<W: Write>(self, output: &mut W) {
        match self {
            Self::StatusResponse(json) => {
                VarInt(0x00).serialize(output);

                json.serialize(output);
            }
            Self::StatusPong(number) => {
                VarInt(0x01).serialize(output);

                number.serialize(output);
            }
            Self::LoginDisconnect(reason) => {
                VarInt(0x00).serialize(output);

                reason.serialize(output);
            }
            Self::PlayDisconnect(reason) => {
                VarInt(0x19).serialize(output);

                reason.serialize(output);
            }
            Self::SetCompression(treshold) => {
                VarInt(0x03).serialize(output);

                treshold.serialize(output);
            }
            Self::KeepAlive(number) => {
                VarInt(0x1F).serialize(output);

                number.serialize(output);
            }
            Self::LoginSuccess(uuid, username) => {
                VarInt(0x02).serialize(output);

                uuid.serialize(output);
                username.serialize(output);
            }
            Self::TimeUpdate(world_age, region_time) => {
                VarInt(0x4E).serialize(output);

                world_age.serialize(output);
                region_time.serialize(output);
            }
            Self::Title(action) => {
                VarInt(0x4F).serialize(output);

                match action {
                    TitleAction::SetTitle(text) => {
                        VarInt(0).serialize(output);
                        text.serialize(output);
                    }
                    TitleAction::SetSubtitle(text) => {
                        VarInt(1).serialize(output);
                        text.serialize(output);
                    }
                    TitleAction::SetActionBar(text) => {
                        VarInt(2).serialize(output);
                        text.serialize(output);
                    }
                    TitleAction::SetDisplayTime(fade_in, display, fade_out) => {
                        VarInt(3).serialize(output);
                        fade_in.serialize(output);
                        display.serialize(output);
                        fade_out.serialize(output);
                    }
                    TitleAction::Hide => {
                        VarInt(4).serialize(output);
                    }
                    TitleAction::Reset => {
                        VarInt(5).serialize(output);
                    }
                }
            }
            Self::PlayerPositionAndLook(x, y, z, yaw, pitch, flags, tp_id) => {
                VarInt(0x34).serialize(output);

                x.serialize(output);
                y.serialize(output);
                z.serialize(output);
                yaw.serialize(output);
                pitch.serialize(output);
                flags.serialize(output);
                tp_id.serialize(output);
            }
            Self::SetBrand(brand) => {
                // this is actually a plugin message
                // but since it's probably the only one we're gonna use
                // it's much easier to make implementation just for this one
                // instead of the generic plugin message
                VarInt(0x17).serialize(output);

                "minecraft:brand".to_string().serialize(output);
                brand.serialize(output);
            }
            Self::DeclareCommands(nodes, root_index) => {
                VarInt(0x10).serialize(output);

                nodes.serialize(output);
                root_index.serialize(output);
            }
            Self::ChatMessage(message, position) => {
                VarInt(0x0E).serialize(output);

                message.serialize(output);
                position.serialize(output);
                0i64.serialize(output);
                0i64.serialize(output);
            }
            Self::ChunkData(
                chunk_x,
                chunk_z,
                primary_bit_mask,
                heightmaps,
                biomes,
                data,
                block_entities,
            ) => {
                VarInt(0x20).serialize(output);

                chunk_x.serialize(output);
                chunk_z.serialize(output);
                true.serialize(output); // always full chunk on this server
                primary_bit_mask.serialize(output);
                heightmaps.to_writer(output).unwrap();
                biomes.serialize(output);
                data.serialize(output);
                VarInt(block_entities.len() as i32).serialize(output);
                for entity in &block_entities {
                    entity.to_writer(output).unwrap();
                }
            }
            Self::JoinGame(
                entity_id,
                is_hardcore,
                gamemode,
                previous_gamemode,
                worlds,
                dimension,
                world_name,
                hashed_seed,
                max_players,
                view_distance,
                reduced_debug_info,
                enable_respawn_screen,
                is_debug,
                is_flat,
            ) => {
                VarInt(0x24).serialize(output);

                entity_id.serialize(output);
                is_hardcore.serialize(output);
                gamemode.serialize(output);
                previous_gamemode.serialize(output);
                VarInt(worlds.len() as i32).serialize(output);
                for world in worlds {
                    world.serialize(output);
                }
                output
                    .write_all(incl!("assets/nbt/dimension_codec.nbt"))
                    .unwrap();
                dimension.to_writer(output).unwrap();
                world_name.serialize(output);
                hashed_seed.serialize(output);
                max_players.serialize(output);
                view_distance.serialize(output);
                reduced_debug_info.serialize(output);
                enable_respawn_screen.serialize(output);
                is_debug.serialize(output);
                is_flat.serialize(output);
            }
        }
    }
}
