use bitcoin::{Transaction, Block};
use ordinals::{Runestone};
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use anyhow::{Result};

#[test]
pub fn test_trace() -> Result<()> {
  let height = 840_000;
  let block: Block = create_block_with_coinbase_tx(height);
  Ok(())
}
