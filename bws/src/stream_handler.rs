use crate::global_state::{Player, PlayerStream};
use crate::internal_communication as ic;
use crate::internal_communication::{SHOutputSender, WBound, WSender};
use crate::world;
use crate::GLOBAL_STATE;
use anyhow::{bail, Context, Result};
use flate2::write::{ZlibDecoder, ZlibEncoder};
use flate2::Compression;
use log::{debug, error, info, warn};
use protocol::datatypes::chat_parse::parse as chat_parse;
use protocol::datatypes::*;
use protocol::packets::*;
use protocol::{Deserializable, Serializable};
use serde::Deserialize;
use serde_json::{json, to_string, to_string_pretty};
use std::cmp::min;
use std::io::{Cursor, Write};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::RwLock;
use tokio::io::{AsyncBufRead, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::pin;
use tokio::sync::{
    mpsc::{self, channel, unbounded_channel},
    oneshot, Mutex,
};
use tokio::time::{Duration, Instant};

#[derive(Clone, Copy)]
enum State {
    Handshake,
    Status,
    Login,
    Play(usize), // the EID of the player
}

impl From<State> for i32 {
    fn from(state: State) -> Self {
        match state {
            State::Handshake => 0,
            State::Status => 1,
            State::Login => 2,
            State::Play(_) => 3,
        }
    }
}

#[derive(Deserialize)]
struct UsernameToUuidResponse {
    #[allow(dead_code)]
    name: String,
    id: String,
}

#[derive(Deserialize)]
struct UuidToProfileAndSkinCapeResponse {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    name: String,
    properties: Vec<Property>,
}

#[derive(Deserialize)]
struct Property {
    name: String,
    value: String,
}

async fn read_varint(
    first_byte: std::io::Result<u8>,
    input: &mut BufReader<TcpStream>,
) -> std::io::Result<VarInt> {
    let mut byte = first_byte?;
    let mut i = 0;
    let mut result: i64 = 0;

    loop {
        let value = (byte & 0b01111111) as i64;
        result |= value << (7 * i);

        if (byte & 0b10000000) == 0 || i == 4 {
            break;
        }
        i += 1;

        byte = input.read_u8().await?;
    }

    Ok(VarInt(result as i32))
}
async fn write_varint(varint: VarInt, output: &mut BufReader<TcpStream>) -> std::io::Result<()> {
    let mut number = varint.0 as u32;

    loop {
        let mut byte: u8 = number as u8 & 0b01111111;

        number >>= 7;
        if number != 0 {
            byte |= 0b10000000;
        }

        output.write_u8(byte).await?;

        if number == 0 {
            break;
        }
    }

    Ok(())
}

pub async fn handle_stream(socket: TcpStream) {
    // first check if the ip isnt banned
    if let Ok(addr) = socket.peer_addr() {
        if GLOBAL_STATE
            .banned_addresses
            .read()
            .await
            .contains(&addr.ip())
        {
            // banned :/
            return;
        }
    } else {
        return;
    }

    let mut state = State::Handshake;

    if let Err(e) = handle(socket, &mut state).await {
        debug!("Error: {:?}", e);
    }

    if let State::Play(id) = state {
        // gracefully remove myself from the `players` Slab
        GLOBAL_STATE.players.write().await.remove(id);
    }
}

async fn handle(socket: TcpStream, state: &mut State) -> Result<()> {
    // get the address of the client
    let address = socket.peer_addr()?;
    debug!("{} connected", address);

    // create the internal communication channels
    let (shinput_sender, mut shinput_receiver) = unbounded_channel::<PlayClientBound<'static>>();
    let (mut shoutput_sender, shoutput_receiver) = channel::<PlayServerBound<'static>>(64);
    let (dc_sender, mut dc_receiver) = oneshot::channel();

    // get the stream ready, even thought it might not be used.
    // for example if this client is only pinging the server
    // This is in an Option so I can move it out in the loop
    let mut player_stream = Some(PlayerStream {
        sender: shinput_sender,
        receiver: shoutput_receiver,
        disconnect: Some(dc_sender),
    });

    // Using a Buffered Reader may increase the performance significantly
    let mut socket = BufReader::new(socket);
    // And we're gonna use this buffer for reading and writing.
    // Especially going to be useful for compressing and decompressing packets.
    let mut buffer = Vec::new();

    let mut client_protocol = -1; // will be set after the handshake packet

    let mut next_keepalive = Instant::now() + Duration::from_secs(5);
    let mut last_keepalive_received = Instant::now();
    let mut last_keepalive_sent = Instant::now();

    loop {
        // tokio::select - whichever arrives first: data from the TcpStream,
        // or SHInput messages from other threads
        // also the timer to send the keepalive packets
        // aaaand also the disconnect oneshot channel
        tokio::select!(
            // Can't read the whole VarInt here, since
            // Other futures may interrupt, and if they do mid-read
            // That will corrupt the stream. So read 1 byte,
            // and then stop polling the other futures until the whole
            // packet is read.
            first_byte = socket.read_u8() => read_and_parse_packet(
                &mut socket,
                &mut buffer,
                &mut client_protocol,
                &address,
                state,
                &mut player_stream,
                &mut shoutput_sender,
                &mut last_keepalive_received,
                last_keepalive_sent,
                first_byte
            ).await?,
            message = shinput_receiver.recv() => {
                let packet = message.context("The PlayerStream was dropped even before the actual stream handler task finished.")?;

                write_packet(&mut socket, &mut buffer, packet.cb()).await?
            },
            // The disconnect oneshot receiver
            _ = &mut dc_receiver => {
                // time to bye
                return Ok(());
            }
            _ = tokio::time::sleep_until(next_keepalive) => {
                if let State::Play(_) = state {
                    // first check if the connection hasn't already timed out
                    if Instant::now().duration_since(last_keepalive_received).as_secs_f32() > 30.0 {
                        bail!("Client timed out ({})", address);
                    }

                    // send the keep alive packet
                    write_packet(&mut socket, &mut buffer, PlayClientBound::KeepAlive(0).cb()).await?;
                    last_keepalive_sent = Instant::now();
                }
                next_keepalive += Duration::from_secs(5);
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
async fn read_and_parse_packet(
    socket: &mut BufReader<TcpStream>,
    buffer: &mut Vec<u8>,
    client_protocol: &mut i32,
    address: &SocketAddr,
    state: &mut State,
    player_stream: &mut Option<PlayerStream>,
    shoutput_sender: &mut SHOutputSender,
    last_keepalive_received: &mut Instant,
    last_keepalive_sent: Instant,
    first_byte: Result<u8, tokio::io::Error>,
) -> Result<()> {
    // read the rest of the VarInt
    let length = read_varint(first_byte, socket).await?.0;

    // read the rest of the packet
    let packet = read_packet(socket, buffer, &*state, length as usize).await?;

    match packet {
        ServerBound::Handshake(HandshakeServerBound::Handshake {
            protocol,
            server_address: _,
            server_port: _,
            next_state,
        }) => {
            *client_protocol = protocol.0;

            match next_state {
                NextState::Status => {
                    *state = State::Status;
                }
                NextState::Login => {
                    *state = State::Login;
                }
            }
        }
        ServerBound::Status(StatusServerBound::Ping(number)) => {
            let packet = StatusClientBound::Pong(number).cb();
            write_packet(socket, buffer, packet).await?;
        }
        ServerBound::Status(StatusServerBound::Request) => {
            let supported = GLOBAL_STATE.description.lock().await;
            let unsupported = chat_parse(
                            &format!("§4Your Minecraft version is §lnot supported§r§4.\n§c§lThe server §r§cis running §b§l{}§r§c.", crate::VERSION_NAME)
                        );

            let packet = StatusClientBound::Response(StatusResponse {
                json: StatusResponseJson {
                    version: StatusVersion {
                        name: crate::VERSION_NAME.into(),
                        protocol: *client_protocol,
                    },
                    players: StatusPlayers {
                        max: *GLOBAL_STATE.max_players.lock().await,
                        online: -(GLOBAL_STATE.players.read().await.len() as i32),
                        sample: GLOBAL_STATE.player_sample.lock().await.clone(),
                    },
                    description: if crate::SUPPORTED_PROTOCOL_VERSIONS
                        .iter()
                        .any(|&i| i == *client_protocol)
                    {
                        supported.clone()
                    } else {
                        unsupported
                    },
                    favicon: GLOBAL_STATE.favicon.lock().await.to_string().into(),
                },
            });
            write_packet(socket, buffer, packet.cb()).await?;
        }
        ServerBound::Login(LoginServerBound::LoginStart { username }) => {
            if player_stream.is_none() {
                // dude did you just send the LoginStart packet twice???
                bail!("Bad client ({})", address);
            }

            // check if not banned
            if let Some(data) = GLOBAL_STATE.player_data.read().await.get(&*username) {
                if let Some((until, reason, issuer)) = &data.banned {
                    let now = chrono::Utc::now();
                    if now < *until {
                        // oh boy this lad is banned!!
                        // better let him know!
                        let issuer_str = match &issuer {
                            Some(username) => username.as_str(),
                            None => "[server]",
                        };
                        let packet = LoginClientBound::Disconnect(
                                chat_parse(
                                    &format!(
                                        "§l§4With all due respect, you have been banned from this server until \n§6{}§4\n(by §5{}§4).\nReason: §r§o§c{}\n\n§6{}",
                                        until.format("%F %T %Z"),
                                        issuer_str,
                                        reason,
                                        if issuer.is_some() {
                                            "This ban can not be revoked, since it was issued manually. Feel free to go outside and take a break from the computer."
                                        } else {
                                            "If you think this ban was undeserved, you can try to get it revoked on our discord."
                                        }
                                    )
                                ),
                            );
                        let _ = write_packet(socket, buffer, packet.cb()).await;
                        return Ok(());
                    }
                }
            }

            // check if version is supported
            if !crate::SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .any(|&i| i == *client_protocol)
            {
                let packet = LoginClientBound::Disconnect(
                                chat_parse(&format!("§4Your Minecraft version is §lnot supported§r§4.\n§c§lThe server §r§cis running §b§l{}§r§c.", crate::VERSION_NAME)),
                            );
                let _ = write_packet(socket, buffer, packet.cb()).await;
                return Ok(());
            }

            if GLOBAL_STATE
                .players
                .read()
                .await
                .iter()
                .any(|(_, player)| player.username == username)
            {
                let packet = LoginClientBound::Disconnect(chat_parse(
                    "§c§lSomeone is already playing with this username!",
                ));
                let _ = write_packet(socket, buffer, packet.cb()).await;
                return Ok(());
            }

            // get the uuid
            let res = reqwest::get(format!(
                "https://api.mojang.com/users/profiles/minecraft/{}",
                username
            ))
            .await?;
            if res.status().is_client_error() {
                error!(
                    "Received {} from api.mojang.com when trying to get the UUID of '{}'",
                    res.status(),
                    username
                );
            }
            let uuid = if res.status().as_u16() == 200 {
                let response: UsernameToUuidResponse = res.json().await?;
                Some(u128::from_str_radix(&response.id, 16)?)
            } else {
                None
            };

            let mut properties = Vec::new();

            if let Some(uuid) = uuid {
                // also query skin/cape data
                let res = reqwest::get(format!(
                    "https://sessionserver.mojang.com/session/minecraft/profile/{:x}",
                    uuid
                ))
                .await?;
                if res.status().is_client_error() {
                    error!(
                        "Received {} from sessionserver.mojang.com when trying to get skin/cape data of '{}' (`{:x}`)",
                        res.status(),
                        username, uuid
                    );
                }

                let response: UuidToProfileAndSkinCapeResponse = res.json().await?;

                for property in response.properties {
                    properties.push(PlayerInfoAddPlayerProperty {
                        name: property.name.into(),
                        value: property.value.into(),
                        signature: None,
                    });
                }
            }

            let uuid =
                uuid.unwrap_or_else(|| uuid_from_string(&format!("OfflinePlayer:{}", username)));

            // set compression if non-negative
            if GLOBAL_STATE.compression_treshold >= 0 {
                let packet = LoginClientBound::SetCompression {
                    treshold: VarInt(GLOBAL_STATE.compression_treshold as i32),
                };
                write_packet(socket, buffer, packet.cb()).await?;
            }

            // everything's alright, come in
            let packet = LoginClientBound::LoginSuccess {
                uuid,
                username: username.clone(),
            };
            write_packet(socket, buffer, packet.cb()).await?;

            // since the keepalives are going to start being sent, reset the timeout timer
            *last_keepalive_received = Instant::now();

            // add the player to the global_state
            let global_id = GLOBAL_STATE.players.write().await.insert(Player {
                // can unwrap since check previously in this function
                stream: Arc::new(Mutex::new(player_stream.take().unwrap())),
                username: username.into_owned(),
                address: *address,
                uuid,
                properties,
                ping: 0.0,
                settings: None,
                logged_in: false,
            });
            *state = State::Play(global_id);

            // add the player to the login world
            GLOBAL_STATE
                .w_login
                .0
                .send(ic::WBound::AddPlayer { id: global_id })
                .context("Login world receiver lost.")?;
        }
        ServerBound::Play(PlayServerBound::KeepAlive(_)) => {
            // Reset the timeout timer
            *last_keepalive_received = Instant::now();

            // update client ping
            if let State::Play(id) = state {
                GLOBAL_STATE.players.write().await[*id].ping = Instant::now()
                    .duration_since(last_keepalive_sent)
                    .as_secs_f32()
                    / 1000.0;
            }
        }
        ServerBound::Play(PlayServerBound::ClientSettings(settings)) => {
            if let State::Play(id) = state {
                // make the client think the server's view distance is the same
                // (as long as its not higher than 16, since thats the limit of this server)
                write_packet(
                    socket,
                    buffer,
                    PlayClientBound::UpdateViewDistance(VarInt(min(
                        16,
                        settings.view_distance as i32 + 2,
                    )))
                    .cb(),
                )
                .await?;

                GLOBAL_STATE
                    .players
                    .write()
                    .await
                    .get_mut(*id)
                    .unwrap()
                    .settings = Some(settings);
            }
        }
        ServerBound::Play(PlayServerBound::TabComplete {
            transaction_id,
            text,
        }) => {
            let id = match state {
                State::Play(id) => *id,
                _ => panic!(), // can't receive a Play packet if the state is not Play
            };

            // handle not world-specific command tabcompletes
            if GLOBAL_STATE.players.read().await[id].logged_in
                && text.starts_with('/')
                && handle_tabcomplete(socket, buffer, id, transaction_id, &text).await?
            {
                return Ok(());
            }

            shoutput_sender.send(PlayServerBound::TabComplete {
                transaction_id,
                text,
            }).await.context(
                "The PlayerStream was dropped even before the actual stream handler task finished.",
            )?;
        }
        ServerBound::Play(PlayServerBound::ChatMessage(message)) => {
            let id = match state {
                State::Play(id) => *id,
                _ => panic!(), // can't receive a Play packet if the state is not Play
            };

            if GLOBAL_STATE.players.read().await[id].logged_in {
                // handle not world-specific commands
                if message.starts_with('/') && handle_command(socket, buffer, id, &message).await? {
                    return Ok(());
                }
            }

            shoutput_sender.send(PlayServerBound::ChatMessage(message)).await.context(
                "The PlayerStream was dropped even before the actual stream handler task finished.",
            )?;
        }
        ServerBound::Play(other) => {
            shoutput_sender.send(other).await.context(
                "The PlayerStream was dropped even before the actual stream handler task finished.",
            )?;
        }
        _ => {}
    }
    Ok(())
}

// returns whether a command was processed, because if not, it will be forwarded to the world
// so it can handle it
async fn handle_command(
    socket: &mut BufReader<TcpStream>,
    buffer: &mut Vec<u8>,
    id: usize,
    message: &str,
) -> Result<bool> {
    // convenience macro
    macro_rules! say {
        ($message:expr) => {
            write_packet(
                socket,
                buffer,
                PlayClientBound::ChatMessage {
                    message: chat_parse($message),
                    position: ChatPosition::System,
                    sender: 0,
                }
                .cb(),
            )
            .await?;
        };
    }

    let permissions = GLOBAL_STATE.player_data.read().await
        [&GLOBAL_STATE.players.read().await[id].username]
        .permissions;

    let mut segments = message.split(' ');

    let command = segments.next().unwrap();
    match command {
        "/banip" | "/unbanip" => {
            if !permissions.ban_ips {
                return Ok(false);
            }

            match segments.next() {
                Some(ip) => {
                    let ip = match ip.parse() {
                        Ok(ip) => ip,
                        Err(e) => {
                            say!(format!("§6Couldn't parse ip: {}", e));
                            return Ok(true);
                        }
                    };

                    if command == "/banip" {
                        // ban
                        info!(
                            "{} banned {}",
                            GLOBAL_STATE.players.read().await[id].username,
                            ip
                        );

                        say!(format!("§7IP {} banned.", &ip));

                        GLOBAL_STATE.banned_addresses.write().await.insert(ip);
                        GLOBAL_STATE.save_banned_ips().await;

                        // if anyone is already connected with that ip, disconnect them
                        for (_id, player) in &*GLOBAL_STATE.players.read().await {
                            if player.address.ip() == ip {
                                player.stream.lock().await.disconnect();
                            }
                        }
                    } else {
                        // unban
                        if GLOBAL_STATE.banned_addresses.write().await.remove(&ip) {
                            info!(
                                "{} unbanned {}",
                                GLOBAL_STATE.players.read().await[id].username,
                                ip
                            );

                            say!(format!("§7IP {} unbanned.", &ip));

                            GLOBAL_STATE.save_banned_ips().await;
                        } else {
                            say!(format!("§7IP {} is not banned.", &ip));
                        }
                    }
                }
                None => {
                    say!("§6Usage: /banip §e<ip address>");
                    return Ok(true);
                }
            }
        }
        "/setperm" => {
            if !permissions.admin && !permissions.owner {
                return Ok(false);
            }

            let username = match segments.next() {
                Some(arg) => arg,
                None => {
                    say!("§6Usage: /setperm §e<username> <permission> <true|false>");
                    return Ok(true);
                }
            };

            let mut lock = GLOBAL_STATE.player_data.write().await;
            let player = if let Some(player) = lock.get_mut(username) {
                player
            } else {
                say!(format!("§6No such player \"{}\".", username));
                return Ok(true);
            };

            let permission_str = match segments.next() {
                Some(arg) => arg,
                None => {
                    say!("§6Usage: /setperm §e<username> <permission> <true|false>");
                    return Ok(true);
                }
            };
            let permission = match permission_str {
                "owner" => {
                    if permissions.owner {
                        &mut player.permissions.owner
                    } else {
                        say!("§6Only owners can set the owner permission.");
                        return Ok(true);
                    }
                }
                "admin" => {
                    if permissions.owner {
                        &mut player.permissions.admin
                    } else {
                        say!("§6Only owners can set the admin permission.");
                        return Ok(true);
                    }
                }
                "edit_lobby" => {
                    if permissions.admin {
                        &mut player.permissions.edit_lobby
                    } else {
                        say!("§6Only admins can set permissions.");
                        return Ok(true);
                    }
                }
                "ban_usernames" => {
                    if permissions.admin {
                        &mut player.permissions.ban_usernames
                    } else {
                        say!("§6Only admins can set permissions.");
                        return Ok(true);
                    }
                }
                "ban_ips" => {
                    if permissions.admin {
                        &mut player.permissions.ban_ips
                    } else {
                        say!("§6Only admins can set permissions.");
                        return Ok(true);
                    }
                }
                other => {
                    say!(format!("§6No such permission \"{}\".", other));
                    return Ok(true);
                }
            };

            let value = match segments.next() {
                Some(arg) => match arg.parse() {
                    Ok(value) => value,
                    Err(e) => {
                        say!(format!("§6Couldn't parse value: {}", e));
                        return Ok(true);
                    }
                },
                None => {
                    say!("§6Usage: /setperm §e<username> <permission> <true|false>");
                    return Ok(true);
                }
            };

            info!(
                "{} set the permission \"{}\" of {} to {}",
                GLOBAL_STATE.players.read().await[id].username,
                permission_str,
                username,
                value
            );
            say!("§7Permission set.");
            *permission = value;

            drop(lock);

            GLOBAL_STATE.save_player_data().await;
        }
        "/perms" => {
            if !permissions.admin {
                return Ok(false);
            }
            match segments.next() {
                Some(username) => {
                    // query the permissions of the given player
                    let lock = GLOBAL_STATE.player_data.read().await;
                    let player = if let Some(player) = lock.get(username) {
                        player
                    } else {
                        say!(format!("§6No such player \"{}\".", username));
                        return Ok(true);
                    };

                    say!(format!(
                        "§l§7§nPermissions of {}:\n§r§o§7{:#?}",
                        username, player.permissions
                    ));
                }
                None => {
                    // return the permissions of self
                    let username = &GLOBAL_STATE.players.read().await[id].username;

                    say!(format!(
                        "§l§7§nPermissions of {}:\n§r§o§7{:#?}",
                        username,
                        GLOBAL_STATE.player_data.read().await[username].permissions
                    ));
                }
            }
        }
        "/ban" => {
            if !permissions.ban_usernames {
                return Ok(false);
            }

            let username = match segments.next() {
                Some(arg) => arg,
                None => {
                    say!("§6Usage: /ban §e<username> <duration in minutes> <reason>");
                    return Ok(true);
                }
            };

            let duration = match segments.next() {
                Some(arg) => match arg.parse() {
                    Ok(i) => i,
                    Err(e) => {
                        say!(format!("§6Couldn't parse duration: {}", e));
                        return Ok(true);
                    }
                },
                None => {
                    say!("§6Usage: /ban §e<username> <duration in minutes> <reason>");
                    return Ok(true);
                }
            };

            let reason = match segments.next() {
                // get the text that starts with the segments, which may be multiple segments
                Some(arg) => &message[(arg.as_ptr() as usize - message.as_ptr() as usize)..],
                None => {
                    say!("§6Usage: /ban §e<username> <duration in minutes> <reason>");
                    return Ok(true);
                }
            };

            let mut lock = GLOBAL_STATE.player_data.write().await;
            let player = if let Some(player) = lock.get_mut(username) {
                player
            } else {
                say!(format!("§6No such player \"{}\".", username));
                return Ok(true);
            };

            player.banned = Some((
                chrono::Utc::now() + chrono::Duration::minutes(duration),
                reason.to_owned(),
                Some(GLOBAL_STATE.players.read().await[id].username.clone()),
            ));

            // if anyone is already connected with that username, disconnect them
            for (_id, player) in &*GLOBAL_STATE.players.read().await {
                if player.username.as_str() == username {
                    player.stream.lock().await.disconnect();
                }
            }

            let human_readable_duration = {
                let mut duration = chrono::Duration::minutes(duration);
                let mut result = String::new();
                if duration.num_weeks() > 0 {
                    result += &format!("{} weeks ", duration.num_weeks());
                    duration = duration - chrono::Duration::weeks(duration.num_weeks());
                }
                if duration.num_days() > 0 {
                    result += &format!("{} days ", duration.num_days());
                    duration = duration - chrono::Duration::days(duration.num_days());
                }
                if duration.num_hours() > 0 {
                    result += &format!("{} hours ", duration.num_hours());
                    duration = duration - chrono::Duration::hours(duration.num_hours());
                }
                if duration.num_minutes() > 0 || result.is_empty() {
                    result += &format!("{} minutes ", duration.num_minutes());
                }
                result
            };

            say!(format!(
                "§7Player {} banned for {}.",
                username,
                human_readable_duration.trim()
            ));
            info!(
                "{} banned {} for {}, because of \"{}\"",
                GLOBAL_STATE.players.read().await[id].username.clone(),
                username,
                human_readable_duration.trim(),
                reason
            );

            drop(lock);

            GLOBAL_STATE.save_player_data().await;
        }
        "/unban" => {
            if !permissions.ban_usernames {
                return Ok(false);
            }

            let username = match segments.next() {
                Some(arg) => arg,
                None => {
                    say!("§6Usage: /unban §e<username>");
                    return Ok(true);
                }
            };

            let mut lock = GLOBAL_STATE.player_data.write().await;
            let player = if let Some(player) = lock.get_mut(username) {
                player
            } else {
                say!(format!("§6No such player \"{}\".", username));
                return Ok(true);
            };

            player.banned = None;

            say!(format!("§7Player {} unbanned.", username));
            info!(
                "{} unbanned {}",
                GLOBAL_STATE.players.read().await[id].username.clone(),
                username,
            );

            drop(lock);

            GLOBAL_STATE.save_player_data().await;
        }
        _ => return Ok(false),
    }

    Ok(true)
}

// returns whether the tabcomplete was handled or not
async fn handle_tabcomplete(
    socket: &mut BufReader<TcpStream>,
    buffer: &mut Vec<u8>,
    id: usize,
    transaction_id: VarInt,
    text: &str,
) -> Result<bool> {
    let permissions = GLOBAL_STATE.player_data.read().await
        [&GLOBAL_STATE.players.read().await[id].username]
        .permissions;

    let mut segments = text.split(' ');

    let command = segments.next().unwrap();
    let (last_segment_index, last_segment) = match segments.enumerate().last() {
        Some(s) => s,
        None => return Ok(true),
    };

    // pointer arithmetics! *evil laugh*...
    // this is the start of the last segment relative to the whole command (in bytes)
    let last_segment_offset = (last_segment.as_ptr() as usize).wrapping_sub(text.as_ptr() as usize);

    // and this is the same but in characters
    let last_segment_start = text[0..last_segment_offset].chars().count() as i32;

    let last_segment_length = last_segment.chars().count() as i32;

    match command {
        "/ban" | "/unban" | "/perms" => {
            if !permissions.ban_usernames {
                return Ok(true);
            }
            if last_segment_index == 0 {
                // list all names registered in the server

                write_packet(
                    socket,
                    buffer,
                    PlayClientBound::TabComplete {
                        transaction_id,
                        start: VarInt(last_segment_start),
                        end: VarInt(last_segment_start + last_segment_length),
                        matches: GLOBAL_STATE
                            .player_data
                            .read()
                            .await
                            .iter()
                            .filter(|e| e.0.starts_with(&last_segment))
                            .map(|p| (p.0.to_owned().into(), None))
                            .collect(),
                    }
                    .cb(),
                )
                .await?;
            }
        }
        "/setperm" => {
            if !permissions.admin {
                return Ok(true);
            }

            if last_segment_index == 0 {
                // usernames
                write_packet(
                    socket,
                    buffer,
                    PlayClientBound::TabComplete {
                        transaction_id,
                        start: VarInt(last_segment_start),
                        end: VarInt(last_segment_start + last_segment_length),
                        matches: GLOBAL_STATE
                            .player_data
                            .read()
                            .await
                            .iter()
                            .filter(|e| e.0.starts_with(&last_segment))
                            .map(|p| (p.0.to_owned().into(), None))
                            .collect(),
                    }
                    .cb(),
                )
                .await?;
            } else if last_segment_index == 1 {
                // permissions
                write_packet(
                    socket,
                    buffer,
                    PlayClientBound::TabComplete {
                        transaction_id,
                        start: VarInt(last_segment_start),
                        end: VarInt(last_segment_start + last_segment_length),
                        matches: vec!["owner", "admin", "edit_lobby", "ban_usernames", "ban_ips"]
                            .iter()
                            .filter(|e| e.starts_with(&last_segment))
                            .map(|p| ((*p).into(), None))
                            .collect(),
                    }
                    .cb(),
                )
                .await?;
            }
        }
        _ => return Ok(false),
    }

    Ok(true)
}

async fn read_packet(
    socket: &mut BufReader<TcpStream>,
    buffer: &mut Vec<u8>,
    state: &State,
    length: usize,
) -> Result<ServerBound<'static>> {
    if length > 0x1F_FFFF {
        bail!("Packet too big. Max size allowed: 0x1F FFFF bytes.");
    }
    if GLOBAL_STATE.compression_treshold >= 0 && matches!(state, State::Play(_)) {
        // compressed packet format
        let uncompressed_size = read_varint(socket.read_u8().await, socket).await?;

        if uncompressed_size.0 > 0x1F_FFFF {
            bail!("Uncompressed packet too big. Max size allowed: 0x1F FFFF bytes.");
        }

        if uncompressed_size.0 == 0 {
            // that means the data isnt actually compressed so we can just read it normally
            buffer.resize(length - 1, 0); // - 1 since we already read the uncompressed_size, which was 1 byte since it was 0

            socket.read_exact(&mut buffer[..]).await?;
        } else {
            // time to decompress
            buffer.clear();
            let mut decoder = ZlibDecoder::new(&mut *buffer);

            let mut to_read = length - uncompressed_size.size() as usize;

            while to_read > 0 {
                use futures::AsyncBufReadExt;
                use tokio_util::compat::TokioAsyncReadCompatExt;

                if socket.buffer().is_empty() {
                    socket.compat().fill_buf().await?;
                }
                if to_read >= socket.buffer().len() {
                    let buffer_size = socket.buffer().len();
                    decoder.write_all(socket.buffer())?;
                    socket.compat().consume_unpin(buffer_size);
                    to_read -= buffer_size;
                } else {
                    decoder.write_all(&socket.buffer()[..to_read])?;
                    socket.compat().consume_unpin(to_read);
                    to_read = 0;
                }
            }
            decoder.finish()?;
        }
    } else {
        // uncompressed packet format
        buffer.resize(length, 0);

        socket.read_exact(&mut buffer[..]).await?;
    }

    let mut cursor = Cursor::new(&*buffer);

    Ok(match *state {
        State::Handshake => HandshakeServerBound::from_reader(&mut cursor)?.sb(),
        State::Status => StatusServerBound::from_reader(&mut cursor)?.sb(),
        State::Login => LoginServerBound::from_reader(&mut cursor)?.sb(),
        State::Play(_) => PlayServerBound::from_reader(&mut cursor)?.sb(),
    })
}

struct NoOp;
impl Write for NoOp {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

async fn write_packet<'a>(
    socket: &'a mut BufReader<TcpStream>,
    buffer: &'a mut Vec<u8>,
    packet: ClientBound<'static>,
) -> tokio::io::Result<()> {
    buffer.clear();
    if (matches!(packet, ClientBound::Play(_))
        || matches!(
            packet,
            ClientBound::Login(LoginClientBound::LoginSuccess { .. })
        ))
        && GLOBAL_STATE.compression_treshold >= 0
    {
        // use the compressed packet format

        // first check if the packet is long enough to actually be compressed
        let uncompressed_length = packet.to_writer(&mut NoOp)?;

        // if the packet is long enough be compressed
        if uncompressed_length as i32 >= GLOBAL_STATE.compression_treshold {
            let mut encoder = ZlibEncoder::new(&mut *buffer, Compression::fast());
            packet.to_writer(&mut encoder)?;
            encoder.finish()?;
            let compressed_length = buffer.len();

            let uncompressed_length = VarInt(uncompressed_length as i32);
            write_varint(
                VarInt(compressed_length as i32 + uncompressed_length.size() as i32),
                socket,
            )
            .await?;
            write_varint(uncompressed_length, socket).await?;
            socket.write_all(&buffer[..]).await?;
        } else {
            // the packet will not actually be compressed
            packet.to_writer(buffer)?;
            write_varint(VarInt(uncompressed_length as i32 + 1), socket).await?; // + 1 because the following VarInt is counted too and it's always 1 byte since it's 0
            write_varint(VarInt(0), socket).await?;
            socket.write_all(&buffer[..]).await?;
        }
    } else {
        // the uncompressed packet format
        // could get better performance if could serialize packets to AsyncWrite (todo?)
        packet.to_writer(buffer)?;
        let length = VarInt(buffer.len() as i32);
        write_varint(length, socket).await?;
        socket.write_all(&buffer[..]).await?;
    }

    Ok(())
}

pub fn uuid_from_string(input: &str) -> u128 {
    let mut uuid = md5::compute(input.as_bytes()).0;
    uuid[6] &= 0x0f; // clear version
    uuid[6] |= 0x30; // set to version 3
    uuid[8] &= 0x3f; // clear variant
    uuid[8] |= 0x80; // set to IETF variant
    u128::from_be_bytes(uuid)
}
