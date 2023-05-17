#[derive(Debug, PartialEq, Clone)]
pub enum LegacyPing {
    Simple,
    WithData {
        protocol: u8,
        hostname: String,
        port: u16,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub struct LegacyPingResponse {
    pub motd: String,
    pub online: String,
    pub max_players: String,
    // The following fields are only available on 1.6 legacy ping:
    pub protocol: String,
    pub version: String,
}
