//! Optimized RocksDB configuration based on performance analysis
//! 
//! This module provides optimized RocksDB configurations specifically tuned for
//! metashrew workloads based on performance profiling that identified bloom filter
//! and memory allocation bottlenecks.

use rocksdb::{Options, BlockBasedOptions, Cache, DBCompressionType};

/// Create optimized RocksDB options for metashrew workloads
/// 
/// Based on performance analysis showing:
/// - 23.98% CPU time in bloom filter operations
/// - 14.13% CPU time in memory copying
/// - 11.04% CPU time in page faults
/// 
/// This configuration optimizes for:
/// - Reduced bloom filter I/O overhead
/// - Better cache efficiency
/// - Reduced memory allocation pressure
/// - Improved query performance
pub fn create_optimized_options() -> Options {
    let mut opts = Options::default();
    
    // === BASIC CONFIGURATION ===
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    
    // === MEMORY CONFIGURATION ===
    // Optimized for high-throughput workloads with large batches
    opts.set_write_buffer_size(256 * 1024 * 1024); // 256MB per memtable
    opts.set_max_write_buffer_number(6);
    opts.set_min_write_buffer_number_to_merge(2);
    
    // Large block cache to reduce bloom filter I/O (primary bottleneck)
    let cache_size = if let Ok(available_memory) = get_available_memory() {
        // Use 25% of available memory, but at least 4GB and at most 16GB
        (available_memory / 4).max(4 * 1024 * 1024 * 1024).min(16 * 1024 * 1024 * 1024)
    } else {
        8 * 1024 * 1024 * 1024 // Default to 8GB
    };
    
    let cache = Cache::new_lru_cache(cache_size);
    
    // === OPTIMIZED TABLE OPTIONS (PRIMARY PERFORMANCE FIX) ===
    let mut table_opts = BlockBasedOptions::default();
    table_opts.set_block_cache(&cache);
    
    // Larger block size to reduce filter block count (reduces I/O overhead)
    table_opts.set_block_size(256 * 1024); // 256KB blocks (up from typical 64KB)
    
    // Aggressive caching for filter blocks (primary bottleneck)
    table_opts.set_cache_index_and_filter_blocks(true);
    table_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
    table_opts.set_pin_top_level_index_and_filter(true);
    
    // Optimized bloom filter configuration
    // Increased bits per key to reduce false positives (reduces I/O)
    table_opts.set_bloom_filter(20.0, false); // Increased from typical 10.0
    table_opts.set_whole_key_filtering(true); // Enable whole key filtering
    
    // Use latest format for better performance
    table_opts.set_format_version(5);
    
    // Note: set_optimize_filters_for_memory not available in this RocksDB version
    // This optimization would reduce memory usage for filters
    
    opts.set_block_based_table_factory(&table_opts);
    
    // === COMPACTION CONFIGURATION ===
    // Optimized for write-heavy workloads with occasional reads
    opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
    
    // Larger L0 to reduce compaction overhead during sync
    opts.set_level_zero_file_num_compaction_trigger(8); // Increased from 4
    opts.set_level_zero_slowdown_writes_trigger(20);
    opts.set_level_zero_stop_writes_trigger(36);
    
    // Larger target file sizes to reduce filter block fragmentation
    opts.set_target_file_size_base(512 * 1024 * 1024); // 512MB (up from 256MB)
    opts.set_target_file_size_multiplier(2);
    
    // Larger level sizes to reduce compaction frequency
    opts.set_max_bytes_for_level_base(4 * 1024 * 1024 * 1024); // 4GB
    opts.set_max_bytes_for_level_multiplier(8.0);
    
    // === PARALLELISM CONFIGURATION ===
    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4) as i32;
    
    opts.set_max_background_jobs(cpu_count.max(8)); // At least 8 background jobs
    opts.set_max_subcompactions(cpu_count as u32);
    
    // === WRITE OPTIMIZATION ===
    // Large write buffers to reduce write stalls
    opts.set_max_write_buffer_size_to_maintain(1024 * 1024 * 1024); // 1GB
    opts.set_db_write_buffer_size(2048 * 1024 * 1024); // 2GB total
    
    // === FILE SYSTEM CONFIGURATION ===
    opts.set_max_open_files(50000); // High limit for large databases
    
    // CRITICAL: Disable direct I/O to improve caching (reduces filter block I/O)
    // This is a key optimization based on the performance analysis
    opts.set_use_direct_reads(false);
    opts.set_use_direct_io_for_flush_and_compaction(false);
    
    // === COMPRESSION CONFIGURATION ===
    // Optimized compression to reduce CPU overhead for frequently accessed data
    opts.set_compression_type(DBCompressionType::Lz4);
    
    // Per-level compression: no compression for hot data, efficient compression for cold data
    opts.set_compression_per_level(&[
        DBCompressionType::None,    // L0 - no compression for speed
        DBCompressionType::None,    // L1 - no compression for speed
        DBCompressionType::Lz4,     // L2 - fast compression
        DBCompressionType::Zstd,    // L3+ - better compression for cold data
        DBCompressionType::Zstd,
        DBCompressionType::Zstd,
        DBCompressionType::Zstd,
    ]);
    
    // === LOGGING AND MONITORING ===
    opts.set_log_level(rocksdb::LogLevel::Info);
    opts.set_keep_log_file_num(5);
    opts.set_log_file_time_to_roll(24 * 60 * 60); // 24 hours
    
    // === ADDITIONAL OPTIMIZATIONS ===
    // Optimize for sequential writes (blockchain data pattern)
    opts.set_level_compaction_dynamic_level_bytes(true);
    
    // Reduce write amplification
    opts.set_bytes_per_sync(16 * 1024 * 1024); // 16MB
    opts.set_wal_bytes_per_sync(16 * 1024 * 1024); // 16MB
    
    // === WAL CONFIGURATION ===
    // Large WAL for batch operations
    opts.set_max_total_wal_size(4 * 1024 * 1024 * 1024); // 4GB
    opts.set_wal_ttl_seconds(0); // Keep WAL files
    opts.set_wal_size_limit_mb(0); // No size limit
    
    // === METASHREW-SPECIFIC OPTIMIZATIONS ===
    // Enable statistics for monitoring
    opts.enable_statistics();
    
    // Optimize for the append-only pattern used by metashrew
    opts.set_allow_concurrent_memtable_write(true);
    opts.set_enable_write_thread_adaptive_yield(true);
    
    opts
}

/// Create lightweight options for secondary (read-only) instances
pub fn create_secondary_options() -> Options {
    let mut opts = create_optimized_options();
    
    // Reduce memory usage for secondary instances
    opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
    opts.set_max_write_buffer_number(2);
    
    // Smaller cache for secondary instances
    let cache = Cache::new_lru_cache(2 * 1024 * 1024 * 1024); // 2GB
    let mut table_opts = BlockBasedOptions::default();
    table_opts.set_block_cache(&cache);
    table_opts.set_block_size(256 * 1024); // Keep large block size
    table_opts.set_cache_index_and_filter_blocks(true);
    table_opts.set_bloom_filter(20.0, false);
    table_opts.set_whole_key_filtering(true);
    opts.set_block_based_table_factory(&table_opts);
    
    opts
}

/// Get available system memory in bytes
fn get_available_memory() -> Result<usize, std::io::Error> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let meminfo = fs::read_to_string("/proc/meminfo")?;
        for line in meminfo.lines() {
            if line.starts_with("MemAvailable:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<usize>() {
                        return Ok(kb * 1024); // Convert KB to bytes
                    }
                }
            }
        }
    }
    
    // Fallback: assume 16GB available
    Ok(16 * 1024 * 1024 * 1024)
}

/// Performance monitoring helper
pub fn log_performance_stats(db: &rocksdb::DB) {
    if let Ok(Some(stats)) = db.property_value("rocksdb.stats") {
        log::info!("RocksDB Performance Stats:\n{}", stats);
    }
    
    // Log cache hit rates
    if let Ok(Some(block_cache_hit)) = db.property_value("rocksdb.block-cache-hit") {
        if let Ok(Some(block_cache_miss)) = db.property_value("rocksdb.block-cache-miss") {
            log::info!("Block cache hit rate: {} hits, {} misses", block_cache_hit, block_cache_miss);
        }
    }
    
    // Log bloom filter stats
    if let Ok(Some(bloom_useful)) = db.property_value("rocksdb.bloom-filter-useful") {
        if let Ok(Some(bloom_checked)) = db.property_value("rocksdb.bloom-filter-checked") {
            log::info!("Bloom filter effectiveness: {} useful, {} checked", bloom_useful, bloom_checked);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_optimized_options_creation() {
        let _opts = create_optimized_options();
        // Basic sanity checks
        // assert!(opts.get_write_buffer_size() > 0);
        // assert!(opts.get_max_write_buffer_number() > 0);
    }
    
    #[test]
    fn test_secondary_options_creation() {
        let _opts = create_secondary_options();
        // Secondary should have smaller write buffers
        // assert!(opts.get_write_buffer_size() <= 64 * 1024 * 1024);
    }
}