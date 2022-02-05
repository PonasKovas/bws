use anyhow::Result;
use bws_plugin_interface::global_state::GState;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub async fn handle_connection(gstate: GState, mut conn: (TcpStream, SocketAddr)) -> Result<()> {
    conn.0.write_all(b"bws").await?;

    Ok(())
}
