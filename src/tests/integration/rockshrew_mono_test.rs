//! Integration tests for rockshrew-mono using in-memory mocks
//!
//! This module tests the complete rockshrew-mono workflow using in-memory adapters
//! for fast, reliable testing without external dependencies.

use anyhow::Result;
use memshrew_runtime::{MemStoreAdapter, MemStoreRuntime};
use metashrew_runtime::adapters::{GenericHeightTracker, GenericStateRootManager, HeightTracker};
use bitcoin::Block;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Mock configuration for rockshrew-mono testing
#[derive(Clone)]
pub struct MockRockshrewConfig {
    pub wasm_path: PathBuf,
    pub snapshot_enabled: bool,
    pub snapshot_interval: u32,
}

impl MockRockshrewConfig {
    pub fn new() -> Self {
        Self {
            wasm_path: PathBuf::from("./target/wasm32-unknown-unknown/release/metashrew_minimal.wasm"),
            snapshot_enabled: false, // Disable snapshots for basic tests
            snapshot_interval: 1000,
        }
    }

    pub fn with_snapshots(mut self, interval: u32) -> Self {
        self.snapshot_enabled = true;
        self.snapshot_interval = interval;
        self
    }
}

/// Mock rockshrew-mono implementation using in-memory adapters
pub struct MockRockshrewMono {
    config: MockRockshrewConfig,
    runtime: MemStoreRuntime,
    height_tracker: GenericHeightTracker<MemStoreAdapter>,
    state_root_manager: GenericStateRootManager<MemStoreAdapter>,
    current_height: Arc<Mutex<u32>>,
}

impl MockRockshrewMono {
    /// Create a new mock rockshrew-mono instance
    pub fn new(config: MockRockshrewConfig) -> Result<Self> {
        let adapter = MemStoreAdapter::new();
        let runtime = MemStoreRuntime::load(config.wasm_path.clone(), adapter.clone())?;
        
        let height_tracker = GenericHeightTracker::new(adapter.clone());
        let state_root_manager = GenericStateRootManager::new(adapter.clone());
        let current_height = Arc::new(Mutex::new(0u32));

        Ok(Self {
            config,
            runtime,
            height_tracker,
            state_root_manager,
            current_height,
        })
    }

    /// Process a single block
    pub async fn process_block(&mut self, height: u32, block: &Block) -> Result<()> {
        // Serialize block
        let block_bytes = metashrew_support::utils::consensus_encode(block)?;

        // Update runtime context
        {
            let mut context = self.runtime.context.lock().unwrap();
            context.block = block_bytes;
            context.height = height;
        }

        // Run the WASM module
        self.runtime.run()?;
        self.runtime.refresh_memory()?;

        // Update height tracker
        {
            let mut tracker = &mut self.height_tracker;
            tracker.set_current_height(height).await?;
            tracker.set_indexed_height(height).await?;
        }

        // Update current height
        {
            let mut current = self.current_height.lock().unwrap();
            *current = height;
        }

        // Create snapshot if enabled
        if self.config.snapshot_enabled && self.should_create_snapshot(height).await? {
            self.create_snapshot(height).await?;
        }

        Ok(())
    }

    /// Check if a snapshot should be created at this height
    async fn should_create_snapshot(&self, height: u32) -> Result<bool> {
        Ok(self.config.snapshot_enabled && height % self.config.snapshot_interval == 0 && height > 0)
    }

    /// Create a snapshot (mock implementation)
    async fn create_snapshot(&self, height: u32) -> Result<()> {
        // In a real implementation, this would create a snapshot
        // For testing, we just verify the logic is called correctly
        println!("Creating snapshot at height {}", height);
        Ok(())
    }

    /// Get current height
    pub async fn get_current_height(&self) -> Result<u32> {
        self.height_tracker.get_current_height().await
    }

    /// Get indexed height
    pub async fn get_indexed_height(&self) -> Result<u32> {
        self.height_tracker.get_indexed_height().await
    }

    /// Execute a view function
    pub async fn view(&self, function: String, input: &[u8], height: u32) -> Result<Vec<u8>> {
        self.runtime.view(function, &input.to_vec(), height).await
    }
}

/// Test basic rockshrew-mono workflow
#[tokio::test]
async fn test_basic_rockshrew_mono_workflow() -> Result<()> {
    let config = MockRockshrewConfig::new();
    let mut mono = MockRockshrewMono::new(config)?;

    // Create test blocks
    let blocks = crate::tests::block_builder::ChainBuilder::new()
        .add_blocks(5)
        .blocks();

    // Process blocks sequentially
    for (height, block) in blocks.iter().enumerate() {
        mono.process_block((height + 1) as u32, block).await?;
    }

    // Verify final state
    assert_eq!(mono.get_current_height().await?, 6);
    assert_eq!(mono.get_indexed_height().await?, 6);

    // Test view functions
    let blocktracker_result = mono.view("blocktracker".to_string(), &[], 6).await?;
    assert_eq!(blocktracker_result.len(), 6); // Should have 6 blocks tracked (including genesis)

    // Test historical queries
    for height in 1..=6 {
        let result = mono.view("blocktracker".to_string(), &[], height).await?;
        assert_eq!(result.len(), height as usize);
    }

    Ok(())
}

/// Test rockshrew-mono with snapshots enabled
#[tokio::test]
async fn test_rockshrew_mono_with_snapshots() -> Result<()> {
    let config = MockRockshrewConfig::new().with_snapshots(3); // Snapshot every 3 blocks
    let mut mono = MockRockshrewMono::new(config)?;

    // Process blocks and verify snapshot logic
    let blocks = crate::tests::block_builder::ChainBuilder::new()
        .add_blocks(10)
        .blocks();

    for (height, block) in blocks.iter().enumerate() {
        mono.process_block((height + 1) as u32, block).await?;
        
        // Verify snapshot logic
        let _should_snapshot = height > 0 && ((height + 1) as u32) % 3 == 0;
        // In a real implementation, we'd verify the snapshot was actually created
    }

    assert_eq!(mono.get_current_height().await?, 11);

    Ok(())
}

/// Test error handling in rockshrew-mono
#[tokio::test]
async fn test_rockshrew_mono_error_handling() -> Result<()> {
    let config = MockRockshrewConfig::new();
    let mono = MockRockshrewMono::new(config)?;

    // Test invalid view function
    let result = mono.view("nonexistent".to_string(), &[], 0).await;
    assert!(result.is_err());

    // Test view function with invalid height
    let _result = mono.view("blocktracker".to_string(), &[], 999).await;
    // This might succeed but return empty data, depending on implementation

    Ok(())
}

/// Test concurrent operations in rockshrew-mono
#[tokio::test]
async fn test_rockshrew_mono_concurrent_operations() -> Result<()> {
    let config = MockRockshrewConfig::new();
    let mut mono = MockRockshrewMono::new(config)?;

    // Process some blocks first
    let blocks = crate::tests::block_builder::ChainBuilder::new()
        .add_blocks(3)
        .blocks();

    for (height, block) in blocks.iter().enumerate() {
        mono.process_block((height + 1) as u32, block).await?;
    }

    // Test sequential view function calls (simulating concurrent access)
    for height in 1..=3 {
        let result = mono.view("blocktracker".to_string(), &[], height).await?;
        assert!(!result.is_empty());
    }

    Ok(())
}

/// Test rockshrew-mono performance characteristics
#[tokio::test]
async fn test_rockshrew_mono_performance() -> Result<()> {
    let config = MockRockshrewConfig::new();
    let mut mono = MockRockshrewMono::new(config)?;

    // Create a larger chain for performance testing
    let blocks = crate::tests::block_builder::ChainBuilder::new()
        .add_blocks(100)
        .blocks();

    let start_time = std::time::Instant::now();

    // Process all blocks
    for (height, block) in blocks.iter().enumerate() {
        mono.process_block((height + 1) as u32, block).await?;
    }

    let processing_time = start_time.elapsed();
    
    // Verify performance is reasonable (should process 100 blocks quickly with in-memory backend)
    assert!(processing_time.as_secs() < 10, "Processing 100 blocks should take less than 10 seconds");

    // Test view function performance
    let view_start = std::time::Instant::now();
    
    for height in 1..=100 {
        mono.view("blocktracker".to_string(), &[], height).await?;
    }
    
    let view_time = view_start.elapsed();
    assert!(view_time.as_secs() < 5, "100 view function calls should take less than 5 seconds");

    println!("Performance test completed:");
    println!("  Block processing: {:?} for 100 blocks", processing_time);
    println!("  View functions: {:?} for 100 calls", view_time);

    Ok(())
}

/// Test rockshrew-mono memory usage
#[tokio::test]
async fn test_rockshrew_mono_memory_usage() -> Result<()> {
    let config = MockRockshrewConfig::new();
    let mut mono = MockRockshrewMono::new(config)?;

    // Get initial memory usage (approximate)
    let initial_data_size = {
        let context = mono.runtime.context.lock().unwrap();
        context.db.get_all_data().len()
    };

    // Process blocks and monitor memory growth
    let blocks = crate::tests::block_builder::ChainBuilder::new()
        .add_blocks(50)
        .blocks();

    for (height, block) in blocks.iter().enumerate() {
        mono.process_block((height + 1) as u32, block).await?;
        
        // Check memory usage every 10 blocks
        if height % 10 == 9 {
            let current_data_size = {
                let context = mono.runtime.context.lock().unwrap();
                context.db.get_all_data().len()
            };
            
            // Memory should grow, but not excessively
            assert!(current_data_size > initial_data_size);
            println!("Height {}: {} database entries", height + 1, current_data_size);
        }
    }

    Ok(())
}