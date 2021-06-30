use protocol::datatypes::VarInt;
use savefile_derive::Savefile;

pub mod lobby;
pub mod login;

// half length of a side of the map, in chunks
pub const MAP_SIZE: i8 = 8; // 8 means 16x16 chunks

#[derive(Savefile, Debug, Clone)]
pub struct WorldChunk {
    biomes: Box<[i32; 1024]>, // damn you stack overflows!
    sections: [Option<WorldChunkSection>; 16],
}

#[derive(Savefile, Debug, Clone)]
pub struct WorldChunkSection {
    block_mappings: Vec<i32>,
    blocks: Vec<u64>,
}
