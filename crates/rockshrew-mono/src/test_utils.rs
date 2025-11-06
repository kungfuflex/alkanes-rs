use anyhow::Result;
use bitcoin::{
    block::{Header, Version},
    hash_types::TxMerkleNode,
    hashes::{self, Hash},
    pow::CompactTarget,
    BlockHash, Block, Transaction,
};
use bitcoin::consensus::serialize;
use memshrew_runtime::MemStoreAdapter;
use metashrew_runtime::MetashrewRuntime;
use metashrew_runtime::traits::KeyValueStoreLike;

pub const WASM: &'static [u8] = include_bytes!(env!("METASHREW_MINIMAL_WASM_PATH"));

/// Configuration for setting up a test environment
pub struct TestConfig {
    pub wasm: &'static [u8],
}

impl TestConfig {
    /// Create a new default test configuration
    pub fn new() -> Self {
        Self { wasm: WASM }
    }

    /// Create a new MetashrewRuntime for testing
    pub async fn create_runtime(&self, engine: wasmtime::Engine) -> Result<MetashrewRuntime<MemStoreAdapter>> {
        MetashrewRuntime::new(self.wasm, MemStoreAdapter::new(), engine).await
    }

    pub async fn create_runtime_from_adapter<T: KeyValueStoreLike + Clone + Send + Sync + 'static>(
        &self,
        store: T,
        engine: wasmtime::Engine,
    ) -> Result<MetashrewRuntime<T>> where <T as KeyValueStoreLike>::Batch: Send {
        MetashrewRuntime::new(self.wasm, store, engine).await
    }
}

impl Default for TestConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// General-purpose test utilities
pub struct TestUtils;

impl TestUtils {
    /// Creates a simple test block with a specified height and previous block hash.
    pub fn create_test_block(height: u32, prev_hash: BlockHash) -> Block {
        let txdata: Vec<Transaction> = vec![];
        let merkle_root = bitcoin::merkle_tree::calculate_root(txdata.iter().map(|tx| tx.compute_txid()))
            .map(|hash| TxMerkleNode::from_raw_hash(hashes::sha256d::Hash::from_byte_array(hash.to_byte_array())))
            .unwrap_or(TxMerkleNode::all_zeros());
        let header = Header {
            version: Version::from_consensus(1),
            prev_blockhash: prev_hash,
            merkle_root,
            time: height,
            bits: CompactTarget::from_consensus(0),
            nonce: 0,
        };
        Block { header, txdata }
    }

    /// Serializes a block into a byte vector.
    pub fn serialize_block(block: &Block) -> Vec<u8> {
        serialize(block)
    }
}
