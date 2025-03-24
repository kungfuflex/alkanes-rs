use metashrew::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use once_cell::sync::Lazy;
use bitcoin;
use std::sync::{Arc};
use bitcoin::consensus::encode::serialize;
use bitcoin::consensus::encode::Decodable;
use anyhow::{anyhow, Result};
use std::io::Cursor;

pub static BLOCKS: Lazy<IndexPointer> = Lazy::new(|| IndexPointer::from_keyword("/blockdata/"));

pub fn index_extensions(height: u32, v: &bitcoin::Block) {
  BLOCKS.select_value(height).set(Arc::new(serialize(v)))
}

pub fn get_block(height: u32) -> Result<bitcoin::Block> {
  let block_data = BLOCKS.select_value(height).get();
  if block_data.len() == 0 {
    return Err(anyhow!("Block not found for height: {}", height));
  }
  
  let mut cursor = Cursor::new(block_data.as_ref().to_vec());
  bitcoin::Block::consensus_decode(&mut cursor).map_err(|e| anyhow!("Failed to decode block: {}", e))
}
