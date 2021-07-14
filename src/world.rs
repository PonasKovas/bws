use bytecheck::CheckBytes;
use protocol::datatypes::VarInt;
use rkyv::{Archive, Deserialize, Serialize};

pub mod lobby;
pub mod login;

pub type WorldChunks<const CHUNKS: usize> = [Box<WorldChunk>; CHUNKS];

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct WorldChunk {
    pub biomes: Box<[i32; 1024]>,
    pub sections: [Option<WorldChunkSection>; 16],
}

#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[archive_attr(derive(CheckBytes))]
pub struct WorldChunkSection {
    pub block_mappings: Vec<i32>,
    pub blocks: Vec<u64>,
}
