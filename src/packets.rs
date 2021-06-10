use crate::{datatypes::*, world};
use nbt::Blob as Nbt;
use std::{
    env::Vars,
    hash,
    io::{self, Cursor, Write},
};

// Sent from the client to the server
#[derive(Debug, Clone)]
pub enum ServerBound {
    Handshake {
        protocol: VarInt,
        address: String,
        port: u16,
        next_state: VarInt,
    },
    StatusRequest,
    StatusPing(i64),
    LoginStart {
        username: String,
    },
    KeepAlive(i64),
    ChatMessage(String), // the raw message
    PlayerPosition {
        x: f64,
        y: f64,
        z: f64,
        on_ground: bool,
    },
    PlayerPositionAndRotation {
        x: f64,
        y: f64,
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
    ClientSettings {
        locale: String,
        view_distance: i8,
        chat_mode: VarInt,
        chat_colors: bool,
        skin_parts: u8,
        main_hand: VarInt,
    },
    // ChatMessage(String), // the raw message, up to 256 characters
    // ClientStatus(VarInt), // 0 - respawn, 1 - request statistics
    // InteractEntity(VarInt, VarInt, bool), // entity id, [0 - interact, 1 - attack, 2 - interact at (not supported)], whether sneaking

    // Animation(VarInt),                                        // 0 - main hand, 1 - off hand
    // TeleportConfirm(VarInt),                                  // teleport id
    // EntityAction(VarInt, VarInt, VarInt), // player's entity id, action (see https://wiki.vg/index.php?title=Protocol&oldid=16091#Entity_Action), jump boost (only for jumping with horse)
    // HeldItemChange(i16),                  // slot id 0-8
    // UseItem(VarInt),                      // 0 - main hand, 1 - off hand
    // PlayerDigging(VarInt, i64, i8),       // action [0-6], position, face
    Unknown(i32), // the packet id of the unknown packet
}

// Sent from the server to the client
#[derive(Debug, Clone)]
pub enum ClientBound {
    StatusResponse(String), // json
    StatusPong(i64),        // the same random number
    LoginDisconnect(Chat),
    KeepAlive(i64),
    SetCompression {
        treshold: VarInt,
    },
    LoginSuccess {
        uuid: u128,
        username: String,
    },
    JoinGame {
        eid: i32, // entity ID, global on the server
        hardcore: bool,
        gamemode: u8,
        previous_gamemode: i8, // whats the purpose of this?
        world_names: Vec<String>,
        dimension: Nbt,
        world_name: String,
        hashed_seed: i64,
        max_players: VarInt, // doesn't do anything
        view_distance: VarInt,
        reduced_debug_info: bool, // shows less on the F3 debug screen
        enable_respawn_screen: bool,
        debug_mode: bool, // debug worlds cannot be modified and have predefined blocks
        flat: bool,       // flat worlds have horizon at y=0 instead of y=63 and different void fog
    },
    TimeUpdate {
        world_age: i64,
        region_time: i64,
    },
    Respawn {
        dimension: Nbt,
        world_name: String,
        hashed_seed: i64,
        gamemode: u8,
        previous_gamemode: u8, // again whats the purpose?
        debug_mode: bool,      // same as in JoinGame
        flat: bool,            // same as in JoinGame
        copy_metadata: bool,   // not sure what kind of data but yeah
    },
    Title(TitleAction),
    PlayerPositionAndLook {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        flags: u8, // bitflags if the previous values were absolute or relative, todo bitflags
        tp_id: VarInt, // the client later confirms the teleport with this id
    },
    SetBrand(String),
    DeclareCommands {
        nodes: Vec<CommandNode>,
        root: VarInt, // index of the root node in the above vector
    },
    ChatMessage {
        message: Chat,
        position: u8, // todo enum, 0: chat (chat box), 1: system message (chat box), 2: game info (above hotbar).
    },
    ChunkData {
        chunk_x: i32,
        chunk_z: i32,
        primary_bit_mask: VarInt, // bits 0-15, if 1 then the chunk section will be sent in this packet
        heightmaps: Nbt,
        biomes: [VarInt; 1024], // 4x4x4 sections in the entire chunk (16x256x16),
        sections: Vec<ChunkSection>,
        block_entities: Vec<Nbt>,
    },
    PlayDisconnect(Chat),
    NamedSoundEffect {
        identifier: String, // of the sound
        category: VarInt,   // music, ambient etc
        // global coordinates multiplied by 8 and casted into an integer
        x: i32,
        y: i32,
        z: i32,
        volume: f32, // 1.0 is 100%
        pitch: f32,
    },
    EntitySoundEffect {
        sound_id: VarInt,
        category: VarInt,  // same as in NamedSoundEffect, todo enum
        entity_id: VarInt, // EID of the entity from which to play the sound from
        volume: f32,       // 1.0 is 100%
        pitch: f32,
    },
    UpdateViewPosition {
        chunk_x: VarInt,
        chunk_z: VarInt,
    },
    Tags, // fixed
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
    SetDisplayTime {
        // time in ticks
        fade_in: i32,
        display: i32,
        fade_out: i32,
    },
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
                    0x00 => Ok(Self::Handshake {
                        protocol: VarInt::deserialize(input)?,
                        address: String::deserialize(input)?,
                        port: u16::deserialize(input)?,
                        next_state: VarInt::deserialize(input)?,
                    }),
                    _ => Ok(Self::Unknown(packet_id)),
                }
            }
            1 => {
                // Status
                match packet_id {
                    0x00 => Ok(Self::StatusRequest),
                    0x01 => Ok(Self::StatusPing(i64::deserialize(input)?)),
                    _ => Ok(Self::Unknown(packet_id)),
                }
            }
            2 => {
                // Login
                match packet_id {
                    0x00 => Ok(Self::LoginStart {
                        username: String::deserialize(input)?,
                    }),
                    _ => Ok(Self::Unknown(packet_id)),
                }
            }
            3 => {
                // Play
                match packet_id {
                    0x10 => Ok(Self::KeepAlive(i64::deserialize(input)?)),
                    0x03 => Ok(Self::ChatMessage(String::deserialize(input)?)),
                    0x05 => Ok(Self::ClientSettings {
                        locale: String::deserialize(input)?,
                        view_distance: i8::deserialize(input)?,
                        chat_mode: VarInt::deserialize(input)?,
                        chat_colors: bool::deserialize(input)?,
                        skin_parts: u8::deserialize(input)?,
                        main_hand: VarInt::deserialize(input)?,
                    }),
                    0x12 => Ok(Self::PlayerPosition {
                        x: f64::deserialize(input)?,
                        y: f64::deserialize(input)?,
                        z: f64::deserialize(input)?,
                        on_ground: bool::deserialize(input)?,
                    }),
                    0x13 => Ok(Self::PlayerPositionAndRotation {
                        x: f64::deserialize(input)?,
                        y: f64::deserialize(input)?,
                        z: f64::deserialize(input)?,
                        yaw: f32::deserialize(input)?,
                        pitch: f32::deserialize(input)?,
                        on_ground: bool::deserialize(input)?,
                    }),
                    0x14 => Ok(Self::PlayerRotation {
                        yaw: f32::deserialize(input)?,
                        pitch: f32::deserialize(input)?,
                        on_ground: bool::deserialize(input)?,
                    }),
                    0x15 => Ok(Self::PlayerMovement {
                        on_ground: bool::deserialize(input)?,
                    }),
                    _ => Ok(Self::Unknown(packet_id)),
                }
            }
            _ => Ok(Self::Unknown(packet_id)),
        };

        result
    }
}

impl ClientBound {
    pub fn serialize<W: Write>(&self, output: &mut W) {
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
            Self::SetCompression { treshold } => {
                VarInt(0x03).serialize(output);

                treshold.serialize(output);
            }
            Self::KeepAlive(number) => {
                VarInt(0x1F).serialize(output);

                number.serialize(output);
            }
            Self::LoginSuccess { uuid, username } => {
                VarInt(0x02).serialize(output);

                uuid.serialize(output);
                username.serialize(output);
            }
            Self::TimeUpdate {
                world_age,
                region_time,
            } => {
                VarInt(0x4E).serialize(output);

                world_age.serialize(output);
                region_time.serialize(output);
            }
            Self::UpdateViewPosition { chunk_x, chunk_z } => {
                VarInt(0x40).serialize(output);

                chunk_x.serialize(output);
                chunk_z.serialize(output);
            }
            Self::Tags => {
                VarInt(0x5B).serialize(output);

                output.write_all(incl!("assets/raw/tags.bin")).unwrap();
            }
            Self::Respawn {
                dimension,
                world_name,
                hashed_seed,
                gamemode,
                previous_gamemode,
                debug_mode,
                flat,
                copy_metadata,
            } => {
                VarInt(0x39).serialize(output);

                dimension.to_writer(output).unwrap();
                world_name.serialize(output);
                hashed_seed.serialize(output);
                gamemode.serialize(output);
                previous_gamemode.serialize(output);
                debug_mode.serialize(output);
                flat.serialize(output);
                copy_metadata.serialize(output);
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
                    TitleAction::SetDisplayTime {
                        fade_in,
                        display,
                        fade_out,
                    } => {
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
            Self::NamedSoundEffect {
                identifier,
                category,
                x,
                y,
                z,
                volume,
                pitch,
            } => {
                VarInt(0x18).serialize(output);

                identifier.serialize(output);
                category.serialize(output);
                x.serialize(output);
                y.serialize(output);
                z.serialize(output);
                volume.serialize(output);
                pitch.serialize(output);
            }
            Self::EntitySoundEffect {
                sound_id,
                category,
                entity_id,
                volume,
                pitch,
            } => {
                VarInt(0x50).serialize(output);

                sound_id.serialize(output);
                category.serialize(output);
                entity_id.serialize(output);
                volume.serialize(output);
                pitch.serialize(output);
            }
            Self::PlayerPositionAndLook {
                x,
                y,
                z,
                yaw,
                pitch,
                flags,
                tp_id,
            } => {
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
            Self::DeclareCommands { nodes, root } => {
                VarInt(0x10).serialize(output);

                nodes.serialize(output);
                root.serialize(output);
            }
            Self::ChatMessage { message, position } => {
                VarInt(0x0E).serialize(output);

                message.serialize(output);
                position.serialize(output);
                0i64.serialize(output);
                0i64.serialize(output);
            }
            Self::ChunkData {
                chunk_x,
                chunk_z,
                primary_bit_mask,
                heightmaps,
                biomes,
                sections,
                block_entities,
            } => {
                VarInt(0x20).serialize(output);

                chunk_x.serialize(output);
                chunk_z.serialize(output);
                true.serialize(output); // always full chunk on this server
                primary_bit_mask.serialize(output);
                heightmaps.to_writer(output).unwrap();
                VarInt(1024).serialize(output); // nice, mojang
                biomes.serialize(output);
                let mut chunk_sections_size = 0i32;
                for chunk_section in sections {
                    chunk_sections_size += 3; // 2 bytes for block count, 1 byte for "bits per block"
                    match &chunk_section.palette {
                        Palette::Indirect(palette) => {
                            chunk_sections_size += VarInt(palette.len() as i32).size() as i32;
                            for block in palette {
                                chunk_sections_size += block.size() as i32;
                            }
                        }
                        Palette::Direct => {}
                    }

                    chunk_sections_size += VarInt(chunk_section.data.len() as i32).size() as i32;
                    chunk_sections_size += 8 * chunk_section.data.len() as i32; // i64s
                }
                VarInt(chunk_sections_size).serialize(output);
                for chunk_section in sections {
                    chunk_section.serialize(output);
                }
                VarInt(block_entities.len() as i32).serialize(output);
                for entity in block_entities {
                    entity.to_writer(output).unwrap();
                }
            }
            Self::JoinGame {
                eid,
                hardcore,
                gamemode,
                previous_gamemode,
                world_names,
                dimension,
                world_name,
                hashed_seed,
                max_players,
                view_distance,
                reduced_debug_info,
                enable_respawn_screen,
                debug_mode,
                flat,
            } => {
                VarInt(0x24).serialize(output);

                eid.serialize(output);
                hardcore.serialize(output);
                gamemode.serialize(output);
                previous_gamemode.serialize(output);
                world_names.serialize(output);
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
                debug_mode.serialize(output);
                flat.serialize(output);
            }
        }
    }
}
