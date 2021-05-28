pub mod login;

use crate::chat_parse;
use crate::datatypes::*;
use crate::internal_communication::SHSender;
use crate::internal_communication::{SHBound, WBound, WReceiver, WSender};
use crate::packets::{ClientBound, ServerBound, TitleAction};
use futures::future::FutureExt;
use serde_json::{json, to_string};
use std::{
    thread::Builder,
    time::{Duration, Instant},
};
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::unconstrained,
};

// The trait that will be needed to be implemented to create types of worlds (login, lobby, game and so on)
pub trait World: Sized {
    // contains only the most basic methods, will add more later as needed

    // the main method, takes over the thread and runs the world
    fn run(mut self, mut w_receiver: WReceiver) {
        loop {
            let start_of_tick = Instant::now();

            // first - process all WBound messages on the channel
            process_wbound_messages(&mut self, &mut w_receiver);

            // and then simulate the game

            // wait until the next tick, if needed
            std::thread::sleep(
                Duration::from_nanos(1_000_000_000 / 20)
                    .saturating_sub(Instant::now().duration_since(start_of_tick)),
            );
        }
    }
    // should return the name of the world, which doesn't have to be unique
    // but should only contain [a-z0-9/._-] characters
    fn get_world_name(&self) -> &str;
    // Should return the dimension of the world, which send to new players
    fn dimension(&self) -> nbt::Blob;
    // is called when new players join
    // should also send the PlayerPositionAndLook packet
    fn add_player(&mut self, username: String, sh_sender: SHSender);
}

pub fn start<W: 'static + World + Send>(world: W) -> WSender {
    let (w_sender, mut w_receiver) = unbounded_channel::<WBound>();

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
            WBound::AddPlayer(username, sh_sender) => {
                // Request to add the player to this world

                let dimension = world.dimension();

                let packet = ClientBound::JoinGame(
                    0,
                    false,
                    0,
                    -1,
                    vec![world.get_world_name().to_string()],
                    dimension,
                    world.get_world_name().to_string(),
                    0,
                    VarInt(20),
                    VarInt(8),
                    false,
                    false,
                    false,
                    true,
                );
                sh_sender.send(SHBound::Packet(packet)).unwrap();

                sh_sender
                    .send(SHBound::Packet(ClientBound::SetBrand("BWS".to_string())))
                    .unwrap();

                world.add_player(username, sh_sender);
            }
            _ => {}
        }
    }
}
