use anyhow::Result;
use bitcoin::Block;
use std::collections::BTreeSet;

pub trait AlkanesHost {
    fn index_block(&self, block: &Block, height: u32) -> Result<BTreeSet<Vec<u8>>>;
    fn flush(&self);
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
}