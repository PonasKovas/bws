use crate::chat_parse;
use crate::global_state::PStream;
use crate::global_state::PlayerStream;
use crate::internal_communication::WBound;
use crate::internal_communication::WReceiver;
use crate::internal_communication::WSender;
use crate::GLOBAL_STATE;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use futures::future::FutureExt;
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use protocol::datatypes::*;
use protocol::packets::*;
use sha2::{Digest, Sha256};
use slab::Slab;
use std::borrow::Cow;
use std::collections::HashMap;
use std::env::Vars;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::spawn;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::MutexGuard;
use tokio::task::unconstrained;
use tokio::time::sleep;
use tokio::time::Instant;

const ACCOUNTS_FILE: &str = "accounts.bwsdata";

lazy_static! {
    static ref TAGS: [&'static [u8]; 4] = {
        use protocol::{Deserializable, Serializable};

        let mut all_tags =
            std::io::Cursor::new([&[0x5Bu8][..], &incl!("assets/raw/tags.bin")[..]].concat());

        let tags = PlayClientBound::from_reader(&mut all_tags).unwrap();
        if let PlayClientBound::Tags {
            blocks,
            items,
            fluids,
            entities,
        } = tags
        {
            [
                {
                    let mut data = Vec::new();
                    blocks.to_writer(&mut data).unwrap();
                    data.leak()
                },
                {
                    let mut data = Vec::new();
                    items.to_writer(&mut data).unwrap();
                    data.leak()
                },
                {
                    let mut data = Vec::new();
                    fluids.to_writer(&mut data).unwrap();
                    data.leak()
                },
                {
                    let mut data = Vec::new();
                    entities.to_writer(&mut data).unwrap();
                    data.leak()
                },
            ]
        } else {
            panic!("the raw tags packet incorrectly parsed as {:?}", tags);
        }
    };
}

pub struct LoginWorld {
    players: HashMap<usize, (String, PStream)>, // username and stream
    accounts: HashMap<String, String>,          // username -> SHA256 hash of password
    login_message: Chat,
    register_message: Chat,
}

impl LoginWorld {
    // might fail since this interacts with the filesystem for the accounts data
    pub fn new() -> Result<Self> {
        // sadly this has to be sync
        // Edit: but I already forgot why and I'm beginning to think that maybe it doesn't
        use std::fs::File;
        use std::io::BufReader;
        // read the accounts data
        let mut accounts = HashMap::new();
        if Path::new(ACCOUNTS_FILE).exists() {
            // read the data
            let f =
                File::open(ACCOUNTS_FILE).context(format!("Failed to open {}.", ACCOUNTS_FILE))?;

            let lines = BufReader::new(f).lines();
            for line in lines {
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

impl LoginWorld {
    async fn process_client_packets(&mut self) {
        // forgive me father, for the borrow checker does not let me do it any other way
        let keys: Vec<usize> = self.players.keys().copied().collect();

        for id in keys {
            'inner: loop {
                let r = self.players[&id].1.lock().await.try_recv();
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
                // this world only parses commands
                if message.starts_with("/login ") {
                    // make sure the player is already registered
                    if let Some(correct_password_hash) = self.accounts.get(&self.players[&id].0) {
                        // get the password
                        let mut iterator = message.split(' ');
                        if let Some(given_password) = iterator.nth(1) {
                            // hash it
                            let hash = format!("{:x}", Sha256::digest(given_password.as_bytes()));
                            // and compare to the correct hash
                            if *correct_password_hash == hash {
                                // they match, so login successful
                                GLOBAL_STATE
                                    .w_login
                                    .send(WBound::MovePlayer {
                                        id,
                                        new_world: GLOBAL_STATE.w_lobby.clone(),
                                    })
                                    .unwrap();
                            } else {
                                // they don't match
                                let _ = self.players[&id].1.lock().await.send(
                                    PlayClientBound::ChatMessage {
                                        message: chat_parse("§4§lIncorrect password!"),
                                        position: ChatPosition::System,
                                        sender: 0,
                                    },
                                );
                            }
                        }
                    }
                } else if message.starts_with("/register ") {
                    if self.accounts.get(&self.players[&id].0) == None {
                        let mut iterator = message.split(' ');
                        if let Some(first_password) = iterator.nth(1) {
                            if let Some(second_password) = iterator.next() {
                                if first_password != second_password {
                                    let _ = self.players[&id].1.lock().await.send(
                                        PlayClientBound::ChatMessage {
                                            message: chat_parse(
                                                "§cThe passwords do not match, try again.",
                                            ),
                                            position: ChatPosition::System,
                                            sender: 0,
                                        },
                                    );
                                }

                                // register the gentleman
                                self.accounts.insert(
                                    self.players[&id].0.to_string(),
                                    format!("{:x}", Sha256::digest(first_password.as_bytes())),
                                );
                                if let Err(e) = self.save_accounts().await {
                                    error!("Error saving accounts data: {}", e);
                                }

                                GLOBAL_STATE
                                    .w_login
                                    .send(WBound::MovePlayer {
                                        id,
                                        new_world: GLOBAL_STATE.w_lobby.clone(),
                                    })
                                    .unwrap();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
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
                    self.players.insert(id, (username, stream));

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
    // sends all the neccessary packets for new players
    async fn new_player(&self, id: usize) -> Result<()> {
        // lock the stream
        let mut stream = self.players[&id].1.lock().await;

        let mut dimension = nbt::Blob::new();

        // rustfmt makes this block reaaally fat and ugly and disgusting oh my god
        #[rustfmt::skip]
        {
            use nbt::Value::{Byte, Float, Int, Long, String as NbtString};

            dimension.insert("piglin_safe".to_string(), Byte(0)).unwrap();
            dimension.insert("natural".to_string(), Byte(1)).unwrap();
            dimension.insert("ambient_light".to_string(), Float(1.0)).unwrap();
            dimension.insert("fixed_time".to_string(), Long(18000)).unwrap();
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

        let packet = PlayClientBound::JoinGame {
            eid: id as i32,
            hardcore: false,
            gamemode: Gamemode::Spectator,
            previous_gamemode: Gamemode::Spectator,
            world_names: vec![],
            dimension_codec: MaybeStatic::Static(incl!("assets/nbt/dimension_codec.nbt")),
            dimension: Nbt(dimension),
            world_name: "authentication".into(),
            hashed_seed: 0,
            max_players: VarInt(20),
            view_distance: VarInt(8),
            reduced_debug_info: false,
            enable_respawn_screen: false,
            debug_mode: false,
            flat: true,
        };
        stream.send(packet)?;

        stream.send(PlayClientBound::PlayerPositionAndLook {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: -20.0,
            flags: PositionAndLookFlags::empty(),
            id: VarInt(0),
        })?;

        stream.send(PlayClientBound::PluginMessage {
            channel: "minecraft:brand".into(),
            data: "\x03BWS".to_owned().into_bytes().into_boxed_slice(),
        })?;

        stream.send(PlayClientBound::Tags {
            blocks: MaybeStatic::Static(TAGS[0]),
            items: MaybeStatic::Static(TAGS[1]),
            fluids: MaybeStatic::Static(TAGS[2]),
            entities: MaybeStatic::Static(TAGS[3]),
        })?;

        let password = self.accounts.get(
            &GLOBAL_STATE
                .players
                .read()
                .await
                .get(id)
                .context("player already disconnected")?
                .username,
        );

        // declare commands
        stream.send(PlayClientBound::DeclareCommands {
            nodes: if password.is_some() {
                // if the user is already registered
                // only register the /login command
                vec![
                    CommandNode::Root {
                        children: vec![VarInt(1)],
                    },
                    CommandNode::Literal {
                        executable: false,
                        children: vec![VarInt(2)],
                        redirect: None,
                        name: "login".into(),
                    },
                    CommandNode::Argument {
                        executable: true,
                        children: Vec::new(),
                        redirect: None,
                        name: "password".into(),
                        parser: Parser::String(StringParserType::SingleWord),
                        suggestions: None,
                    },
                ]
            } else {
                // and if the user is not registered yet
                // only register the /register command
                vec![
                    CommandNode::Root {
                        children: vec![VarInt(1)],
                    },
                    CommandNode::Literal {
                        executable: false,
                        children: vec![VarInt(2)],
                        redirect: None,
                        name: "register".into(),
                    },
                    CommandNode::Argument {
                        executable: false,
                        children: vec![VarInt(3)],
                        redirect: None,
                        name: "password".into(),
                        parser: Parser::String(StringParserType::SingleWord),
                        suggestions: None,
                    },
                ]
            },
            root: VarInt(0),
        })?;

        stream.send(PlayClientBound::Title(TitleAction::Reset))?;

        stream.send(PlayClientBound::Title(TitleAction::SetTitle(chat_parse(
            "§bWelcome to §d§lBWS§r§b!",
        ))))?;

        stream.send(PlayClientBound::Title(TitleAction::SetDisplayTime {
            fade_in: 15,
            display: 60,
            fade_out: 15,
        }))?;

        stream.send(PlayClientBound::EntitySoundEffect {
            sound_id: VarInt(482),
            category: SoundCategory::Master,
            entity_id: VarInt(id as i32), // player
            volume: 1.0,
            pitch: 1.0,
        })?;

        Ok(())
    }
    async fn save_accounts(&self) -> Result<()> {
        let mut f = File::create(ACCOUNTS_FILE).await?;

        for account in &self.accounts {
            // I wish to apologize for the readability of the following statement
            #[rustfmt::skip]
            f.write_all(account.0.as_bytes()).await.and(
                f.write_all(b" ").await.and(
                    f.write_all(account.1.as_bytes()).await.and(
                        f.write_all(b"\n").await
                    )
                ),
            ).context(format!("Couldn't write to {}", ACCOUNTS_FILE))?;
        }

        Ok(())
    }
    async fn tick(&mut self, counter: u128) {
        // every second sends all the connected players an above-hotbar instructions
        if counter % 20 == 0 {
            for (_id, player) in &self.players {
                let subtitle = if self.accounts.contains_key(&player.0) {
                    &self.login_message
                } else {
                    &self.register_message
                };
                // if this returns Err, that would mean that the player is already disconnected
                // and the disconnected clients will be cleaned on the part where we try to read
                // from them so we can just ignore this error.
                let _ =
                    player
                        .1
                        .lock()
                        .await
                        .send(PlayClientBound::Title(TitleAction::SetActionBar(
                            subtitle.clone(),
                        )));
            }
        }
    }
}

pub fn start() -> Result<WSender> {
    lazy_static::initialize(&TAGS);

    let (w_sender, w_receiver) = unbounded_channel::<WBound>();

    let mut world = LoginWorld::new()?;

    spawn(async move {
        world.run(w_receiver).await;
    });

    Ok(w_sender)
}
