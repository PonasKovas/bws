use std::{borrow::Cow, io::Write};

use crate::world::{WorldChunk, WorldChunks, MAP_SIZE};
use anyhow::Result;
use savefile_derive::Savefile;

pub const VERSION: u32 = 0;

#[derive(Savefile, Debug)]
pub struct Map<'a> {
    pub chunks: Cow<'a, WorldChunks>,
}

impl<'a> Map<'a> {
    pub fn load(path: &str) -> Result<Self> {
        Ok(savefile::load_file(path, VERSION)?)
    }
    pub fn save(&self, path: &str) -> Result<()> {
        Ok(savefile::save_file(path, VERSION, self)?)
    }
}
