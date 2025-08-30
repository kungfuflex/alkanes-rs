// Journal
// This file contains the native adapters for the metashrew-sync traits.
// It implements `BitcoinNodeAdapter` using `bitcoincore-rpc` and `StorageAdapter` using `rocksdb`.
// The `NativeRuntimeAdapter` is a placeholder for now.
//
// The main challenge has been dependency version conflicts, specifically with the `bitcoin` crate.
// The workspace uses one version, while `bitcoincore-rpc` uses another.
// This caused `Encodable` trait errors.
// The current strategy is to align the workspace's `bitcoin` version with `bitcoincore-rpc`'s version.
//
// Another issue was converting `BlockHash` to `Vec<u8>`. The correct method is `as_ref().to_vec()`.

use anyhow::Result;
use async_trait::async_trait;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use metashrew_sync::{
    BitcoinNodeAdapter, BlockInfo, ChainTip, RuntimeAdapter, StorageAdapter, StorageStats,
    SyncError, SyncResult, AtomicBlockResult, ViewCall, ViewResult, PreviewCall, RuntimeStats
};
use bitcoin::consensus::serialize;
use bitcoin::Network as BitcoinNetwork;
use rocksdb::{DB, Options};
use std::sync::Arc;
use alkanes::alkanes_indexer;
use crate::Network;

pub struct RpcAdapter {
    rpc: Client,
    network: Network,
}

impl RpcAdapter {
    pub fn new(url: &str, user: &str, pass: &str, network: Network) -> Result<Self> {
        let rpc = Client::new(url, Auth::UserPass(user.to_string(), pass.to_string()))?;
        Ok(Self { rpc, network })
    }
}

#[async_trait]
impl BitcoinNodeAdapter for RpcAdapter {
    async fn get_tip_height(&self) -> SyncResult<u32> {
        self.rpc.get_block_count().map(|h| h as u32).map_err(|e| SyncError::BitcoinNode(e.to_string()))
    }

    async fn get_block_hash(&self, height: u32) -> SyncResult<Vec<u8>> {
        if height == 0 {
            let genesis_hash = match self.network {
                Network::Mainnet => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Bitcoin).block_hash(),
                Network::Regtest => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Regtest).block_hash(),
                Network::Signet => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Signet).block_hash(),
            };
            return Ok(<bitcoin::BlockHash as AsRef<[u8]>>::as_ref(&genesis_hash).to_vec());
        }
        self.rpc.get_block_hash(height as u64).map(|h| <bitcoin::BlockHash as AsRef<[u8]>>::as_ref(&h).to_vec()).map_err(|e| SyncError::BitcoinNode(e.to_string()))
    }

    async fn get_block_data(&self, height: u32) -> SyncResult<Vec<u8>> {
        let hash = if height == 0 {
            match self.network {
                Network::Mainnet => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Bitcoin).block_hash(),
                Network::Regtest => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Regtest).block_hash(),
                Network::Signet => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Signet).block_hash(),
            }
        } else {
            self.rpc.get_block_hash(height as u64).map_err(|e| SyncError::BitcoinNode(e.to_string()))?
        };
        if height == 0 {
            let block = match self.network {
                Network::Mainnet => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Bitcoin),
                Network::Regtest => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Regtest),
                Network::Signet => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Signet),
            };
            return Ok(serialize(&block));
        }
        let block = self.rpc.get_block(&hash).map_err(|e| SyncError::BitcoinNode(e.to_string()))?;
        Ok(serialize(&block))
    }

    async fn get_block_info(&self, height: u32) -> SyncResult<BlockInfo> {
        let (hash, block) = if height == 0 {
            let block = match self.network {
                Network::Mainnet => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Bitcoin),
                Network::Regtest => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Regtest),
                Network::Signet => bitcoin::blockdata::constants::genesis_block(BitcoinNetwork::Signet),
            };
            (block.block_hash(), block)
        } else {
            let hash = self.rpc.get_block_hash(height as u64).map_err(|e| SyncError::BitcoinNode(e.to_string()))?;
            let block = self.rpc.get_block(&hash).map_err(|e| SyncError::BitcoinNode(e.to_string()))?;
            (hash, block)
        };
        let data = serialize(&block);
        Ok(BlockInfo {
            height,
            hash: <bitcoin::BlockHash as AsRef<[u8]>>::as_ref(&hash).to_vec(),
            data,
        })
    }

    async fn get_chain_tip(&self) -> SyncResult<ChainTip> {
        let count = self.rpc.get_block_count().map_err(|e| SyncError::BitcoinNode(e.to_string()))?;
        let hash = self.rpc.get_block_hash(count).map_err(|e| SyncError::BitcoinNode(e.to_string()))?;
        Ok(ChainTip {
            height: count as u32,
            hash: <bitcoin::BlockHash as AsRef<[u8]>>::as_ref(&hash).to_vec(),
        })
    }

    async fn is_connected(&self) -> bool {
        self.rpc.get_blockchain_info().is_ok()
    }
}

#[derive(Clone)]
pub struct RocksDBAdapter {
    pub db: Arc<DB>,
}

impl RocksDBAdapter {
    pub fn new(path: &str) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)?;
        Ok(Self { db: Arc::new(db) })
    }
}

#[async_trait]
impl StorageAdapter for RocksDBAdapter {
    async fn get_indexed_height(&self) -> SyncResult<u32> {
        let key = b"indexed_height";
        match self.db.get(key).map_err(|e| SyncError::Storage(e.to_string()))? {
            Some(value) => Ok(u32::from_le_bytes(value.try_into().unwrap())),
            None => Ok(0),
        }
    }

    async fn set_indexed_height(&mut self, height: u32) -> SyncResult<()> {
        let key = b"indexed_height";
        self.db.put(key, &height.to_le_bytes()).map_err(|e| SyncError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn store_block_hash(&mut self, height: u32, hash: &[u8]) -> SyncResult<()> {
        let key = format!("block_hash_{}", height);
        self.db.put(key.as_bytes(), hash).map_err(|e| SyncError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_block_hash(&self, height: u32) -> SyncResult<Option<Vec<u8>>> {
        let key = format!("block_hash_{}", height);
        self.db.get(key.as_bytes()).map_err(|e| SyncError::Storage(e.to_string()))
    }

    async fn store_state_root(&mut self, height: u32, root: &[u8]) -> SyncResult<()> {
        let key = format!("state_root_{}", height);
        self.db.put(key.as_bytes(), root).map_err(|e| SyncError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_state_root(&self, height: u32) -> SyncResult<Option<Vec<u8>>> {
        let key = format!("state_root_{}", height);
        self.db.get(key.as_bytes()).map_err(|e| SyncError::Storage(e.to_string()))
    }

    async fn rollback_to_height(&mut self, height: u32) -> SyncResult<()> {
        let current_height = self.get_indexed_height().await?;
        for h in (height + 1)..=current_height {
            let block_hash_key = format!("block_hash_{}", h);
            let state_root_key = format!("state_root_{}", h);
            self.db.delete(block_hash_key.as_bytes()).map_err(|e| SyncError::Storage(e.to_string()))?;
            self.db.delete(state_root_key.as_bytes()).map_err(|e| SyncError::Storage(e.to_string()))?;
        }
        self.set_indexed_height(height).await?;
        Ok(())
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn get_stats(&self) -> SyncResult<StorageStats> {
        let indexed_height = self.get_indexed_height().await?;
        Ok(StorageStats {
            total_entries: 0,
            indexed_height,
            storage_size_bytes: None,
        })
    }
    
    async fn get_db_handle(&self) -> SyncResult<Arc<DB>> {
        Ok(self.db.clone())
    }
}

pub struct NativeRuntimeAdapter;

#[async_trait]
impl RuntimeAdapter for NativeRuntimeAdapter {
    async fn process_block(&mut self, _height: u32, _block_data: &[u8]) -> SyncResult<()> {
        unimplemented!("process_block is not used in the native indexer")
    }

    async fn process_block_atomic(
        &mut self,
        height: u32,
        block_data: &[u8],
        block_hash: &[u8],
    ) -> SyncResult<AtomicBlockResult> {
        alkanes_indexer(height, block_data).map_err(|e| SyncError::Runtime(e.to_string()))?;
        Ok(AtomicBlockResult {
            state_root: vec![0; 32], // This will be replaced with the actual state root
            batch_data: vec![],      // This will be replaced with the actual batch data
            height,
            block_hash: block_hash.to_vec(),
        })
    }

    async fn execute_view(&self, _call: ViewCall) -> SyncResult<ViewResult> {
        Ok(ViewResult {
            data: vec![],
        })
    }

    async fn execute_preview(&self, _call: PreviewCall) -> SyncResult<ViewResult> {
        Ok(ViewResult {
            data: vec![],
        })
    }

    async fn get_state_root(&self, _height: u32) -> SyncResult<Vec<u8>> {
        Ok(vec![0; 32])
    }

    async fn refresh_memory(&mut self) -> SyncResult<()> {
        Ok(())
    }

    async fn is_ready(&self) -> bool {
        true
    }

    async fn get_stats(&self) -> SyncResult<RuntimeStats> {
        Ok(RuntimeStats {
            memory_usage_bytes: 0,
            blocks_processed: 0,
            last_refresh_height: None,
        })
    }

    async fn get_prefix_root(&self, _name: &str, _height: u32) -> SyncResult<Option<[u8; 32]>> {
        Ok(None)
    }

    async fn log_prefix_roots(&self) -> SyncResult<()> {
        Ok(())
    }
}