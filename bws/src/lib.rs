use futures::Future;
use graceful_exit::GracefulExit;
use networking::{Conn, ConnCtx};
use slab::Slab;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    io::BufReader,
    net::TcpListener,
    select,
    sync::{broadcast, mpsc::unbounded_channel, RwLock},
};
use tracing::error;

pub use networking::legacy_ping::{LegacyPingPayload, LegacyPingResponse};

mod networking;

pub struct Server {
    connections: RwLock<Slab<Conn>>,
    graceful_exit: GracefulExit,
    pub global_events: GlobalEvents,
}

#[derive(Default)]
pub struct GlobalEvents {
    pub legacy_ping: Vec<
        fn(
            server: Arc<Server>,
            id: usize,
            payload: &LegacyPingPayload,
            response: &mut Option<LegacyPingResponse>,
        ),
    >,
}

impl Server {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(Slab::new()),
            graceful_exit: GracefulExit::new(),
            global_events: Default::default(),
        }
    }
    pub async fn run(self, tcp_listener: TcpListener) -> Result<(), Box<dyn std::error::Error>> {
        let server = Arc::new(self);

        // todo create initial worlds and start ticking them here probably

        loop {
            let (socket, addr) = select! {
                s = tcp_listener.accept() => s,
                _ = server.graceful_exit.wait_for_exit() => {
                    return Ok(());
                },
            }?;

            socket.set_nodelay(true)?;

            let (input_writer, input_reader) = broadcast::channel(16);
            let (output_writer, output_reader) = unbounded_channel();

            let id = server.connections.write().await.insert(Conn {
                addr: addr.clone(),
                input: input_reader,
                output: output_writer,
            });

            let server = server.clone();
            tokio::spawn(async move {
                // handle connection
                if let Err(e) = networking::handle_new_conn(
                    server,
                    ConnCtx {
                        id,
                        stream: BufReader::new(socket),
                        addr,
                        input: input_writer,
                        output: output_reader,
                        buf: Vec::new(),
                    },
                )
                .await
                {
                    error!("Stream error: {e:?}");
                }
            });
        }
    }
}
