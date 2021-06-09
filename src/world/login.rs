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

const ACCOUNTS_FILE: &str = "accounts.bwsdata";

pub struct LoginWorld {
    players: HashMap<usize, (String, SHSender, Option<String>)>, // username and SHSender, and the password hash, if registered
    accounts: HashMap<String, String>,
    login_message: Chat,
    register_message: Chat,
}

impl World for LoginWorld {
    fn get_world_name(&self) -> &str {
        "authentication"
    }
    fn is_fixed_time(&self) -> Option<i64> {
        Some(18000)
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

        let packet = ClientBound::JoinGame {
            eid: id as i32,
            hardcore: false,
            gamemode: 3,
            previous_gamemode: -1,
            world_names: vec![self.get_world_name().to_string()],
            dimension,
            world_name: self.get_world_name().to_string(),
            hashed_seed: 0,
            max_players: VarInt(20),
            view_distance: VarInt(8),
            reduced_debug_info: false,
            enable_respawn_screen: false,
            debug_mode: false,
            flat: true,
        };
        sh_sender.send(SHBound::Packet(packet))?;

        sh_sender.send(SHBound::Packet(ClientBound::PlayerPositionAndLook {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: -20.0,
            flags: 0,
            tp_id: VarInt(0),
        }))?;

        sh_sender.send(SHBound::Packet(ClientBound::SetBrand("BWS".to_string())))?;

        sh_sender.send(SHBound::Packet(ClientBound::Tags))?;

        let password = self.accounts.get(&username);

        // declare commands
        sh_sender.send(SHBound::Packet(ClientBound::DeclareCommands {
            nodes: if password.is_some() {
                vec![
                    CommandNode::Root(vec![VarInt(1)]),
                    CommandNode::Literal(false, vec![VarInt(2)], None, "login".to_string()),
                    CommandNode::Argument(
                        true,
                        Vec::new(),
                        None,
                        "password".to_string(),
                        Parser::String(VarInt(0)),
                        false,
                    ),
                ]
            } else {
                vec![
                    CommandNode::Root(vec![VarInt(1)]),
                    CommandNode::Literal(false, vec![VarInt(2)], None, "register".to_string()),
                    CommandNode::Argument(
                        false,
                        vec![VarInt(3)],
                        None,
                        "password".to_string(),
                        Parser::String(VarInt(0)),
                        false,
                    ),
                    CommandNode::Argument(
                        true,
                        Vec::new(),
                        None,
                        "password".to_string(),
                        Parser::String(VarInt(0)),
                        false,
                    ),
                ]
            },
            root: VarInt(0),
        }))?;

        sh_sender.send(SHBound::Packet(ClientBound::Title(TitleAction::Reset)))?;

        sh_sender.send(SHBound::Packet(ClientBound::Title(TitleAction::SetTitle(
            chat_parse("§bWelcome to §d§lBWS§r§b!"),
        ))))?;

        sh_sender.send(SHBound::Packet(ClientBound::Title(
            TitleAction::SetDisplayTime {
                fade_in: 15,
                display: 60,
                fade_out: 15,
            },
        )))?;

        sh_sender.send(SHBound::Packet(ClientBound::EntitySoundEffect {
            sound_id: VarInt(482),
            category: VarInt(0),          // MASTER category
            entity_id: VarInt(id as i32), // player
            volume: 1.0,
            pitch: 1.0,
        }))?;

        // add the player
        self.players
            .insert(id, (username, sh_sender, password.cloned()));

        Ok(())
    }
    fn remove_player(&mut self, id: usize) {
        self.players.remove(&id);
    }
    fn sh_send(&self, id: usize, message: SHBound) -> Result<()> {
        self.players
            .get(&id)
            .context("No player with given ID in this world")?
            .1
            .send(message)?;
        Ok(())
    }
    fn tick(&mut self, counter: u64) {
        if counter % 20 == 0 {
            for (id, player) in &self.players {
                let subtitle = if self.accounts.contains_key(&player.0) {
                    &self.login_message
                } else {
                    &self.register_message
                };
                if let Err(e) = self.sh_send(
                    *id,
                    SHBound::Packet(ClientBound::Title(TitleAction::SetActionBar(
                        subtitle.clone(),
                    ))),
                ) {
                    debug!("Couldn't send packet to client: {}", e);
                }
            }
        }
    }
    fn chat(&mut self, id: usize, message: String) -> Result<()> {
        match &self.players.get(&id).context("No player with given ID")?.2 {
            Some(password_hash) => {
                if message.starts_with("/login ") {
                    let mut iterator = message.split(' ');
                    if let Some(password) = iterator.nth(1) {
                        let hash = format!("{:x}", Sha256::digest(password.as_bytes()));
                        if *password_hash == hash {
                            self.sh_send(id, SHBound::ChangeWorld(GLOBAL_STATE.w_lobby.clone()))?;
                        } else {
                            self.tell(id, "§4§lIncorrect password!".to_string())?;
                        }
                        return Ok(());
                    }
                }
            }
            None => {
                if message.starts_with("/register ") {
                    let mut iterator = message.split(' ');
                    if let Some(first_password) = iterator.nth(1) {
                        if let Some(second_password) = iterator.next() {
                            if first_password != second_password {
                                self.tell(id, "§cThe passwords do not match, try again.")?;
                                return Ok(());
                            }

                            // register the gentleman
                            self.accounts.insert(
                                self.username(id)?.to_string(),
                                format!("{:x}", Sha256::digest(first_password.as_bytes())),
                            );
                            self.save_accounts()?;

                            self.sh_send(id, SHBound::ChangeWorld(GLOBAL_STATE.w_lobby.clone()))?;

                            return Ok(());
                        }
                    }
                }
            }
        }

        if message.starts_with("/") {
            self.tell(id, "§cInvalid command.")?;
        }
        Ok(())
    }
    fn username(&self, id: usize) -> Result<&str> {
        Ok(&self
            .players
            .get(&id)
            .context("No player with given ID in this world")?
            .0)
    }
}

pub fn new() -> Result<LoginWorld> {
    // read the accounts data
    let mut accounts = HashMap::new();
    if Path::new(ACCOUNTS_FILE).exists() {
        // read the data
        let f = File::open(ACCOUNTS_FILE).context(format!("Failed to open {}.", ACCOUNTS_FILE))?;

        let file = BufReader::new(f);
        for line in file.lines() {
            let line = line.context(format!("Error reading {}.", ACCOUNTS_FILE))?;
            let mut iterator = line.split(' ');

            let username = iterator
                .next()
                .context(format!("Incorrect {} format.", ACCOUNTS_FILE))?;
            let password_hash = iterator
                .next()
                .context(format!("Incorrect {} format.", ACCOUNTS_FILE))?;

            accounts.insert(username.to_string(), password_hash.to_string());
        }
    } else {
        // create the file
        File::create(ACCOUNTS_FILE)?;
    }

    Ok(LoginWorld {
        players: HashMap::new(),
        accounts,
        login_message: chat_parse("§aType §6/login §3<password> §ato continue"),
        register_message: chat_parse(
            "§aType §6/register §3<password> <password again> §ato continue",
        ),
    })
}

impl LoginWorld {
    pub fn tell<T: AsRef<str>>(&self, id: usize, message: T) -> Result<()> {
        self.sh_send(
            id,
            SHBound::Packet(ClientBound::ChatMessage {
                message: chat_parse(message),
                position: 1,
            }),
        )?;
        Ok(())
    }
    pub fn save_accounts(&self) -> Result<()> {
        let mut f = File::create(ACCOUNTS_FILE)?;

        for account in &self.accounts {
            // I wish to apologize for the readability of the following statement
            #[rustfmt::skip]
            f.write_all(account.0.as_bytes()).and(
                f.write_all(b" ").and(
                    f.write_all(account.1.as_bytes()).and(
                        f.write_all(b"\n")
                    )
                ),
            ).context(format!("Couldn't write to {}", ACCOUNTS_FILE))?;
        }

        Ok(())
    }
}
