use crate::chat_parse;
use crate::datatypes::*;
use crate::global_state::PStream;
use crate::internal_communication::WBound;
use crate::internal_communication::WReceiver;
use crate::internal_communication::WSender;
use crate::packets::{ClientBound, TitleAction};
use crate::GLOBAL_STATE;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use log::{debug, error, info, warn};
use sha2::{Digest, Sha256};
use slab::Slab;
use std::collections::HashMap;
use std::env::Vars;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::spawn;
use tokio::sync::mpsc::unbounded_channel;
use tokio::time::sleep;
use tokio::time::Instant;

const ACCOUNTS_FILE: &str = "accounts.bwsdata";

pub struct LoginWorld {
    players: HashMap<usize, (String, PStream)>, // username and stream
    accounts: HashMap<String, String>,          // username -> SHA256 hash of password
    login_message: Chat,
    register_message: Chat,
}

impl LoginWorld {
    // might fail since this interacts with the filesystem for the accounts data
    pub fn new() -> Result<Self> {
        // sadly this has to be sync
        use std::fs::File;
        use std::io::BufReader;
        // read the accounts data
        let mut accounts = HashMap::new();
        if Path::new(ACCOUNTS_FILE).exists() {
            // read the data
            let f =
                File::open(ACCOUNTS_FILE).context(format!("Failed to open {}.", ACCOUNTS_FILE))?;

            let mut lines = BufReader::new(f).lines();
            for line in lines {
                let line = line.context(format!("Error reading {}.", ACCOUNTS_FILE))?;
                let mut iterator = line.split(' ');

                let username = iterator
                    .next()
                    .context(format!("Incorrect {} format.", ACCOUNTS_FILE))?;
                let password_hash = iterator
                    .next()
                    .context(format!("Incorrect {} format.", ACCOUNTS_FILE))?;

                accounts.insert(username.to_string(), password_hash.to_string());
            }
        } else {
            // create the file
            File::create(ACCOUNTS_FILE)?;
        }

        Ok(LoginWorld {
            players: HashMap::new(),
            accounts,
            login_message: chat_parse("§aType §6/login §3<password> §ato continue"),
            register_message: chat_parse(
                "§aType §6/register §3<password> <password again> §ato continue",
            ),
        })
    }
    pub async fn run(&mut self, w_receiver: WReceiver) {
        let mut counter = 0;
        loop {
            let start_of_tick = Instant::now();

            // first - process all WBound messages on the channel
            // process_wbound_messages(&mut self, &mut w_receiver);

            self.tick(counter);

            // and then simulate the game

            // wait until the next tick, if needed
            sleep(
                Duration::from_nanos(1_000_000_000 / 20)
                    .saturating_sub(Instant::now().duration_since(start_of_tick)),
            )
            .await;
            counter += 1;
        }
    }
}

impl LoginWorld {
    pub async fn save_accounts(&self) -> Result<()> {
        let mut f = File::create(ACCOUNTS_FILE).await?;

        for account in &self.accounts {
            // I wish to apologize for the readability of the following statement
            #[rustfmt::skip]
            f.write_all(account.0.as_bytes()).await.and(
                f.write_all(b" ").await.and(
                    f.write_all(account.1.as_bytes()).await.and(
                        f.write_all(b"\n").await
                    )
                ),
            ).context(format!("Couldn't write to {}", ACCOUNTS_FILE))?;
        }

        Ok(())
    }
    fn tick(&mut self, counter: u128) {
        info!("counter: {}", counter);
    }
}

pub fn start() -> Result<WSender> {
    let (w_sender, w_receiver) = unbounded_channel::<WBound>();

    let mut world = LoginWorld::new()?;

    spawn(async move {
        world.run(w_receiver).await;
    });

    Ok(w_sender)
}
