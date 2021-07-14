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
        result = result | (value << (7 * i));

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

        number = number >> 7;
        if number != 0 {
            byte = byte | 0b10000000;
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
        if e.is::<std::io::Error>() {
            debug!("IO error: {:?}", e);
        } else if e.is::<mpsc::error::SendError<ServerBound>>() || e.is::<mpsc::error::RecvError>()
        {
            error!("PlayerStream dropped before the actual task ended: {}", e);
        } else {
            debug!("Error: {}", e);
        }
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

    Ok(match packet {
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
                address: address.clone(),
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
            if GLOBAL_STATE.players.read().await[id].logged_in {
                if text.starts_with("/") {
                    if handle_tabcomplete(socket, buffer, id, transaction_id, &text).await? {
                        return Ok(());
                    }
                }
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
                if message.starts_with("/") {
                    if handle_command(socket, buffer, id, &message).await? {
                        return Ok(());
                    }
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
    })
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

    match message.split(' ').nth(0).unwrap() {
        "/banip" => {
            if permissions.ban_ips {
                // ban the ip

                if let Some(ip) = message.split(' ').nth(1) {
                    use std::str::FromStr;
                    if let Ok(ip) = IpAddr::from_str(ip) {
                        info!(
                            "{} banned {}",
                            GLOBAL_STATE.players.read().await[id].username,
                            ip
                        );

                        say!(format!("§2IP {} banned.", &ip));

                        GLOBAL_STATE.banned_addresses.write().await.insert(ip);
                        GLOBAL_STATE.save_banned_ips().await;

                        // if anyone is already connected with that ip, disconnect them
                        for (_id, player) in &*GLOBAL_STATE.players.read().await {
                            if player.address.ip() == ip {
                                player.stream.lock().await.disconnect();
                            }
                        }
                    }
                }
            }

            Ok(true)
        }
        "/setperm" => {
            // attempt to get the 3 arguments
            if let Some(username) = message.split(' ').nth(1) {
                if let Some(permission) = message.split(' ').nth(2) {
                    if let Some(value) = message.split(' ').nth(3) {
                        // try to parse the 3rd argument into a bool
                        if let Ok(value) = value.parse() {
                            // try to find a player with the given name
                            let mut lock = GLOBAL_STATE.player_data.write().await;
                            let player = if let Some(player) = lock.get_mut(username) {
                                player
                            } else {
                                say!("§4No such player.");
                                return Ok(true);
                            };

                            match permission {
                                "owner" => {
                                    if permissions.owner {
                                        player.permissions.owner = value;
                                    } else {
                                        say!("§4Only owners can set the owner permission.");
                                        return Ok(true);
                                    }
                                }
                                "admin" => {
                                    if permissions.owner {
                                        player.permissions.admin = value;
                                    } else {
                                        say!("§4Only owners can set the admin permission.");
                                        return Ok(true);
                                    }
                                }
                                "edit_lobby" => {
                                    if permissions.admin {
                                        player.permissions.edit_lobby = value;
                                    } else {
                                        say!("§4Only admins can set permissions.");
                                        return Ok(true);
                                    }
                                }
                                "ban_usernames" => {
                                    if permissions.admin {
                                        player.permissions.ban_usernames = value;
                                    } else {
                                        say!("§4Only admins can set permissions.");
                                        return Ok(true);
                                    }
                                }
                                "ban_ips" => {
                                    if permissions.admin {
                                        player.permissions.ban_ips = value;
                                    } else {
                                        say!("§4Only admins can set permissions.");
                                        return Ok(true);
                                    }
                                }
                                _ => {
                                    say!("§4No such permission.");
                                    return Ok(true);
                                }
                            }
                            drop(player);
                            drop(lock);

                            say!("§2Success.");
                            info!(
                                "{} set the {} permission of {} to {}",
                                GLOBAL_STATE.players.read().await[id].username,
                                permission,
                                username,
                                value,
                            );

                            drop(permissions);

                            GLOBAL_STATE.save_player_data().await;
                        } else {
                            say!("§4Couldn't parse the value");
                        }
                    }
                }
            }

            Ok(true)
        }
        "/ban" => {
            if permissions.ban_usernames {
                // todo
                say!("§4Not implemented yet.");
            }
            Ok(true)
        }
        _ => Ok(false),
    }
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

    match segments.nth(0).unwrap() {
        "/ban" => {
            if !permissions.ban_usernames {
                return Ok(true);
            }
            if let Some(next) = segments.next() {
                // list all names registered in the server
                write_packet(
                    socket,
                    buffer,
                    PlayClientBound::TabComplete {
                        transaction_id,
                        start: VarInt(5),
                        end: VarInt(5 + next.len() as i32),
                        matches: GLOBAL_STATE
                            .player_data
                            .read()
                            .await
                            .iter()
                            .filter(|e| e.0.starts_with(&next))
                            .map(|p| (p.0.to_owned().into(), None))
                            .collect(),
                    }
                    .cb(),
                )
                .await?;
            }
            Ok(true)
        }
        "/setperm" => {
            if !permissions.admin {
                return Ok(true);
            }
            if let Some((i, last)) = segments.clone().enumerate().last() {
                if i == 0 {
                    // usernames
                    write_packet(
                        socket,
                        buffer,
                        PlayClientBound::TabComplete {
                            transaction_id,
                            start: VarInt(9),
                            end: VarInt(9 + last.len() as i32),
                            matches: GLOBAL_STATE
                                .player_data
                                .read()
                                .await
                                .iter()
                                .filter(|e| e.0.starts_with(&last))
                                .map(|p| (p.0.to_owned().into(), None))
                                .collect(),
                        }
                        .cb(),
                    )
                    .await?;
                } else if i == 1 {
                    // permissions
                    write_packet(
                        socket,
                        buffer,
                        PlayClientBound::TabComplete {
                            transaction_id,
                            start: VarInt(10 + segments.nth(0).unwrap().len() as i32),
                            end: VarInt(
                                10 + segments.nth(0).unwrap().len() as i32 + last.len() as i32,
                            ),
                            matches: vec![
                                "owner",
                                "admin",
                                "edit_lobby",
                                "ban_usernames",
                                "ban_ips",
                            ]
                            .iter()
                            .filter(|e| e.starts_with(&last))
                            .map(|p| ((*p).into(), None))
                            .collect(),
                        }
                        .cb(),
                    )
                    .await?;
                }
            }

            Ok(true)
        }
        _ => Ok(false),
    }
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

                if socket.buffer().len() == 0 {
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

    Ok(match state {
        &State::Handshake => HandshakeServerBound::from_reader(&mut cursor)?.sb(),
        &State::Status => StatusServerBound::from_reader(&mut cursor)?.sb(),
        &State::Login => LoginServerBound::from_reader(&mut cursor)?.sb(),
        &State::Play(_) => PlayServerBound::from_reader(&mut cursor)?.sb(),
    })
}

struct Noop;
impl Write for Noop {
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
        let uncompressed_length = packet.to_writer(&mut Noop)?;

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
