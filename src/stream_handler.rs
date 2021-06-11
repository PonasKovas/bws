use crate::chat_parse;
use crate::datatypes::*;
use crate::global_state::Player;
use crate::global_state::PlayerStream;
use crate::internal_communication as ic;
use crate::internal_communication::WBound;
use crate::internal_communication::WSender;
use crate::packets::{ClientBound, ServerBound};
use crate::world;
use crate::GLOBAL_STATE;
use anyhow::{bail, Context, Result};
use flate2::write::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use log::{debug, error, info, warn};
use serde_json::to_string_pretty;
use serde_json::{json, to_string};
use std::io::Cursor;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::io::AsyncBufRead;
use tokio::io::BufReader;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::pin;
use tokio::sync::mpsc;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::oneshot;
use tokio::sync::Mutex;
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

pub async fn handle_stream(socket: TcpStream) {
    let mut state = State::Handshake;

    if let Err(e) = handle(socket, &mut state).await {
        if e.is::<std::io::Error>() {
            debug!("IO error: {}", e);
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
    let (shinput_sender, mut shinput_receiver) = unbounded_channel::<ClientBound>();
    let (mut shoutput_sender, shoutput_receiver) = unbounded_channel::<ServerBound>();
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
                first_byte
            ).await?,
            message = shinput_receiver.recv() => {
                let packet = message.context("The PlayerStream was dropped even before the actual stream handler task finished.")?;

                write_packet(&mut socket, &mut buffer, packet, GLOBAL_STATE.compression_treshold).await?
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
                    write_packet(&mut socket, &mut buffer, ClientBound::KeepAlive(0), GLOBAL_STATE.compression_treshold).await?;
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
    shoutput_sender: &mut mpsc::UnboundedSender<ServerBound>,
    last_keepalive_received: &mut Instant,
    first_byte: Result<u8, tokio::io::Error>,
) -> Result<()> {
    // read the rest of the VarInt
    let mut number = first_byte?;
    let mut i: usize = 0;
    let mut length: i32 = 0;
    loop {
        let value = (number & 0b01111111) as i32;
        length = length | (value << (7 * i));

        if (number & 0b10000000) == 0 {
            break;
        }

        number = socket.read_u8().await?;
        i += 1;
    }

    // read the rest of the packet
    let packet = read_packet(
        socket,
        buffer,
        (*state).into(),
        length as usize,
        if let State::Play(_) = *state {
            GLOBAL_STATE.compression_treshold
        } else {
            -1
        },
    )
    .await?;

    Ok(match packet {
        ServerBound::Handshake {
            protocol,
            address: _,
            port: _,
            next_state,
        } => {
            *client_protocol = protocol.0;

            if next_state.0 == 1 {
                *state = State::Status;
            } else if next_state.0 == 2 {
                *state = State::Login;
            } else {
                // wrong choice buddy
                bail!("Bad client ({})", address);
            }
        }
        ServerBound::StatusPing(number) => {
            let packet = ClientBound::StatusPong(number);
            write_packet(socket, buffer, packet, -1).await?;
        }
        ServerBound::StatusRequest => {
            let supported = GLOBAL_STATE.description.lock().await;
            let unsupported = crate::chat_parse::parse(
                            &format!("§4Your Minecraft version is §lnot supported§r§4.\n§c§lThe server §r§cis running §b§l{}§r§c.", crate::VERSION_NAME)
                        );

            let packet = ClientBound::StatusResponse(StatusResponse {
                version: StatusVersion {
                    name: crate::VERSION_NAME.to_string(),
                    protocol: *client_protocol,
                },
                players: StatusPlayers {
                    max: *GLOBAL_STATE.max_players.lock().await,
                    online: -1095,
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
                favicon: GLOBAL_STATE.favicon.lock().await.clone(),
            });
            write_packet(socket, buffer, packet, -1).await?;
        }
        ServerBound::LoginStart { username } => {
            if player_stream.is_none() {
                // dude did you just send the LoginStart packet twice???
                bail!("Bad client ({})", address);
            }
            // check if version is supported
            if !crate::SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .any(|&i| i == *client_protocol)
            {
                let packet = ClientBound::LoginDisconnect(
                                chat_parse(&format!("§4Your Minecraft version is §lnot supported§r§4.\n§c§lThe server §r§cis running §b§l{}§r§c.", crate::VERSION_NAME)),
                            );
                let _ = write_packet(socket, buffer, packet, -1).await;
                return Ok(());
            }

            if GLOBAL_STATE
                .players
                .read()
                .await
                .iter()
                .any(|(_, player)| player.username == username)
            {
                let packet = ClientBound::LoginDisconnect(chat_parse(
                    "§c§lSomeone is already playing with this username!",
                ));
                let _ = write_packet(socket, buffer, packet, -1).await;
                return Ok(());
            }

            // set compression if non-negative
            if GLOBAL_STATE.compression_treshold >= 0 {
                let packet = ClientBound::SetCompression {
                    treshold: VarInt(GLOBAL_STATE.compression_treshold as i32),
                };
                write_packet(socket, buffer, packet, -1).await?;
            }

            // everything's alright, come in
            let packet = ClientBound::LoginSuccess {
                uuid: 0,
                username: username.clone(),
            };
            write_packet(socket, buffer, packet, GLOBAL_STATE.compression_treshold).await?;

            // since the keepalives are going to start being sent, reset the timeout timer
            *last_keepalive_received = Instant::now();

            // add the player to the global_state
            let global_id = GLOBAL_STATE.players.write().await.insert(Player {
                // can unwrap since check previously in this function
                stream: Arc::new(Mutex::new(player_stream.take().unwrap())),
                username: username.clone(),
                address: address.clone(),
                view_distance: None,
            });
            *state = State::Play(global_id);

            // add the player to the login world
            GLOBAL_STATE
                .w_login
                .send(ic::WBound::AddPlayer { id: global_id })
                .context("Login world receiver lost.")?;
        }
        ServerBound::KeepAlive(_) => {
            // Reset the timeout timer
            *last_keepalive_received = Instant::now();
        }
        other => {
            shoutput_sender.send(other).context(
                "The PlayerStream was dropped even before the actual stream handler task finished.",
            )?;
        }
    })
}

async fn read_packet(
    socket: &mut BufReader<TcpStream>,
    buffer: &mut Vec<u8>,
    state: i32,
    length: usize,
    compression_treshold: i32, // might seem dumb that this is passed as an argument when its a static, but actually compression is enabled only in the play state, so it has to be set to -1 in earlier ones
) -> tokio::io::Result<ServerBound> {
    if compression_treshold >= 0 && state == 3 {
        // compressed packet format
        let uncompressed_size = VarInt::read(socket).await?;

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

    ServerBound::deserialize(&mut cursor, state)
}

async fn write_packet(
    socket: &mut BufReader<TcpStream>,
    buffer: &mut Vec<u8>,
    packet: ClientBound,
    compression_treshold: i32, // might seem dumb that this is passed as an argument when its a static, but actually compression is enabled only in the play state, so it has to be set to -1 in earlier ones
) -> tokio::io::Result<()> {
    buffer.clear();
    if compression_treshold >= 0 {
        // use the compressed packet format

        // first check if the packet is long enough to actually be compressed
        packet.clone().serialize(buffer);
        let uncompressed_length = buffer.len();

        // if the packet is long enough be compressed
        if uncompressed_length as i32 >= compression_treshold {
            buffer.clear();
            let mut encoder = ZlibEncoder::new(&mut *buffer, Compression::fast());
            packet.serialize(&mut encoder);
            encoder.finish()?;
            let compressed_length = buffer.len();

            let uncompressed_length = VarInt(uncompressed_length as i32);
            VarInt(compressed_length as i32 + uncompressed_length.size() as i32)
                .write(socket)
                .await?;
            uncompressed_length.write(socket).await?;
            socket.write_all(&buffer[..]).await?;
        } else {
            // the packet is not actually compressed
            // ok just send it then
            VarInt(uncompressed_length as i32 + 1).write(socket).await?; // + 1 because the following VarInt is counted too and it's always 1 byte since it's 0
            VarInt(0).write(socket).await?;
            socket.write_all(&buffer[..]).await?;
        }
    } else {
        // the uncompressed packet format
        packet.serialize(buffer);
        let length = VarInt(buffer.len() as i32);
        length.write(socket).await?;
        socket.write_all(&buffer[..]).await?;
    }

    Ok(())
}
