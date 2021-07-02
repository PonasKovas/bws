use crate::chat_parse;
use crate::global_state::PStream;
use crate::internal_communication::WBound;
use crate::internal_communication::WReceiver;
use crate::internal_communication::WSender;
use crate::map::Map;
use crate::world::WorldChunkSection;
use crate::GLOBAL_STATE;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use futures::executor::block_on;
use futures::FutureExt;
use log::{debug, error, info, warn};
use protocol::datatypes::*;
use protocol::packets::*;
use sha2::{Digest, Sha256};
use slab::Slab;
use std::borrow::Cow;
use std::cmp::max;
use std::cmp::min;
use std::collections::HashMap;
use std::env::Vars;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use tokio::spawn;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::unconstrained;
use tokio::time::sleep;

use super::{WorldChunk, MAP_SIZE};

const MAP_PATH: &'static str = "assets/maps/lobby.map";

struct Player {
    username: String,
    stream: PStream,
    position: (f64, f64, f64),
    // is synced on each tick
    new_position: (f64, f64, f64),
    rotation: (f32, f32),
    // is synced on each tick
    new_rotation: (f32, f32),
    on_ground: bool,
    // is synced on each tick
    new_on_ground: bool,
    uuid: u128,
    loaded_chunks: Vec<(i8, i8)>, // i8s work because the worlds aren't going to be that big
    inventory: [Slot; 46],
    held_item: i16,
    placing_block: i32,
}

pub struct LobbyWorld {
    players: HashMap<usize, Player>,
    chunks: [WorldChunk; 4 * MAP_SIZE as usize * MAP_SIZE as usize], // 16x16 chunks, resulting in 256x256 world
}

impl LobbyWorld {
    pub async fn new() -> Self {
        // try reading the map data from the fs
        match Map::load(MAP_PATH) {
            Ok(map) => Self {
                players: HashMap::new(),
                chunks: map.chunks.into_owned(),
            },
            Err(e) => {
                error!("Couldn't load the lobby map: {}", e);
                warn!("Falling back to the default map");

                Self {
                    players: HashMap::new(),
                    chunks: {
                        let mut i = 0;
                        [(); 4 * MAP_SIZE as usize * MAP_SIZE as usize].map(|_| {
                            i += 1;
                            WorldChunk {
                                biomes: Box::new([174; 1024]),
                                sections: [
                                    if i == (get_chunk_index(0, 0) + 1)
                                        || i == (get_chunk_index(-1, 0) + 1)
                                        || i == (get_chunk_index(0, -1) + 1)
                                        || i == (get_chunk_index(-1, -1) + 1)
                                    {
                                        Some(WorldChunkSection {
                                            block_mappings: vec![0, 4104],
                                            blocks: {
                                                let mut data = vec![0x1111111111111111; 16];
                                                data.extend(vec![0; 240]);
                                                data
                                            },
                                        })
                                    } else {
                                        None
                                    },
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                ],
                            }
                        })
                    },
                }
            }
        }
    }
    pub async fn run(&mut self, mut w_receiver: WReceiver) {
        let mut counter = 0;
        loop {
            let start_of_tick = Instant::now();

            // first - process all WBound messages on the channel
            self.process_wbound_messages(&mut w_receiver).await;

            // second - read and handle all input from players on this world
            self.process_client_packets().await;

            self.tick(counter).await;

            // and then simulate the game

            // wait until the next tick, if needed
            sleep(
                Duration::from_nanos(1_000_000_000 / 20)
                    .saturating_sub(Instant::now().duration_since(start_of_tick)),
            )
            .await;
            counter += 1;
        }
    }
}
impl LobbyWorld {
    async fn process_wbound_messages(&mut self, w_receiver: &mut WReceiver) {
        loop {
            // Tries executing the future exactly once, without forcing it to yield earlier (because non-cooperative multitasking).
            // If it returns Pending, then break the whole loop, because that means there
            // are no more messages queued up at this moment.
            let message = match unconstrained(w_receiver.recv()).now_or_never().flatten() {
                Some(m) => m,
                None => break,
            };

            match message {
                WBound::AddPlayer { id } => {
                    let (username, stream, uuid) = match GLOBAL_STATE.players.read().await.get(id) {
                        Some(p) => (p.username.clone(), p.stream.clone(), p.uuid),
                        None => {
                            debug!("Tried to add player to world, but the player is already disconnected");
                            continue;
                        }
                    };
                    debug!("client {} joined", id);
                    self.players.insert(
                        id,
                        Player {
                            username,
                            stream,
                            position: (0.0, 20.0, 0.0),
                            new_position: (0.0, 20.0, 0.0),
                            rotation: (0.0, 0.0),
                            new_rotation: (0.0, 0.0),
                            on_ground: false,
                            new_on_ground: false,
                            uuid,
                            loaded_chunks: Vec::new(),
                            inventory: [(); 46].map(|_| Slot(None)),
                            held_item: 0,
                            placing_block: 1,
                        },
                    );

                    if let Err(e) = self.new_player(id).await {
                        debug!("Couldn't send the greetings to a new player: {}", e);
                    }
                }
                WBound::MovePlayer { id, new_world } => match self.players.remove(&id) {
                    Some(_) => {
                        if let Err(_) = new_world.send(WBound::AddPlayer { id }) {
                            error!("Received a request to move a player to a dead world");
                        }
                    }
                    None => {
                        error!("Received a request to move a player, but I don't even have the player.");
                    }
                },
            }
        }
    }
    // both this and new_player dont actually add or remove the player to the hashmap
    async fn player_leave(&mut self, disconnected_id: usize) {
        for (id, player) in &self.players {
            if *id == disconnected_id {
                continue;
            }

            let _ = player
                .stream
                .lock()
                .await
                .send(PlayClientBound::DestroyEntities(vec![VarInt(
                    disconnected_id as i32,
                )]));

            let _ = player.stream.lock().await.send(PlayClientBound::PlayerInfo(
                PlayerInfo::RemovePlayer(vec![(self.players[&disconnected_id].uuid)]),
            ));
        }
    }
    // both this and player_leave dont actually add or remove the player to the hashmap
    async fn new_player(&mut self, id: usize) -> Result<()> {
        // lock the stream
        let mut stream = self.players[&id].stream.lock().await;

        let mut dimension = quartz_nbt::NbtCompound::new();
        dimension.insert("piglin_safe", false);
        dimension.insert("natural", true);
        dimension.insert("ambient_light", 1.0f32);
        dimension.insert("infiniburn", "");
        dimension.insert("respawn_anchor_works", true);
        dimension.insert("has_skylight", true);
        dimension.insert("bed_works", false);
        dimension.insert("effects", "minecraft:overworld");
        dimension.insert("has_raids", false);
        dimension.insert("logical_height", 256i32);
        dimension.insert("coordinate_scale", 1.0f32);
        dimension.insert("ultrawarm", false);
        dimension.insert("has_ceiling", false);

        stream.send(PlayClientBound::Respawn {
            dimension: Nbt(dimension),
            world_name: "lobby".into(),
            hashed_seed: 0,
            gamemode: Gamemode::Creative,
            previous_gamemode: Gamemode::Spectator,
            debug: false,
            flat: true,
            copy_metadata: false,
        })?;

        stream.send(PlayClientBound::PlayerPositionAndLook {
            x: 0.0,
            y: 20.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            flags: PositionAndLookFlags::empty(),
            id: VarInt(0),
        })?;

        stream.send(PlayClientBound::WindowItems {
            window_id: 0,
            slots: ArrWithLen::new([(); 46].map(|_| Slot(None))),
        })?;

        stream.send(PlayClientBound::WorldBorder(
            WorldBorderAction::Initialize {
                x: 0.0,
                z: 0.0,
                old_diameter: 0.0,
                new_diameter: 256.0,
                speed: VarInt(0),
                portal_teleport_boundary: VarInt(128),
                warning_blocks: VarInt(0),
                warning_time: VarInt(0),
            },
        ))?;

        stream.send(PlayClientBound::PluginMessage {
            channel: "minecraft:brand".into(),
            data: "\x03BWS".to_owned().into_bytes().into_boxed_slice(),
        })?;

        stream.send(PlayClientBound::Title(TitleAction::Reset))?;

        stream.send(PlayClientBound::Title(TitleAction::SetTitle(chat_parse(
            "§aLogged in§7!",
        ))))?;

        stream.send(PlayClientBound::Title(TitleAction::SetSubtitle(
            chat_parse("§bhave fun§7!"),
        )))?;

        stream.send(PlayClientBound::Title(TitleAction::SetActionBar(
            chat_parse(""),
        )))?;

        stream.send(PlayClientBound::Title(TitleAction::SetDisplayTime {
            fade_in: 15,
            display: 20,
            fade_out: 15,
        }))?;

        stream.send(PlayClientBound::PlayerListHeaderAndFooter {
            header: chat_parse("\n                    §f§lBWS §rlobby                    \n"),
            footer: chat_parse(""),
        })?;

        drop(stream);

        // inform all players of the new player
        for (_, player) in &self.players {
            let _ = player.stream.lock().await.send(PlayClientBound::PlayerInfo(
                PlayerInfo::AddPlayer(vec![(
                    self.players[&id].uuid,
                    PlayerInfoAddPlayer {
                        name: self.players[&id].username.clone().into(),
                        properties: GLOBAL_STATE.players.read().await[id].properties.clone(),
                        gamemode: Gamemode::Creative,
                        ping: VarInt(GLOBAL_STATE.players.read().await[id].ping as i32),
                        display_name: None,
                    },
                )]),
            ));
        }

        // and now inform the new player of all the old players
        let mut entries = Vec::new();

        for (old_id, player) in &self.players {
            if *old_id == id {
                // dont send info about self!
                continue;
            }

            entries.push((
                self.players[old_id].uuid,
                PlayerInfoAddPlayer {
                    name: player.username.clone().into(),
                    properties: GLOBAL_STATE.players.read().await[*old_id]
                        .properties
                        .clone(),
                    gamemode: Gamemode::Creative,
                    ping: VarInt(GLOBAL_STATE.players.read().await[*old_id].ping as i32),
                    display_name: None,
                },
            ));
        }

        self.players[&id]
            .stream
            .lock()
            .await
            .send(PlayClientBound::PlayerInfo(PlayerInfo::AddPlayer(entries)))?;

        for (old_id, player) in &self.players {
            if *old_id == id {
                // dont spawn myself!
                continue;
            }
            let _ = player
                .stream
                .lock()
                .await
                .send(PlayClientBound::SpawnPlayer {
                    entity_id: VarInt(id as i32),
                    uuid: self.players[&id].uuid,
                    x: self.players[&id].position.0,
                    y: self.players[&id].position.1,
                    z: self.players[&id].position.2,
                    yaw: Angle::from_degrees(self.players[&id].rotation.0),
                    pitch: Angle::from_degrees(self.players[&id].rotation.1),
                });
            let _ = player
                .stream
                .lock()
                .await
                .send(PlayClientBound::EntityHeadLook {
                    entity_id: VarInt(id as i32),
                    head_yaw: Angle::from_degrees(self.players[&id].rotation.0),
                });

            self.players[&id]
                .stream
                .lock()
                .await
                .send(PlayClientBound::SpawnPlayer {
                    entity_id: VarInt(*old_id as i32),
                    uuid: player.uuid,
                    x: player.position.0,
                    y: player.position.1,
                    z: player.position.2,
                    yaw: Angle::from_degrees(player.rotation.0),
                    pitch: Angle::from_degrees(player.rotation.1),
                })?;
            self.players[&id]
                .stream
                .lock()
                .await
                .send(PlayClientBound::EntityHeadLook {
                    entity_id: VarInt(*old_id as i32),
                    head_yaw: Angle::from_degrees(player.rotation.0),
                })?;
        }

        self.send_chunks(id).await?;

        Ok(())
    }
    async fn send_chunks(&mut self, id: usize) -> Result<()> {
        // check which chunk the player currently is
        let player_chunk = (
            (self.players[&id].position.0.floor() / 16.0).floor() as i8,
            (self.players[&id].position.2.floor() / 16.0).floor() as i8,
        );

        let client_view_distance: i8 = GLOBAL_STATE
            .players
            .read()
            .await
            .get(id)
            .context("Player already disconnected")?
            .view_distance
            .unwrap_or(8);

        // the limit is 16, nerds
        let cvd = min(16, client_view_distance + 2);

        let mut needed_chunks = Vec::with_capacity(2 * cvd as usize); // should be enough in most cases
        for z in -cvd..cvd {
            for x in -cvd..cvd {
                // only if its not already sent
                if !self.players[&id]
                    .loaded_chunks
                    .contains(&(player_chunk.0 + x, player_chunk.1 + z))
                {
                    // and only if not outside of map
                    // (1 empty chunk outside of map must be sent for the client to render everything correctly)
                    if (-(MAP_SIZE + 1)..(MAP_SIZE + 1)).contains(&(player_chunk.0 + x))
                        && (-(MAP_SIZE + 1)..(MAP_SIZE + 1)).contains(&(player_chunk.1 + z))
                    {
                        needed_chunks.push((player_chunk.0 + x, player_chunk.1 + z));
                    }
                }
            }
        }

        // update the loaded_chunks
        // retain only those that are in the view distance of the client
        self.players
            .get_mut(&id)
            .unwrap()
            .loaded_chunks
            .retain(|(x, z)| {
                (-client_view_distance..client_view_distance).contains(&(x - player_chunk.0))
                    && (-client_view_distance..client_view_distance).contains(&(z - player_chunk.1))
            });
        // and then add those that we're gonna send in a second
        self.players
            .get_mut(&id)
            .unwrap()
            .loaded_chunks
            .extend(&needed_chunks);

        // and then finally send all the chunks that are needed
        for chunk in needed_chunks {
            let temp_chunk = if (-MAP_SIZE..MAP_SIZE).contains(&chunk.0)
                && (-MAP_SIZE..MAP_SIZE).contains(&chunk.1)
            {
                let chunk_index = get_chunk_index(chunk.0, chunk.1);

                let mut primary_bitmask = 0;
                let mut chunk_sections = Vec::new();

                for (i, section) in self.chunks[chunk_index].sections.iter().enumerate() {
                    if let Some(section) = section {
                        primary_bitmask |= 2i32.pow(i as u32);

                        chunk_sections.push(ChunkSection {
                            block_count: 16 * 16 * 16, // this is foolproof, but might want to send the real block count in the future
                            palette: Palette::Indirect(
                                section.block_mappings.iter().map(|v| VarInt(*v)).collect(),
                            ),
                            data: section.blocks.clone(),
                        });
                    }
                }

                Chunk::Full {
                    primary_bitmask: VarInt(primary_bitmask),
                    heightmaps: Nbt(quartz_nbt::NbtCompound::new()),
                    biomes: ArrWithLen::new(
                        self.chunks[chunk_index].biomes.clone().map(|e| VarInt(e)),
                    ),
                    sections: ChunkSections(chunk_sections),
                    block_entities: Vec::new(),
                }
            } else {
                // just an empty chunk
                Chunk::Full {
                    primary_bitmask: VarInt(0b0),
                    heightmaps: Nbt(quartz_nbt::NbtCompound::new()),
                    biomes: ArrWithLen::new([VarInt(174); 1024]),
                    sections: ChunkSections(vec![]),
                    block_entities: Vec::new(),
                }
            };
            self.players[&id]
                .stream
                .lock()
                .await
                .send(PlayClientBound::ChunkData {
                    chunk_x: chunk.0 as i32,
                    chunk_z: chunk.1 as i32,
                    chunk: temp_chunk,
                })?;
        }

        Ok(())
    }
    async fn process_client_packets(&mut self) {
        // forgive me father, for the borrow checker does not let me do it any other way
        let keys: Vec<usize> = self.players.keys().copied().collect();

        for id in keys {
            'inner: loop {
                let r = self.players[&id].stream.lock().await.try_recv();
                match r {
                    Ok(Some(packet)) => {
                        self.handle_packet(id, packet).await;
                    }
                    Ok(None) => break 'inner, // go on to the next client
                    Err(_) => {
                        // eww, looks like someone disconnected!!
                        // time to clean this up
                        self.player_leave(id).await;
                        self.players.remove(&id);
                        break 'inner;
                    }
                }
            }
        }
    }
    async fn handle_packet(&mut self, id: usize, packet: PlayServerBound<'static>) {
        match packet {
            PlayServerBound::ChatMessage(message) => {
                if message.starts_with('/') {
                    if message.starts_with("/block ") {
                        if let Some(block_id) = message.split(' ').nth(1) {
                            if let Ok(block_id) = i32::from_str_radix(block_id, 10) {
                                self.players.get_mut(&id).unwrap().placing_block = block_id;
                            }
                        }
                    }
                } else {
                    for (_, player) in &self.players {
                        let _ = player
                            .stream
                            .lock()
                            .await
                            .send(PlayClientBound::ChatMessage {
                                message: chat_parse(format!(
                                    "§a§l{}§r§7: §f{}",
                                    self.players[&id].username, message
                                )),
                                position: ChatPosition::Chat,
                                sender: self.players[&id].uuid,
                            });
                    }
                }
            }
            PlayServerBound::PlayerPosition {
                x,
                feet_y,
                z,
                on_ground,
            } => {
                let _ = self.set_player_position(id, (x, feet_y, z)).await;
                self.set_player_on_ground(id, on_ground).await;
            }
            PlayServerBound::PlayerPositionAndRotation {
                x,
                feet_y,
                z,
                yaw,
                pitch,
                on_ground,
            } => {
                let _ = self.set_player_position(id, (x, feet_y, z)).await;
                self.set_player_rotation(id, (yaw, pitch)).await;
                self.set_player_on_ground(id, on_ground).await;
            }
            PlayServerBound::PlayerRotation {
                yaw,
                pitch,
                on_ground,
            } => {
                self.set_player_rotation(id, (yaw, pitch)).await;
                self.set_player_on_ground(id, on_ground).await;
            }
            PlayServerBound::PlayerMovement { on_ground } => {
                self.set_player_on_ground(id, on_ground).await;
            }
            PlayServerBound::Animation { hand } => {
                for (r_id, player) in &self.players {
                    if *r_id == id {
                        continue;
                    }

                    let _ = player
                        .stream
                        .lock()
                        .await
                        .send(PlayClientBound::EntityAnimation {
                            entity_id: VarInt(id as i32),
                            animation: match hand {
                                Hand::Left => EntityAnimation::SwingOffhand,
                                Hand::Right => EntityAnimation::SwingMainArm,
                            },
                        });
                }
            }
            PlayServerBound::CreativeInventoryAction { slot, item } => {
                // first make sure the client even has the permissions
                // (todo)

                // some sanity checks
                if !(-1..=45).contains(&slot) {
                    debug!(
                        "Client {} sent CreativeInventoryAction with invalid slot id ({})",
                        id, slot
                    );
                    return;
                }
                if slot < 0 {
                    // should drop the item
                    // but no need for this in the lobby
                } else {
                    self.players.get_mut(&id).unwrap().inventory[slot as usize] = item.clone();
                    let _ = self.players[&id]
                        .stream
                        .lock()
                        .await
                        .send(PlayClientBound::SetSlot {
                            window_id: 0,
                            slot,
                            slot_data: item,
                        });
                }
            }
            PlayServerBound::HeldItemChange { slot } => {
                self.players.get_mut(&id).unwrap().held_item = slot;
            }
            PlayServerBound::PlayerBlockPlacement {
                hand,
                location,
                face,
                cursor_position_x: _,
                cursor_position_y: _,
                cursor_position_z: _,
                inside_block: _,
            } => {
                let mut target = location.clone();
                match face {
                    Direction::Down => {
                        target.y -= 1;
                    }
                    Direction::Up => {
                        target.y += 1;
                    }
                    Direction::North => {
                        target.z -= 1;
                    }
                    Direction::South => {
                        target.z += 1;
                    }
                    Direction::West => {
                        target.x -= 1;
                    }
                    Direction::East => {
                        target.x += 1;
                    }
                }

                // get the item in hand of player
                // let block = match hand {
                //     Hand::Left => {

                //     }
                //     Hand::Right => {

                //     }
                // };
                let block = self.players[&id].placing_block;

                if let Err(e) = self.set_block(target, block).await {
                    debug!("Error placing block: {:?}", e);
                }
            }
            PlayServerBound::EntityAction {
                entity_id: _,
                action,
                jump_boost: _,
            } => {
                if action == EntityAction::StartSprinting {
                    for (_id, player) in &self.players {
                        let _ = player
                            .stream
                            .lock()
                            .await
                            .send(PlayClientBound::EntityStatus {
                                entity_id: id as i32,
                                status: 43,
                            });
                    }
                } else if action == EntityAction::StartSneaking {
                    info!("[{}] started sneaking", id);
                    for (_id, player) in &self.players {
                        let _ = player
                            .stream
                            .lock()
                            .await
                            .send(PlayClientBound::EntityMetadata {
                                entity_id: VarInt(id as i32),
                                metadata: EntityMetadata(vec![
                                    (0, EntityMetadataEntry::Byte(0x02 | 0x40)),
                                    (6, EntityMetadataEntry::Pose(Pose::Sneaking)),
                                ]),
                            });
                    }
                } else if action == EntityAction::StopSneaking {
                    info!("[{}] stopped sneaking", id);
                    for (_id, player) in &self.players {
                        let _ = player
                            .stream
                            .lock()
                            .await
                            .send(PlayClientBound::EntityMetadata {
                                entity_id: VarInt(id as i32),
                                metadata: EntityMetadata(vec![
                                    (0, EntityMetadataEntry::Byte(0x00)),
                                    (6, EntityMetadataEntry::Pose(Pose::Standing)),
                                ]),
                            });
                    }
                }
            }
            PlayServerBound::PlayerDigging {
                status,
                location,
                face,
            } => match status {
                // this means block broken but only when in creative mode
                PlayerDiggingStatus::StartedDigging => {
                    if let Err(e) = self.set_block(location, 0).await {
                        debug!("Error breaking block: {:?}", e);
                    }
                }
                _ => {
                    debug!(
                        "[{}] received {:?}",
                        id,
                        PlayServerBound::PlayerDigging {
                            status,
                            location,
                            face,
                        }
                    );
                }
            },
            other => {
                debug!("[{}] received {:?}", id, other);
            }
        }
    }
    async fn set_block(&mut self, position: Position, glob_block: i32) -> Result<()> {
        // sanity checks
        if !(0..256).contains(&position.y)
            || !((-MAP_SIZE as i32 * 16)..(MAP_SIZE as i32 * 16)).contains(&position.x)
            || !((-MAP_SIZE as i32 * 16)..(MAP_SIZE as i32 * 16)).contains(&position.z)
        {
            bail!("Position out of bounds");
        }

        let mut block_chunk = position;
        if position.x < 0 {
            block_chunk.x -= 15;
        }
        if position.z < 0 {
            block_chunk.z -= 15;
        }
        block_chunk.x /= 16;
        block_chunk.y /= 16;
        block_chunk.z /= 16;

        // block position relative to the chunk
        let iblock = Position {
            x: ((position.x % 16) + 16) % 16,
            y: ((position.y % 16) + 16) % 16,
            z: ((position.z % 16) + 16) % 16,
        };

        let section = &mut self.chunks[get_chunk_index(block_chunk.x as i8, block_chunk.z as i8)]
            .sections[block_chunk.y as usize];

        if section.is_none() {
            // initialize the section with air
            section.replace(WorldChunkSection {
                block_mappings: vec![0],
                blocks: vec![0u64; 256],
            });
        }

        let section = section.as_mut().unwrap();

        // removing blocks from the palette would require remapping of the whole chunk section
        // with little benefit, so we're just not gonna do that right now.
        // maybe later.
        //
        // // adjust palette
        // // might want to remove a block from the palette if the block which was previously
        // // in this position is no longer present in the whole section
        // // so get the block that was in that position previously
        // let old_block = get_section_block(section, position);
        //
        // // check if any other blocks of this type are in this section
        // let mut more_of_old = false;
        // for y in 0..16 {
        //     for z in 0..16 {
        //         for x in 0..16 {
        //             let p = Position { x, y, z };
        //             // don't compare to the self
        //             if p == iblock {
        //                 continue;
        //             }

        //             if old_block == get_section_block(section, p) {
        //                 more_of_old = true;
        //                 break;
        //             }
        //         }
        //     }
        // }

        // add the new block to the palette, if not there already
        let block = match section.block_mappings.iter().position(|v| *v == glob_block) {
            Some(index) => index as i32,
            None => {
                // Insert the block into the pallete
                section.block_mappings.push(glob_block);
                (section.block_mappings.len() - 1) as i32
            }
        };

        set_section_block(section, iblock, block);

        // if the block was set to air, might want to remove the whole chunk section
        // todo use a const or something
        if glob_block == 0 {
            // if there are no more non-air blocks we can just unload the chunk to save memory
            let air_in_palette = section.block_mappings.iter().position(|v| *v == 0);
            match air_in_palette {
                Some(air) => {
                    let mut empty = true;

                    'outermost: for z in 0..16 {
                        for y in 0..16 {
                            for x in 0..16 {
                                let pos = Position { x, y, z };
                                if get_section_block(section, pos) == air as i32 {
                                    empty = false;
                                    break 'outermost;
                                }
                            }
                        }
                    }

                    if empty {
                        self.chunks[get_chunk_index(block_chunk.x as i8, block_chunk.z as i8)]
                            .sections[block_chunk.y as usize] = None;
                    }
                }
                None => {
                    // what?
                    // no air in the palette?
                }
            }
        }

        // save the changes
        // TODO make this async somehow
        if let Err(e) = (Map {
            chunks: Cow::Borrowed(&self.chunks),
        }
        .save(MAP_PATH))
        {
            error!("Error saving map data: {}", e);
        }

        self.inform_players_of_block_change(position, VarInt(glob_block))
            .await?;

        Ok(())
    }
    async fn inform_players_of_block_change(
        &self,
        position: Position,
        new_id: VarInt,
    ) -> Result<()> {
        let chunk = (position.x / 16, position.z / 16);
        for (_id, player) in &self.players {
            if player
                .loaded_chunks
                .contains(&(chunk.0 as i8, chunk.1 as i8))
            {
                player
                    .stream
                    .lock()
                    .await
                    .send(PlayClientBound::BlockChange {
                        location: position.clone(),
                        new_block_id: new_id,
                    })?;
            }
        }

        Ok(())
    }
    async fn set_player_position(
        &mut self,
        id: usize,
        new_position: (f64, f64, f64),
    ) -> Result<()> {
        self.players.get_mut(&id).unwrap().new_position = new_position;

        Ok(())
    }
    async fn set_player_rotation(&mut self, id: usize, rotation: (f32, f32)) {
        self.players.get_mut(&id).unwrap().new_rotation = rotation;
    }
    async fn set_player_on_ground(&mut self, id: usize, on_ground: bool) {
        self.players.get_mut(&id).unwrap().new_on_ground = on_ground;
    }
    async fn tick(&mut self, counter: u128) {
        if counter % 100 == 0 {
            // every 5 seconds, update all pings
            let mut entries = Vec::with_capacity(self.players.len());
            for (id, player) in &self.players {
                entries.push((
                    player.uuid,
                    PlayerInfoUpdateLatency {
                        ping: VarInt(GLOBAL_STATE.players.read().await[*id].ping as i32),
                    },
                ));
            }
            for (_id, player) in &self.players {
                let _ = player.stream.lock().await.send(PlayClientBound::PlayerInfo(
                    PlayerInfo::UpdateLatency(entries.clone()),
                ));
            }
        }

        // test
        // for (_id, player) in &self.players {
        //     for (id, _player) in &self.players {
        //         let _ = player
        //             .stream
        //             .lock()
        //             .await
        //             .send(PlayClientBound::EntityStatus {
        //                 entity_id: *id as i32,
        //                 status: 43,
        //             });
        //     }
        // }

        // sync all the player positions and rotation
        for id in self.players.keys().copied().collect::<Vec<usize>>() {
            let mut packets = Vec::new();

            let position_change = if self.players[&id].position == self.players[&id].new_position {
                // position didnt change
                None
            } else {
                // position changed

                // check if chunk passed
                let old_chunks = (
                    (self.players[&id].position.0.floor() / 16.0).floor(),
                    (self.players[&id].position.2.floor() / 16.0).floor(),
                );
                let old_y = self.players[&id].position.1.floor();
                let new_chunks = (
                    (self.players[&id].new_position.0.floor() / 16.0).floor(),
                    (self.players[&id].new_position.2.floor() / 16.0).floor(),
                );
                let new_y = self.players[&id].new_position.1.floor();

                let chunk_passed =
                    !((old_chunks.0 == new_chunks.0) && (old_chunks.1 == new_chunks.1));

                if chunk_passed || old_y != new_y {
                    let _ = self.players[&id].stream.lock().await.send(
                        PlayClientBound::UpdateViewPosition {
                            chunk_x: VarInt(new_chunks.0 as i32),
                            chunk_z: VarInt(new_chunks.1 as i32),
                        },
                    );
                }

                let position_change = (
                    self.players[&id].new_position.0 - self.players[&id].position.0,
                    self.players[&id].new_position.1 - self.players[&id].position.1,
                    self.players[&id].new_position.2 - self.players[&id].position.2,
                );

                self.players.get_mut(&id).unwrap().position = self.players[&id].new_position;

                if chunk_passed {
                    // send new chunks
                    let _ = self.send_chunks(id).await;
                }

                Some(position_change)
            };

            let rotation_change = if self.players[&id].rotation == self.players[&id].new_rotation {
                // Rotation didnt change
                None
            } else {
                // Rotation changed
                // sync
                self.players.get_mut(&id).unwrap().rotation = self.players[&id].new_rotation;

                Some(self.players[&id].rotation)
            };

            // sync on_ground
            self.players.get_mut(&id).unwrap().on_ground = self.players[&id].new_on_ground;

            let on_ground = self.players[&id].on_ground;

            match (position_change, rotation_change) {
                (None, None) => packets.push(PlayClientBound::EntityMovement {
                    entity_id: VarInt(id as i32),
                }),
                (Some(pos_change), None) => {
                    // check if the change is on any of the axes is larger than 8, in that case
                    // we must send EntityTeleport instead of EntityPosition
                    if (pos_change.0 < -8.0)
                        || (pos_change.0 > 8.0)
                        || (pos_change.1 < -8.0)
                        || (pos_change.1 > 8.0)
                        || (pos_change.2 < -8.0)
                        || (pos_change.2 > 8.0)
                    {
                        packets.push(PlayClientBound::EntityTeleport {
                            entity_id: VarInt(id as i32),
                            x: self.players[&id].new_position.0,
                            y: self.players[&id].new_position.1,
                            z: self.players[&id].new_position.2,
                            yaw: Angle::from_degrees(self.players[&id].rotation.0),
                            pitch: Angle::from_degrees(self.players[&id].rotation.1),
                            on_ground,
                        });
                    } else {
                        packets.push(PlayClientBound::EntityPosition {
                            entity_id: VarInt(id as i32),
                            delta_x: (pos_change.0 * 4096.0) as i16,
                            delta_y: (pos_change.1 * 4096.0) as i16,
                            delta_z: (pos_change.2 * 4096.0) as i16,
                            on_ground,
                        });
                    }
                }
                (None, Some(rotation)) => {
                    packets.push(PlayClientBound::EntityRotation {
                        entity_id: VarInt(id as i32),
                        yaw: Angle::from_degrees(rotation.0),
                        pitch: Angle::from_degrees(rotation.1),
                        on_ground,
                    });
                    packets.push(PlayClientBound::EntityHeadLook {
                        entity_id: VarInt(id as i32),
                        head_yaw: Angle::from_degrees(rotation.0),
                    });
                }
                (Some(pos_change), Some(rotation)) => {
                    // check if the change is on any of the axes is larger than 8, in that case
                    // we must send EntityTeleport and EntityRotation instead of EntityPositionAndRotation
                    if (pos_change.0 < -8.0)
                        || (pos_change.0 > 8.0)
                        || (pos_change.1 < -8.0)
                        || (pos_change.1 > 8.0)
                        || (pos_change.2 < -8.0)
                        || (pos_change.2 > 8.0)
                    {
                        packets.push(PlayClientBound::EntityTeleport {
                            entity_id: VarInt(id as i32),
                            x: self.players[&id].new_position.0,
                            y: self.players[&id].new_position.1,
                            z: self.players[&id].new_position.2,
                            yaw: Angle::from_degrees(rotation.0),
                            pitch: Angle::from_degrees(rotation.1),
                            on_ground,
                        });
                    } else {
                        packets.push(PlayClientBound::EntityPositionAndRotation {
                            entity_id: VarInt(id as i32),
                            delta_x: (pos_change.0 * 4096.0) as i16,
                            delta_y: (pos_change.1 * 4096.0) as i16,
                            delta_z: (pos_change.2 * 4096.0) as i16,
                            yaw: Angle::from_degrees(rotation.0),
                            pitch: Angle::from_degrees(rotation.1),
                            on_ground,
                        });
                    }

                    packets.push(PlayClientBound::EntityHeadLook {
                        entity_id: VarInt(id as i32),
                        head_yaw: Angle::from_degrees(rotation.0),
                    });
                }
            }

            for (r_id, r_player) in &self.players {
                if id == *r_id {
                    continue; // dont sent info about self
                }

                for packet in packets.clone() {
                    let _ = r_player.stream.lock().await.send(packet.clone());
                }
            }
        }
    }
}

fn get_chunk_index(x: i8, z: i8) -> usize {
    (x + MAP_SIZE) as usize + 2 * MAP_SIZE as usize * (z + MAP_SIZE) as usize
}

fn bits_per_block(section: &WorldChunkSection) -> u8 {
    max(
        4,
        32u8 - max(section.block_mappings.len() as u32 - 1, 1).leading_zeros() as u8,
    )
}

// position is not global, but relative to the chunk. negative or bigger than 15 values are illegal
fn get_section_block(section: &WorldChunkSection, position: Position) -> i32 {
    let bits_per_block = bits_per_block(section);
    let blocks_per_i64 = 64 / bits_per_block as usize;

    let block_index =
        position.x as usize + 16 * (position.z as usize) + 16 * 16 * (position.y as usize);

    let mut t = section.blocks[block_index / blocks_per_i64] as u64;

    let bits_to_the_right =
        64 - bits_per_block as i32 * (block_index as i32 % blocks_per_i64 as i32 + 1);
    t = t << bits_to_the_right;

    let bits_to_the_left = bits_per_block as i32 * (block_index as i32 % blocks_per_i64 as i32);
    t = t >> (bits_to_the_right + bits_to_the_left);

    t as i32
}

// position is not global, but relative to the chunk. negative or bigger than 15 values are illegal
// and the block value is of the local pallete, not global
fn set_section_block(section: &mut WorldChunkSection, position: Position, block: i32) {
    let old_block = get_section_block(section, position);

    let bits_per_block = bits_per_block(section);
    let blocks_per_i64 = 64 / bits_per_block as usize;

    let block_index =
        position.x as usize + 16 * (position.z as usize) + 16 * 16 * (position.y as usize);

    // logic magic
    section.blocks[block_index / blocks_per_i64] ^= ((block ^ old_block) as u64)
        << (bits_per_block as i32 * (block_index as i32 % blocks_per_i64 as i32));
}

pub fn start() -> WSender {
    let (w_sender, w_receiver) = unbounded_channel::<WBound>();

    spawn(async move { LobbyWorld::new().await.run(w_receiver).await });

    w_sender
}
