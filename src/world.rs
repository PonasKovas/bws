pub mod login;

use std::time::Duration;

use crate::internal_communication::{SHBound, WBound, WReceiver};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

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
