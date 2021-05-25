use crate::chat_parse;
use crate::datatypes::*;
use crate::internal_communication as ic;
use crate::packets::{ClientBound, ServerBound};
use crate::GlobalState;
use serde_json::{json, to_string};
use std::io::Cursor;
use std::sync::Arc;
use tokio::io::BufReader;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc::unbounded_channel, Mutex};
use tokio::time::{Duration, Instant};

pub async fn handle_stream(socket: TcpStream, global_state: GlobalState) {
    // get the address of the client
    let address = match socket.peer_addr() {
        Ok(addr) => addr,
        Err(_) => {
            // not even worth reporting
            return;
        }
    };

    // create the internal communication channels
    let (sh_sender, mut sh_receiver) = unbounded_channel::<ic::SHBound>();
    let (mut w_sender, w_receiver) = unbounded_channel::<ic::WBound>();

    // Using a Buffered Reader may increase the performance significantly
    let mut socket = BufReader::new(socket);
    let mut buffer = Vec::new();

    let mut client_protocol = 0; // will be set when the client sends the handshake packet

    let mut next_keepalive = Instant::now() + Duration::from_secs(5);
    let mut last_keepalive_received = Instant::now();

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
                let mut length: i64 = 0;
                loop {
                    let value = (number & 0b01111111) as i64;
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
                let packet = match read_packet(&mut socket, &mut buffer, state, length as usize).await {
                    Err(_) => {
                        // another one lost ;(
                        return;
                    }
                    Ok(p) => p,
                };

                println!("{:?}", packet);

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
                        if let Err(_) = write_packet(&mut socket, &mut buffer, packet).await {
                            // bro... :((
                            return;
                        }
                    }
                    ServerBound::StatusRequest => {
                        let supported = global_state.description.lock().await;
                        let unsupported = crate::chat_parse(
                            format!("§4Your Minecraft version is §lnot supported§r§4.\n§c§lThe server §r§cis running §b§l{}§r§c.", crate::VERSION_NAME)
                        );

                        let packet = ClientBound::StatusResponse(MString(
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
                        ));
                        if let Err(_) = write_packet(&mut socket, &mut buffer, packet).await {
                            // dude no!!
                            return;
                        }
                    }
                    ServerBound::LoginStart(username) => {
                        // TODO: check if anyone is already playing with this username
                        if false {
                            let packet = ClientBound::LoginDisconnect(MString(
                                to_string(&chat_parse("§c§lSomeone is already playing with this username!".to_string())).unwrap(),
                            ));
                            let _ = write_packet(&mut socket, &mut buffer, packet).await;
                        }

                        // everything's alright, come in
                        let packet = ClientBound::LoginSuccess(0, username);
                        if let Err(_) = write_packet(&mut socket, &mut buffer, packet).await {
                            return;
                        }

                        // since the keepalives are going to start being sent, reset the timeout timer
                        last_keepalive_received = Instant::now();
                        state = 3;

                        // add the player to the login world
                        // todo
                    }
                    ServerBound::KeepAlive(_) => {
                        // Reset the timeout timer
                        last_keepalive_received = Instant::now();
                    }
                    _ => {
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
                println!("Received SHBound message");
            },
            _ = tokio::time::sleep_until(next_keepalive) => {
                if state == 3 {
                    // first check if the connection hasn't already timed out
                    if Instant::now().duration_since(last_keepalive_received).as_secs_f32() > 30.0 {
                        return;
                    }
                    // send the keep alive packet
                    let packet = ClientBound::KeepAlive(0);
                    if let Err(_) = write_packet(&mut socket, &mut buffer, packet).await {
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
    state: i64,
    length: usize,
) -> tokio::io::Result<ServerBound> {
    buffer.resize(length, 0);

    socket.read_exact(&mut buffer[..]).await?;

    let mut cursor = Cursor::new(&*buffer);

    ServerBound::deserialize(&mut cursor, state)
}

async fn write_packet(
    socket: &mut BufReader<TcpStream>,
    buffer: &mut Vec<u8>,
    packet: ClientBound,
) -> tokio::io::Result<()> {
    buffer.clear();
    packet.serialize(buffer);
    let length = VarInt(buffer.len() as i64);
    length.write(socket).await?;
    socket.write_all(&buffer[..]).await?;

    Ok(())
}
