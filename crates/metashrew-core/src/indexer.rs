use anyhow::Result;
use bitcoin::Block;

pub trait Indexer {
    fn index_block(&self, block: &Block, height: u32) -> Result<()>;
}