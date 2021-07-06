use protocol::datatypes::VarInt;
use savefile_derive::Savefile;

pub mod lobby;
pub mod login;

// half length of a side of the map, in chunks
pub const MAP_SIZE: i8 = 8; // 8 means 16x16 chunks

pub type WorldChunks = [Box<WorldChunk>; 4 * MAP_SIZE as usize * MAP_SIZE as usize];

#[derive(Savefile, Debug, Clone)]
pub struct WorldChunk {
    pub biomes: Box<[i32; 1024]>,
    pub sections: [Option<WorldChunkSection>; 16],
}

#[derive(Savefile, Debug, Clone)]
pub struct WorldChunkSection {
    pub block_mappings: Vec<i32>,
    pub blocks: Vec<u64>,
}
