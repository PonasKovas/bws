use crate::chat_parse;
use crate::global_state::PStream;
use crate::internal_communication::WBound;
use crate::internal_communication::WReceiver;
use crate::internal_communication::WSender;
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

// half length of a side of the map, in chunks
const MAP_SIZE: i8 = 8; // 8 means 16x16 chunks

struct Player {
    username: String,
    stream: PStream,
    position: (f64, f64, f64),
    rotation: (f32, f32),
    uuid: u128,
    loaded_chunks: Vec<(i8, i8)>, // i8s work because the worlds aren't going to be that big
}

#[derive(Debug, Clone)]
struct WorldChunk {
    biomes: Box<[i32; 1024]>, // damn you stack overflows!
    sections: [Option<WorldChunkSection>; 16],
}

#[derive(Debug, Clone)]

struct WorldChunkSection {
    block_mappings: Vec<VarInt>,
    blocks: Vec<i64>,
}

pub struct LobbyWorld {
    players: HashMap<usize, Player>,
    chunks: [WorldChunk; 4 * MAP_SIZE as usize * MAP_SIZE as usize], // 16x16 chunks, resulting in 256x256 world
}

impl LobbyWorld {
    pub async fn new() -> Self {
        LobbyWorld {
            players: HashMap::new(),
            chunks: [(); 4 * MAP_SIZE as usize * MAP_SIZE as usize].map(|_| WorldChunk {
                biomes: Box::new([174; 1024]),
                sections: [(); 16].map(|_| {
                    Some(WorldChunkSection {
                        block_mappings: vec![VarInt(0), VarInt(1)],
                        blocks: {
                            let mut data = vec![1];
                            data.extend(vec![0; 255]);
                            data
                        },
                    })
                }),
            }),
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
                            position: (0.0, 30.0, 0.0),
                            rotation: (0.0, 0.0),
                            uuid,
                            loaded_chunks: Vec::new(),
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
            header: chat_parse("§f§lBWS §rlobby"),
            footer: chat_parse(""),
        })?;

        drop(stream);

        // inform all players of the new player
        for (_, player) in &self.players {
            player
                .stream
                .lock()
                .await
                .send(PlayClientBound::PlayerInfo(PlayerInfo::AddPlayer(vec![(
                    self.players[&id].uuid,
                    PlayerInfoAddPlayer {
                        name: self.players[&id].username.clone().into(),
                        properties: GLOBAL_STATE.players.read().await[id].properties.clone(),
                        gamemode: Gamemode::Creative,
                        ping: VarInt(0),
                        display_name: None,
                    },
                )])))?;
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
                    ping: VarInt(0),
                    display_name: None,
                },
            ));
        }

        self.players[&id]
            .stream
            .lock()
            .await
            .send(PlayClientBound::PlayerInfo(PlayerInfo::AddPlayer(entries)))?;

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
        let cvd = min(16, client_view_distance);

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
                            block_count: 1,
                            palette: Palette::Indirect(section.block_mappings.clone()),
                            data: section.blocks.clone(),
                        });
                    }
                }

                Chunk::Full {
                    primary_bitmask: VarInt(primary_bitmask),
                    heightmaps: Nbt(quartz_nbt::NbtCompound::new()),
                    biomes: ArrWithLen(self.chunks[chunk_index].biomes.clone().map(|e| VarInt(e))),
                    sections: ChunkSections(chunk_sections),
                    block_entities: Vec::new(),
                }
            } else {
                // just an empty chunk
                Chunk::Full {
                    primary_bitmask: VarInt(0b0),
                    heightmaps: Nbt(quartz_nbt::NbtCompound::new()),
                    biomes: ArrWithLen([VarInt(174); 1024]),
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
            PlayServerBound::PlayerBlockPlacement {
                hand: _,
                location: _,
                face: _,
                cursor_position_x: _,
                cursor_position_y: _,
                cursor_position_z: _,
                inside_block: _,
            } => {
                // todo
                // oops turns out first we need to handle the inventory
            }
            PlayServerBound::PlayerDigging {
                status,
                location,
                face,
            } => match status {
                // this means block broken but only when in creative mode
                PlayerDiggingStatus::StartedDigging => {
                    // let mut target = location.clone();
                    // match face {
                    //     Face::Bottom => {
                    //         target.y -= 1;
                    //     }
                    //     Face::Top => {
                    //         target.y += 1;
                    //     }
                    //     Face::North => {
                    //         target.z -= 1;
                    //     }
                    //     Face::South => {
                    //         target.z += 1;
                    //     }
                    //     Face::West => {
                    //         target.x -= 1;
                    //     }
                    //     Face::East => {
                    //         target.x += 1;
                    //     }
                    // }
                    if !(0..255).contains(&location.y) {
                        debug!("StartedDigging packet received with the coordinates out of range! Disconnecting.");
                        self.players[&id].stream.lock().await.disconnect();
                    }
                    if let Err(e) = self.break_block(location).await {
                        debug!("Error breaking block: {:?}", e);
                    }
                }
                _ => {
                    debug!(
                        "received {:?}",
                        PlayServerBound::PlayerDigging {
                            status,
                            location,
                            face,
                        }
                    );
                }
            },
            other => {
                debug!("received {:?}", other);
            }
        }
    }
    async fn break_block(&mut self, block: Position) -> Result<()> {
        let block_chunk = (block.x / 16, block.y / 16, block.z / 16);
        // block position relative to the chunk
        let iblock = Position {
            x: ((block.x % 16) + 16) % 16,
            y: ((block.y % 16) + 16) % 16,
            z: ((block.z % 16) + 16) % 16,
        };

        let section = &mut self.chunks[get_chunk_index(block_chunk.0 as i8, block_chunk.2 as i8)]
            .sections[block_chunk.1 as usize];

        // This belongs in the block placement lmao my head isnt working
        //
        // if section.is_none() {
        //     // initialize the section with air
        //     section.replace(WorldChunkSection {
        //         block_mappings: vec![VarInt(0)],
        //         blocks: vec![0i64; 256],
        //     });
        // }
        let to_remove = match section {
            None => {
                // this gentleman is breaking blocks in an empty chunk smh
                return Ok(());
            }
            Some(section) => {
                let bits_per_block = max(
                    4,
                    32u8 - max(section.block_mappings.len() as u32 - 1, 1).leading_zeros() as u8,
                );
                let blocks_per_i64 = 64 / bits_per_block as usize;

                let block_index =
                    iblock.x as usize + 16 * (iblock.z as usize) + 16 * 16 * (iblock.y as usize);

                // zero out the specific bits that correspond to the given block
                // because zero is always air
                section.blocks[block_index / blocks_per_i64] &= !((!0 as usize
                    >> (64 - bits_per_block))
                    << (bits_per_block as i32 * (block_index as i32 % blocks_per_i64 as i32)))
                    as i64;

                // check if there are any non-air blocks left in the section
                let mut all_empty = true;
                for i in &section.blocks {
                    if *i != 0 {
                        all_empty = false;
                        break;
                    }
                }

                all_empty
            }
        };

        // time to remove the section to save memory and bandwidth when sending it to clients
        if to_remove {
            drop(section.take());
        }

        self.inform_players_of_block_change(block, VarInt(0))
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
        let old_position = &mut self.players.get_mut(&id).unwrap().position;

        // check if chunk passed
        let old_chunks = (
            (old_position.0.floor() / 16.0).floor(),
            (old_position.2.floor() / 16.0).floor(),
        );
        let old_y = old_position.1.floor() as i32;
        let new_chunks = (
            (new_position.0.floor() / 16.0).floor(),
            (new_position.2.floor() / 16.0).floor(),
        );
        let chunk_passed = !((old_chunks.0 == new_chunks.0) && (old_chunks.1 == new_chunks.1));
        *old_position = new_position;

        if chunk_passed || old_y != new_position.1.floor() as i32 {
            self.players
                .get_mut(&id)
                .unwrap()
                .stream
                .lock()
                .await
                .send(PlayClientBound::UpdateViewPosition {
                    chunk_x: VarInt(new_chunks.0 as i32),
                    chunk_z: VarInt(new_chunks.1 as i32),
                })?;
        }

        if chunk_passed {
            // send new chunks
            self.send_chunks(id).await?;
        }
        Ok(())
    }
    async fn set_player_rotation(&mut self, id: usize, rotation: (f32, f32)) {
        self.players.get_mut(&id).unwrap().rotation = rotation;
    }
    async fn set_player_on_ground(&mut self, _id: usize, _on_ground: bool) {}
    async fn tick(&mut self, _counter: u128) {}
}

fn get_chunk_index(x: i8, z: i8) -> usize {
    (x + MAP_SIZE) as usize + 2 * MAP_SIZE as usize * (z + MAP_SIZE) as usize
}

pub fn start() -> WSender {
    let (w_sender, w_receiver) = unbounded_channel::<WBound>();

    spawn(async move { LobbyWorld::new().await.run(w_receiver).await });

    w_sender
}
