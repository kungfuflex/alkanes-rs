use bitcoin::hashes::Hash;
#[path = "helpers.rs"]
mod helpers;
use anyhow::Result;
use helpers::TestHarness;
use metashrew_sync::StorageAdapter;
use bitcoin::block::{Block, Header as BlockHeader};
use bitcoin::pow::CompactTarget;

#[tokio::test]
async fn test_native_alkane_logic() -> Result<()> {
    let mut harness = TestHarness::new();
    let block = Block {
        header: BlockHeader {
            version: bitcoin::block::Version::from_consensus(0),
            prev_blockhash: bitcoin::BlockHash::all_zeros(),
            merkle_root: bitcoin::TxMerkleNode::all_zeros(),
            time: 0,
            bits: CompactTarget::from_consensus(0),
            nonce: 0,
        },
        txdata: vec![],
    };
    harness.add_block(block);
    harness.process_block().await;
    let height = harness.runtime.context.lock().unwrap().db.get_indexed_height().await?;
    assert_eq!(height, 1);
    Ok(())
}