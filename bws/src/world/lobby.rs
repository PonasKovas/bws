use crate::chat_parse;
use crate::collision::is_colliding;
use crate::data::Block;
use crate::global_state::PStream;
use crate::internal_communication::{WBound, WReceiver, WSender};
use crate::map::Map;
use crate::shared::*;
use crate::world::WorldChunkSection;
use crate::GLOBAL_STATE;
use anyhow::{bail, Context, Result};
use futures::{executor::block_on, FutureExt};
use log::{debug, error, info, warn};
use protocol::{command, datatypes::*, packets::*};
use sha2::{Digest, Sha256};
use slab::Slab;
use std::borrow::Cow;
use std::cmp::max;
use std::convert::TryInto;
use std::env::Vars;
use std::path::Path;
use std::time::{Duration, Instant};
use std::{cmp::min, collections::HashMap};
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
};
use tokio::spawn;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::unconstrained;
use tokio::time::sleep;

use super::{WorldChunk, WorldChunks};

const MAP_SIZE: i8 = 4;
const MAP_CHUNKS: usize = MAP_SIZE as usize * MAP_SIZE as usize * 4;

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
    nickname_color: u32,
    editing_lobby: bool,
}

pub struct LobbyWorld {
    players: HashMap<usize, Player>,
    chunks: WorldChunks<MAP_CHUNKS>, // 16x16 chunks, resulting in 256x256 world
    flowing_liquids: Vec<Position>, // positions of blocks that are liquids and need to be updated every 5 ticks
}

impl LobbyWorld {
    pub async fn new() -> Self {
        // try reading the map data from the fs
        match Map::load(MAP_PATH).await {
            Ok(map) => Self {
                players: HashMap::new(),
                chunks: map.chunks.into_owned(),
                flowing_liquids: Vec::new(),
            },
            Err(e) => {
                error!("Couldn't load the lobby map: {:?}", e);
                warn!("Falling back to the default map");

                Self {
                    players: HashMap::new(),
                    chunks: [(); 4 * MAP_SIZE as usize * MAP_SIZE as usize].map(|_| {
                        box WorldChunk {
                            biomes: box [174; 1024],
                            sections: [(); 16].map(|_| None),
                        }
                    }),
                    flowing_liquids: Vec::new(),
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
                            nickname_color: (rand::random::<u8>() as u32)
                                | (rand::random::<u8>() as u32) << 8
                                | (rand::random::<u8>() as u32) << 16,
                            editing_lobby: false,
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

            let _ = player
                .stream
                .lock()
                .await
                .send(PlayClientBound::ChatMessage {
                    message: chat_parse(format!(
                        "§#{:06X}{} §7 left.",
                        self.players[&disconnected_id].nickname_color,
                        self.players[&disconnected_id].username
                    )),
                    position: ChatPosition::System,
                    sender: 0,
                });
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
            gamemode: Gamemode::Adventure,
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
            "§alogged in!",
        ))))?;

        stream.send(PlayClientBound::Title(TitleAction::SetSubtitle(
            chat_parse("§bhave fun!"),
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
            footer: chat_parse(" "),
        })?;

        // declare commands
        let mut commands = command!();

        let permissions =
            GLOBAL_STATE.player_data.read().await[&self.players[&id].username].permissions;

        if permissions.edit_lobby {
            commands.extend(command!(
            (X "editmode", literal => []),
            ("setblock", literal => [
                (X "block id", argument (Integer: Some(0), None) => []),
            ]),
            (X "printchunk", literal => []),
            (X "clearchunk", literal => []),
            ));
        }
        // other non-world specific commands
        permissions.extend_permission_commands(&mut commands);

        stream.send(commands.build())?;

        drop(stream);

        // inform all players of the new player
        for (_, player) in &self.players {
            let _ = player
                .stream
                .lock()
                .await
                .send(PlayClientBound::ChatMessage {
                    message: chat_parse(format!(
                        "§#{:06X}{} §7joined.",
                        self.players[&id].nickname_color, self.players[&id].username
                    )),
                    position: ChatPosition::System,
                    sender: 0,
                });
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

        let global_state_lock = GLOBAL_STATE.players.read().await;

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
                .send(PlayClientBound::EntityMetadata {
                    entity_id: VarInt(id as i32),
                    metadata: EntityMetadata(vec![(
                        16,
                        EntityMetadataEntry::Byte(
                            global_state_lock
                                .get(id)
                                .context("player already disconnected")?
                                .settings
                                .as_ref()
                                .map(|s| s.displayed_skin_parts)
                                .unwrap_or(SkinParts::all())
                                .bits(),
                        ),
                    )]),
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
                .send(PlayClientBound::EntityMetadata {
                    entity_id: VarInt(*old_id as i32),
                    metadata: EntityMetadata(vec![(
                        16,
                        EntityMetadataEntry::Byte(
                            global_state_lock
                                .get(*old_id)
                                .context("player already disconnected")?
                                .settings
                                .as_ref()
                                .map(|s| s.displayed_skin_parts)
                                .unwrap_or(SkinParts::all())
                                .bits(),
                        ),
                    )]),
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

        // metadata about self
        self.players[&id]
            .stream
            .lock()
            .await
            .send(PlayClientBound::EntityMetadata {
                entity_id: VarInt(id as i32),
                metadata: EntityMetadata(vec![(
                    16,
                    EntityMetadataEntry::Byte(
                        global_state_lock
                            .get(id)
                            .context("player already disconnected")?
                            .settings
                            .as_ref()
                            .map(|s| s.displayed_skin_parts)
                            .unwrap_or(SkinParts::all())
                            .bits(),
                    ),
                )]),
            })?;

        drop(global_state_lock);

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
            .settings
            .as_ref()
            .map(|s| s.view_distance)
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
            self.send_chunk(id, chunk).await;
        }

        Ok(())
    }
    async fn send_chunk(&self, id: usize, chunk: (i8, i8)) {
        let temp_chunk = if (-MAP_SIZE..MAP_SIZE).contains(&chunk.0)
            && (-MAP_SIZE..MAP_SIZE).contains(&chunk.1)
        {
            let chunk_index = get_chunk_index(chunk.0, chunk.1);

            let mut primary_bitmask = 0;
            let mut chunk_sections = Vec::new();

            for (i, section) in self.chunks[chunk_index].sections.iter().enumerate() {
                if let Some(section) = section {
                    primary_bitmask |= 2i32.pow(i as u32);

                    if bits_per_block(section.block_mappings.len()) > 8 {
                        // they don't accept palettes with more than 256 blocks, and we must use a global palette
                        // but i don't want to overcomplicate the whole server side logic so i will just
                        // convert it here when sending, which is inefficient but I don't suspect there
                        // will be many chunks with that many different blocks
                        chunk_sections.push(ChunkSection {
                            block_count: 16 * 16 * 16, // this is foolproof, but might want to send the real block count in the future
                            palette: Palette::Direct,
                            data: {
                                let mut data = vec![0u64; 1024];
                                let local_bits_per_block =
                                    bits_per_block(section.block_mappings.len());
                                let local_blocks_per_u64 = 64 / local_bits_per_block as usize;
                                for i in 0..(16 * 16 * 16) {
                                    let mut t = section.blocks[i / local_blocks_per_u64];
                                    let bits_to_the_right = 64
                                        - local_bits_per_block as i32
                                            * (i as i32 % local_blocks_per_u64 as i32 + 1);
                                    t = t << bits_to_the_right;

                                    let bits_to_the_left = local_bits_per_block as i32
                                        * (i as i32 % local_blocks_per_u64 as i32);
                                    t = t >> (bits_to_the_right + bits_to_the_left);

                                    data[i / 4] |= (section.block_mappings[t as usize] as u64)
                                        << ((i % 4) * 15) as usize;
                                }

                                data
                            },
                        });
                    } else {
                        chunk_sections.push(ChunkSection {
                            block_count: 16 * 16 * 16, // this is foolproof, but might want to send the real block count in the future
                            palette: Palette::Indirect(
                                section.block_mappings.iter().map(|v| VarInt(*v)).collect(),
                            ),
                            data: section.blocks.clone(),
                        });
                    }
                }
            }

            Chunk::Full {
                primary_bitmask: VarInt(primary_bitmask),
                heightmaps: Nbt(quartz_nbt::NbtCompound::new()),
                biomes: ArrWithLen::new(self.chunks[chunk_index].biomes.clone().map(|e| VarInt(e))),
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

        let _ = self.players[&id]
            .stream
            .lock()
            .await
            .send(PlayClientBound::ChunkData {
                chunk_x: chunk.0 as i32,
                chunk_z: chunk.1 as i32,
                chunk: temp_chunk,
            });
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
                    if message.starts_with("/printchunk") {
                        if !self.players[&id].editing_lobby {
                            return;
                        }
                        let player_chunk = (
                            (self.players[&id].position.0.floor() / 16.0).floor() as i8,
                            (self.players[&id].position.1.floor() / 16.0).floor() as i8,
                            (self.players[&id].position.2.floor() / 16.0).floor() as i8,
                        );
                        let chunk = &self.chunks[get_chunk_index(player_chunk.0, player_chunk.2)]
                            .sections[player_chunk.1 as usize];

                        for (_id, player) in &self.players {
                            let _ = player
                                .stream
                                .lock()
                                .await
                                .send(PlayClientBound::ChatMessage {
                                    message: chat_parse(format!(
                                        "§l§8Chunk section [{}, {}, {}]: §r§7{:?}",
                                        player_chunk.0, player_chunk.1, player_chunk.2, chunk
                                    )),
                                    position: ChatPosition::System,
                                    sender: 0,
                                });
                        }
                    } else if message.starts_with("/clearchunk") {
                        if !self.players[&id].editing_lobby {
                            return;
                        }
                        let player_chunk = (
                            (self.players[&id].position.0.floor() / 16.0).floor() as i8,
                            (self.players[&id].position.1.floor() / 16.0).floor() as i8,
                            (self.players[&id].position.2.floor() / 16.0).floor() as i8,
                        );
                        self.chunks[get_chunk_index(player_chunk.0, player_chunk.2)].sections
                            [player_chunk.1 as usize] = None;

                        // resend it to all players who had it loaded
                        for (id, player) in &self.players {
                            if player
                                .loaded_chunks
                                .contains(&(player_chunk.0, player_chunk.2))
                            {
                                self.send_chunk(*id, (player_chunk.0, player_chunk.2)).await;
                            }
                        }
                    } else if message.starts_with("/setblock ") {
                        if !self.players[&id].editing_lobby {
                            return;
                        }
                        if let Ok(block_id) = message[10..].parse::<i32>() {
                            let position = self.players[&id].position;
                            let position = Position {
                                x: position.0.floor() as i32,
                                y: position.1.floor() as i32,
                                z: position.2.floor() as i32,
                            };
                            if let Err(e) = self.set_block(position, block_id).await {
                                debug!("Error executing /setblock: {}", e);
                            }
                        }
                    } else if message.starts_with("/editmode") {
                        let permissions = GLOBAL_STATE.player_data.read().await
                            [&self.players[&id].username]
                            .permissions;

                        if permissions.edit_lobby {
                            // toggle it
                            let old = self.players[&id].editing_lobby;
                            self.players.get_mut(&id).unwrap().editing_lobby = !old;

                            if self.players[&id].editing_lobby {
                                // turn on creative mode
                                let _ = self.players[&id].stream.lock().await.send(
                                    PlayClientBound::ChangeGameState {
                                        reason: GameStateChangeReason::ChangeGamemode,
                                        value: Gamemode::Creative as u8 as f32,
                                    },
                                );
                            } else {
                                // save the lobby
                                if let Err(e) = (Map {
                                    chunks: Cow::Borrowed(&self.chunks),
                                    extra: HashMap::new(),
                                }
                                .save(MAP_PATH)
                                .await)
                                {
                                    error!("Error saving map data: {}", e);
                                }

                                // turn on adventure mode
                                let _ = self.players[&id].stream.lock().await.send(
                                    PlayClientBound::ChangeGameState {
                                        reason: GameStateChangeReason::ChangeGamemode,
                                        value: Gamemode::Adventure as u8 as f32,
                                    },
                                );
                                // show an elder guardian
                                let _ =
                                    self.players[&id]
                                        .stream
                                        .lock()
                                        .await
                                        .send(PlayClientBound::ChangeGameState {
                                        reason:
                                            GameStateChangeReason::PlayElderGuardianMobAppearance,
                                        value: 0f32,
                                    });
                                // and clear inventory?
                                // todo
                            }
                        }
                    }
                } else {
                    let permissions = GLOBAL_STATE.player_data.read().await
                        [&self.players[&id].username]
                        .permissions;
                    for (_, player) in &self.players {
                        let _ = player
                            .stream
                            .lock()
                            .await
                            .send(PlayClientBound::ChatMessage {
                                message: chat_parse(format!(
                                    "{prefix}§#{:06X}§l{}§r§7: §#eeeeee{}",
                                    self.players[&id].nickname_color,
                                    self.players[&id].username,
                                    message,
                                    prefix = {
                                        if permissions.owner {
                                            "§f§l[Owner] §r"
                                        } else if permissions.admin {
                                            "§c§l[§4Admin§c] §r"
                                        } else {
                                            ""
                                        }
                                    }
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
                if !self.players[&id].editing_lobby {
                    return;
                }

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
                cursor_position_y,
                cursor_position_z: _,
                inside_block: _,
            } => {
                if !self.players[&id].editing_lobby {
                    return;
                }

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
                let slot = match hand {
                    Hand::Left => 36 + self.players[&id].held_item as usize,
                    Hand::Right => 45usize,
                };
                match self.players[&id].inventory[slot].0.as_ref() {
                    Some(item) => {
                        let item_id = item.item_id.0;

                        if let Some(block) = crate::data::ITEMS_TO_BLOCKS[item_id as usize].as_ref()
                        {
                            // target must be air or a liquid
                            match self.get_block(target) {
                                Ok(old_block) => {
                                    if old_block != 0 && !parse_fluid_state(old_block).is_some() {
                                        return;
                                    }
                                }
                                Err(_) => {
                                    return;
                                }
                            }

                            let state = get_placed_state(
                                block,
                                &face,
                                &cursor_position_y,
                                self.players[&id].new_rotation.0,
                            );

                            if parse_fluid_state(state).is_none() {
                                for (_, player) in &self.players {
                                    let player_pos = player.position;
                                    let player_height = 1.8;
                                    let player_width = 0.6;
                                    if is_colliding(
                                        player_pos.0 - player_width / 2.0,
                                        player_pos.1,
                                        player_pos.2 - player_width / 2.0,
                                        player_width,
                                        player_height,
                                        player_width,
                                        target.x as f64,
                                        target.y as f64,
                                        target.z as f64,
                                        1.0,
                                        1.0,
                                        1.0,
                                    ) {
                                        // a player is standing in the way :/
                                        // dont place it
                                        //
                                        // and since the server is more strict (because we assume all blocks are 1x1x1, even slabs)
                                        // send the server-side block to the client in case they placed something locally
                                        let _ = self.players[&id].stream.lock().await.send(
                                            PlayClientBound::BlockChange {
                                                location: target,
                                                new_block_id: VarInt(
                                                    self.get_block(target).unwrap(),
                                                ),
                                            },
                                        );
                                        return;
                                    }
                                }
                            }

                            if let Err(e) = self.set_block(target, state).await {
                                debug!("Error placing block: {:?}", e);
                            }

                            // if there are any neighbor liquids, may need to update them
                            self.update_nearby_liquids(target);
                        } else {
                            // might be a special case like buckets with fluids

                            if item_id == 661 || item_id == 662 {
                                // water and lava buckets, respectfully
                                if let Ok(old_block) = self.get_block(target) {
                                    if old_block == 0 || parse_fluid_state(old_block).is_some() {
                                        // just place the water/lava
                                        if let Err(e) = self
                                            .set_block(target, if item_id == 661 { 34 } else { 50 })
                                            .await
                                        {
                                            debug!("Error placing block: {:?}", e);
                                        }
                                        // and add it to the flowing fluid list so it spreads
                                        self.flowing_liquids.push(target);

                                        self.update_nearby_liquids(target);
                                    } else {
                                        // undo client local changes
                                        let _ = self.players[&id].stream.lock().await.send(
                                            PlayClientBound::BlockChange {
                                                location: target,
                                                new_block_id: VarInt(old_block),
                                            },
                                        );
                                    }
                                }
                            } else if item_id == 660 {
                                // empty bucket
                                if let Ok(old_block) = self.get_block(target) {
                                    if let Some((_is_lava, 0, false)) = parse_fluid_state(old_block)
                                    {
                                        // remove the water
                                        if let Err(e) = self.set_block(target, 0).await {
                                            debug!("Error placing block: {:?}", e);
                                        }
                                        self.update_nearby_liquids(target);
                                        // and fill the bucket TODO
                                        //
                                    } else {
                                        // undo whatever the client may have done locally
                                        let _ = self.players[&id].stream.lock().await.send(
                                            PlayClientBound::BlockChange {
                                                location: target,
                                                new_block_id: VarInt(old_block),
                                            },
                                        );
                                    }
                                }
                            } else {
                                // just remove whatever the client might have placed locally
                                if let Ok(old_block) = self.get_block(target) {
                                    let _ = self.players[&id].stream.lock().await.send(
                                        PlayClientBound::BlockChange {
                                            location: target,
                                            new_block_id: VarInt(old_block),
                                        },
                                    );
                                }
                            }
                        }
                    }
                    None => {}
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
                    if !self.players[&id].editing_lobby {
                        return;
                    }

                    if let Err(e) = self.set_block(location, 0).await {
                        debug!("Error breaking block: {:?}", e);
                    }
                    // if there are any neighbor liquids, may need to update them
                    self.update_nearby_liquids(location);
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
    fn update_nearby_liquids(&mut self, position: Position) {
        for neighbor in &[
            (-1, 0, 0),
            (1, 0, 0),
            (0, 0, -1),
            (0, 0, 1),
            (0, 1, 0),
            (0, -1, 0),
        ] {
            let mut neighbor_block = position;
            neighbor_block.x += neighbor.0;
            neighbor_block.y += neighbor.1;
            neighbor_block.z += neighbor.2;

            if let Ok(block) = self.get_block(neighbor_block) {
                if parse_fluid_state(block).is_some() {
                    self.flowing_liquids.push(neighbor_block)
                }
            }
        }
    }
    // takes a global position and returns a global block state
    fn get_block(&self, position: Position) -> Result<i32> {
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

        match &self.chunks[get_chunk_index(block_chunk.x as i8, block_chunk.z as i8)].sections
            [block_chunk.y as usize]
        {
            Some(section) => {
                Ok(section.block_mappings[get_section_block(section, iblock) as usize])
            }
            None => Ok(0),
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
        if glob_block < 0 {
            bail!("Block IDs can't be negative");
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

        let opt_section = &mut self.chunks
            [get_chunk_index(block_chunk.x as i8, block_chunk.z as i8)]
        .sections[block_chunk.y as usize];

        if opt_section.is_none() {
            // if the target block is air too, then no need to anything at all
            if glob_block == 0 {
                return Ok(());
            }

            // initialize the section with air
            opt_section.replace(WorldChunkSection {
                block_mappings: vec![0],
                blocks: vec![0u64; 256],
            });
        }

        let section = opt_section.as_mut().unwrap();

        let old_block = get_section_block(section, iblock);

        if section.block_mappings[old_block as usize] == glob_block {
            // Trying to set the block to the same
            return Ok(());
        }

        // if there are no more blocks of type that was in this position previously
        // we will want to remove it from the palette
        let old_block_to_be_removed_from_palette = {
            let mut has_more = false;

            'outermost: for z in 0..16 {
                for y in 0..16 {
                    for x in 0..16 {
                        let pos = Position { x, y, z };
                        if pos == iblock {
                            continue;
                        }

                        if get_section_block(section, pos) == old_block {
                            has_more = true;
                            break 'outermost;
                        }
                    }
                }
            }

            !has_more
        };

        if old_block_to_be_removed_from_palette {
            // if palette has the new block
            match section.block_mappings.iter().position(|v| *v == glob_block) {
                Some(_new_block_palette_position) => {
                    // Remove the old block from the palette and then remap it,
                    // because all of the blocks that go after that block in the palette
                    // will be shifted
                    // PLUS the bits_per_block might change after removing a block too

                    // these are the GLOBAL palette indexes
                    let mut blocks = box [0i32; 16 * 16 * 16];

                    for z in 0..16 {
                        for y in 0..16 {
                            for x in 0..16 {
                                let local_palette_block = get_section_block(
                                    section,
                                    Position {
                                        x: x as i32,
                                        y: y as i32,
                                        z: z as i32,
                                    },
                                );
                                let global_palette_block =
                                    section.block_mappings[local_palette_block as usize];

                                blocks[x | y << 4 | z << 8] = global_palette_block;
                            }
                        }
                    }

                    // remove the old block from the palette
                    section.block_mappings.remove(old_block as usize);
                    // and set the new block
                    blocks[(iblock.x as usize)
                        | (iblock.y as usize) << 4
                        | (iblock.z as usize) << 8] = glob_block;

                    section.blocks.clear();
                    let blocks_per_u64 = 64 / bits_per_block(section.block_mappings.len()) as usize;
                    // needs to be rounded up `(x + y - 1) / y` is
                    // equivalent of x / y except that its rounded up
                    let u64s_needed = ((16 * 16 * 16) + blocks_per_u64 - 1) / blocks_per_u64;
                    section.blocks.resize(u64s_needed, 0u64);

                    for z in 0..16 {
                        for y in 0..16 {
                            for x in 0..16 {
                                let global_palette_block = blocks[x | y << 4 | z << 8];
                                let local_palette_block = section
                                    .block_mappings
                                    .iter()
                                    .position(|i| *i == global_palette_block)
                                    .unwrap();

                                set_section_block(
                                    section,
                                    Position {
                                        x: x as i32,
                                        y: y as i32,
                                        z: z as i32,
                                    },
                                    local_palette_block as i32,
                                );
                            }
                        }
                    }

                    // if the block was set to air, might want to remove the whole chunk section
                    if glob_block == 0 {
                        check_if_section_empty(opt_section);
                    }
                }
                None => {
                    // Since the old block needs to be removed from the palette and the new one
                    // needs to be added we can simple change the global block id in the palette
                    // and all will be done.
                    section.block_mappings[old_block as usize] = glob_block;
                }
            }
        } else {
            // if palette has the new block
            match section.block_mappings.iter().position(|v| *v == glob_block) {
                Some(new_block_palette_position) => {
                    // No need to do anything with the palette
                    set_section_block(section, iblock, new_block_palette_position as i32);
                }
                None => {
                    // We need to add the new block to the palette
                    // If that changes the bits_per_block
                    // We will need to remap the data
                    let old_bits_per_block = bits_per_block(section.block_mappings.len());
                    let new_bits_per_block = bits_per_block(section.block_mappings.len() + 1);

                    if old_bits_per_block != new_bits_per_block {
                        // need to remap the data to fit the new bits_per_block
                        let mut blocks = box [0i32; 16 * 16 * 16];

                        for z in 0..16 {
                            for y in 0..16 {
                                for x in 0..16 {
                                    let block = get_section_block(
                                        section,
                                        Position {
                                            x: x as i32,
                                            y: y as i32,
                                            z: z as i32,
                                        },
                                    );

                                    blocks[x | y << 4 | z << 8] = block;
                                }
                            }
                        }

                        // add the new block to the section
                        blocks[(iblock.x as usize)
                            | (iblock.y as usize) << 4
                            | (iblock.z as usize) << 8] = section.block_mappings.len() as i32;

                        // add the new block to the palette
                        section.block_mappings.push(glob_block);

                        section.blocks.clear();
                        let blocks_per_u64 = 64 / new_bits_per_block as usize;
                        // needs to be rounded up `(x + y - 1) / y` is
                        // equivalent of x / y except that its rounded up
                        let u64s_needed = ((16 * 16 * 16) + blocks_per_u64 - 1) / blocks_per_u64;
                        section.blocks.resize(u64s_needed, 0u64);

                        for z in 0..16 {
                            for y in 0..16 {
                                for x in 0..16 {
                                    let block = blocks[x | y << 4 | z << 8];

                                    set_section_block(
                                        section,
                                        Position {
                                            x: x as i32,
                                            y: y as i32,
                                            z: z as i32,
                                        },
                                        block,
                                    );
                                }
                            }
                        }
                    } else {
                        // yay just simply add it to the palette
                        section.block_mappings.push(glob_block);

                        set_section_block(section, iblock, section.block_mappings.len() as i32 - 1);
                    }
                }
            }
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
    async fn liquid_spread(&mut self, liquid: Position, is_lava: bool, level: i32) {
        // first, check if can flow downwards
        let mut below = liquid.clone();
        below.y -= 1;
        let can_flow_downwards = if let Ok(below_block_id) = self.get_block(below) {
            // can flow if below is air or another liquid of the same type
            if let Some(below_liquid) = parse_fluid_state(below_block_id) {
                if below_liquid.0 == is_lava {
                    if below_liquid.1 == 0 || below_liquid.2 {
                        // no need to flow anywhere, because it would flow down,
                        // but there's already a source or falling block there
                        return;
                    }
                    true
                } else {
                    // only if air
                    below_block_id == 0
                }
            } else {
                // only if air
                below_block_id == 0
            }
        } else {
            return; // blocks above void dont need to do anything, just stop flowing
        };

        if can_flow_downwards {
            if let Err(e) = self.set_block(below, if is_lava { 58 } else { 42 }).await {
                debug!("flowing error: {}", e);
            } else {
                self.flowing_liquids.push(below);
            }
        } else {
            // try flowing to sides, but only if enough current
            if level < 7 {
                for side in &[(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let mut side_block = liquid.clone();
                    side_block.x += side.0;
                    side_block.z += side.1;

                    if let Ok(side_block_id) = self.get_block(side_block) {
                        if side_block_id == 0 {
                            // its just air, free to flow
                            if let Err(e) = self
                                .set_block(side_block, if is_lava { 50 } else { 34 } + level + 1)
                                .await
                            {
                                debug!("flowing error: {}", e);
                            } else {
                                self.flowing_liquids.push(side_block);
                            }
                        } else if let Some((side_is_lava, side_level, side_is_falling)) =
                            parse_fluid_state(side_block_id)
                        {
                            if side_is_lava == is_lava && !side_is_falling {
                                // its another fluid of the same type
                                // if the current there is weaker, we can increase it
                                if side_level >= level + 1 {
                                    if let Err(e) = self
                                        .set_block(
                                            side_block,
                                            if is_lava { 50 } else { 34 } + level + 1,
                                        )
                                        .await
                                    {
                                        debug!("flowing error: {}", e);
                                    } else {
                                        self.flowing_liquids.push(side_block);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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

        if counter % 5 == 0 {
            // every 5 ticks, liquids update
            // remove duplications
            self.flowing_liquids.sort();
            self.flowing_liquids.dedup();

            let liquids = self.flowing_liquids.clone();
            self.flowing_liquids.clear();
            for liquid in liquids {
                if let Ok(fluid_id) = self.get_block(liquid) {
                    let (is_lava, level, is_falling) = match parse_fluid_state(fluid_id) {
                        Some(d) => d,
                        None => {
                            // this happens a lot, if a block is placed where water was before it got updated
                            continue;
                        }
                    };

                    self.liquid_spread(liquid, is_lava, level).await;

                    // check if this block is a source block or supported by any stronger block nearby
                    let supported = if is_falling {
                        // if falling, check if above block is a liquid
                        let mut above_block = liquid.clone();
                        above_block.y += 1;

                        if let Ok(above_block_id) = self.get_block(above_block) {
                            if let Some((above_is_lava, _, _)) = parse_fluid_state(above_block_id) {
                                if above_is_lava == is_lava {
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        if level == 0 {
                            true
                        } else {
                            let mut supported = false;
                            for neighbor in &[(-1, 0), (1, 0), (0, -1), (0, 1)] {
                                let mut neighbor_block = liquid.clone();
                                neighbor_block.x += neighbor.0;
                                neighbor_block.z += neighbor.1;

                                if let Ok(neighbor_block_id) = self.get_block(neighbor_block) {
                                    if let Some((neighbor_is_lava, neighbor_level, _)) =
                                        parse_fluid_state(neighbor_block_id)
                                    {
                                        if neighbor_is_lava == is_lava && neighbor_level < level {
                                            supported = true;
                                            break;
                                        }
                                    }
                                }
                            }
                            supported
                        }
                    };

                    // if not supported, remove 2 levels or if no levels left just remove the liquid
                    if !supported {
                        if is_falling || level >= 6 {
                            // remove the block
                            if let Err(e) = self.set_block(liquid, 0).await {
                                debug!("flowing error: {}", e);
                            }
                        } else {
                            // remove 2 levels
                            if let Err(e) = self
                                .set_block(liquid, if is_lava { 50 } else { 34 } + level + 2)
                                .await
                            {
                                debug!("flowing error: {}", e);
                            } else {
                                self.flowing_liquids.push(liquid);
                            }
                        }

                        // if there are any neighbor liquids, may need to update them
                        self.update_nearby_liquids(liquid);
                    }
                }
            }
        }

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

// returns whether the fluid is lava, the level of the fluid and whether its falling
fn parse_fluid_state(fluid_id: i32) -> Option<(bool, i32, bool)> {
    let is_lava = if (34..=49).contains(&fluid_id) {
        false
    } else if (50..=65).contains(&fluid_id) {
        true
    } else {
        return None;
    };

    let level = fluid_id - if is_lava { 50 } else { 34 };

    let is_falling = level > 7;

    Some((is_lava, if is_falling { 0 } else { level }, is_falling))
}

// returns a global palette block state based on the circumstances
fn get_placed_state(
    block: &Block,
    face: &Direction,
    cursor_position_y: &f32,
    player_yaw: f32,
) -> i32 {
    match block.class.as_str() {
        "Block" | "BambooBlock" => {
            // these either have no properties or the properties are irrelevant when placing
            block.default_state
        }
        "AnvilBlock" => {
            // these only have the "facing" property with 4 values
            // and the direction is to the right of the direction the player is facing
            let facing = match player_yaw.rem_euclid(360.0) {
                x if x.in_range(0.0, 45.0) | x.in_range(315.0, 360.0) => "west",
                x if x.in_range(45.0, 135.0) => "north",
                x if x.in_range(135.0, 225.0) => "east",
                x if x.in_range(225.0, 315.0) => "south",
                _ => "north", // default
            };

            block
                .states
                .iter()
                .find(|e| e.properties.search("facing").as_str() == facing)
                .unwrap()
                .state_id
        }
        "StairsBlock" => {
            let half = match face {
                Direction::Down => "top",
                Direction::Up => "bottom",
                _ => {
                    if *cursor_position_y > 0.5 {
                        "top"
                    } else {
                        "bottom"
                    }
                }
            };

            let shape = "straight"; // todo

            let facing = match player_yaw.rem_euclid(360.0) {
                x if x.in_range(0.0, 45.0) | x.in_range(315.0, 360.0) => "south",
                x if x.in_range(45.0, 135.0) => "west",
                x if x.in_range(135.0, 225.0) => "north",
                x if x.in_range(225.0, 315.0) => "east",
                _ => "north", // default
            };

            block
                .states
                .iter()
                .find(|e| {
                    e.properties.search("facing").as_str() == facing
                        && e.properties.search("shape").as_str() == shape
                        && e.properties.search("half").as_str() == half
                        && e.properties.search("waterlogged").as_str() == "false"
                })
                .unwrap()
                .state_id
        }
        "SlabBlock" => {
            // doesnt support double slabs, just use the normal block
            let slab_type = match face {
                Direction::Down => "top",
                Direction::Up => "bottom",
                _ => {
                    if *cursor_position_y > 0.5 {
                        "top"
                    } else {
                        "bottom"
                    }
                }
            };

            block
                .states
                .iter()
                .find(|e| {
                    e.properties.search("type").as_str() == slab_type
                        && e.properties.search("waterlogged").as_str() == "false"
                })
                .unwrap()
                .state_id
        }
        "PillarBlock" => {
            let axis = match face {
                Direction::Down | Direction::Up => "y",
                Direction::West | Direction::East => "x",
                Direction::South | Direction::North => "z",
            };

            block
                .states
                .iter()
                .find(|e| e.properties.search("axis").as_str() == axis)
                .unwrap()
                .state_id
        }
        other => {
            debug!(
                "Class \"{}\" not handled in get_placed_state(). Returning default state.",
                other
            );
            block.default_state
        }
    }
}

fn get_chunk_index(x: i8, z: i8) -> usize {
    (x + MAP_SIZE) as usize + 2 * MAP_SIZE as usize * (z + MAP_SIZE) as usize
}

fn bits_per_block(palette_length: usize) -> u8 {
    max(
        4,
        32u8 - max(palette_length as u32 - 1, 1).leading_zeros() as u8,
    )
}

// position is not global, but relative to the chunk. negative or bigger than 15 values are illegal
// and returns the local palette block ID, not global
fn get_section_block(section: &WorldChunkSection, position: Position) -> i32 {
    let bits_per_block = bits_per_block(section.block_mappings.len());
    let blocks_per_u64 = 64 / bits_per_block as usize;

    let block_index = position.x as usize | (position.z as usize) << 4 | (position.y as usize) << 8;

    let mut t = section.blocks[block_index / blocks_per_u64] as u64;

    let bits_to_the_right =
        64 - bits_per_block as i32 * (block_index as i32 % blocks_per_u64 as i32 + 1);
    t = t << bits_to_the_right;

    let bits_to_the_left = bits_per_block as i32 * (block_index as i32 % blocks_per_u64 as i32);
    t = t >> (bits_to_the_right + bits_to_the_left);

    t as i32
}

// position is not global, but relative to the chunk. negative or bigger than 15 values are illegal
// and the block value is of the local pallete, not global
// this function assumes that the block value is present in the palette and the data is mapped according to the size of the palette
fn set_section_block(section: &mut WorldChunkSection, position: Position, block: i32) {
    let old_block = get_section_block(section, position);

    let bits_per_block = bits_per_block(section.block_mappings.len());
    let blocks_per_u64 = 64 / bits_per_block as usize;

    let block_index = position.x as usize | (position.z as usize) << 4 | (position.y as usize) << 8;

    // logic magic
    section.blocks[block_index / blocks_per_u64] ^= ((block ^ old_block) as u64)
        << (bits_per_block as i32 * (block_index as i32 % blocks_per_u64 as i32));
}

// check if the section is empty, if so, removes it
fn check_if_section_empty(section: &mut Option<WorldChunkSection>) {
    let inner_section = match section {
        Some(s) => s,
        None => {
            // its already empty
            return;
        }
    };

    // if there are no more non-air blocks we can just unload the chunk to save memory
    let air_in_palette = inner_section.block_mappings.iter().position(|v| *v == 0);
    match air_in_palette {
        Some(air) => {
            let mut empty = true;

            'outermost: for z in 0..16 {
                for y in 0..16 {
                    for x in 0..16 {
                        let pos = Position { x, y, z };
                        if get_section_block(inner_section, pos) != air as i32 {
                            empty = false;
                            break 'outermost;
                        }
                    }
                }
            }

            if empty {
                *section = None;
            }
        }
        None => {
            // what?
            // no air in the palette? these folks are dedicated!
        }
    }
}

pub fn start() -> WSender {
    let (w_sender, w_receiver) = unbounded_channel::<WBound>();

    spawn(async move { LobbyWorld::new().await.run(w_receiver).await });

    w_sender
}
