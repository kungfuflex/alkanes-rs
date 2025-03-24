use metashrew::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use once_cell::sync::Lazy;
use bitcoin;
use std::sync::{Arc};
use bitcoin::consensus::encode::serialize;

pub static BLOCKS: Lazy<IndexPointer> = Lazy::new(|| IndexPointer::from_keyword("/blockdata/"));

pub fn index_extensions(height: u32, v: &bitcoin::Block) {
  BLOCKS.select_value(height).set(Arc::new(serialize(v)))
}
