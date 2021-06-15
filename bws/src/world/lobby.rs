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

const SERVER_VIEW_DISTANE: i8 = 8;

struct Player {
    username: String,
    stream: PStream,
    position: (f64, f64, f64),
    rotation: (f32, f32),
}

pub struct LobbyWorld {
    players: HashMap<usize, Player>,
}

impl LobbyWorld {
    pub async fn new() -> Self {
        LobbyWorld {
            players: HashMap::new(),
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
                    let (username, stream) = match GLOBAL_STATE.players.read().await.get(id) {
                        Some(p) => (p.username.clone(), p.stream.clone()),
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
    async fn new_player(&self, id: usize) -> Result<()> {
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

        stream.send(PlayClientBound::UpdateViewPosition {
            chunk_x: VarInt(0),
            chunk_z: VarInt(0),
        })?;

        let client_view_distance = GLOBAL_STATE
            .players
            .read()
            .await
            .get(id)
            .context("Player already disconnected")?
            .view_distance
            .unwrap_or(8);

        let c = min(SERVER_VIEW_DISTANE, client_view_distance);

        for z in -c..c {
            for x in -c..c {
                stream.send(PlayClientBound::ChunkData {
                    chunk_x: x as i32,
                    chunk_z: z as i32,
                    chunk: Chunk::Full {
                        primary_bitmask: VarInt(0b1),
                        heightmaps: Nbt(quartz_nbt::NbtCompound::new()),
                        biomes: ArrWithLen([VarInt(174); 1024]),
                        sections: ChunkSections(vec![ChunkSection {
                            block_count: 4096,
                            palette: Palette::Direct,
                            data: {
                                let mut x = vec![0x200040008001; 1023];
                                x.extend_from_slice(&vec![0x2C005800B0016; 1]);
                                x
                            },
                        }]),
                        block_entities: Vec::new(),
                    },
                })?;
            }
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
                        self.players.remove(&id);
                        break 'inner;
                    }
                }
            }
        }
    }
    async fn handle_packet<'a>(&mut self, id: usize, packet: PlayServerBound) {
        match packet {
            PlayServerBound::ChatMessage(message) => {
                let _ = self.players[&id]
                    .stream
                    .lock()
                    .await
                    .send(PlayClientBound::ChatMessage {
                        message: chat_parse(format!(
                            "§a§l{}: §r§f{}",
                            self.players[&id].username, message
                        )),
                        position: ChatPosition::Chat,
                        sender: 0,
                    });
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
            _ => {}
        }
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

            let client_view_distance = GLOBAL_STATE
                .players
                .read()
                .await
                .get(id)
                .context("Player already disconnected")?
                .view_distance
                .unwrap_or(8);

            let c = min(SERVER_VIEW_DISTANE, client_view_distance) as i32;

            // todo yo this is ugly and not really efficient, but I gotta know more about chunks before implementing it properly
            let mut needed_chunks = Vec::with_capacity(16 * 16);
            for z in -c..c {
                for x in -c..c {
                    needed_chunks.push((new_chunks.0 as i32 + x, new_chunks.1 as i32 + z));
                }
            }
            for z in -c..c {
                for x in -c..c {
                    for i in (0..needed_chunks.len()).rev() {
                        if needed_chunks[i] == (old_chunks.0 as i32 + x, old_chunks.1 as i32 + z) {
                            needed_chunks.remove(i);
                        }
                    }
                }
            }
            for chunk in needed_chunks {
                self.players
                    .get_mut(&id)
                    .unwrap()
                    .stream
                    .lock()
                    .await
                    .send(PlayClientBound::ChunkData {
                        chunk_x: chunk.0,
                        chunk_z: chunk.1,
                        chunk: Chunk::Full {
                            primary_bitmask: VarInt(0b1),
                            heightmaps: Nbt(quartz_nbt::NbtCompound::new()),
                            biomes: ArrWithLen([VarInt(174); 1024]),
                            sections: ChunkSections(vec![ChunkSection {
                                block_count: 4096,
                                palette: Palette::Direct,
                                data: {
                                    let mut x = vec![0x200040008001; 1023];
                                    x.extend_from_slice(&vec![0x2C005800B0016; 1]);
                                    x
                                },
                            }]),
                            block_entities: Vec::new(),
                        },
                    })?;
            }
        }
        Ok(())
    }
    async fn set_player_rotation(&mut self, id: usize, rotation: (f32, f32)) {
        self.players.get_mut(&id).unwrap().rotation = rotation;
    }
    async fn set_player_on_ground(&mut self, _id: usize, _on_ground: bool) {}
    async fn tick(&mut self, _counter: u128) {}
}

pub fn start() -> WSender {
    let (w_sender, w_receiver) = unbounded_channel::<WBound>();

    spawn(async move { LobbyWorld::new().await.run(w_receiver).await });

    w_sender
}
