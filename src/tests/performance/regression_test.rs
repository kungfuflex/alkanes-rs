//! Performance regression tests
//!
//! This module contains benchmarks and performance tests to detect regressions
//! in the Metashrew indexing system after refactoring.

use anyhow::Result;
use memshrew_runtime::{MemStoreAdapter, MemStoreRuntime};
use metashrew_runtime::adapters::{GenericHeightTracker, HeightTracker};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Performance test configuration
#[derive(Clone, Debug)]
pub struct PerformanceConfig {
    pub wasm_path: PathBuf,
    pub block_count: usize,
    pub view_function_calls: usize,
    pub concurrent_operations: usize,
}

impl PerformanceConfig {
    pub fn new() -> Self {
        Self {
            wasm_path: PathBuf::from("./target/wasm32-unknown-unknown/release/metashrew_minimal.wasm"),
            block_count: 100,
            view_function_calls: 100,
            concurrent_operations: 10,
        }
    }

    pub fn with_block_count(mut self, count: usize) -> Self {
        self.block_count = count;
        self
    }

    pub fn stress_test() -> Self {
        Self {
            wasm_path: PathBuf::from("./target/wasm32-unknown-unknown/release/metashrew_minimal.wasm"),
            block_count: 1000,
            view_function_calls: 1000,
            concurrent_operations: 50,
        }
    }
}

/// Performance benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    pub block_processing_time: Duration,
    pub blocks_per_second: f64,
    pub view_function_time: Duration,
    pub views_per_second: f64,
    pub memory_usage_bytes: usize,
    pub database_entries: usize,
}

impl BenchmarkResults {
    /// Check if results meet performance thresholds
    pub fn meets_thresholds(&self) -> bool {
        // Define performance thresholds
        const MIN_BLOCKS_PER_SECOND: f64 = 10.0;
        const MIN_VIEWS_PER_SECOND: f64 = 50.0;
        const MAX_MEMORY_PER_BLOCK: usize = 10_000; // bytes

        let memory_per_block = self.memory_usage_bytes / self.blocks_per_second as usize;

        self.blocks_per_second >= MIN_BLOCKS_PER_SECOND
            && self.views_per_second >= MIN_VIEWS_PER_SECOND
            && memory_per_block <= MAX_MEMORY_PER_BLOCK
    }

    /// Generate performance report
    pub fn report(&self) -> String {
        format!(
            "Performance Benchmark Results:\n\
             - Block Processing: {:.2} blocks/sec ({:?} total)\n\
             - View Functions: {:.2} views/sec ({:?} total)\n\
             - Memory Usage: {} bytes ({} database entries)\n\
             - Meets Thresholds: {}",
            self.blocks_per_second,
            self.block_processing_time,
            self.views_per_second,
            self.view_function_time,
            self.memory_usage_bytes,
            self.database_entries,
            self.meets_thresholds()
        )
    }
}

/// Performance benchmark runner
pub struct PerformanceBenchmark {
    config: PerformanceConfig,
    runtime: MemStoreRuntime,
    height_tracker: GenericHeightTracker<MemStoreAdapter>,
}

impl PerformanceBenchmark {
    /// Create a new performance benchmark
    pub fn new(config: PerformanceConfig) -> Result<Self> {
        let adapter = MemStoreAdapter::new();
        let runtime = MemStoreRuntime::load(config.wasm_path.clone(), adapter.clone())?;
        let height_tracker = GenericHeightTracker::new(adapter);

        Ok(Self {
            config,
            runtime,
            height_tracker,
        })
    }

    /// Run complete performance benchmark
    pub async fn run_benchmark(&mut self) -> Result<BenchmarkResults> {
        // Benchmark block processing
        let block_processing_time = self.benchmark_block_processing().await?;
        let blocks_per_second = self.config.block_count as f64 / block_processing_time.as_secs_f64();

        // Benchmark view function calls
        let view_function_time = self.benchmark_view_functions().await?;
        let views_per_second = self.config.view_function_calls as f64 / view_function_time.as_secs_f64();

        // Measure memory usage
        let (memory_usage_bytes, database_entries) = self.measure_memory_usage();

        Ok(BenchmarkResults {
            block_processing_time,
            blocks_per_second,
            view_function_time,
            views_per_second,
            memory_usage_bytes,
            database_entries,
        })
    }

    /// Benchmark block processing performance
    async fn benchmark_block_processing(&mut self) -> Result<Duration> {
        let blocks = crate::tests::block_builder::ChainBuilder::new()
            .add_blocks(self.config.block_count as u32)
            .blocks();

        let start_time = Instant::now();

        for (height, block) in blocks.iter().enumerate() {
            let block_bytes = metashrew_support::utils::consensus_encode(block)?;

            {
                let mut context = self.runtime.context.lock().unwrap();
                context.block = block_bytes;
                context.height = height as u32;
            }

            self.runtime.run()?;
            self.runtime.refresh_memory()?;

            // Update height tracker
            {
                let mut tracker = &mut self.height_tracker;
                tracker.set_current_height(height as u32).await?;
            }
        }

        Ok(start_time.elapsed())
    }

    /// Benchmark view function performance
    async fn benchmark_view_functions(&self) -> Result<Duration> {
        let final_height = (self.config.block_count - 1) as u32;
        let start_time = Instant::now();

        for i in 0..self.config.view_function_calls {
            let height = i as u32 % (final_height + 1);
            
            // Test blocktracker view function
            let _result = self.runtime
                .view("blocktracker".to_string(), &vec![], height)
                .await?;

            // Test getblock view function every 10th call
            if i % 10 == 0 {
                let height_input = height.to_le_bytes().to_vec();
                let _result = self.runtime
                    .view("getblock".to_string(), &height_input, height)
                    .await?;
            }
        }

        Ok(start_time.elapsed())
    }

    /// Measure memory usage
    fn measure_memory_usage(&self) -> (usize, usize) {
        let adapter = &self.runtime.context.lock().unwrap().db;
        let all_data = adapter.get_all_data();
        
        let total_bytes = all_data.iter()
            .map(|(key, value)| key.len() + value.len())
            .sum();

        (total_bytes, all_data.len())
    }

    /// Run concurrent operations benchmark (simplified to sequential for now)
    pub async fn benchmark_concurrent_operations(&self) -> Result<Duration> {
        let final_height = (self.config.block_count - 1) as u32;
        let total_operations = self.config.view_function_calls;

        let start_time = Instant::now();

        // Run operations sequentially (simulating concurrent load)
        for i in 0..total_operations {
            let height = i as u32 % (final_height + 1);
            let _result = self.runtime
                .view("blocktracker".to_string(), &vec![], height)
                .await?;
        }

        Ok(start_time.elapsed())
    }
}

/// Test basic performance benchmarks
#[tokio::test]
async fn test_basic_performance_benchmark() -> Result<()> {
    let config = PerformanceConfig::new().with_block_count(50);
    let mut benchmark = PerformanceBenchmark::new(config)?;

    let results = benchmark.run_benchmark().await?;
    println!("{}", results.report());

    // Verify performance meets basic thresholds
    assert!(results.blocks_per_second > 5.0, "Block processing too slow: {:.2} blocks/sec", results.blocks_per_second);
    assert!(results.views_per_second > 20.0, "View functions too slow: {:.2} views/sec", results.views_per_second);

    Ok(())
}

/// Test performance with larger dataset
#[tokio::test]
async fn test_large_dataset_performance() -> Result<()> {
    let config = PerformanceConfig::new().with_block_count(200);
    let mut benchmark = PerformanceBenchmark::new(config)?;

    let results = benchmark.run_benchmark().await?;
    println!("Large dataset {}", results.report());

    // Performance should scale reasonably with larger datasets
    assert!(results.blocks_per_second > 3.0, "Large dataset block processing too slow");
    assert!(results.views_per_second > 15.0, "Large dataset view functions too slow");

    Ok(())
}

/// Test concurrent operations performance
#[tokio::test]
async fn test_concurrent_operations_performance() -> Result<()> {
    let config = PerformanceConfig::new().with_block_count(100);
    let mut benchmark = PerformanceBenchmark::new(config.clone())?;

    // First run the benchmark to populate data
    let _results = benchmark.run_benchmark().await?;

    // Then test concurrent operations
    let concurrent_time = benchmark.benchmark_concurrent_operations().await?;
    let concurrent_ops_per_second = config.view_function_calls as f64 / concurrent_time.as_secs_f64();

    println!("Concurrent operations: {:.2} ops/sec ({:?} total)", 
             concurrent_ops_per_second, concurrent_time);

    assert!(concurrent_ops_per_second > 30.0, "Concurrent operations too slow");

    Ok(())
}

/// Memory usage regression test
#[tokio::test]
async fn test_memory_usage_regression() -> Result<()> {
    let config = PerformanceConfig::new().with_block_count(100);
    let mut benchmark = PerformanceBenchmark::new(config.clone())?;

    let results = benchmark.run_benchmark().await?;
    
    // Memory usage should be reasonable
    let memory_per_block = results.memory_usage_bytes / config.block_count;
    println!("Memory usage per block: {} bytes", memory_per_block);

    // Should not use more than 5KB per block on average
    assert!(memory_per_block < 5_000, "Memory usage per block too high: {} bytes", memory_per_block);

    // Database should not have excessive entries
    let entries_per_block = results.database_entries / config.block_count;
    println!("Database entries per block: {}", entries_per_block);

    assert!(entries_per_block < 50, "Too many database entries per block: {}", entries_per_block);

    Ok(())
}

/// Stress test for performance limits
#[tokio::test]
#[ignore] // Ignore by default as this is a long-running test
async fn test_stress_performance() -> Result<()> {
    let config = PerformanceConfig::stress_test();
    let mut benchmark = PerformanceBenchmark::new(config.clone())?;

    println!("Running stress test with {} blocks and {} view calls", 
             config.block_count, config.view_function_calls);

    let start_time = Instant::now();
    let results = benchmark.run_benchmark().await?;
    let total_time = start_time.elapsed();

    println!("Stress test completed in {:?}", total_time);
    println!("{}", results.report());

    // Even under stress, should maintain minimum performance
    assert!(results.blocks_per_second > 2.0, "Stress test block processing too slow");
    assert!(results.views_per_second > 10.0, "Stress test view functions too slow");

    // Total test should complete within reasonable time
    assert!(total_time.as_secs() < 300, "Stress test took too long: {:?}", total_time);

    Ok(())
}

/// Performance comparison test between different configurations
#[tokio::test]
async fn test_performance_scaling() -> Result<()> {
    let small_config = PerformanceConfig::new().with_block_count(25);
    let large_config = PerformanceConfig::new().with_block_count(100);

    let mut small_benchmark = PerformanceBenchmark::new(small_config)?;
    let mut large_benchmark = PerformanceBenchmark::new(large_config)?;

    let small_results = small_benchmark.run_benchmark().await?;
    let large_results = large_benchmark.run_benchmark().await?;

    println!("Small dataset: {}", small_results.report());
    println!("Large dataset: {}", large_results.report());

    // Performance should not degrade too much with larger datasets
    let performance_ratio = large_results.blocks_per_second / small_results.blocks_per_second;
    println!("Performance scaling ratio: {:.2}", performance_ratio);

    assert!(performance_ratio > 0.5, "Performance degrades too much with larger datasets");

    Ok(())
}

/// Test adapter performance comparison
#[tokio::test]
async fn test_adapter_performance_comparison() -> Result<()> {
    // Test in-memory adapter performance
    let config = PerformanceConfig::new().with_block_count(50);
    let mut mem_benchmark = PerformanceBenchmark::new(config.clone())?;
    let mem_results = mem_benchmark.run_benchmark().await?;

    println!("In-memory adapter: {}", mem_results.report());

    // For comparison, we could test RocksDB adapter here if we had a similar benchmark setup
    // This demonstrates how to compare different storage backends

    // Verify in-memory adapter meets performance expectations
    assert!(mem_results.meets_thresholds(), "In-memory adapter should meet performance thresholds");

    Ok(())
}