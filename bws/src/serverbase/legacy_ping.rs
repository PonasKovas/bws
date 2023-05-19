use std::net::SocketAddr;

use crate::serverbase::ServerBase;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing::{error, info, instrument};

#[derive(Debug, PartialEq, Clone)]
pub enum LegacyPing {
    Simple,
    WithData {
        protocol: u8,
        hostname: String,
        port: u16,
    },
}

/// Keep in mind that a valid response has a length limit of 256 characters
#[derive(Debug, PartialEq, Clone)]
pub struct LegacyPingResponse {
    motd: String,
    online: String,
    max_players: String,
    // The following fields are only available on 1.4-1.6 legacy ping:
    protocol: String,
    version: String,
}

impl LegacyPingResponse {
    pub fn new() -> Self {
        Self {
            motd: format!(""),
            online: format!(""),
            max_players: format!("0"),
            protocol: format!(""),
            version: format!(""),
        }
    }
    pub fn motd(mut self, motd: String) -> Self {
        self.motd = motd;

        self
    }
    /// panics if `online` bigger than [`i32::MAX`] (`2_147_483_647`)
    pub fn online(mut self, online: u32) -> Self {
        assert!(online <= i32::MAX as u32);

        self.online = format!("{online}");

        self
    }
    /// panics if `max` bigger than [`i32::MAX`] (`2_147_483_647`)
    pub fn max(mut self, max: u32) -> Self {
        assert!(max <= i32::MAX as u32);

        self.max_players = format!("{max}");

        self
    }
    /// Only 1.6 clients receive this data
    pub fn protocol(mut self, protocol: i32) -> Self {
        self.protocol = format!("{protocol}");

        self
    }
    /// Only 1.6 clients receive this data
    pub fn version(mut self, version: String) -> Self {
        self.version = format!("{version}");

        self
    }
}

// Returns true if legacy ping detected and handled
#[instrument(skip(server, socket, buf))]
pub(super) async fn handle<S: ServerBase>(
    server: &S,
    socket: &mut BufReader<TcpStream>,
    addr: &SocketAddr,
    buf: &mut Vec<u8>,
) -> std::io::Result<bool> {
    match *socket.fill_buf().await? {
        [0xFE] => {
            // Legacy ping before 1.4
            /////////////////////////

            if let Some(response) = server.legacy_ping(addr, LegacyPing::Simple) {
                // Write response
                let payload = format!(
                    "{}§{}§{}",
                    response.motd, response.online, response.max_players
                );

                let len = payload.chars().count();

                if len > 256 {
                    error!(
                        "Sending bad legacy ping response: too long:\n{:?}",
                        response
                    );
                }

                buf.push(0xFF); // packet ID
                buf.extend_from_slice(&(len as u16).to_be_bytes()); // length in characters
                buf.extend(payload.encode_utf16().flat_map(|c| c.to_be_bytes())); // payload

                socket.write_all(buf).await?;
            }
            Ok(true)
        }
        [0xFE, 0x01] => {
            // Legacy ping 1.4-1.5
            //////////////////////

            if let Some(response) = server.legacy_ping(addr, LegacyPing::Simple) {
                // Write response
                send_14_16_response(socket, buf, &response).await?;
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

            if let Some(response) = server.legacy_ping(
                addr,
                LegacyPing::WithData {
                    protocol,
                    hostname,
                    port,
                },
            ) {
                // Write response
                send_14_16_response(socket, buf, &response).await?;
            }

            Ok(true)
        }
        _ => Ok(false),
    }
}

// 1.4-1.6 response format
async fn send_14_16_response(
    socket: &mut BufReader<TcpStream>,
    buf: &mut Vec<u8>,
    response: &LegacyPingResponse,
) -> std::io::Result<()> {
    buf.clear();
    buf.push(0xFF); // packet ID
    buf.extend_from_slice(&[0x00, 0x00]); // placeholder for length
    buf.extend_from_slice(&[0x00, 0xA7, 0x00, 0x31, 0x00, 0x00]); // and some constant values

    // payload
    for s in [
        &response.protocol,
        &response.version,
        &response.motd,
        &response.online,
        &response.max_players,
    ] {
        buf.extend(s.encode_utf16().flat_map(|c| c.to_be_bytes()));
        buf.extend_from_slice(&[0x00, 0x00]); // separation
    }
    buf.truncate(buf.len() - 2); // remove trailing 0x00 0x00

    let chars = (buf.len() - 3) / 2;

    if chars > 256 {
        error!(
            "Sending bad legacy ping response: too long:\n{:?}",
            response
        );
    }

    buf[1..3].copy_from_slice(&(chars as u16).to_be_bytes()); // Length

    socket.write_all(buf).await?;

    Ok(())
}
