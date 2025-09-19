use anyhow::{anyhow, Result};
use bitcoin;
use bitcoin::consensus::encode::serialize;
use bitcoin::consensus::encode::Decodable;
use metashrew_core::environment::RuntimeEnvironment;
use metashrew_support::index_pointer::{IndexPointer, KeyValuePointer};
use std::io::Cursor;
use std::sync::Arc;

pub fn blocks<E: RuntimeEnvironment>() -> IndexPointer<E> {
    IndexPointer::from_keyword("/blockdata/")
}

pub fn index_extensions<E: RuntimeEnvironment>(env: &mut E, height: u32, v: &bitcoin::Block) {
    blocks().select_value(height).set(env, Arc::new(serialize(v)))
}

pub fn get_block<E: RuntimeEnvironment>(env: &mut E, height: u32) -> Result<bitcoin::Block> {
    let block_data = blocks().select_value(height).get(env);
    if block_data.len() == 0 {
        return Err(anyhow!("Block not found for height: {}", height));
    }

    let mut cursor = Cursor::new(block_data.as_ref().to_vec());
    bitcoin::Block::consensus_decode(&mut cursor)
        .map_err(|e| anyhow!("Failed to decode block: {}", e))
}
