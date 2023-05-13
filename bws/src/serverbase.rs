use std::net::SocketAddr;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::graceful_shutdown::ShutdownSystem;
use protocol::packets::{ClientBound, LegacyPing, LegacyPingResponse, SBHandshake, ServerBound};
use protocol::{FromBytes, ToBytes};
use tokio::io::{AsyncBufReadExt, BufReader, ReadBuf};
use tokio::sync::mpsc::{self, Sender};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    runtime::Handle,
};
use tracing::{debug, error, info, instrument};

/// Data storage required to operate a base server
#[derive(Debug)]
pub struct ServerBaseStore {
    pub shutdown: ShutdownSystem,
}

impl ServerBaseStore {
    pub fn new() -> Self {
        Self {
            shutdown: ShutdownSystem::new(),
        }
    }
}

/// Represents basic server capabilities, such as listening on a TCP port and handling connections, managing worlds
pub trait ServerBase: Sized + Sync + Send + 'static {
    fn store(&self) -> &ServerBaseStore;

    fn legacy_ping(&self, _packet: LegacyPing) -> Option<LegacyPingResponse> {
        Some(LegacyPingResponse {
            motd: format!("A BWS server"),
            online: format!("0"),
            max_players: format!("1400"),
            protocol: format!("127"),
            version: format!("BWS"),
        })
    }
}

// TODO error type?
pub fn run<S: ServerBase>(server: S, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    Handle::current().block_on(async {
        let server = Arc::new(server);

        let _shutdown_guard = server.store().shutdown.guard();

        let listener = TcpListener::bind(("127.0.0.1", port)).await?;

        tokio::select! {
            _ = server.store().shutdown.wait_for_shutdown() => {},
            _ = serve(server.clone(), listener) => {},
        }

        Ok(())
    })
}

async fn serve<S: ServerBase>(
    server: Arc<S>,
    listener: TcpListener,
) -> Result<(), tokio::io::Error> {
    loop {
        let (socket, addr) = listener.accept().await?;
        socket.set_nodelay(true)?;

        let server = server.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_conn(server, BufReader::new(socket), addr).await {
                error!("{}", e);
            }
        });
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum State {
    Handshake,
    Status,
    Login,
    Play,
}

#[derive(Clone, Debug, PartialEq)]
enum ConnControl {
    SendPacket(ClientBound),
    Disconnect,
}

async fn handle_conn<S: ServerBase>(
    server: Arc<S>,
    mut socket: BufReader<TcpStream>,
    _addr: SocketAddr,
) -> Result<(), tokio::io::Error> {
    let _shutdown_guard = server.store().shutdown.guard();

    let (output_sender, mut output_receiver) = mpsc::channel(10);

    info!("Connection!");

    let mut buf = Vec::new();

    let mut state = State::Handshake;
    let mut control = ControlFlow::Continue(());

    // Need to handle special case of legacy ping which uses different protocols...
    if handle_legacy_ping(server.as_ref(), &mut socket, &mut buf).await? {
        return Ok(());
    }

    loop {
        if control.is_break() {
            break;
        }

        tokio::select! {
            packet = read_packet(&mut socket, &mut state, &mut buf, &mut control) => {
                info!("received: {:?}", packet?);
            },
            Some(conn_control) = output_receiver.recv() => {
                match conn_control {
                    ConnControl::SendPacket(_packet) => {
                        // send_packet(&mut socket, &mut buf, packet).await?;
                    },
                    ConnControl::Disconnect => {
                        control = ControlFlow::Break(());
                    },
                }
            },
            _ = server.store().shutdown.wait_for_shutdown() => {
                socket.write_all(b"Sorry gotta go!..\n").await?;
                break;
            },
        }
    }

    Ok(())
}

// Returns true if legacy ping detected and handled
async fn handle_legacy_ping<S: ServerBase>(
    server: &S,
    socket: &mut BufReader<TcpStream>,
    buf: &mut Vec<u8>,
) -> std::io::Result<bool> {
    match *socket.fill_buf().await? {
        [0xFE] | [0xFE, 0x01] => {
            // Legacy ping before 1.6
            /////////////////////////

            if let Some(response) = server.legacy_ping(LegacyPing::Simple) {
                // Write response
                let payload = format!(
                    "{}§{}§{}",
                    response.motd, response.online, response.max_players
                );
                buf.push(0xFF); // packet ID
                buf.extend_from_slice(&(payload.chars().count() as u16).to_be_bytes()); // length in characters
                buf.extend(payload.encode_utf16().flat_map(|c| c.to_be_bytes())); // payload

                socket.write_all(buf).await?;
            }
            Ok(true)
        }
        [0xFE, 0x01, 0xFA, ..] => {
            // Legacy ping 1.6
            //////////////////

            // consume first 27 bytes which are always the same
            buf.resize(27, 0);
            socket.read_exact(buf).await?;

            let hostname_len = socket.read_u16().await? - 7;
            let protocol = socket.read_u8().await?;

            socket.read_u16().await?; // hostname length again...

            buf.resize(hostname_len as usize, 0);
            socket.read_exact(buf).await?;
            let hostname = String::from_utf16_lossy(
                &buf.chunks(2)
                    .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                    .collect::<Vec<_>>(),
            );

            let port = socket.read_i32().await? as u16;

            if let Some(response) = server.legacy_ping(LegacyPing::WithData {
                protocol,
                hostname,
                port,
            }) {
                // Write response
                buf.clear();
                buf.push(0xFF); // packet ID
                buf.extend_from_slice(&[0x00, 0x00]); // placeholder for length
                buf.extend_from_slice(&[0x00, 0xA7, 0x00, 0x31, 0x00, 0x00]); // and some constant values

                // payload
                for s in [
                    response.protocol,
                    response.version,
                    response.motd,
                    response.online,
                    response.max_players,
                ] {
                    buf.extend(s.encode_utf16().flat_map(|c| c.to_be_bytes()));
                    buf.extend_from_slice(&[0x00, 0x00]); // separation
                }
                let len = (buf.len() - 5) / 2;
                buf[1..3].copy_from_slice(&(len as u16).to_be_bytes()); // Length

                buf.truncate(buf.len() - 2); // remove trailing 0x00 0x00

                socket.write_all(buf).await?;
            }

            Ok(true)
        }
        _ => Ok(false),
    }
}

#[instrument(skip(socket, buf, control))]
async fn read_packet(
    socket: &mut BufReader<TcpStream>,
    state: &mut State,
    buf: &mut Vec<u8>,
    control: &mut ControlFlow<(), ()>,
) -> std::io::Result<ServerBound> {
    buf.clear();

    let packet_length = read_packet_length(socket).await?;

    buf.resize(packet_length as usize, 0x00);
    socket.read_exact(buf).await?;

    match *state {
        State::Handshake => {
            let packet = SBHandshake::read_from(&mut &buf[..]);

            info!("Received handshake packet: {:?}", packet);

            todo!()
        }
        State::Status => todo!(),
        State::Login => todo!(),
        State::Play => todo!(),
    }
}

/// Async read varint
async fn read_packet_length(socket: &mut BufReader<TcpStream>) -> std::io::Result<i32> {
    let mut num_read = 0; // Count of bytes that have been read
    let mut result = 0i32; // The VarInt being constructed

    loop {
        // VarInts are at most 5 bytes long.
        if num_read == 5 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "VarInt is too big",
            ));
        }

        // Read a byte
        let byte = socket.read_u8().await?;

        // Extract the 7 lower bits (the data bits) and cast to i32
        let value = (byte & 0b0111_1111) as i32;

        // Shift the data bits to the correct position and add them to the result
        result |= value << (7 * num_read);

        num_read += 1;

        // If the high bit is not set, this was the last byte in the VarInt
        if (byte & 0b1000_0000) == 0 {
            break;
        }
    }

    Ok(result)
}
