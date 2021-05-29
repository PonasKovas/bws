use crate::chat_parse;
use crate::datatypes::*;
use crate::internal_communication::{SHBound, SHSender};
use crate::packets::{ClientBound, TitleAction};
use crate::world::World;
use sha2::{Digest, Sha256};
use slab::Slab;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;

const ACCOUNTS_FILE: &str = "accounts.bwsdata";

pub struct LoginWorld {
    players: Slab<(String, SHSender, Option<String>)>, // username and SHSender, and the password hash, if registered
    accounts: HashMap<String, String>,
}

impl World for LoginWorld {
    fn get_world_name(&self) -> &str {
        "authentication"
    }
    fn add_player(&mut self, username: String, sh_sender: SHSender) -> usize {
        let mut dimension = nbt::Blob::new();
        dimension
            .insert("piglin_safe".to_string(), nbt::Value::Byte(0))
            .unwrap();
        dimension
            .insert("natural".to_string(), nbt::Value::Byte(1))
            .unwrap();
        dimension
            .insert("ambient_light".to_string(), nbt::Value::Float(1.0))
            .unwrap();
        dimension
            .insert("fixed_time".to_string(), nbt::Value::Long(18000))
            .unwrap();
        dimension
            .insert("infiniburn".to_string(), nbt::Value::String("".to_string()))
            .unwrap();
        dimension
            .insert("respawn_anchor_works".to_string(), nbt::Value::Byte(0))
            .unwrap();
        dimension
            .insert("has_skylight".to_string(), nbt::Value::Byte(1))
            .unwrap();
        dimension
            .insert("bed_works".to_string(), nbt::Value::Byte(0))
            .unwrap();
        dimension
            .insert(
                "effects".to_string(),
                nbt::Value::String("minecraft:overworld".to_string()),
            )
            .unwrap();
        dimension
            .insert("has_raids".to_string(), nbt::Value::Byte(0))
            .unwrap();
        dimension
            .insert("logical_height".to_string(), nbt::Value::Int(256))
            .unwrap();
        dimension
            .insert("coordinate_scale".to_string(), nbt::Value::Float(1.0))
            .unwrap();
        dimension
            .insert("ultrawarm".to_string(), nbt::Value::Byte(0))
            .unwrap();
        dimension
            .insert("has_ceiling".to_string(), nbt::Value::Byte(0))
            .unwrap();

        let packet = ClientBound::JoinGame(
            0,
            false,
            3,
            -1,
            vec![self.get_world_name().to_string()],
            dimension,
            self.get_world_name().to_string(),
            0,
            VarInt(20),
            VarInt(8),
            false,
            false,
            false,
            true,
        );
        sh_sender.send(SHBound::Packet(packet)).unwrap();

        sh_sender
            .send(SHBound::Packet(ClientBound::PlayerPositionAndLook(
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0,
                VarInt(0),
            )))
            .unwrap();

        let password = self.accounts.get(&username);

        // declare commands
        sh_sender
            .send(SHBound::Packet(ClientBound::DeclareCommands(
                if password.is_some() {
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
                VarInt(0),
            )))
            .unwrap();

        sh_sender
            .send(SHBound::Packet(ClientBound::Title(TitleAction::SetTitle(
                chat_parse("§bWelcome to §d§lBWS§r§b!".to_string()),
            ))))
            .unwrap();

        sh_sender
            .send(SHBound::Packet(ClientBound::Title(
                TitleAction::SetDisplayTime(15, 60, 15),
            )))
            .unwrap();

        // return the id of player
        self.players
            .insert((username, sh_sender, password.cloned()))
    }
    fn remove_player(&mut self, id: usize) {
        self.players.remove(id);
    }
    fn sh_send(&self, id: usize, message: SHBound) {
        let _ = self.players.get(id).unwrap().1.send(message);
    }
    fn tick(&mut self, counter: u32) {
        // this here looks inefficient, but we'll see if it actually causes any performance issues later.
        if counter % 20 == 0 {
            let login = chat_parse("§aType §6/login §3<password> §ato continue".to_string());
            let register = chat_parse(
                "§aType §6/register §3<password> <password again> §ato continue".to_string(),
            );

            for (id, player) in &self.players {
                let subtitle = if self.accounts.contains_key(&player.0) {
                    &login
                } else {
                    &register
                };
                self.sh_send(
                    id,
                    SHBound::Packet(ClientBound::Title(TitleAction::SetActionBar(
                        subtitle.clone(),
                    ))),
                );
            }
        }
    }
    fn chat(&mut self, id: usize, message: String) {
        match &self.players.get(id).unwrap().2 {
            Some(password_hash) => {
                if message.starts_with("/login ") {
                    let mut iterator = message.split(' ');
                    if let Some(password) = iterator.nth(1) {
                        let hash = format!("{:x}", Sha256::digest(password.as_bytes()));
                        if *password_hash == hash {
                            self.tell(id, "§a§lSuccess!".to_string());
                        } else {
                            self.tell(id, "§4§lIncorrect password!".to_string());
                        }
                        return;
                    }
                }
            }
            None => {
                if message.starts_with("/register ") {
                    let mut iterator = message.split(' ');
                    if let Some(first_password) = iterator.nth(1) {
                        if let Some(second_password) = iterator.next() {
                            if first_password != second_password {
                                self.tell(
                                    id,
                                    "§cThe passwords do not match, try again.".to_string(),
                                );
                                return;
                            }

                            self.tell(id, "§a§lSuccess!".to_string());

                            // register the gentleman
                            self.accounts.insert(
                                self.username(id).to_string(),
                                format!("{:x}", Sha256::digest(first_password.as_bytes())),
                            );
                            self.save_accounts();

                            return;
                        }
                    }
                }
            }
        }

        if message.starts_with("/") {
            self.tell(id, "§cInvalid command.".to_string());
            return;
        }
        println!("{}: {}", self.username(id), message);
    }
    fn username(&self, id: usize) -> &str {
        &self.players.get(id).unwrap().0
    }
}

pub fn new() -> LoginWorld {
    // read the accounts data (this should probably be done in main() and then passed as an argument, but whatever)
    let mut accounts = HashMap::new();
    if Path::new(ACCOUNTS_FILE).exists() {
        // read the data
        match File::open(ACCOUNTS_FILE) {
            Err(e) => {
                println!("Failed to open {}. {:?}", ACCOUNTS_FILE, e);
                std::process::exit(1);
            }
            Ok(f) => {
                let file = BufReader::new(f);
                for line in file.lines() {
                    match line {
                        Ok(l) => {
                            let mut iterator = l.split(' ');
                            let username = iterator
                                .next()
                                .expect(&format!("Incorrect {} format.", ACCOUNTS_FILE));
                            let password_hash = iterator
                                .next()
                                .expect(&format!("Incorrect {} format.", ACCOUNTS_FILE));
                            accounts.insert(username.to_string(), password_hash.to_string());
                        }
                        Err(e) => {
                            println!("Failed to read {}. {:?}", ACCOUNTS_FILE, e);
                            std::process::exit(1);
                        }
                    }
                }
            }
        }
    } else {
        // create the file
        if let Err(e) = File::create(ACCOUNTS_FILE) {
            println!("Failed to create the accounts datafile. {:?}", e);
            std::process::exit(1);
        }
    }
    LoginWorld {
        players: Slab::new(),
        accounts,
    }
}

impl LoginWorld {
    pub fn tell(&self, id: usize, message: String) {
        self.sh_send(
            id,
            SHBound::Packet(ClientBound::ChatMessage(chat_parse(message), 1)),
        );
    }
    pub fn save_accounts(&self) {
        match File::create(ACCOUNTS_FILE) {
            Err(e) => {
                println!(
                    "Failed to create the accounts datafile ({}). {:?}",
                    ACCOUNTS_FILE, e
                );
                std::process::exit(1);
            }
            Ok(mut f) => {
                // yes i know and im sorry
                for account in &self.accounts {
                    if let Err(e) = f.write_all(account.0.as_bytes()).and(
                        f.write_all(b" ")
                            .and(f.write_all(account.1.as_bytes()).and(f.write_all(b"\n"))),
                    ) {
                        println!(
                            "Failed to write to the accounts datafile ({}). {:?}",
                            ACCOUNTS_FILE, e
                        );
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}
