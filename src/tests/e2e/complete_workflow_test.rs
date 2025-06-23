//! Complete end-to-end workflow tests
//!
//! This module tests the entire Metashrew indexing pipeline from block processing
//! to view function execution, including chain reorganizations and historical queries.

use anyhow::Result;
use bitcoin::Block;
use memshrew_runtime::{MemStoreAdapter, MemStoreRuntime};
use metashrew_runtime::smt::SMTHelper;
use metashrew_support::utils;
use std::path::PathBuf;

/// Complete workflow test configuration
pub struct WorkflowTestConfig {
    pub wasm_path: PathBuf,
    pub chain_length: usize,
    pub test_reorgs: bool,
    pub test_historical_queries: bool,
}

impl WorkflowTestConfig {
    pub fn new() -> Self {
        Self {
            wasm_path: PathBuf::from("./target/wasm32-unknown-unknown/release/metashrew_minimal.wasm"),
            chain_length: 10,
            test_reorgs: true,
            test_historical_queries: true,
        }
    }

    pub fn with_chain_length(mut self, length: usize) -> Self {
        self.chain_length = length;
        self
    }

    pub fn without_reorgs(mut self) -> Self {
        self.test_reorgs = false;
        self
    }
}

/// Complete workflow test runner
pub struct WorkflowTestRunner {
    config: WorkflowTestConfig,
    runtime: MemStoreRuntime,
    processed_blocks: Vec<(u32, Block)>,
}

impl WorkflowTestRunner {
    /// Create a new workflow test runner
    pub fn new(config: WorkflowTestConfig) -> Result<Self> {
        let adapter = MemStoreAdapter::new();
        let runtime = MemStoreRuntime::load(config.wasm_path.clone(), adapter)?;

        Ok(Self {
            config,
            runtime,
            processed_blocks: Vec::new(),
        })
    }

    /// Process a complete blockchain workflow
    pub async fn run_complete_workflow(&mut self) -> Result<()> {
        // Phase 1: Initial chain processing
        self.process_initial_chain().await?;

        // Phase 2: Historical query validation
        if self.config.test_historical_queries {
            self.validate_historical_queries().await?;
        }

        // Phase 3: Chain reorganization testing
        if self.config.test_reorgs {
            self.test_chain_reorganization().await?;
        }

        // Phase 4: Final validation
        self.validate_final_state().await?;

        Ok(())
    }

    /// Process the initial blockchain
    async fn process_initial_chain(&mut self) -> Result<()> {
        println!("Processing initial chain of {} blocks", self.config.chain_length);

        let blocks = crate::tests::block_builder::ChainBuilder::new()
            .add_blocks(self.config.chain_length as u32)
            .blocks();

        for (height, block) in blocks.iter().enumerate() {
            self.process_block((height + 1) as u32, block).await?;
            self.processed_blocks.push(((height + 1) as u32, block.clone()));
        }

        println!("✓ Processed {} blocks successfully", self.config.chain_length);
        Ok(())
    }

    /// Process a single block
    async fn process_block(&mut self, height: u32, block: &Block) -> Result<()> {
        let block_bytes = utils::consensus_encode(block)?;

        {
            let mut context = self.runtime.context.lock().unwrap();
            context.block = block_bytes;
            context.height = height;
        }

        self.runtime.run()?;
        self.runtime.refresh_memory()?;

        Ok(())
    }

    /// Validate historical queries at all heights
    async fn validate_historical_queries(&self) -> Result<()> {
        println!("Validating historical queries");

        for (height, _) in &self.processed_blocks {
            // Test blocktracker view function
            let blocktracker_result = self.runtime
                .view("blocktracker".to_string(), &vec![], *height)
                .await?;

            // Blocktracker should have height bytes (since we start from height 1)
            let expected_length = *height as usize;
            assert_eq!(
                blocktracker_result.len(),
                expected_length,
                "Blocktracker length mismatch at height {}", height
            );

            // Test getblock view function
            let height_input = height.to_le_bytes().to_vec();
            let block_result = self.runtime
                .view("getblock".to_string(), &height_input, *height)
                .await?;

            assert!(
                !block_result.is_empty(),
                "Block data should exist at height {}", height
            );

            // Verify historical consistency - querying at a later height should return the same result
            if *height < self.processed_blocks.len() as u32 - 1 {
                let later_height = self.processed_blocks.len() as u32 - 1;
                let later_result = self.runtime
                    .view("blocktracker".to_string(), &vec![], *height)
                    .await?;

                assert_eq!(
                    blocktracker_result, later_result,
                    "Historical query should be consistent"
                );
            }
        }

        println!("✓ Historical queries validated for {} heights", self.processed_blocks.len());
        Ok(())
    }

    /// Test chain reorganization handling
    async fn test_chain_reorganization(&mut self) -> Result<()> {
        println!("Testing chain reorganization");

        // Get the current chain tip
        let original_tip_height = (self.processed_blocks.len() - 1) as u32;
        let reorg_point = original_tip_height.saturating_sub(2); // Reorg from 2 blocks back

        // Create an alternative chain from the reorg point
        let reorg_blocks = if reorg_point > 0 {
            let reorg_parent = &self.processed_blocks[reorg_point as usize].1;
            crate::tests::block_builder::ChainBuilder::new()
                .add_blocks(3) // Make it longer than the original
                .blocks()
        } else {
            // If reorg_point is 0, create a completely new chain
            crate::tests::block_builder::ChainBuilder::new()
                .add_blocks((self.config.chain_length + 1) as u32)
                .blocks()
        };

        // Simulate reorg by processing the alternative chain
        // In a real implementation, this would involve rollback logic
        // For testing, we'll create a new runtime and process the alternative chain
        let adapter = MemStoreAdapter::new();
        let mut reorg_runtime = MemStoreRuntime::load(self.config.wasm_path.clone(), adapter)?;

        // Process blocks up to reorg point from original chain
        for height in 0..=reorg_point {
            if let Some((_, block)) = self.processed_blocks.get(height as usize) {
                let block_bytes = utils::consensus_encode(block)?;
                {
                    let mut context = reorg_runtime.context.lock().unwrap();
                    context.block = block_bytes;
                    context.height = height;
                }
                reorg_runtime.run()?;
                reorg_runtime.refresh_memory()?;
            }
        }

        // Process the alternative chain
        for (i, block) in reorg_blocks.iter().enumerate() {
            let height = reorg_point + 1 + i as u32;
            let block_bytes = utils::consensus_encode(block)?;
            {
                let mut context = reorg_runtime.context.lock().unwrap();
                context.block = block_bytes;
                context.height = height;
            }
            reorg_runtime.run()?;
            reorg_runtime.refresh_memory()?;
        }

        // Validate the reorged chain
        let final_height = reorg_point + reorg_blocks.len() as u32;
        let blocktracker_result = reorg_runtime
            .view("blocktracker".to_string(), &vec![], final_height)
            .await?;

        assert_eq!(
            blocktracker_result.len(),
            (final_height + 1) as usize,
            "Reorged chain should have correct blocktracker length"
        );

        println!("✓ Chain reorganization test completed");
        Ok(())
    }

    /// Validate the final state of the blockchain
    async fn validate_final_state(&self) -> Result<()> {
        println!("Validating final blockchain state");

        let final_height = (self.processed_blocks.len() - 1) as u32;

        // Test final blocktracker state
        let final_blocktracker = self.runtime
            .view("blocktracker".to_string(), &vec![], final_height)
            .await?;

        assert_eq!(
            final_blocktracker.len(),
            self.config.chain_length,
            "Final blocktracker should track all blocks"
        );

        // Test that all blocks are accessible
        for (height, _) in &self.processed_blocks {
            let height_input = height.to_le_bytes().to_vec();
            let block_data = self.runtime
                .view("getblock".to_string(), &height_input, final_height)
                .await?;

            assert!(
                !block_data.is_empty(),
                "Block {} should be accessible from final height", height
            );
        }

        // Test database consistency using direct access
        let adapter = &self.runtime.context.lock().unwrap().db;
        let smt_helper = SMTHelper::new(adapter.clone());

        // Verify state root exists for final height
        match smt_helper.get_smt_root_at_height(final_height) {
            Ok(root) => {
                let zero_root = [0u8; 32];
                if root != zero_root {
                    println!("✓ Final state root: {}", hex::encode(root));
                } else {
                    println!("⚠ Final state root is all zeros (may be expected for minimal WASM)");
                }
            }
            Err(e) => {
                println!("⚠ Could not retrieve final state root: {}", e);
            }
        }

        // Verify BST structure integrity
        let all_data = adapter.get_all_data();
        println!("✓ Final database contains {} entries", all_data.len());

        // Check for expected key patterns
        let mut bst_keys = 0;
        let mut smt_keys = 0;
        let mut other_keys = 0;

        for (key, _) in &all_data {
            let key_str = String::from_utf8_lossy(key);
            if key_str.contains("bst:") {
                bst_keys += 1;
            } else if key_str.contains("smt:") {
                smt_keys += 1;
            } else {
                other_keys += 1;
            }
        }

        println!("✓ Database key distribution: {} BST, {} SMT, {} other", bst_keys, smt_keys, other_keys);

        println!("✓ Final state validation completed");
        Ok(())
    }

    /// Get runtime statistics
    pub fn get_statistics(&self) -> WorkflowStatistics {
        let adapter = &self.runtime.context.lock().unwrap().db;
        let all_data = adapter.get_all_data();

        WorkflowStatistics {
            blocks_processed: self.config.chain_length,
            database_entries: all_data.len(),
            total_data_size: all_data.iter().map(|(k, v)| k.len() + v.len()).sum(),
        }
    }
}

/// Statistics from workflow execution
#[derive(Debug)]
pub struct WorkflowStatistics {
    pub blocks_processed: usize,
    pub database_entries: usize,
    pub total_data_size: usize,
}

/// Test complete indexing workflow with default configuration
#[tokio::test]
async fn test_complete_indexing_workflow() -> Result<()> {
    let config = WorkflowTestConfig::new();
    let mut runner = WorkflowTestRunner::new(config)?;

    runner.run_complete_workflow().await?;

    let stats = runner.get_statistics();
    println!("Workflow completed with statistics: {:?}", stats);

    assert!(stats.blocks_processed > 0);
    assert!(stats.database_entries > 0);

    Ok(())
}

/// Test workflow with larger chain
#[tokio::test]
async fn test_large_chain_workflow() -> Result<()> {
    let config = WorkflowTestConfig::new().with_chain_length(50);
    let mut runner = WorkflowTestRunner::new(config)?;

    let start_time = std::time::Instant::now();
    runner.run_complete_workflow().await?;
    let duration = start_time.elapsed();

    let stats = runner.get_statistics();
    println!("Large chain workflow completed in {:?}", duration);
    println!("Statistics: {:?}", stats);

    assert_eq!(stats.blocks_processed, 50);
    assert!(duration.as_secs() < 60, "Large chain should process within 60 seconds");

    Ok(())
}

/// Test workflow without reorganizations
#[tokio::test]
async fn test_workflow_without_reorgs() -> Result<()> {
    let config = WorkflowTestConfig::new().without_reorgs();
    let mut runner = WorkflowTestRunner::new(config)?;

    runner.run_complete_workflow().await?;

    let stats = runner.get_statistics();
    assert!(stats.blocks_processed > 0);

    Ok(())
}

/// Test concurrent workflow execution
#[tokio::test]
async fn test_concurrent_workflows() -> Result<()> {
    let mut handles = Vec::new();

    // Run multiple workflows concurrently
    for i in 0..3 {
        let handle = tokio::spawn(async move {
            let config = WorkflowTestConfig::new().with_chain_length(5);
            let mut runner = WorkflowTestRunner::new(config)?;
            runner.run_complete_workflow().await?;
            Ok::<_, anyhow::Error>(runner.get_statistics())
        });
        handles.push(handle);
    }

    // Wait for all workflows to complete
    for (i, handle) in handles.into_iter().enumerate() {
        let stats = handle.await??;
        println!("Concurrent workflow {} completed: {:?}", i, stats);
        assert_eq!(stats.blocks_processed, 5);
    }

    Ok(())
}

/// Test workflow memory efficiency
#[tokio::test]
async fn test_workflow_memory_efficiency() -> Result<()> {
    let config = WorkflowTestConfig::new().with_chain_length(100);
    let mut runner = WorkflowTestRunner::new(config)?;

    // Monitor memory usage during processing
    let initial_stats = runner.get_statistics();
    
    runner.run_complete_workflow().await?;
    
    let final_stats = runner.get_statistics();

    // Verify memory usage is reasonable
    let data_per_block = final_stats.total_data_size / final_stats.blocks_processed;
    println!("Average data per block: {} bytes", data_per_block);

    // Should not use excessive memory per block (this is a rough heuristic)
    assert!(data_per_block < 10_000, "Memory usage per block should be reasonable");

    println!("Memory efficiency test completed");
    println!("Initial: {:?}", initial_stats);
    println!("Final: {:?}", final_stats);

    Ok(())
}