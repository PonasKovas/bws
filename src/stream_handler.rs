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
use tokio::sync::{mpsc::channel, Mutex};

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
    let (sh_sender, mut sh_receiver) = channel::<ic::SHBound>(10);
    let (mut w_sender, w_receiver) = channel::<ic::WBound>(10);

    // Using a Buffered Reader may increase the performance significantly
    let mut socket = BufReader::new(socket);
    let mut buffer = Vec::new();

    let mut client_protocol = 0;

    let mut state = 0; // 0 - handshake, 1 - status, 2 - login, 3 - play
    loop {
        // tokio::select - whichever arrives first: SHBound messages from other threads or input from the client
        //
        // There is a problem with the current implementation, that is, if the length of the packet is started
        // being read but not finished, since it's done in multiple read calls and is interrupted by the
        // sh_receiver, the stream will be left in a corrupted state.
        // However, this should be very unlikely and probably won't be a real issue.
        tokio::select!(
            length = VarInt::read(&mut socket) => {
                // read the rest of the packet
                let length = match length {
                    Err(_) => {
                        // bruh.. :(
                        return;
                    }
                    Ok(l) => l,
                };
                let packet = match read_packet(&mut socket, &mut buffer, state, length.0 as usize).await {
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
                                "description": &*global_state.description.lock().await,
                                "favicon": &*global_state.favicon.lock().await,
                            }))
                            .unwrap(),
                        ));
                        if let Err(_) = write_packet(&mut socket, &mut buffer, packet).await {
                            // dude no!!
                            return;
                        }
                    }
                    _ => {
                        if state == 2 {
                            let packet = ClientBound::LoginDisconnect(MString(
                                to_string(&chat_parse("§l§4Not implemented yet! :(".to_string())).unwrap(),
                            ));
                            if let Err(_) = write_packet(&mut socket, &mut buffer, packet).await {
                                // nooo...
                                return;
                            }
                            // lol
                            return;
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
                println!("Received SHBound message");
            }
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
