use bws::{LegacyPingPayload, LegacyPingResponse, Server};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_ansi(true)
        .init();

    let mut server = Server::new();

    server.global_events.legacy_ping.push(legacy_ping);

    server
        .run(TcpListener::bind(("127.0.0.1", 25565)).await?)
        .await
}

fn legacy_ping(
    server: Arc<Server>,
    id: usize,
    payload: &LegacyPingPayload,
    response: &mut Option<LegacyPingResponse>,
) {
    *response = Some(LegacyPingResponse {
        motd: "a".to_string(),
        online: "0".to_string(),
        max_players: "10".to_string(),
        protocol: "".to_string(),
        version: "".to_string(),
    })
}
