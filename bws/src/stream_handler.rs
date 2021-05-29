use crate::chat_parse;
use crate::datatypes::*;
use crate::internal_communication as ic;
use crate::packets::{ClientBound, ServerBound};
use crate::GlobalState;
use flate2::write::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use scopeguard::defer;
use serde_json::{json, to_string};
use std::io::Cursor;
use std::io::Write;
use std::sync::RwLock;
use tokio::io::BufReader;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::{Duration, Instant};

pub async fn handle_stream(socket: TcpStream, global_state: GlobalState) {
    // get the address of the client
    let address = match socket.peer_addr() {
        Ok(addr) => addr,
        Err(_) => {
            // bye
            return;
        }
    };

    // create the internal communication channels
    let (sh_sender, mut sh_receiver) = unbounded_channel::<ic::SHBound>();

    // Using a Buffered Reader may increase the performance significantly
    let mut socket = BufReader::new(socket);
    let mut buffer = Vec::new();

    let mut client_protocol = 0; // will be set when the client sends the handshake packet

    let mut next_keepalive = Instant::now() + Duration::from_secs(5);
    let mut last_keepalive_received = Instant::now();

    let player_id_in_world = RwLock::new(None);

    // scopeguard
    defer! {
        if let Some(id) = *player_id_in_world.read().unwrap() {
            // if the player is still in some world, send them a message telling about the disconnection
            global_state.w_login.send(ic::WBound::RemovePlayer(id)).unwrap();
        }
    }

    let mut state = 0; // 0 - handshake, 1 - status, 2 - login, 3 - play
    loop {
        // tokio::select - whichever arrives first: SHBound messages from other threads or input from the client
        // also the timer to send the keepalive packets
        tokio::select!(
            byte = socket.read_u8() => {
                // read the rest of the VarInt
                let mut number = match byte {
                    Err(_) => {
                        // sad
                        return;
                    },
                    Ok(b) => b,
                };
                let mut i = 0;
                let mut length: i32 = 0;
                loop {
                    let value = (number & 0b01111111) as i32;
                    length = length | (value << (7 * i));

                    if (number & 0b10000000) == 0 {
                        break;
                    }

                    number = match socket.read_u8().await {
                        Err(_) => {
                            // oh no!
                            return;
                        },
                        Ok(b) => b,
                    };
                    i += 1;
                }


                // read the rest of the packet
                let packet = match read_packet(&mut socket, &mut buffer, state, length as usize, global_state.compression_treshold).await {
                    Err(_) => {
                        // another one lost ;(
                        return;
                    }
                    Ok(p) => p,
                };

                match packet {
                    ServerBound::Handshake(protocol, _ip, _port, next_state) => {
                        if state != 0 {
                            // wrong state buddy
                            continue;
                        }
                        client_protocol = protocol.0;
                        if next_state.0 != 1 && next_state.0 != 2 {
                            // The only other 2 valid states are 0 which is the current one
                            // and 3 which is play, and can only be moved on to from the login state
                            // therefore this packet doesn't make sense.
                            return;
                        }
                        state = next_state.0;
                    }
                    ServerBound::StatusPing(number) => {
                        let packet = ClientBound::StatusPong(number);
                        if let Err(_) = write_packet(&mut socket, &mut buffer, packet, -1).await {
                            // bro... :((
                            return;
                        }
                    }
                    ServerBound::StatusRequest => {
                        let supported = global_state.description.lock().await;
                        let unsupported = crate::chat_parse::parse_json(
                            format!("§4Your Minecraft version is §lnot supported§r§4.\n§c§lThe server §r§cis running §b§l{}§r§c.", crate::VERSION_NAME)
                        );

                        let packet = ClientBound::StatusResponse(
                            to_string(&json!({
                                "version": {
                                    "name": crate::VERSION_NAME,
                                    "protocol": client_protocol,
                                },
                                "players": {
                                    "max": &*global_state.max_players.lock().await,
                                    "online": -1095,
                                    "sample": &*global_state.player_sample.lock().await,
                                },
                                "description": if crate::SUPPORTED_PROTOCOL_VERSIONS.iter().any(|&i| i==client_protocol) {
                                        &*supported
                                    } else {
                                        &unsupported
                                    },
                                "favicon": &*global_state.favicon.lock().await,
                            }))
                            .unwrap(),
                        );
                        if let Err(_) = write_packet(&mut socket, &mut buffer, packet, -1).await {
                            // dude no!!
                            return;
                        }
                    }
                    ServerBound::LoginStart(username) => {
                        // check if version is supported
                        if !crate::SUPPORTED_PROTOCOL_VERSIONS.iter().any(|&i| i==client_protocol) {
                            let packet = ClientBound::LoginDisconnect(
                                chat_parse(format!("§4Your Minecraft version is §lnot supported§r§4.\n§c§lThe server §r§cis running §b§l{}§r§c.", crate::VERSION_NAME)),
                            );
                            let _ = write_packet(&mut socket, &mut buffer, packet, -1).await;
                            return;
                        }

                        // TODO: check if anyone is already playing with this username
                        if false {
                            let packet = ClientBound::LoginDisconnect(
                                chat_parse("§c§lSomeone is already playing with this username!".to_string()),
                            );
                            let _ = write_packet(&mut socket, &mut buffer, packet, -1).await;
                            return;
                        }

                        // set compression if non-negative
                        if global_state.compression_treshold >= 0 {
                            let packet = ClientBound::SetCompression(VarInt(global_state.compression_treshold as i32));
                            if let Err(_) = write_packet(&mut socket, &mut buffer, packet, -1).await {
                                return;
                            }
                        }

                        // everything's alright, come in
                        let packet = ClientBound::LoginSuccess(0, username.clone());
                        if let Err(_) = write_packet(&mut socket, &mut buffer, packet, global_state.compression_treshold).await {
                            return;
                        }

                        // since the keepalives are going to start being sent, reset the timeout timer
                        last_keepalive_received = Instant::now();
                        state = 3;

                        // add the player to the login world
                        global_state.w_login.send(ic::WBound::AddPlayer(username, sh_sender.clone())).unwrap();
                    }
                    ServerBound::KeepAlive(_) => {
                        // Reset the timeout timer
                        last_keepalive_received = Instant::now();
                    }
                    other => {
                        if let Some(id) = *player_id_in_world.read().unwrap() {
                            // TODO should send to the world currently in, not just w_login
                            global_state.w_login.send(ic::WBound::Packet(id, other)).unwrap();
                        }
                    }
                }
            },
            message = sh_receiver.recv() => {
                let message = match message {
                    None => {
                        // There are no more senders to this channels
                        //
                        // As of writing this I am not sure if/when
                        // should this happen and how to properly handle it
                        // so I'm just going to drop the connection.
                        return;
                    }
                    Some(m) => m,
                };

                match message {
                    ic::SHBound::Packet(packet) => {
                        if let Err(_) = write_packet(&mut socket, &mut buffer, packet, global_state.compression_treshold).await {
                            return;
                        }
                    }
                    ic::SHBound::AssignId(id) => {
                        *player_id_in_world.write().unwrap() = Some(id);
                    }
                }
            },
            _ = tokio::time::sleep_until(next_keepalive) => {
                if state == 3 {
                    // first check if the connection hasn't already timed out
                    if Instant::now().duration_since(last_keepalive_received).as_secs_f32() > 30.0 {
                        return;
                    }
                    // send the keep alive packet
                    let packet = ClientBound::KeepAlive(0);
                    if let Err(_) = write_packet(&mut socket, &mut buffer, packet, global_state.compression_treshold).await {
                        return;
                    }
                }
                next_keepalive += Duration::from_secs(5);
            },
        );
    }
}

async fn read_packet(
    socket: &mut BufReader<TcpStream>,
    buffer: &mut Vec<u8>,
    state: i32,
    length: usize,
    compression_treshold: i32,
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
            for _ in 0..(length - uncompressed_size.size() as usize) {
                // this probably isn't really good but since the ZlibDecoder can't read from an async stream directly
                // I will just feed it bytes one by one
                let mut byte = [0];
                socket.read_exact(&mut byte).await?;
                decoder.write_all(&byte).unwrap();
            }
            decoder.finish().unwrap();
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
    compression_treshold: i32,
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
            encoder.finish().unwrap();
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
            VarInt(uncompressed_length as i32 + 1).write(socket).await?; // + 1 because the following VarInt is counter too and it's always 1 byte since it's 0
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
