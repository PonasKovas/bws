use super::ConnCtx;
use crate::Server;
use std::{io, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    task::{block_in_place, spawn_blocking},
};

#[derive(Default, Debug)]
pub struct LegacyPingPayload {
    pub protocol: u8,
    pub hostname: String,
    pub port: u16,
}

/// Keep in mind that a valid response has a length limit of 256 characters
#[derive(Debug, Default)]
pub struct LegacyPingResponse {
    pub motd: String,
    pub online: String,
    pub max_players: String,
    /// Only available on 1.4-1.6 legacy ping
    pub protocol: String,
    /// Only available on 1.4-1.6 legacy ping
    pub version: String,
}

enum PingType {
    Pre1_4,
    Pre1_6,
    Pre1_7,
}

// returns true if legacy ping detected and handled
pub(crate) async fn handle(
    server: Arc<Server>,
    ctx: &mut ConnCtx,
) -> Result<bool, Box<dyn std::error::Error>> {
    let (ping_type, payload) = match ctx.stream.fill_buf().await? {
        [0xFE] => (PingType::Pre1_4, Default::default()), // Legacy ping before 1.4
        [0xFE, 0x01] => (PingType::Pre1_6, Default::default()), // Legacy ping before 1.6
        [0xFE, 0x01, 0xFA, ..] => {
            // Legacy ping 1.6
            let payload = read_1_6(ctx).await?;

            (PingType::Pre1_7, payload)
        }
        _ => {
            // Not a legacy ping
            return Ok(false);
        }
    };

    let mut response = None;

    // Just in case any handlers decide to block or do something inappropriate
    block_in_place(|| {
        for handler in &server.global_events.legacy_ping {
            handler(server.clone(), ctx.id, &payload, &mut response);
        }
    });

    if let Some(res) = response {
        write_response(ctx, ping_type, res).await?;
    }

    Ok(true)
}

async fn write_response(
    ctx: &mut ConnCtx,
    ping_type: PingType,
    response: LegacyPingResponse,
) -> std::io::Result<()> {
    // Validate length to help debug invalid packets

    if response.motd.chars().count()
        + response.online.chars().count()
        + response.max_players.chars().count()
        + response.protocol.chars().count()
        + response.version.chars().count()
        + 4 // delimiters count too.. (2 in case of pre-1.4, but we want to support all)
        + 3 // packet prefix (only 1.4-1.6)
        > 256
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Legacy ping response too long",
        ));
    }

    match ping_type {
        PingType::Pre1_4 => write_pre_1_4(ctx, response).await,
        PingType::Pre1_6 | PingType::Pre1_7 => write_pre_1_7(ctx, response).await,
    }
}

/// Writes response for pre-1.4 legacy ping
async fn write_pre_1_4(ctx: &mut ConnCtx, response: LegacyPingResponse) -> std::io::Result<()> {
    let payload = format!(
        "{}§{}§{}",
        response.motd, response.online, response.max_players
    );

    let len = payload.chars().count(); // lol...

    ctx.buf.push(0xFF); // packet ID
    ctx.buf.extend_from_slice(&(len as u16).to_be_bytes()); // length in characters
    ctx.buf
        .extend(payload.encode_utf16().flat_map(|c| c.to_be_bytes())); // payload

    ctx.stream.write_all(&ctx.buf).await
}

/// Writes response for 1.4-1.6 legacy ping
async fn write_pre_1_7(ctx: &mut ConnCtx, response: LegacyPingResponse) -> std::io::Result<()> {
    ctx.buf.clear();
    ctx.buf.push(0xFF); // packet ID
    ctx.buf.extend_from_slice(&[0x00, 0x00]); // placeholder for length
    ctx.buf
        .extend_from_slice(&[0x00, 0xA7, 0x00, 0x31, 0x00, 0x00]); // and some constant values

    // payload
    for s in [
        &response.protocol,
        &response.version,
        &response.motd,
        &response.online,
        &response.max_players,
    ] {
        ctx.buf
            .extend(s.encode_utf16().flat_map(|c| c.to_be_bytes()));
        ctx.buf.extend_from_slice(&[0x00, 0x00]); // separation
    }
    ctx.buf.truncate(ctx.buf.len() - 2); // remove trailing 0x00 0x00

    let chars = (ctx.buf.len() - 3) / 2;

    ctx.buf[1..3].copy_from_slice(&(chars as u16).to_be_bytes()); // Length in characters

    ctx.stream.write_all(&ctx.buf).await
}

/// Reads payload from a 1.6 legacy ping
async fn read_1_6(ctx: &mut ConnCtx) -> std::io::Result<LegacyPingPayload> {
    // consume first 27 bytes which are always the same
    ctx.buf.resize(27, 0);
    ctx.stream.read_exact(&mut ctx.buf).await?;

    let hostname_len = ctx.stream.read_u16().await? - 7;
    let protocol = ctx.stream.read_u8().await?;

    ctx.stream.read_u16().await?; // hostname length again...

    ctx.buf.resize(hostname_len as usize, 0);
    ctx.stream.read_exact(&mut ctx.buf).await?;
    let hostname = String::from_utf16_lossy(
        &ctx.buf
            .chunks(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>(),
    );

    let port = ctx.stream.read_i32().await? as u16;

    Ok(LegacyPingPayload {
        protocol,
        hostname,
        port,
    })
}
