pub mod login;

use std::{thread::Builder, time::Duration};

use crate::internal_communication::{SHBound, WBound, WReceiver, WSender};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

// The trait that will be needed to be implemented to create types of worlds (login, lobby, game and so on)
pub trait World: Sized {
    // only contains the most basic methods, will add more later as needed

    // the main method, takes over the thread and runs the world
    fn run(self, w_receiver: WReceiver) {
        loop {
            println!("{} world is running.", self.get_world_name());
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    // should return the name of the world, which doesn't have to be unique
    fn get_world_name(&self) -> &str;
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
