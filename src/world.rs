pub mod login;

use crate::internal_communication::{SHBound, WBound};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

// This struct has all the closures for handling events
// and the world data which is in a separate field so it can be passed to the closures
// without the borrow checker crying.
pub struct World {
    pub data: WorldData,
    pub c_player_join: Box<dyn FnMut(&mut WorldData) -> bool + Send>,
}

pub struct WorldData {
    // Name of the world, doesn't have to be unique
    pub name: String,
    // All the blocks of the world
    pub blocks: Blocks,
    // Players currently in this world
    pub players: Vec<Player>,
}

#[derive(Clone)]
pub struct Blocks {
    // The world is divided into 32x16x32 segments and each segment consists of 512 blocks
    // (8x8x8) which results in a 256x128x256 map.
    data: [[[Option<Box<[[[Block; 8]; 8]; 8]>>; 32]; 16]; 32],
}

#[derive(Debug)]
pub struct Player {
    pub username: String,
    pub sh_channel: (UnboundedSender<SHBound>, UnboundedReceiver<WBound>),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Block {
    pub id: u16,
}

impl Default for Blocks {
    fn default() -> Self {
        Self {
            data: [(); 32].map(|_| [(); 16].map(|_| [(); 32].map(|_| None))),
        }
    }
}

impl Default for Block {
    fn default() -> Self {
        // Default is air
        Self { id: 0 }
    }
}

impl Blocks {
    pub fn get(&self, index: (i8, u8, i8)) -> Option<&Block> {
        // calculate which segment owns the coordinate
        let sx = (index.0 / 8 + 16) as usize;
        let sy = (index.1 / 8) as usize;
        let sz = (index.2 / 8 + 16) as usize;

        // internal segment coordinates
        let ix = (index.0 % 8) as usize;
        let iy = (index.1 % 8) as usize;
        let iz = (index.2 % 8) as usize;

        match &self.data[sx][sy][sz] {
            Some(segment) => Some(&segment[ix][iy][iz]),
            None => None,
        }
    }
    pub fn get_mut(&mut self, index: (i8, u8, i8)) -> &mut Block {
        // calculate which segment owns the coordinate
        let sx = (index.0 / 8 + 16) as usize;
        let sy = (index.1 / 8) as usize;
        let sz = (index.2 / 8 + 16) as usize;

        // internal segment coordinates
        let ix = (index.0 % 8) as usize;
        let iy = (index.1 % 8) as usize;
        let iz = (index.2 % 8) as usize;

        &mut self.data[sx][sy][sz].get_or_insert_with(|| Box::new([[[Block::default(); 8]; 8]; 8]))
            [ix][iy][iz]
    }
}
