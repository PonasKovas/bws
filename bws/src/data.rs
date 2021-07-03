use anyhow::{Context, Result};
use lazy_static::lazy_static;
use log::{error, info};
use serde_json::{Map, Value};
use std::{fs::File, io::Cursor};

#[derive(Clone, Debug)]
pub struct Block {
    pub default_state: i32,
    pub states: &'static [BlockState],
}

#[derive(Clone, Debug)]
pub struct BlockState {
    pub state_id: i32,
    pub properties: &'static [(String, String)],
}

lazy_static! {
    // maps item IDs to the corresponding block states
    // damn takes a long time to initialize, whats the problem?
    pub static ref ITEMS_TO_BLOCKS: &'static [Option<Block>] = {
        match gen() {
            Ok(d) => d,
            Err(e) => {
                error!("Error generating ITEMS_TO_BLOCKS structure: {}", e);
                std::process::exit(1);
            }
        }
    };

}

fn gen() -> Result<&'static [Option<Block>]> {
    // this file was taken from https://gitlab.bixilon.de/bixilon/pixlyzer-data/-/blob/master/version/1.16.5/blocks.json
    let blocks: Map<String, Value> =
        serde_json::from_reader(Cursor::new(incl!("data/blocks.json")))?;
    // uhmm if I use serde_json::from_slice with this the stack overflows?? and this works

    // this file was taken from https://gitlab.bixilon.de/bixilon/pixlyzer-data/-/blob/master/version/1.16.5/items.json
    let items: Map<String, Value> = serde_json::from_slice(incl!("data/items.json"))?;

    let mut mappings = vec![None; items.len()];

    for (_block_id, block) in blocks {
        let block = block
            .as_object()
            .context("blocks.json: blocks must be objects")?;

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
                        properties.push((
                            property_name.to_owned(),
                            if property_value.is_string() {
                                format!("{}", property_value)
                            } else {
                                format!("\"{}\"", property_value)
                            },
                        ));
                    }
                }

                states.push(BlockState {
                    state_id,
                    properties: properties.leak(),
                });
            }

            mappings[id] = Some(Block {
                default_state,
                states: states.leak(),
            });
        }
    }

    Ok(mappings.leak())
}
