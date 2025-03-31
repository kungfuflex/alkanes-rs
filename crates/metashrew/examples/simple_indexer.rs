//! A simple example of a Metashrew indexer program.

use anyhow::Result;
use metashrew_core::{metashrew_indexer, indexer::{Indexer, KeyValueStore}};
use serde::{Deserialize, Serialize};

/// A simple indexer that counts blocks
struct BlockCounter {
    store: KeyValueStore,
    block_count: u32,
}

impl Default for BlockCounter {
    fn default() -> Self {
        Self {
            store: KeyValueStore::new(),
            block_count: 0,
        }
    }
}

impl Indexer for BlockCounter {
    fn index_block(&mut self, height: u32, _block: &[u8]) -> Result<()> {
        // Increment the block count
        self.block_count += 1;
        
        // Store the block count
        self.store.set(b"block_count".to_vec(), self.block_count.to_le_bytes().to_vec());
        
        // Store the last processed height
        self.store.set(b"last_height".to_vec(), height.to_le_bytes().to_vec());
        
        Ok(())
    }
    
    fn flush(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        Ok(self.store.pairs())
    }
}

/// Input for the get_stats view function
#[derive(Deserialize)]
struct GetStatsInput {
    include_details: bool,
}

/// Output for the get_stats view function
#[derive(Serialize)]
struct GetStatsOutput {
    block_count: u32,
    last_height: u32,
    details: Option<String>,
}

impl BlockCounter {
    /// Get statistics about the indexed blocks
    fn get_stats(&self, input: GetStatsInput) -> Result<GetStatsOutput> {
        let block_count = if let Some(count) = self.store.get(b"block_count") {
            u32::from_le_bytes([count[0], count[1], count[2], count[3]])
        } else {
            0
        };
        
        let last_height = if let Some(height) = self.store.get(b"last_height") {
            u32::from_le_bytes([height[0], height[1], height[2], height[3]])
        } else {
            0
        };
        
        let details = if input.include_details {
            Some(format!("Processed {} blocks up to height {}", block_count, last_height))
        } else {
            None
        };
        
        Ok(GetStatsOutput {
            block_count,
            last_height,
            details,
        })
    }
}

// Define the Metashrew indexer program
metashrew_indexer! {
    struct BlockCounterProgram {
        indexer: BlockCounter,
        views: {
            "get_stats" => get_stats(GetStatsInput) -> GetStatsOutput,
        }
    }
}

// This is just for the example to compile
fn main() {
    println!("This is an example of a Metashrew indexer program.");
    println!("It should be compiled to WebAssembly and loaded by Metashrew.");
}
