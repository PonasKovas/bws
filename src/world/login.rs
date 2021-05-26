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
        "authentication"
    }
}

pub fn new() -> LoginWorld {
    LoginWorld {}
}
