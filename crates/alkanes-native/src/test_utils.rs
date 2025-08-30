use anyhow::Result;
use async_trait::async_trait;
use bitcoin::{Block, BlockHash, Network as BitcoinNetwork};
use metashrew_sync::{BitcoinNodeAdapter, Result as SyncResult, StorageAdapter, SyncError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct MemStorageAdapter {
    pub db: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

#[async_trait]
impl StorageAdapter for MemStorageAdapter {
    async fn get(&self, key: &[u8]) -> SyncResult<Option<Vec<u8>>> {
        let db = self.db.lock().unwrap();
        Ok(db.get(key).cloned())
    }

    async fn put(&mut self, key: &[u8], value: &[u8]) -> SyncResult<()> {
        let mut db = self.db.lock().unwrap();
        db.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    async fn del(&mut self, key: &[u8]) -> SyncResult<()> {
        let mut db = self.db.lock().unwrap();
        db.remove(key);
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct MockNodeAdapter {
    pub blocks: Arc<Mutex<HashMap<u32, Block>>>,
}

#[async_trait]
impl BitcoinNodeAdapter for MockNodeAdapter {
    async fn get_block_hash(&self, height: u32) -> SyncResult<Vec<u8>> {
        let blocks = self.blocks.lock().unwrap();
        let block = blocks.get(&height).ok_or(SyncError::BitcoinNode("Block not found".to_string()))?;
        Ok(block.block_hash().to_vec())
    }

    async fn get_block_data(&self, hash: &[u8]) -> SyncResult<Vec<u8>> {
        let blocks = self.blocks.lock().unwrap();
        let block_hash = BlockHash::from_slice(hash).unwrap();
        let block = blocks.values().find(|b| b.block_hash() == block_hash).ok_or(SyncError::BitcoinNode("Block not found".to_string()))?;
        Ok(bitcoin::consensus::encode::serialize(block))
    }

    async fn get_block_info(&self, hash: &[u8]) -> SyncResult<(u32, Option<Vec<u8>>)> {
        let blocks = self.blocks.lock().unwrap();
        let block_hash = BlockHash::from_slice(hash).unwrap();
        let (height, block) = blocks.iter().find(|(_, b)| b.block_hash() == block_hash).ok_or(SyncError::BitcoinNode("Block not found".to_string()))?;
        Ok((*height, Some(block.header.prev_blockhash.to_vec())))
    }
}