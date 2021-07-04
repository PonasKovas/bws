use anyhow::{Context, Result};
use flate2::write::DeflateDecoder;
use lazy_static::lazy_static;
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    fs::File,
    io::{Cursor, Read, Write},
};

#[derive(Clone, Debug, Deserialize)]
pub struct Block {
    pub default_state: i32,
    pub states: Vec<BlockState>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BlockState {
    pub state_id: i32,
    pub properties: Vec<(String, String)>,
}

lazy_static! {
    // maps item IDs to the corresponding block states
    // damn takes a long time to initialize, whats the problem?
    pub static ref ITEMS_TO_BLOCKS: Vec<Option<Block>> = {
        match read_items_to_blocks() {
            Ok(d) => d,
            Err(e) => {
                error!("Error reading ITEMS_TO_BLOCKS: {:?}", e);
                std::process::exit(1);
            }
        }
    };

}

fn read_items_to_blocks() -> Result<Vec<Option<Block>>> {
    let compressed = include_bytes!(concat!(env!("OUT_DIR"), "/items-to-blocks.bincode"));
    let mut uncompressed: Vec<u8> = Vec::new();
    let mut decoder = DeflateDecoder::new(&mut uncompressed);
    decoder.write_all(compressed)?;
    decoder.finish()?;

    Ok(bincode::deserialize(&uncompressed).context("bincode deserialization")?)
}
