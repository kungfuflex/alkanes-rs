//! Comprehensive benchmark tests comparing stateful vs non-stateful view performance
//!
//! This module contains benchmarks that demonstrate the performance benefits of
//! stateful WASM memory persistence between view calls. The tests use a modified
//! metashrew-minimal WASM module that creates 1000 storage entries and then
//! benchmarks reading all of them in a single view call.

use crate::runtime::MetashrewRuntime;
use crate::traits::KeyValueStoreLike;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// In-memory key-value store for testing
#[derive(Debug, Clone)]
pub struct InMemoryStore {
    data: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl KeyValueStoreLike for InMemoryStore {
    type Error = std::io::Error;
    type Batch = InMemoryBatch;

    fn write(&mut self, batch: Self::Batch) -> Result<(), Self::Error> {
        let mut data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        for (key, value_opt) in batch.operations {
            match value_opt {
                Some(value) => {
                    data.insert(key, value);
                }
                None => {
                    data.remove(&key);
                }
            }
        }
        Ok(())
    }

    fn get<K: AsRef<[u8]>>(&mut self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        Ok(data.get(key.as_ref()).cloned())
    }

    fn get_immutable<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        Ok(data.get(key.as_ref()).cloned())
    }

    fn put<K, V>(&mut self, key: K, value: V) -> Result<(), Self::Error>
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        let mut data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        data.insert(key.as_ref().to_vec(), value.as_ref().to_vec());
        Ok(())
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) -> Result<(), Self::Error> {
        let mut data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        data.remove(key.as_ref());
        Ok(())
    }

    fn scan_prefix<K: AsRef<[u8]>>(
        &self,
        prefix: K,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Self::Error> {
        let data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        let prefix_bytes = prefix.as_ref();
        Ok(data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix_bytes))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }

    fn create_batch(&self) -> Self::Batch {
        InMemoryBatch::default()
    }

    fn keys<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Vec<u8>> + 'a>, Self::Error> {
        let data = self.data.lock().map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::Other, format!("Lock error: {}", e))
        })?;
        let keys: Vec<Vec<u8>> = data.keys().cloned().collect();
        Ok(Box::new(keys.into_iter()))
    }

    fn create_isolated_copy(&self) -> Self {
        let data = self.data.lock().unwrap();
        let new_data = Arc::new(Mutex::new(data.clone()));
        Self { data: new_data }
    }

    fn track_kv_update(&mut self, _key: Vec<u8>, _value: Vec<u8>) {
        // No-op for in-memory store
    }
}

#[derive(Debug, Clone)]
pub struct InMemoryBatch {
    operations: Vec<(Vec<u8>, Option<Vec<u8>>)>,
}

impl InMemoryBatch {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }
}

impl crate::traits::BatchLike for InMemoryBatch {
    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, key: K, value: V) {
        self.operations
            .push((key.as_ref().to_vec(), Some(value.as_ref().to_vec())));
    }

    fn delete<K: AsRef<[u8]>>(&mut self, key: K) {
        self.operations.push((key.as_ref().to_vec(), None));
    }

    fn default() -> Self {
        Self::new()
    }
}

impl Default for InMemoryBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a simple Bitcoin block for testing
pub fn create_test_block() -> Vec<u8> {
    // Create a minimal valid Bitcoin block
    let mut block = Vec::new();

    // Block header (80 bytes)
    block.extend_from_slice(&[0u8; 80]);

    // Transaction count (1 transaction)
    block.push(1);

    // Transaction (minimal coinbase transaction)
    // Version
    block.extend_from_slice(&1u32.to_le_bytes());
    // Input count
    block.push(1);
    // Previous output hash (null for coinbase)
    block.extend_from_slice(&[0u8; 32]);
    // Previous output index (0xffffffff for coinbase)
    block.extend_from_slice(&0xffffffffu32.to_le_bytes());
    // Script length
    block.push(0);
    // Sequence
    block.extend_from_slice(&0xffffffffu32.to_le_bytes());
    // Output count
    block.push(0);
    // Lock time
    block.extend_from_slice(&0u32.to_le_bytes());

    block
}

/// Benchmark result containing timing and performance metrics
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub test_name: String,
    pub stateful_enabled: bool,
    pub view_calls: usize,
    pub total_duration: Duration,
    pub average_per_call: Duration,
    pub data_size: usize,
}

impl BenchmarkResult {
    pub fn new(
        test_name: String,
        stateful_enabled: bool,
        view_calls: usize,
        total_duration: Duration,
        data_size: usize,
    ) -> Self {
        let average_per_call = if view_calls > 0 {
            total_duration / view_calls as u32
        } else {
            Duration::ZERO
        };

        Self {
            test_name,
            stateful_enabled,
            view_calls,
            total_duration,
            average_per_call,
            data_size,
        }
    }

    pub fn print_summary(&self) {
        println!("=== {} ===", self.test_name);
        println!("Stateful Views: {}", self.stateful_enabled);
        println!("View Calls: {}", self.view_calls);
        println!("Total Duration: {:?}", self.total_duration);
        println!("Average per Call: {:?}", self.average_per_call);
        println!("Data Size: {} bytes", self.data_size);
        println!(
            "Throughput: {:.2} calls/sec",
            self.view_calls as f64 / self.total_duration.as_secs_f64()
        );
        println!();
    }
}

/// Run a benchmark test with the specified configuration
async fn run_benchmark_test(
    test_name: &str,
    stateful_enabled: bool,
    view_calls: usize,
    test_set: bool,
) -> Result<BenchmarkResult> {
    // Load the WASM module using absolute path
    let wasm_path = std::path::PathBuf::from(
        "../../target/wasm32-unknown-unknown/release/metashrew_minimal.wasm",
    );

    // Check if the WASM file exists
    if !wasm_path.exists() {
        return Err(anyhow::anyhow!("WASM file not found at {:?}. Please run: cargo build --target wasm32-unknown-unknown --release --package metashrew-minimal", wasm_path));
    }

    // Create storage backend
    let store = InMemoryStore::new();

    // Create runtime
    let mut runtime = MetashrewRuntime::load(wasm_path, store, vec![])?;

    // Configure stateful views
    if stateful_enabled {
        runtime.enable_stateful_views().await?;
        log::info!("Stateful views enabled for benchmark");
    } else {
        runtime.disable_stateful_views();
        log::info!("Stateful views disabled for benchmark");
    }

    // Process block 0 with benchmark data (creates 1000 storage entries)
    let block_data = create_test_block();

    // Set up the runtime to use the benchmark_main_impl function
    // We need to call the benchmark version that creates 1000 storage entries
    {
        let mut guard = runtime.context.lock().unwrap();
        guard.block = block_data.clone();
        guard.height = 0;
        guard.state = 0;
    }

    // Execute the benchmark main function to populate storage
    runtime.run()?;
    log::info!("Populated storage with 1000 entries for benchmark");

    if test_set {
        let result = runtime
            .view("benchmark_view_with_set".to_string(), &vec![], 0)
            .await?;
        assert_eq!(
            result.len(),
            1000,
            "Expected 1000 bytes from benchmark view"
        );
        assert!(
            result.iter().all(|&b| b == 0x02),
            "Expected all bytes to be 0x02"
        );
    }

    // Benchmark view function calls
    let start_time = Instant::now();
    let mut total_data_size = 0;

    for i in 0..view_calls {
        let input = vec![]; // Empty input for benchmark view
        let result = runtime
            .view("benchmark_view".to_string(), &input, 0)
            .await?;
        total_data_size = result.len(); // All calls should return the same size

        if i == 0 {
            log::info!("First view call returned {} bytes", result.len());
            // Verify we got the expected data (1000 bytes of 0x01)
            assert_eq!(
                result.len(),
                1000,
                "Expected 1000 bytes from benchmark view"
            );
            assert!(
                result.iter().all(|&b| b == 0x01),
                "Expected all bytes to be 0x01"
            );
        }

        // Log progress every 10 calls
        if (i + 1) % 10 == 0 {
            let elapsed = start_time.elapsed();
            log::info!(
                "Completed {}/{} view calls in {:?}",
                i + 1,
                view_calls,
                elapsed
            );
        }
    }

    let total_duration = start_time.elapsed();

    Ok(BenchmarkResult::new(
        test_name.to_string(),
        stateful_enabled,
        view_calls,
        total_duration,
        total_data_size,
    ))
}

/// Compare performance between stateful and non-stateful view modes
pub async fn compare_stateful_performance() -> Result<()> {
    println!("=== Stateful vs Non-Stateful View Performance Benchmark ===\n");

    // Test parameters
    let view_calls = 50; // Number of view calls to benchmark

    // Run benchmark with stateful views disabled
    println!("Running benchmark with stateful views DISABLED...");
    let non_stateful_result =
        run_benchmark_test("Non-Stateful Views", false, view_calls, false).await?;

    // Run benchmark with stateful views enabled
    println!("Running benchmark with stateful views ENABLED...");
    let stateful_result = run_benchmark_test("Stateful Views", true, view_calls, false).await?;

    // Print results
    non_stateful_result.print_summary();
    stateful_result.print_summary();

    // Calculate performance improvement
    let speedup = non_stateful_result.total_duration.as_nanos() as f64
        / stateful_result.total_duration.as_nanos() as f64;

    println!("=== Performance Comparison ===");
    println!(
        "Stateful views are {:.2}x faster than non-stateful views",
        speedup
    );
    println!(
        "Time saved per call: {:?}",
        non_stateful_result
            .average_per_call
            .saturating_sub(stateful_result.average_per_call)
    );
    println!(
        "Total time saved: {:?}",
        non_stateful_result
            .total_duration
            .saturating_sub(stateful_result.total_duration)
    );

    // Verify stateful views are faster (they should be)
    assert!(
        stateful_result.total_duration * 4 < non_stateful_result.total_duration,
        "Stateful views should be at least 4x faster than non-stateful views"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_stateful_vs_non_stateful_benchmark() {
        // Initialize logging
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .is_test(true)
            .try_init();

        // Run the benchmark comparison
        let result = compare_stateful_performance().await;

        match result {
            Ok(()) => {
                println!("Benchmark completed successfully!");
            }
            Err(e) => {
                eprintln!("Benchmark failed: {}", e);
                panic!("Benchmark test failed: {}", e);
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_single_stateful_view_call() {
        // Initialize logging
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .is_test(true)
            .try_init();

        // Test a single view call with stateful views enabled
        let result = run_benchmark_test("Single Stateful Call", true, 1, false).await;

        match result {
            Ok(benchmark_result) => {
                benchmark_result.print_summary();
                assert_eq!(benchmark_result.view_calls, 1);
                assert_eq!(benchmark_result.data_size, 1000);
                assert!(benchmark_result.stateful_enabled);
            }
            Err(e) => {
                eprintln!("Single stateful view test failed: {}", e);
                panic!("Single stateful view test failed: {}", e);
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_single_stateful_view_call_cannot_modify() {
        // Initialize logging
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .is_test(true)
            .try_init();

        // Test a single view call with stateful views enabled
        let result = run_benchmark_test("Single Stateful Call", true, 1, true).await;

        match result {
            Ok(benchmark_result) => {
                benchmark_result.print_summary();
                assert_eq!(benchmark_result.view_calls, 1);
                assert_eq!(benchmark_result.data_size, 1000);
                assert!(benchmark_result.stateful_enabled);
            }
            Err(e) => {
                eprintln!("Single stateful view test failed: {}", e);
                panic!("Single stateful view test failed: {}", e);
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_single_non_stateful_view_call() {
        // Initialize logging
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .is_test(true)
            .try_init();

        // Test a single view call with stateful views disabled
        let result = run_benchmark_test("Single Non-Stateful Call", false, 1, false).await;

        match result {
            Ok(benchmark_result) => {
                benchmark_result.print_summary();
                assert_eq!(benchmark_result.view_calls, 1);
                assert_eq!(benchmark_result.data_size, 1000);
                assert!(!benchmark_result.stateful_enabled);
            }
            Err(e) => {
                eprintln!("Single non-stateful view test failed: {}", e);
                panic!("Single non-stateful view test failed: {}", e);
            }
        }
    }
}
