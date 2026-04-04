//! Full-stack test harness using vendored qubitcoin crates.
//!
//! This provides a true e2e environment:
//! - TestChain: in-memory regtest chain with UTXO tracking
//! - WasmIndexerRuntime + IndexerStorage: production wasmtime config + RocksDB
//! - JSON-RPC server: alkanes-cli-common can talk to it via standard provider

use anyhow::{Context, Result};
use qubitcoin_consensus::block::Block as QBlock;
use qubitcoin_indexer::storage::IndexerStorage;
use qubitcoin_indexer::runtime::WasmIndexerRuntime;
use qubitcoin_node::test_framework::TestChain;
use qubitcoin_serialize::serialize;
use protorune_support::network::{set_network, NetworkParams};
use std::sync::Arc;
use tempfile::TempDir;

use crate::fixtures;

/// Full-stack test harness: TestChain + real WasmIndexerRuntime.
pub struct FullStackHarness {
    pub chain: TestChain,
    alkanes_runtime: WasmIndexerRuntime,
    alkanes_storage: Arc<IndexerStorage>,
    esplora_runtime: WasmIndexerRuntime,
    esplora_storage: Arc<IndexerStorage>,
    _datadir: TempDir,
}

impl FullStackHarness {
    /// Create a full-stack harness with alkanes + esplora indexers.
    pub fn new() -> Result<Self> {
        let datadir = TempDir::new().context("Failed to create temp dir")?;

        // Create indexer storage (RocksDB per indexer)
        let alkanes_db_path = datadir.path().join("alkanes");
        std::fs::create_dir_all(&alkanes_db_path)?;
        let alkanes_storage = Arc::new(
            IndexerStorage::open(&alkanes_db_path)
                .map_err(|e| anyhow::anyhow!("Failed to open alkanes storage: {}", e))?,
        );

        let esplora_db_path = datadir.path().join("esplora");
        std::fs::create_dir_all(&esplora_db_path)?;
        let esplora_storage = Arc::new(
            IndexerStorage::open(&esplora_db_path)
                .map_err(|e| anyhow::anyhow!("Failed to open esplora storage: {}", e))?,
        );

        // Compile WASM modules using the real qubitcoin WasmIndexerRuntime
        let alkanes_runtime = WasmIndexerRuntime::new(fixtures::ALKANES_WASM)
            .map_err(|e| anyhow::anyhow!("Failed to create alkanes runtime: {}", e))?;
        let esplora_runtime = WasmIndexerRuntime::new(fixtures::ESPLORA_WASM)
            .map_err(|e| anyhow::anyhow!("Failed to create esplora runtime: {}", e))?;

        // Set network params for protorune test helpers
        set_network(NetworkParams {
            bech32_prefix: String::from("bcrt"),
            p2pkh_prefix: 0x64,
            p2sh_prefix: 0xc4,
        });

        // Create in-memory regtest chain
        let chain = TestChain::new();

        Ok(Self {
            chain,
            alkanes_runtime,
            alkanes_storage,
            esplora_runtime,
            esplora_storage,
            _datadir: datadir,
        })
    }

    /// Mine an empty block and process it through all indexers.
    pub fn mine_empty_block(&mut self) -> Result<QBlock> {
        let block = self.chain.mine_block(vec![]);
        self.index_block(&block)?;
        Ok(block)
    }

    /// Mine N empty blocks.
    pub fn mine_empty_blocks(&mut self, count: u32) -> Result<()> {
        for _ in 0..count {
            self.mine_empty_block()?;
        }
        Ok(())
    }

    /// Serialize a qubitcoin block and feed it to all indexers.
    pub fn index_block(&self, block: &QBlock) -> Result<()> {
        let height = self.chain.height() as u32;
        let block_bytes = serialize(block)
            .context("Failed to serialize block")?;

        // Process through alkanes indexer
        let pairs = self.alkanes_runtime
            .run_block(
                Self::build_indexer_input(height, &block_bytes),
                Arc::clone(&self.alkanes_storage),
                "alkanes",
            )
            .map_err(|e| anyhow::anyhow!("alkanes indexer failed at height {}: {}", height, e))?;

        // Write pairs to storage using raw write_batch (NOT append_batch).
        // The WASM module already manages its own append-only key layout
        // (length_key/index_key pattern via metashrew-core). Using append_batch
        // would double-wrap the keys, breaking subsequent __get/__get_len reads.
        self.alkanes_storage
            .write_batch(&pairs)
            .map_err(|e| anyhow::anyhow!("alkanes storage write failed: {}", e))?;
        self.alkanes_storage
            .set_tip_height(height)
            .map_err(|e| anyhow::anyhow!("alkanes set tip failed: {}", e))?;

        // Process through esplora indexer
        let esplora_pairs = self.esplora_runtime
            .run_block(
                Self::build_indexer_input(height, &block_bytes),
                Arc::clone(&self.esplora_storage),
                "esplora",
            )
            .map_err(|e| anyhow::anyhow!("esplora indexer failed at height {}: {}", height, e))?;

        self.esplora_storage
            .write_batch(&esplora_pairs)
            .map_err(|e| anyhow::anyhow!("esplora storage write failed: {}", e))?;
        self.esplora_storage
            .set_tip_height(height)
            .map_err(|e| anyhow::anyhow!("esplora set tip failed: {}", e))?;

        Ok(())
    }

    /// Build indexer input: [height_le32] ++ [block_bytes]
    fn build_indexer_input(height: u32, block_bytes: &[u8]) -> Vec<u8> {
        let mut input = Vec::with_capacity(4 + block_bytes.len());
        input.extend_from_slice(&height.to_le_bytes());
        input.extend_from_slice(block_bytes);
        input
    }

    /// Call an alkanes view function.
    /// Input should NOT include the height prefix — this method adds it.
    pub fn alkanes_view(&self, fn_name: &str, input: &[u8]) -> Result<Vec<u8>> {
        // The WasmIndexerRuntime's call_view_async already prepends tip height.
        // We just pass the raw input.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(async {
            self.alkanes_runtime
                .call_view_async(
                    fn_name,
                    input.to_vec(),
                    Arc::clone(&self.alkanes_storage),
                    "alkanes",
                )
                .await
                .map_err(|e| anyhow::anyhow!("alkanes view '{}' failed: {}", fn_name, e))
        })
    }

    /// Call an esplora view function.
    pub fn esplora_view(&self, fn_name: &str, input: &[u8]) -> Result<Vec<u8>> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        rt.block_on(async {
            self.esplora_runtime
                .call_view_async(
                    fn_name,
                    input.to_vec(),
                    Arc::clone(&self.esplora_storage),
                    "esplora",
                )
                .await
                .map_err(|e| anyhow::anyhow!("esplora view '{}' failed: {}", fn_name, e))
        })
    }

    /// Index a rust-bitcoin Block by converting to qubitcoin-consensus format.
    /// This lets us use the existing block_builder helpers with the full-stack harness.
    pub fn index_bitcoin_block(&self, btc_block: &bitcoin::Block, height: u32) -> Result<()> {
        // Convert via serialization round-trip (byte-identical)
        let block_bytes = bitcoin::consensus::serialize(btc_block);
        eprintln!("[harness] index_bitcoin_block height={} block_bytes={} txs={}",
            height, block_bytes.len(), btc_block.txdata.len());

        // Process through alkanes indexer
        let pairs = self.alkanes_runtime
            .run_block(
                Self::build_indexer_input(height, &block_bytes),
                Arc::clone(&self.alkanes_storage),
                "alkanes",
            )
            .map_err(|e| anyhow::anyhow!("alkanes indexer failed at height {}: {}", height, e))?;
        let max_val = pairs.iter().map(|p| p.1.len()).max().unwrap_or(0);
        let total_bytes: usize = pairs.iter().map(|p| p.0.len() + p.1.len()).sum();
        if height == 5 {
            for (k, v) in &pairs {
                let key_preview = hex::encode(&k[..std::cmp::min(k.len(), 60)]);
                let key_str = String::from_utf8_lossy(&k[..std::cmp::min(k.len(), 30)]);
                eprintln!("[h5] key_len={} val_len={} key_hex={} key_str={}", k.len(), v.len(), key_preview, key_str);
            }
        }
        for (k, v) in &pairs {
            if v.len() > 100000 {
                let key_preview = hex::encode(&k[..std::cmp::min(k.len(), 60)]);
                eprintln!("[harness] LARGE pair: key_len={} val_len={} key={}", k.len(), v.len(), key_preview);
            }
        }
        eprintln!("[harness] alkanes returned {} kv pairs, total={} bytes, largest_val={} bytes", pairs.len(), total_bytes, max_val);

        self.alkanes_storage
            .write_batch(&pairs)
            .map_err(|e| anyhow::anyhow!("alkanes storage write failed: {}", e))?;
        self.alkanes_storage
            .set_tip_height(height)
            .map_err(|e| anyhow::anyhow!("alkanes set tip failed: {}", e))?;

        // Process through esplora indexer
        let esplora_pairs = self.esplora_runtime
            .run_block(
                Self::build_indexer_input(height, &block_bytes),
                Arc::clone(&self.esplora_storage),
                "esplora",
            )
            .map_err(|e| anyhow::anyhow!("esplora indexer failed at height {}: {}", height, e))?;

        self.esplora_storage
            .write_batch(&esplora_pairs)
            .map_err(|e| anyhow::anyhow!("esplora storage write failed: {}", e))?;
        self.esplora_storage
            .set_tip_height(height)
            .map_err(|e| anyhow::anyhow!("esplora set tip failed: {}", e))?;

        Ok(())
    }

    /// Current chain height.
    pub fn height(&self) -> i32 {
        self.chain.height()
    }

    // Debug helpers for storage inspection
    pub fn alkanes_storage_raw_get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.alkanes_storage.get(key)
    }
    pub fn alkanes_storage_get_latest(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.alkanes_storage.get_latest(key)
    }
    pub fn alkanes_storage_get_length(&self, key: &[u8]) -> u32 {
        self.alkanes_storage.get_length(key)
    }
}

    // Debug helpers for storage inspection
