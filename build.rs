use anyhow::{bail, Context, Result};
use bytecheck::CheckBytes;
use rkyv::{
    check_archived_root,
    ser::{serializers::AllocSerializer, Serializer},
    Archive, Serialize,
};
use serde_json::{Map, Value};
use std::io::Write;
use std::{env::var_os, fs::File, path::Path};

// These MUST match the structures defined in src/data.rs

// Can't use normal tuples since their ABI is not defined
#[derive(Clone, Debug, Archive, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub struct Tuple<T1, T2>(pub T1, pub T2);

#[derive(Clone, Debug, Archive, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub struct Block {
    pub default_state: i32,
    pub class: String,
    pub states: Vec<BlockState>,
}

#[derive(Clone, Debug, Archive, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub struct BlockState {
    pub state_id: i32,
    pub properties: Vec<Tuple<String, String>>,
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = var_os("OUT_DIR").unwrap();

    let blocks: Map<String, Value> = {
        // this file was taken from
        let blocks_file = reqwest::blocking::get("https://gitlab.bixilon.de/bixilon/pixlyzer-data/-/raw/master/version/1.16.5/blocks.min.json").unwrap().bytes().unwrap();
        serde_json::from_reader(blocks_file.as_ref()).unwrap()
    };

    let items: Map<String, Value> = {
        // this file was taken from
        let items_file = reqwest::blocking::get("https://gitlab.bixilon.de/bixilon/pixlyzer-data/-/raw/master/version/1.16.5/items.min.json").unwrap().bytes().unwrap();
        serde_json::from_reader(items_file.as_ref()).unwrap()
    };

    let items_to_blocks =
        gen_items_to_blocks(blocks, items).expect("Couldn't generate items-to-blocks");

    // // debug/dev
    // serde_json::to_writer_pretty(
    //     File::create(Path::new(&out_dir).join("items-to-blocks.json")).unwrap(),
    //     &items_to_blocks,
    // )
    // .unwrap();

    let mut output = File::create(Path::new(&out_dir).join("items-to-blocks.rkyv")).unwrap();

    let buf = {
        let mut serializer = AllocSerializer::<4096>::default();
        serializer.serialize_value(&items_to_blocks).unwrap();

        serializer.into_serializer().into_inner()
    };

    output.write_all(&buf).unwrap();

    // debug
    println!(
        "{:?}",
        check_archived_root::<Vec<Option<Block>>>(buf.as_slice()).is_ok()
    );
}

fn gen_items_to_blocks(
    blocks: Map<String, Value>,
    items: Map<String, Value>,
) -> Result<Vec<Option<Block>>> {
    let mut mappings = vec![None; items.len()];

    for (_block_id, block) in blocks {
        let mut block = if let Value::Object(obj) = block {
            obj
        } else {
            bail!("blocks.json: blocks must be objects");
        };

        if let Some(id) = block.get("item") {
            let id = id
                .as_i64()
                .context("blocks.json: Blocks' \"item\" field must be an integer")?
                as usize;

            let default_state = block
                .get("default_state")
                .context("blocks.json: Blocks must have a \"default_state\"")?
                .as_i64()
                .context("blocks.json: Blocks' field \"default_state\" must be an integer")?
                as i32;

            let class = if let Value::String(string) = block
                .get_mut("class")
                .context("blocks.json: Blocks must have a \"class\"")?
                .take()
            {
                string
            } else {
                bail!("blocks.json: Blocks' field \"class\" must be a string");
            };

            let mut states = Vec::new();

            for (state_id, state) in block
                .get("states")
                .context("blocks.json: Blocks must have a \"states\"")?
                .as_object()
                .context("blocks.json: Blocks' field \"states\" must be an object")?
            {
                let state_id = state_id
                    .parse::<i32>()
                    .context("blocks.json: Block's state IDs must be legal integers")?;

                let mut properties = Vec::new();

                if let Some(raw_properties) = state.get("properties") {
                    for (property_name, property_value) in raw_properties
                        .as_object()
                        .context("Block's state's \"properties\" field must be an object")?
                    {
                        properties.push(Tuple(
                            property_name.to_owned(),
                            if let Some(string) = property_value.as_str() {
                                string.to_string()
                            } else {
                                format!("{}", property_value)
                            },
                        ));
                    }
                }

                states.push(BlockState {
                    state_id,
                    properties,
                });
            }

            mappings[id] = Some(Block {
                default_state,
                class,
                states,
            });
        }
    }

    Ok(mappings)
}
