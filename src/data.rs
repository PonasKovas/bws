use anyhow::{Context, Result};
use bytecheck::CheckBytes;
use include_bytes_aligned::include_bytes_aligned;
use lazy_static::lazy_static;
use log::{error, info};
use rkyv::{
    check_archived_root,
    ser::{serializers::AllocSerializer, Serializer},
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, ArchiveUnsized, Archived, Deserialize, DeserializeUnsized, Fallible, Infallible,
    Serialize, SerializeUnsized,
};
use serde_json::{Map, Value};
use std::{
    fs::File,
    io::{Cursor, Read, Write},
};

// Can't use normal tuples since their ABI is not defined
#[derive(Clone, Debug, Archive)]
#[archive_attr(derive(CheckBytes))]
pub struct Tuple<T1, T2>(pub T1, pub T2);

#[derive(Clone, Debug, Archive)]
#[archive_attr(derive(CheckBytes))]
pub struct Block {
    pub default_state: i32,
    pub class: String,
    pub states: Vec<BlockState>,
}

#[derive(Clone, Debug, Archive)]
#[archive_attr(derive(CheckBytes))]
pub struct BlockState {
    pub state_id: i32,
    pub properties: Vec<Tuple<String, String>>,
}

lazy_static! {
    // maps item IDs to the corresponding block states
    pub static ref ITEMS_TO_BLOCKS: &'static Archived<Vec<Option<Block>>> = read_items_to_blocks();
}

fn read_items_to_blocks() -> &'static Archived<Vec<Option<Block>>> {
    match check_archived_root::<Vec<Option<Block>>>(
        &include_bytes_aligned!(16, concat!(env!("OUT_DIR"), "/items-to-blocks.rkyv"))[..],
    ) {
        Ok(r) => r,
        Err(e) => {
            error!("Error reading ITEMS_TO_BLOCKS: {}", e);
            std::process::exit(1);
        }
    }
}
