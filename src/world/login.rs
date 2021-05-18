use std::{
    thread::{sleep, Builder},
    time::Duration,
};

use crate::internal_communication::{SHBound, WBound};
use crate::world::{Blocks, Player, World, WorldData};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub fn new() -> UnboundedSender<WBound> {
    let (w_sender, mut w_receiver) = unbounded_channel::<WBound>();
    let w_sender_clone = w_sender.clone();

    Builder::new()
        .name("Login World Thread".to_string())
        .stack_size(6 * 1024 * 1024)
        .spawn(move || {
            let mut world = World {
                data: WorldData {
                    name: "Login".to_string(),
                    blocks: Blocks::default(),
                    players: Vec::new(),
                },
                c_player_join: Box::new(|world| {
                    println!("Player joined");
                    true
                }),
            };

            loop {
                sleep(Duration::from_secs(1));
                (world.c_player_join)(&mut world.data);
                println!("Login world is live!");
            }
        })
        .unwrap();

    w_sender_clone
}
