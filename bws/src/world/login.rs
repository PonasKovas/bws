use std::{
    thread::{sleep, Builder},
    time::Duration,
};

use crate::internal_communication::{SHBound, WBound, WSender};
use crate::world::World;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub struct LoginWorld {}

impl World for LoginWorld {
    fn get_world_name(&self) -> &str {
        "Authorization"
    }
}

pub fn new() -> WSender {
    let (w_sender, mut w_receiver) = unbounded_channel::<WBound>();

    Builder::new()
        .name("Login World Thread".to_string())
        .spawn(move || {
            let mut world = LoginWorld {};

            world.run(w_receiver);
        })
        .unwrap();

    w_sender
}
