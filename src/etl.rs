use anyhow::{anyhow, Result};
use bitcoin;
use bitcoin::consensus::encode::serialize;
use bitcoin::consensus::encode::Decodable;
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use once_cell::sync::Lazy;
use std::io::Cursor;
use std::sync::Arc;

pub static BLOCKS: Lazy<IndexPointer> = Lazy::new(|| IndexPointer::from_keyword("/blockdata/"));

pub fn index_extensions(height: u32, v: &Arc<bitcoin::Block>) {
    BLOCKS.select_value(height).set(Arc::new(serialize(v.as_ref())))
}

pub fn get_block(height: u32) -> Result<Arc<bitcoin::Block>> {
    let block_data = BLOCKS.select_value(height).get();
    if block_data.is_empty() {
        return Err(anyhow!("Block not found for height: {}", height));
    }
    let mut cursor = Cursor::new(block_data.as_ref());
    Ok(Arc::new(bitcoin::Block::consensus_decode(&mut cursor)?))
}
