pub mod lobby;
pub mod login;

use crate::chat_parse;
use crate::datatypes::*;
use crate::global_state;
use crate::global_state::Player;
use crate::internal_communication::SHSender;
use crate::internal_communication::{SHBound, WBound, WReceiver, WSender};
use crate::packets::{ClientBound, ServerBound, TitleAction};
use crate::GLOBAL_STATE;
use anyhow::Result;
use futures::future::FutureExt;
use log::{debug, error, info, warn};
use serde_json::{json, to_string};
use std::{
    thread::Builder,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::error::SendError;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::unconstrained,
};

// The trait that will be needed to be implemented to create types of worlds (login, lobby, game and so on)
pub trait World: Sized {
    // contains only the most basic methods, will add more later as needed

    // the main method, takes over the thread and runs the world
    fn run(mut self, mut w_receiver: WReceiver) {
        let mut counter = 0;
        loop {
            let start_of_tick = Instant::now();

            // first - process all WBound messages on the channel
            process_wbound_messages(&mut self, &mut w_receiver);

            self.tick(counter);

            // and then simulate the game

            // wait until the next tick, if needed
            std::thread::sleep(
                Duration::from_nanos(1_000_000_000 / 20)
                    .saturating_sub(Instant::now().duration_since(start_of_tick)),
            );
            counter += 1;
        }
    }
    // is called every tick
    fn tick(&mut self, counter: u64);
    // should return the name of the world, which doesn't have to be unique
    // but should only contain [a-z0-9/._-] characters
    fn get_world_name(&self) -> &str;
    // Adds a player to the world and the world starts sending packets
    fn add_player(&mut self, id: usize) -> Result<()>;
    // removes the player from memory
    fn remove_player(&mut self, id: usize);
    // sends a SHBound message to the SHSender of the specified player
    // panics if no player with the given ID is in the world
    fn sh_send(&self, id: usize, message: SHBound) -> Result<()>;
    // called when players type something in the chat. Could be a command
    fn chat(&mut self, id: usize, message: String) -> Result<()>;
    // should return the uesername of the given player
    fn username(&self, id: usize) -> Result<&str>;
    // disconnectes the player from the server.
    fn disconnect(&self, id: usize) -> Result<()> {
        self.sh_send(id, SHBound::Disconnect)?;
        Ok(())
    }
    fn is_fixed_time(&self) -> Option<i64>;
}

pub fn start<W: 'static + World + Send>(world: W) -> WSender {
    let (w_sender, w_receiver) = unbounded_channel::<WBound>();

    Builder::new()
        .name(format!("'{}' world thread", world.get_world_name()))
        .spawn(move || {
            world.run(w_receiver);
        })
        .unwrap();

    w_sender
}

fn process_wbound_messages<W: World>(world: &mut W, w_receiver: &mut WReceiver) {
    loop {
        // Tries executing the future exactly once, without forcing it to yield earlier (because non-cooperative multitasking).
        // If it returns Pending, then break the whole loop, because that means there
        // are no more messages queued up at this moment.
        let message = match unconstrained(w_receiver.recv()).now_or_never().flatten() {
            Some(m) => m,
            None => break,
        };

        match message {
            WBound::AddPlayer(id) => {
                // Request to add the player to this world
                if let Err(e) = world.add_player(id) {
                    error!("Couldn't add player to world: {}", e);
                    continue;
                }
            }
            WBound::RemovePlayer(id) => {
                world.remove_player(id);
            }
            WBound::Packet(id, packet) => match packet {
                ServerBound::ChatMessage(message) => {
                    if let Err(e) = world.chat(id, message) {
                        error!("Error handling chat message from {}: {}", id, e);
                    }
                }
                _ => {
                    info!("from {}: {:?}", id, packet);
                }
            },
            _ => {}
        }
    }
}
