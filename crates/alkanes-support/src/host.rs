use anyhow::Result;
use bitcoin::Block;
use std::collections::BTreeSet;

pub trait AlkanesHost: protorune_support::host::Host {
    fn index_block(&self, block: &Block, height: u32) -> Result<BTreeSet<Vec<u8>>>;
}