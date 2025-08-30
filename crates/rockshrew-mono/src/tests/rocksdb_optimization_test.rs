//! Test suite for optimized RocksDB configuration
//!
//! This test verifies that the optimized RocksDB configuration works correctly
//! for large-scale deployment scenarios with the following characteristics:
//! - Database size: 500GB-2TB
//! - Key-value pairs: ~1.5 billion
//! - Key size: up to 256 bytes
//! - Value size: typically 64 bytes, up to 4MB
//! - Batch size: 50K-150K operations per batch
//! - Use case: Fast initial sync on multithreaded systems

use anyhow::Result;
use rocksdb::DB;
use rockshrew_runtime::optimized_config::create_optimized_options as create_optimized_rocksdb_options;
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

/// Test that the optimized RocksDB configuration can be created and opened successfully
#[tokio::test]
async fn test_optimized_rocksdb_creation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test_db");

    // Create optimized options
    let opts = create_optimized_rocksdb_options();

    // Verify we can open the database with optimized options
    let db = DB::open(&opts, &db_path)?;

    // Verify basic operations work
    db.put(b"test_key", b"test_value")?;
    let value = db.get(b"test_key")?;
    assert_eq!(value, Some(b"test_value".to_vec()));

    Ok(())
}

/// Test large batch operations similar to blockchain block processing
#[tokio::test]
async fn test_large_batch_operations() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("batch_test_db");

    let opts = create_optimized_rocksdb_options();
    let db = Arc::new(DB::open(&opts, &db_path)?);

    // Test batch sizes similar to blockchain processing (50K-150K operations)
    let batch_sizes = vec![50_000, 100_000, 150_000];

    for batch_size in batch_sizes {
        println!("Testing batch size: {}", batch_size);

        let start_time = Instant::now();

        // Create a large batch
        let mut batch = rocksdb::WriteBatch::default();
        for i in 0..batch_size {
            let key = format!("batch_key_{:08}", i);
            let value = format!(
                "batch_value_{:08}_with_some_additional_data_to_simulate_real_blockchain_data",
                i
            );
            batch.put(key.as_bytes(), value.as_bytes());
        }

        // Write the batch atomically
        db.write(batch)?;

        let duration = start_time.elapsed();
        println!("Batch of {} operations took: {:?}", batch_size, duration);

        // Verify some random keys from the batch
        for i in (0..batch_size).step_by(batch_size / 10) {
            let key = format!("batch_key_{:08}", i);
            let expected_value = format!(
                "batch_value_{:08}_with_some_additional_data_to_simulate_real_blockchain_data",
                i
            );
            let actual_value = db.get(key.as_bytes())?;
            assert_eq!(actual_value, Some(expected_value.into_bytes()));
        }
    }

    Ok(())
}

/// Test operations with various key and value sizes
#[tokio::test]
async fn test_variable_key_value_sizes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("size_test_db");

    let opts = create_optimized_rocksdb_options();
    let db = DB::open(&opts, &db_path)?;

    // Test different key sizes (up to 256 bytes as specified)
    let key_sizes = vec![32, 64, 128, 256];

    // Test different value sizes (64 bytes typical, up to 4MB as specified)
    let value_sizes = vec![64, 1024, 64 * 1024, 1024 * 1024, 4 * 1024 * 1024];

    for key_size in key_sizes {
        for value_size in &value_sizes {
            let key = vec![b'k'; key_size];
            let value = vec![b'v'; *value_size];

            let start_time = Instant::now();
            db.put(&key, &value)?;
            let write_duration = start_time.elapsed();

            let start_time = Instant::now();
            let retrieved_value = db.get(&key)?;
            let read_duration = start_time.elapsed();

            assert_eq!(retrieved_value, Some(value));

            println!(
                "Key size: {} bytes, Value size: {} bytes, Write: {:?}, Read: {:?}",
                key_size, value_size, write_duration, read_duration
            );
        }
    }

    Ok(())
}

/// Test concurrent operations to verify multithreaded performance
#[tokio::test]
async fn test_concurrent_operations() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("concurrent_test_db");

    let opts = create_optimized_rocksdb_options();
    let db = Arc::new(DB::open(&opts, &db_path)?);

    let num_threads = std::thread::available_parallelism()?.get().min(8);
    let operations_per_thread = 10_000;

    println!("Testing concurrent operations with {} threads", num_threads);

    let start_time = Instant::now();

    // Spawn multiple threads to perform concurrent writes
    let mut handles = Vec::new();
    for thread_id in 0..num_threads {
        let db_clone = db.clone();
        let handle = tokio::spawn(async move {
            for i in 0..operations_per_thread {
                let key = format!("thread_{}_key_{:06}", thread_id, i);
                let value = format!("thread_{}_value_{:06}_with_additional_data", thread_id, i);

                if let Err(e) = db_clone.put(key.as_bytes(), value.as_bytes()) {
                    eprintln!("Write error in thread {}: {}", thread_id, e);
                    return Err(e);
                }
            }
            Ok(())
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.await??;
    }

    let duration = start_time.elapsed();
    let total_operations = num_threads * operations_per_thread;
    let ops_per_second = total_operations as f64 / duration.as_secs_f64();

    println!(
        "Concurrent test completed: {} operations in {:?} ({:.2} ops/sec)",
        total_operations, duration, ops_per_second
    );

    // Verify some random keys from each thread
    for thread_id in 0..num_threads {
        for i in (0..operations_per_thread).step_by(operations_per_thread / 5) {
            let key = format!("thread_{}_key_{:06}", thread_id, i);
            let expected_value =
                format!("thread_{}_value_{:06}_with_additional_data", thread_id, i);
            let actual_value = db.get(key.as_bytes())?;
            assert_eq!(actual_value, Some(expected_value.into_bytes()));
        }
    }

    Ok(())
}

/// Test database statistics and configuration verification
#[tokio::test]
async fn test_database_statistics() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("stats_test_db");

    let opts = create_optimized_rocksdb_options();
    let db = DB::open(&opts, &db_path)?;

    // Insert some test data
    let mut batch = rocksdb::WriteBatch::default();
    for i in 0..10_000 {
        let key = format!("stats_key_{:06}", i);
        let value = format!("stats_value_{:06}_with_additional_data_for_testing", i);
        batch.put(key.as_bytes(), value.as_bytes());
    }
    db.write(batch)?;

    // Get database statistics
    if let Ok(Some(stats)) = db.property_value("rocksdb.stats") {
        println!("RocksDB Statistics:\n{}", stats);
    }

    // Check specific properties
    let properties = vec![
        "rocksdb.num-files-at-level0",
        "rocksdb.num-files-at-level1",
        "rocksdb.num-files-at-level2",
        "rocksdb.total-sst-files-size",
        "rocksdb.live-sst-files-size",
        "rocksdb.estimate-num-keys",
        "rocksdb.block-cache-usage",
        "rocksdb.block-cache-capacity",
    ];

    for property in properties {
        if let Ok(Some(value)) = db.property_value(property) {
            println!("{}: {}", property, value);
        }
    }

    Ok(())
}

/// Performance benchmark test for large-scale operations
#[tokio::test]
async fn test_performance_benchmark() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("benchmark_db");

    let opts = create_optimized_rocksdb_options();
    let db = Arc::new(DB::open(&opts, &db_path)?);

    println!("Running performance benchmark...");

    // Benchmark 1: Sequential writes (simulating blockchain sync)
    let num_sequential_ops = 100_000;
    let start_time = Instant::now();

    for i in 0..num_sequential_ops {
        let key = format!("seq_key_{:08}", i);
        let value = format!("seq_value_{:08}_blockchain_data_simulation", i);
        db.put(key.as_bytes(), value.as_bytes())?;
    }

    let sequential_duration = start_time.elapsed();
    let sequential_ops_per_sec = num_sequential_ops as f64 / sequential_duration.as_secs_f64();

    println!(
        "Sequential writes: {} ops in {:?} ({:.2} ops/sec)",
        num_sequential_ops, sequential_duration, sequential_ops_per_sec
    );

    // Benchmark 2: Random reads
    let num_random_reads = 50_000;
    let start_time = Instant::now();

    for i in 0..num_random_reads {
        let key_index = i % num_sequential_ops; // Read from existing keys
        let key = format!("seq_key_{:08}", key_index);
        let _value = db.get(key.as_bytes())?;
    }

    let random_read_duration = start_time.elapsed();
    let random_read_ops_per_sec = num_random_reads as f64 / random_read_duration.as_secs_f64();

    println!(
        "Random reads: {} ops in {:?} ({:.2} ops/sec)",
        num_random_reads, random_read_duration, random_read_ops_per_sec
    );

    // Benchmark 3: Large batch write (simulating block processing)
    let batch_size = 75_000; // Middle of the 50K-150K range
    let start_time = Instant::now();

    let mut batch = rocksdb::WriteBatch::default();
    for i in 0..batch_size {
        let key = format!("batch_perf_key_{:08}", i);
        let value = format!("batch_perf_value_{:08}_block_processing_simulation", i);
        batch.put(key.as_bytes(), value.as_bytes());
    }

    db.write(batch)?;

    let batch_duration = start_time.elapsed();
    let batch_ops_per_sec = batch_size as f64 / batch_duration.as_secs_f64();

    println!(
        "Large batch write: {} ops in {:?} ({:.2} ops/sec)",
        batch_size, batch_duration, batch_ops_per_sec
    );

    // Performance assertions (these are reasonable expectations for optimized RocksDB)
    assert!(
        sequential_ops_per_sec > 10_000.0,
        "Sequential write performance too low"
    );
    assert!(
        random_read_ops_per_sec > 50_000.0,
        "Random read performance too low"
    );
    assert!(
        batch_ops_per_sec > 20_000.0,
        "Batch write performance too low"
    );

    Ok(())
}
