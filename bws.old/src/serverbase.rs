pub mod legacy_ping;
mod store;

use base64::Engine;
use legacy_ping::{LegacyPing, LegacyPingResponse};
use protocol::newtypes::NextState;
use protocol::packets::handshake::Handshake;
use protocol::packets::login::Disconnect;
use protocol::packets::status::{
    PingResponse, PlayerSample, StatusResponse, StatusResponseBuilder,
};
use protocol::packets::{
    CBLogin, CBStatus, ClientBound, SBHandshake, SBLogin, SBStatus, ServerBound,
};
use protocol::{BString, FromBytes, ToBytes, VarInt};
use serde_json::json;
use sha1::Sha1;
use std::io::Write;
use std::net::SocketAddr;

use std::sync::Arc;
pub use store::ServerBaseStore;
use tokio::io::BufReader;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{error, info, instrument, trace};

/// Represents basic server capabilities, such as listening on a TCP port and handling connections, managing worlds
pub trait ServerBase: Sized + Sync + Send + 'static {
    fn store(&self) -> &ServerBaseStore;

    /// Legacy ping (requested by clients older than 1.7)
    ///
    /// Return `Some` to send a response.
    /// If you don't send a respond, the server will appear as offline in their server list,
    /// but they may still attempt to connect.
    ///
    /// Note: see also [`ping`][Self::ping] for post-1.6 ping.
    fn legacy_ping(
        &self,
        _client_addr: &SocketAddr,
        _packet: LegacyPing,
    ) -> Option<LegacyPingResponse> {
        Some(LegacyPingResponse::new().online(0).max(2_147_483_647))
    }
    /// Server list ping
    ///
    /// Return `Some` to send a response
    /// If you don't respond, the server will appear as offline to the client in their server list,
    /// but they may still try to connect if they want to.
    ///
    /// Note: see also [`legacy_ping`][Self::legacy_ping] for pre-1.7 ping.
    fn ping(
        &self,
        _client_addr: &SocketAddr,
        protocol: i32,
        _address: &str,
        _port: u16,
    ) -> Option<StatusResponse> {
        let packet = StatusResponseBuilder::new(format!("BWS"), protocol)
            .players(
                -1,
                2_147_483_647,
                [["6"; 10].as_slice(), ["2"; 10].as_slice(), ["4";10].as_slice(), ["0";5].as_slice()].into_iter().flatten().cycle().take(63).map(|color| PlayerSample::from_text(format!("§{color}§kbetter world servers. there are worms under your skin pull them out you have to pull them out right now do it they are eating you alive you will feel so much better when you pull them out they are crawling under your skin so slippery and disgusting they are eating your life away pull them out right now or it may be too late grab a knife and cut them out of your body it will hurt for a bit but you will feel so much better when you finish"))).collect::<Vec<_>>(),
            )
            .build();

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
            let ctx = StreamCtx {
                socket: BufReader::new(socket),
                addr,
                buf: Vec::new(),
                cipher: None,
                compression_threshold: None,
            };

            if let Err(e) = handle_conn(server, ctx).await {
                error!("{}", e);
            }
        });
    }
}

async fn handle_conn<S: ServerBase>(
    server: Arc<S>,
    mut ctx: StreamCtx,
) -> Result<(), tokio::io::Error> {
    let _shutdown_guard = server.store().shutdown.guard();

    info!("Connection!");

    if legacy_ping::handle(server.as_ref(), &mut ctx).await? {
        // Legacy ping detected and handled
        return Ok(());
    }

    let handshake = tokio::select! {
        packet = ctx.read_packet() => {
            match packet? { SBHandshake::Handshake(p) => p, }
        },
        _ = server.store().shutdown.wait_for_shutdown() => { return Ok(()); },
    };

    match handshake.next_state {
        NextState::Status => tokio::select! {
            _ = handle_conn_status(server.as_ref(), &mut ctx, &handshake) => {},
            _ = server.store().shutdown.wait_for_shutdown() => { return Ok(()); },
        },
        NextState::Login => tokio::select! {
            _ = handle_conn_login(server.as_ref(), &mut ctx, &handshake) => {},
            _ = server.store().shutdown.wait_for_shutdown() => {
                // TODO send disconnect package
                return Ok(());
            },
        },
    }

    Ok(())
}

async fn handle_conn_status<S: ServerBase>(
    server: &S,
    ctx: &mut StreamCtx,
    handshake: &Handshake,
) -> std::io::Result<()> {
    loop {
        match ctx.read_packet().await? {
            SBStatus::StatusRequest => {
                if let Some(p) = server.ping(
                    &ctx.addr,
                    handshake.protocol_version.0,
                    &handshake.server_address,
                    handshake.server_port,
                ) {
                    trace!("Sending StatusResponse: {p:?}");

                    // Status Response JSON payload can't be longer than 32767
                    // in CHARACTERS (not bytes)
                    if serde_json::to_string(&p.json).unwrap().len() > 32767 {
                        error!("Sending invalid status response: too long.\n{:?}", p);
                    }

                    let packet = CBStatus::StatusResponse(p);

                    ctx.write_packet(&packet).await?;
                } else {
                    break Ok(()); // end connection
                }
            }
            SBStatus::PingRequest(r) => {
                trace!("Sending PingResponse: {r:?}");
                let packet = CBStatus::PingResponse(PingResponse { payload: r.payload });

                ctx.write_packet(&packet).await?;

                break Ok(()); // end connection
            }
        }
    }
}

async fn handle_conn_login<S: ServerBase>(
    server: &S,
    ctx: &mut StreamCtx,
    _handshake: &Handshake,
) -> std::io::Result<()> {
    let start = if let SBLogin::LoginStart(p) = ctx.read_packet().await? {
        p
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Client not following format",
        ));
    };

    use aes::cipher::AsyncStreamCipher;
    use aes::cipher::{generic_array::GenericArray, KeyIvInit};
    use rand::Rng;
    use rsa::pkcs8::EncodePublicKey;

    let mut token = [0u8; 32];
    rand::thread_rng().fill(&mut token[..]);

    let public_key = server
        .store()
        .rsa_keypair
        .to_public_key()
        .to_public_key_der()
        .unwrap()
        .into_vec();

    ctx.write_packet(&CBLogin::EncryptionRequest(
        protocol::packets::login::EncryptionRequest {
            server_id: BString::new("".to_string()).unwrap(),
            public_key: public_key.clone(),
            verify_token: token.to_vec(),
        },
    ))
    .await?;

    let encryption_response = if let SBLogin::EncryptionResponse(p) = ctx.read_packet().await? {
        p
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Client not following format",
        ));
    };

    if &server
        .store()
        .rsa_keypair
        .decrypt(rsa::Pkcs1v15Encrypt, &encryption_response.verify_token)
        .expect("error decrypting verify token")
        != &token
    {
        error!("Verify token incorrect");
        ctx.write_packet(&CBLogin::Disconnect(Disconnect {
            reason: json!("Incorrect verify token"),
        }))
        .await?;
        return Ok(());
    }

    let shared_secret = server
        .store()
        .rsa_keypair
        .decrypt(rsa::Pkcs1v15Encrypt, &encryption_response.shared_secret)
        .expect("error decrypting shared secret");

    let encrypt = cfb8::Encryptor::<aes::Aes128>::new(
        GenericArray::from_slice(&shared_secret),
        GenericArray::from_slice(&shared_secret),
    );
    let decrypt = cfb8::Decryptor::<aes::Aes128>::new(
        GenericArray::from_slice(&shared_secret),
        GenericArray::from_slice(&shared_secret),
    );

    ctx.cipher = Some((encrypt, decrypt));

    use sha1::Digest;

    // Calculate server id hash
    let mut hasher = sha1::Sha1::new();
    hasher.update(&shared_secret);
    hasher.update(&public_key);
    let mut hash = hasher.finalize();

    let negative = hash[0] & 0b1000_0000_u8 != 0;
    if negative {
        // Perform two's complement
        let mut carry = true;
        for i in (0..hash.len()).rev() {
            hash[i] = !hash[i];
            if carry {
                carry = hash[i] == 0xff;
                hash[i] = hash[i].overflowing_add(1).0;
            }
        }
    }
    let url = format!("https://sessionserver.mojang.com/session/minecraft/hasJoined?username={}&serverId={}{hash:x}&ip={}", start.name.to_inner(), if negative { "-" } else { "" }, ctx.addr.ip());

    let body = reqwest::get(&url).await.unwrap().text().await.unwrap();

    info!("res: {body:?}");

    Ok(())
}

struct StreamCtx {
    socket: BufReader<TcpStream>,
    addr: SocketAddr,
    buf: Vec<u8>,
    cipher: Option<(cfb8::Encryptor<aes::Aes128>, cfb8::Decryptor<aes::Aes128>)>,
    compression_threshold: Option<usize>,
}

impl StreamCtx {
    #[instrument(skip(self, packet))]
    async fn write_packet<P: ToBytes + Into<ClientBound>>(
        &mut self,
        packet: &P,
    ) -> std::io::Result<()> {
        struct NoopWriter;
        impl Write for NoopWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        self.buf.clear();

        let len = packet.write_to(&mut NoopWriter)?;
        VarInt(len as i32).write_to(&mut self.buf)?;
        packet.write_to(&mut self.buf)?;

        self.socket.write_all(&self.buf).await?;

        Ok(())
    }

    #[instrument(skip(self))]
    async fn read_packet<P: FromBytes + Into<ServerBound>>(&mut self) -> std::io::Result<P> {
        self.buf.clear();

        let packet_length = read_packet_length(&mut self.socket).await?;

        self.buf.resize(packet_length as usize, 0x00);
        self.socket.read_exact(&mut self.buf).await?;

        P::read_from(&mut &self.buf[..])
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
