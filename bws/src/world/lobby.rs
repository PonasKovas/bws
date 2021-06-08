use crate::chat_parse;
use crate::datatypes::*;
use crate::internal_communication::{SHBound, SHSender};
use crate::packets::{ClientBound, TitleAction};
use crate::world::World;
use crate::GLOBAL_STATE;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use log::{debug, error, info, warn};
use sha2::{Digest, Sha256};
use slab::Slab;
use std::collections::HashMap;
use std::env::Vars;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;

struct Player {
    username: String,
    sh_sender: SHSender,
    position: (f64, f64, f64),
    rotation: (f32, f32),
}

pub struct LobbyWorld {
    players: HashMap<usize, Player>,
}

impl World for LobbyWorld {
    fn get_world_name(&self) -> &str {
        "lobby"
    }
    fn is_fixed_time(&self) -> Option<i64> {
        None
    }
    fn add_player(&mut self, id: usize) -> Result<()> {
        let lock = futures::executor::block_on(GLOBAL_STATE.players.lock());
        let sh_sender = lock
            .get(id)
            .context("tried to add non-existing player")?
            .sh_sender
            .clone();
        let username = lock
            .get(id)
            .context("tried to add non-existing player")?
            .username
            .clone();
        drop(lock);

        let mut dimension = nbt::Blob::new();

        // rustfmt makes this block reaaally fat and ugly and disgusting oh my god
        #[rustfmt::skip]
        {
            use nbt::Value::{Byte, Float, Int, Long, String as NbtString};

            dimension.insert("piglin_safe".to_string(), Byte(0)).unwrap();
            dimension.insert("natural".to_string(), Byte(1)).unwrap();
            dimension.insert("ambient_light".to_string(), Float(1.0)).unwrap();
            if let Some(time) = self.is_fixed_time() {
                dimension.insert("fixed_time".to_string(), Long(time)).unwrap();
            }
            dimension.insert("infiniburn".to_string(), NbtString("".to_string())).unwrap();
            dimension.insert("respawn_anchor_works".to_string(), Byte(1)).unwrap();
            dimension.insert("has_skylight".to_string(), Byte(1)).unwrap();
            dimension.insert("bed_works".to_string(), Byte(0)).unwrap();
            dimension.insert("effects".to_string(), NbtString("minecraft:overworld".to_string())).unwrap();
            dimension.insert("has_raids".to_string(), Byte(0)).unwrap();
            dimension.insert("logical_height".to_string(), Int(256)).unwrap();
            dimension.insert("coordinate_scale".to_string(), Float(1.0)).unwrap();
            dimension.insert("ultrawarm".to_string(), Byte(0)).unwrap();
            dimension.insert("has_ceiling".to_string(), Byte(0)).unwrap();
        };

        sh_sender.send(SHBound::Packet(ClientBound::Respawn(
            dimension,
            self.get_world_name().to_string(),
            0,
            1,
            3,
            false,
            true,
            false,
        )))?;

        sh_sender.send(SHBound::Packet(ClientBound::PlayerPositionAndLook(
            0.0,
            20.0,
            0.0,
            0.0,
            0.0,
            0,
            VarInt(0),
        )))?;

        sh_sender.send(SHBound::Packet(ClientBound::SetBrand("BWS".to_string())))?;

        sh_sender.send(SHBound::Packet(ClientBound::Title(TitleAction::Reset)))?;

        sh_sender.send(SHBound::Packet(ClientBound::Title(TitleAction::SetTitle(
            chat_parse("§aLogged in§7!"),
        ))))?;

        sh_sender.send(SHBound::Packet(ClientBound::Title(
            TitleAction::SetSubtitle(chat_parse("§bhave fun§7!")),
        )))?;

        sh_sender.send(SHBound::Packet(ClientBound::Title(
            TitleAction::SetActionBar(chat_parse("")),
        )))?;

        sh_sender.send(SHBound::Packet(ClientBound::Title(
            TitleAction::SetDisplayTime(15, 20, 15),
        )))?;

        sh_sender.send(SHBound::Packet(ClientBound::UpdateViewPosition(
            VarInt(0),
            VarInt(0),
        )))?;

        for y in -8..=7 {
            for x in -8..=7 {
                sh_sender.send(SHBound::Packet(ClientBound::ChunkData(
                    x,
                    y,
                    VarInt(0b1),
                    nbt::Blob::new(),
                    vec![VarInt(127); 1024],
                    vec![ChunkSection {
                        block_count: 4096,
                        palette: Palette::Direct,
                        data: {
                            let mut x = vec![0x200040008001; 1023];
                            x.extend_from_slice(&vec![0x2C005800B0016; 1]);
                            x
                        },
                    }],
                    Vec::new(),
                )))?;
            }
        }

        // add the player
        self.players.insert(
            id,
            Player {
                username,
                sh_sender,
                position: (0.0, 20.0, 0.0),
                rotation: (0.0, 0.0),
            },
        );

        Ok(())
    }
    fn remove_player(&mut self, id: usize) {
        self.players.remove(&id);
    }
    fn sh_send(&self, id: usize, message: SHBound) -> Result<()> {
        self.players
            .get(&id)
            .context("No player with given ID in world")?
            .sh_sender
            .send(message)?;
        Ok(())
    }
    fn tick(&mut self, _counter: u64) {
        for (_id, player) in &self.players {
            // info!(
            //     "{} is in {:?} and looking {:?}",
            //     player.username, player.position, player.rotation
            // );
        }
    }
    fn chat(&mut self, id: usize, message: String) -> Result<()> {
        self.tell(id, format!("§a§l{}: §r§f{}", self.username(id)?, message))?;
        Ok(())
    }
    fn username(&self, id: usize) -> Result<&str> {
        Ok(&self
            .players
            .get(&id)
            .context("No player with given ID in this world")?
            .username)
    }
    fn set_player_position(&mut self, id: usize, new_position: (f64, f64, f64)) -> Result<()> {
        let position = &mut self
            .players
            .get_mut(&id)
            .context("No player with given ID in this world")?
            .position;

        // check if chunk passed
        let old_chunks = (
            (position.0.floor() / 16.0).floor(),
            (position.2.floor() / 16.0).floor(),
        );
        let old_y = position.1.floor() as i32;
        let new_chunks = (
            (new_position.0.floor() / 16.0).floor(),
            (new_position.2.floor() / 16.0).floor(),
        );
        let chunk_passed = !((old_chunks.0 == new_chunks.0) && (old_chunks.1 == new_chunks.1));
        *position = new_position;

        if chunk_passed || old_y != new_position.1.floor() as i32 {
            self.sh_send(
                id,
                SHBound::Packet(ClientBound::UpdateViewPosition(
                    VarInt(new_chunks.0 as i32),
                    VarInt(new_chunks.1 as i32),
                )),
            )?;
        }

        if chunk_passed {
            // send new chunks
            // todo yo this is ugly and not really efficient, but I gotta know more about chunks before implementing it properly
            let mut needed_chunks = Vec::with_capacity(16 * 16);
            for z in -8..=7 {
                for x in -8..=7 {
                    needed_chunks.push((new_chunks.0 as i32 + x, new_chunks.1 as i32 + z));
                }
            }
            for z in -8..=7 {
                for x in -8..=7 {
                    for i in (0..needed_chunks.len()).rev() {
                        if needed_chunks[i] == (old_chunks.0 as i32 + x, old_chunks.1 as i32 + z) {
                            needed_chunks.remove(i);
                        }
                    }
                }
            }
            for chunk in needed_chunks {
                self.sh_send(
                    id,
                    SHBound::Packet(ClientBound::ChunkData(
                        chunk.0,
                        chunk.1,
                        VarInt(0b1),
                        nbt::Blob::new(),
                        vec![VarInt(127); 1024],
                        vec![ChunkSection {
                            block_count: 4096,
                            palette: Palette::Direct,
                            data: {
                                let mut x = vec![0x200040008001; 1023];
                                x.extend_from_slice(&vec![0x2C005800B0016; 1]);
                                x
                            },
                        }],
                        Vec::new(),
                    )),
                )?;
            }
        }

        Ok(())
    }
    // is called when the player rotation changes
    fn set_player_rotation(&mut self, id: usize, new_rotation: (f32, f32)) -> Result<()> {
        self.players
            .get_mut(&id)
            .context("No player with given ID in this world")?
            .rotation = new_rotation;
        Ok(())
    }
}

pub fn new() -> Result<LobbyWorld> {
    Ok(LobbyWorld {
        players: HashMap::new(),
    })
}

impl LobbyWorld {
    pub fn tell<T: AsRef<str>>(&self, id: usize, message: T) -> Result<()> {
        self.sh_send(
            id,
            SHBound::Packet(ClientBound::ChatMessage(chat_parse(message), 1)),
        )?;
        Ok(())
    }
}
