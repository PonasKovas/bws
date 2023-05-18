mod legacy_ping;
mod store;

use base64::Engine;
use legacy_ping::{LegacyPing, LegacyPingResponse};
use protocol::newtypes::NextState;
use protocol::packets::handshake::Handshake;
use protocol::packets::status::{PingResponse, StatusResponse};
use protocol::packets::{CBStatus, ClientBound, SBHandshake, SBStatus, ServerBound};
use protocol::{FromBytes, ToBytes, VarInt};
use serde_json::json;
use std::io::Write;
use std::net::SocketAddr;
use std::ops::ControlFlow;
use std::sync::Arc;
pub use store::ServerBaseStore;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc::{self, Sender};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    runtime::Handle,
};
use tracing::{debug, error, info, instrument};

/// Represents basic server capabilities, such as listening on a TCP port and handling connections, managing worlds
pub trait ServerBase: Sized + Sync + Send + 'static {
    fn store(&self) -> &ServerBaseStore;

    fn legacy_ping(
        &self,
        _client_addr: &SocketAddr,
        _packet: LegacyPing,
    ) -> Option<LegacyPingResponse> {
        Some(LegacyPingResponse {
            motd: format!("BWS: Better World Servers"),
            online: format!("-1"),
            max_players: format!("1400"),
            protocol: format!("127"),
            version: format!("BWS"),
        })
    }
    fn ping(
        &self,
        client_addr: &SocketAddr,
        protocol: i32,
        _address: &str,
        _port: u16,
    ) -> Option<StatusResponse> {
        let favicon = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD
                .encode(include_bytes!("/home/mykolas/Downloads/icon.png"))
        );
        let packet = StatusResponse {
            json: json!({
                "version": {
                    "name": "BWS",
                    "protocol": protocol,
                },
                "players": {
                    "max": 20,
                    "online": -1,
                    "sample": [
                        {
                            "name": "§k§lbetter world servers",
                            "id": "00000000-0000-0000-0000-000000000000"
                        }
                    ]
                },
                "description": [
                    {
                        "text": "BWS: ",
                        "color": "#ffffff",
                        "bold": true,
                    },
                    {
                        "text": "Better World Servers\n",
                        "color": "#ddddff",
                        "bold": false,
                    },
                    {
                        "text": "THERE ARE WORMS UNDER YOUR SKIN",
                        "color": "#333333",
                        "font": "minecraft:alt",
                    }
                ],
                "favicon": favicon,
                "enforcesSecureChat": true
            }),
        };

        Some(packet)
    }
}

/// Accepts connections and spawns tokio tasks for further handling
pub(crate) async fn serve<S: ServerBase>(
    server: Arc<S>,
    listener: TcpListener,
) -> std::io::Result<()> {
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

async fn handle_conn<S: ServerBase>(
    server: Arc<S>,
    mut socket: BufReader<TcpStream>,
    addr: SocketAddr,
) -> Result<(), tokio::io::Error> {
    let _shutdown_guard = server.store().shutdown.guard();

    info!("Connection!");

    let mut buf = Vec::new();

    if legacy_ping::handle(server.as_ref(), &mut socket, &addr, &mut buf).await? {
        // Legacy ping detected and handled
        return Ok(());
    }

    let handshake = tokio::select! {
        packet = read_packet(&mut socket, &mut buf) => {
            match packet? { SBHandshake::Handshake(p) => p, }
        },
        _ = server.store().shutdown.wait_for_shutdown() => { return Ok(()); },
    };

    match handshake.next_state {
        NextState::Status => tokio::select! {
            _ = handle_conn_status(server.as_ref(), &mut socket, &addr, &mut buf, &handshake) => {},
            _ = server.store().shutdown.wait_for_shutdown() => { return Ok(()); },
        },
        NextState::Login => tokio::select! {
            _ = handle_conn_login(&mut socket, &mut buf, &handshake) => {},
            _ = server.store().shutdown.wait_for_shutdown() => {
                // TODO send disconnect package
                return Ok(());
            },
        },
    }

    Ok(())
}

struct NoopWriter;
impl Write for NoopWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

async fn handle_conn_status<S: ServerBase>(
    server: &S,
    socket: &mut BufReader<TcpStream>,
    addr: &SocketAddr,
    buf: &mut Vec<u8>,
    handshake: &Handshake,
) -> std::io::Result<()> {
    loop {
        match read_packet(socket, buf).await? {
            SBStatus::StatusRequest => {
                if let Some(p) = server.ping(
                    addr,
                    handshake.protocol_version.0,
                    &handshake.server_address,
                    handshake.server_port,
                ) {
                    let packet = CBStatus::StatusResponse(p);

                    buf.clear();
                    VarInt(packet.write_to(&mut NoopWriter)? as i32).write_to(buf)?;
                    packet.write_to(buf)?;
                    socket.write_all(buf).await?;
                } else {
                    break Ok(()); // end connection
                }
            }
            SBStatus::PingRequest(r) => {
                let packet = CBStatus::PingResponse(PingResponse { payload: r.payload });

                buf.clear();
                VarInt(packet.write_to(&mut NoopWriter)? as i32).write_to(buf)?;
                packet.write_to(buf)?;
                socket.write_all(buf).await?;

                break Ok(()); // end connection
            }
        }
    }
}

async fn handle_conn_login(
    socket: &mut BufReader<TcpStream>,
    buf: &mut Vec<u8>,
    handshake: &Handshake,
) -> std::io::Result<()> {
    loop {}
}

#[instrument(skip(socket, buf))]
async fn read_packet<P: FromBytes>(
    socket: &mut BufReader<TcpStream>,
    buf: &mut Vec<u8>,
) -> std::io::Result<P> {
    buf.clear();

    let packet_length = read_packet_length(socket).await?;

    buf.resize(packet_length as usize, 0x00);
    socket.read_exact(buf).await?;

    P::read_from(&mut &buf[..])
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
