// ALKANES-RS Consolidated Logging System
//
// This module provides block-level summary logging with specific metrics:
// 1) Number of transactions + outpoints indexed
// 2) Number of protostones run
// 3) Number of protostones with message payloads (cellpacks) attached
// 4) New alkanes created (each alkaneid as [2, n] or [4, n] printed alongside bytesize in kb of each WASM added, and how many of those were factoried with [5, n] or [6, n] vs how many were initialized with [1, 0] or [3, n])
// 5) Total fuel used by all execution for the block / excess fuel unused by transactions (under minimum_fuel)
// 6) LRU cache stats
// 7) Individual alkane __log statements are only activated with --features logs

use crate::vm::fuel::VirtualFuelBytes;
use alkanes_support::id::AlkaneId;
use bitcoin::Block;

// Conditional compilation for different targets
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};

/// Statistics for a single block's processing
#[derive(Debug, Default, Clone)]
pub struct BlockStats {
    /// Number of transactions processed
    pub transactions_processed: u32,
    /// Number of outpoints indexed
    pub outpoints_indexed: u32,
    /// Number of protostones executed
    pub protostones_run: u32,
    /// Number of protostones with cellpack payloads
    pub protostones_with_cellpacks: u32,
    /// New alkanes created in this block
    pub new_alkanes: Vec<AlkaneCreation>,
    /// Total fuel consumed by all executions
    pub total_fuel_consumed: u64,
    /// Fuel unused due to minimum fuel requirements
    pub excess_fuel_unused: u64,
    /// LRU cache statistics
    pub cache_stats: CacheStats,
    /// GPU pipeline execution statistics
    pub pipeline_stats: PipelineStats,
}

/// Information about a newly created alkane
#[derive(Debug, Clone)]
pub struct AlkaneCreation {
    /// The alkane ID assigned ([2, n] or [4, n])
    pub alkane_id: AlkaneId,
    /// Size of the WASM bytecode in KB
    pub wasm_size_kb: f64,
    /// How the alkane was created
    pub creation_method: CreationMethod,
}

/// Method used to create an alkane
#[derive(Debug, Clone)]
pub enum CreationMethod {
    /// Direct initialization with [1, 0] header
    DirectInit,
    /// Predictable address with [3, n] header
    PredictableAddress(u128),
    /// Factory clone from [5, n] header (source alkane ID)
    FactoryClone(AlkaneId),
    /// Factory clone from [6, n] header (source alkane ID)
    FactoryClonePredictable(AlkaneId),
}

/// LRU cache statistics
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Current cache size
    pub current_size: u64,
    /// Maximum cache capacity
    pub max_capacity: u64,
    /// Number of evictions
    pub evictions: u64,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Top key prefixes (only available with lru-debug feature)
    #[cfg(feature = "lru-debug")]
    pub top_prefixes: Vec<(String, u64)>,
}

/// GPU pipeline execution statistics
#[derive(Debug, Default, Clone)]
pub struct PipelineStats {
    /// Total number of GPU shards executed
    pub gpu_shards_executed: u32,
    /// Total number of messages processed on GPU
    pub gpu_messages_processed: u32,
    /// Number of shards that fell back to WASM
    pub wasm_fallback_shards: u32,
    /// Total GPU execution time in microseconds
    pub gpu_execution_time_us: u64,
    /// GPU memory usage in bytes
    pub gpu_memory_used_bytes: u64,
    /// Number of GPU clustering passes performed
    pub clustering_passes: u32,
    /// Number of shard merges during clustering
    pub shard_merges: u32,
    /// Number of conflicts detected and resolved
    pub conflicts_resolved: u32,
    /// Average shard size (messages per shard)
    pub avg_shard_size: f64,
    /// Pipeline efficiency (GPU messages / total messages)
    pub pipeline_efficiency: f64,
}

// Global state for tracking block statistics
#[cfg(not(target_arch = "wasm32"))]
static BLOCK_STATS: Mutex<Option<BlockStats>> = Mutex::new(None);

#[cfg(target_arch = "wasm32")]
thread_local! {
    static BLOCK_STATS: RefCell<Option<BlockStats>> = RefCell::new(None);
}

/// Initialize block statistics for a new block
#[cfg(not(target_arch = "wasm32"))]
pub fn init_block_stats() {
    let mut stats = BLOCK_STATS.lock().unwrap();
    *stats = Some(BlockStats::default());
}

#[cfg(target_arch = "wasm32")]
pub fn init_block_stats() {
    BLOCK_STATS.with(|stats| {
        *stats.borrow_mut() = Some(BlockStats::default());
    });
}

/// Record a transaction being processed
#[cfg(not(target_arch = "wasm32"))]
pub fn record_transaction() {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.transactions_processed += 1;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn record_transaction() {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.transactions_processed += 1;
        }
    });
}

/// Record multiple transactions being processed
#[cfg(not(target_arch = "wasm32"))]
pub fn record_transactions(count: u32) {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.transactions_processed += count;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn record_transactions(count: u32) {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.transactions_processed += count;
        }
    });
}

/// Record outpoints being indexed
#[cfg(not(target_arch = "wasm32"))]
pub fn record_outpoints(count: u32) {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.outpoints_indexed += count;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn record_outpoints(count: u32) {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.outpoints_indexed += count;
        }
    });
}

/// Record a protostone execution
#[cfg(not(target_arch = "wasm32"))]
pub fn record_protostone_run() {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.protostones_run += 1;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn record_protostone_run() {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.protostones_run += 1;
        }
    });
}

/// Record a protostone with cellpack payload
#[cfg(not(target_arch = "wasm32"))]
pub fn record_protostone_with_cellpack() {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.protostones_with_cellpacks += 1;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn record_protostone_with_cellpack() {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.protostones_with_cellpacks += 1;
        }
    });
}

/// Record a new alkane creation
#[cfg(not(target_arch = "wasm32"))]
pub fn record_alkane_creation(creation: AlkaneCreation) {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.new_alkanes.push(creation);
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn record_alkane_creation(creation: AlkaneCreation) {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.new_alkanes.push(creation);
        }
    });
}

/// Record fuel consumption
#[cfg(not(target_arch = "wasm32"))]
pub fn record_fuel_consumed(amount: u64) {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.total_fuel_consumed += amount;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn record_fuel_consumed(amount: u64) {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.total_fuel_consumed += amount;
        }
    });
}

/// Record excess fuel unused
#[cfg(not(target_arch = "wasm32"))]
pub fn record_excess_fuel_unused(amount: u64) {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.excess_fuel_unused += amount;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn record_excess_fuel_unused(amount: u64) {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.excess_fuel_unused += amount;
        }
    });
}

/// Update cache statistics
#[cfg(not(target_arch = "wasm32"))]
pub fn update_cache_stats(cache_stats: CacheStats) {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.cache_stats = cache_stats;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn update_cache_stats(cache_stats: CacheStats) {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.cache_stats = cache_stats;
        }
    });
}

/// Update pipeline statistics
#[cfg(not(target_arch = "wasm32"))]
pub fn update_pipeline_stats(pipeline_stats: PipelineStats) {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.pipeline_stats = pipeline_stats;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn update_pipeline_stats(pipeline_stats: PipelineStats) {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.pipeline_stats = pipeline_stats;
        }
    });
}

/// Record GPU shard execution
#[cfg(not(target_arch = "wasm32"))]
pub fn record_gpu_shard_execution(messages_count: u32, execution_time_us: u64, memory_used: u64) {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.pipeline_stats.gpu_shards_executed += 1;
            s.pipeline_stats.gpu_messages_processed += messages_count;
            s.pipeline_stats.gpu_execution_time_us += execution_time_us;
            s.pipeline_stats.gpu_memory_used_bytes = s.pipeline_stats.gpu_memory_used_bytes.max(memory_used);
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn record_gpu_shard_execution(messages_count: u32, execution_time_us: u64, memory_used: u64) {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.pipeline_stats.gpu_shards_executed += 1;
            s.pipeline_stats.gpu_messages_processed += messages_count;
            s.pipeline_stats.gpu_execution_time_us += execution_time_us;
            s.pipeline_stats.gpu_memory_used_bytes = s.pipeline_stats.gpu_memory_used_bytes.max(memory_used);
        }
    });
}

/// Record WASM fallback shard
#[cfg(not(target_arch = "wasm32"))]
pub fn record_wasm_fallback_shard() {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.pipeline_stats.wasm_fallback_shards += 1;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn record_wasm_fallback_shard() {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.pipeline_stats.wasm_fallback_shards += 1;
        }
    });
}

/// Record clustering statistics
#[cfg(not(target_arch = "wasm32"))]
pub fn record_clustering_stats(passes: u32, merges: u32, conflicts: u32) {
    if let Ok(mut stats) = BLOCK_STATS.lock() {
        if let Some(ref mut s) = *stats {
            s.pipeline_stats.clustering_passes += passes;
            s.pipeline_stats.shard_merges += merges;
            s.pipeline_stats.conflicts_resolved += conflicts;
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn record_clustering_stats(passes: u32, merges: u32, conflicts: u32) {
    BLOCK_STATS.with(|stats| {
        if let Some(ref mut s) = &mut *stats.borrow_mut() {
            s.pipeline_stats.clustering_passes += passes;
            s.pipeline_stats.shard_merges += merges;
            s.pipeline_stats.conflicts_resolved += conflicts;
        }
    });
}

/// Enable LRU cache debugging mode (only available with lru-debug feature)
#[cfg(feature = "lru-debug")]
pub fn enable_lru_debug_mode() {
    metashrew_support::lru_cache::enable_lru_debug_mode();
}

/// Disable LRU cache debugging mode (only available with lru-debug feature)
#[cfg(feature = "lru-debug")]
pub fn disable_lru_debug_mode() {
    metashrew_support::lru_cache::disable_lru_debug_mode();
}

/// Generate detailed LRU cache debug report (only available with lru-debug feature)
#[cfg(feature = "lru-debug")]
pub fn generate_lru_debug_report() -> String {
    metashrew_support::lru_cache::generate_lru_debug_report()
}

/// Get current cache statistics from metashrew-support
pub fn get_cache_stats() -> CacheStats {
    // Get actual cache stats from metashrew-support LRU cache
    let metashrew_stats = metashrew_support::lru_cache::get_cache_stats();

    #[cfg(feature = "lru-debug")]
    let top_prefixes = {
        // Get debug stats and extract top prefixes
        let debug_stats = metashrew_support::lru_cache::get_lru_debug_stats();
        debug_stats
            .prefix_stats
            .into_iter()
            .take(10) // Top 10 prefixes
            .map(|stat| (stat.prefix_readable, stat.hits))
            .collect()
    };

    CacheStats {
        hits: metashrew_stats.hits,
        misses: metashrew_stats.misses,
        current_size: metashrew_stats.items as u64,
        max_capacity: 1024 * 1024 * 1024 / 1024, // Approximate max items (1GB / 1KB avg)
        evictions: metashrew_stats.evictions,
        memory_usage: metashrew_stats.memory_usage as u64,
        #[cfg(feature = "lru-debug")]
        top_prefixes,
    }
}

/// Log block summary at the end of block processing
#[cfg(not(target_arch = "wasm32"))]
pub fn log_block_summary(block: &Block, height: u32) {
    log_block_summary_with_size(block, height, block.vfsize().try_into().unwrap());
}

/// Log block summary with actual block data size
#[cfg(not(target_arch = "wasm32"))]
pub fn log_block_summary_with_size(block: &Block, height: u32, block_size_bytes: usize) {
    // Update cache stats before logging
    let current_cache_stats = get_cache_stats();
    update_cache_stats(current_cache_stats);

    let stats = {
        let stats_guard = BLOCK_STATS.lock().unwrap();
        stats_guard.clone()
    };

    if let Some(stats) = stats {
        // Use println! to ensure block summaries are always visible regardless of logs feature
        println!();
        println!("🏗️  ═══════════════════════════════════════════════════════════════");
        println!("📦 BLOCK {} PROCESSING SUMMARY", height);
        println!("🏗️  ═══════════════════════════════════════════════════════════════");
        println!("🔗 Block Hash: {}", block.block_hash());
        println!(
            "📏 Block Size: {} bytes",
            format_number_with_commas(block_size_bytes)
        );
        println!();

        // Transaction & Outpoint Processing
        println!("💳 TRANSACTION PROCESSING");
        println!("├── 📊 Transactions: {}", stats.transactions_processed);
        println!("└── 🎯 Outpoints: {}", stats.outpoints_indexed);
        println!();

        // Protostone Execution
        println!("⚡ PROTOSTONE EXECUTION");
        println!("├── 🚀 Total Executed: {}", stats.protostones_run);
        println!(
            "└── 📦 With Cellpacks: {}",
            stats.protostones_with_cellpacks
        );
        println!();

        // New Alkanes Created
        if !stats.new_alkanes.is_empty() {
            println!("🧪 NEW ALKANES DEPLOYED ({})", stats.new_alkanes.len());

            let mut direct_init_count = 0;
            let mut predictable_count = 0;
            let mut factory_clone_count = 0;
            let mut factory_clone_predictable_count = 0;
            let mut total_wasm_size_kb = 0.0;

            for (i, alkane) in stats.new_alkanes.iter().enumerate() {
                let is_last = i == stats.new_alkanes.len() - 1;
                let prefix = if is_last { "└──" } else { "├──" };

                match alkane.creation_method {
                    CreationMethod::DirectInit => {
                        direct_init_count += 1;
                        println!(
                            "{} 🆕 [2, {}]: {:.2} KB WASM (direct init [1, 0])",
                            prefix, alkane.alkane_id.tx, alkane.wasm_size_kb
                        );
                    }
                    CreationMethod::PredictableAddress(n) => {
                        predictable_count += 1;
                        println!(
                            "{} 🎯 [4, {}]: {:.2} KB WASM (predictable [3, {}])",
                            prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, n
                        );
                    }
                    CreationMethod::FactoryClone(source) => {
                        factory_clone_count += 1;
                        println!(
                            "{} 🏭 [2, {}]: {:.2} KB WASM (factory clone [5, {}])",
                            prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx
                        );
                    }
                    CreationMethod::FactoryClonePredictable(source) => {
                        factory_clone_predictable_count += 1;
                        println!(
                            "{} 🎯🏭 [2, {}]: {:.2} KB WASM (factory clone [6, {}])",
                            prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx
                        );
                    }
                }
                total_wasm_size_kb += alkane.wasm_size_kb;
            }

            println!();
            println!("📈 DEPLOYMENT BREAKDOWN:");
            println!("├── 🆕 Direct Init: {}", direct_init_count);
            println!("├── 🎯 Predictable: {}", predictable_count);
            println!("├── 🏭 Factory Clones: {}", factory_clone_count);
            println!(
                "├── 🎯🏭 Factory Predictable: {}",
                factory_clone_predictable_count
            );
            println!("└── 💾 Total WASM: {:.2} KB", total_wasm_size_kb);
        } else {
            println!("🧪 NEW ALKANES DEPLOYED");
            println!("└── ❌ None deployed this block");
        }
        println!();

        // Fuel Usage
        println!("⛽ FUEL CONSUMPTION");
        println!("├── 🔥 Total Consumed: {}", stats.total_fuel_consumed);
        println!("└── 💨 Excess Unused: {}", stats.excess_fuel_unused);
        println!();

        // Cache Performance
        println!("🗄️  CACHE PERFORMANCE");
        if stats.cache_stats.hits > 0 || stats.cache_stats.misses > 0 {
            let hit_rate = if stats.cache_stats.hits + stats.cache_stats.misses > 0 {
                (stats.cache_stats.hits as f64
                    / (stats.cache_stats.hits + stats.cache_stats.misses) as f64)
                    * 100.0
            } else {
                0.0
            };
            let hit_emoji = if hit_rate >= 80.0 {
                "🎯"
            } else if hit_rate >= 60.0 {
                "👍"
            } else {
                "⚠️"
            };

            println!(
                "├── {} Hit Rate: {:.1}% ({} hits, {} misses)",
                hit_emoji, hit_rate, stats.cache_stats.hits, stats.cache_stats.misses
            );
            println!(
                "├── 📊 Usage: {}/{} entries",
                stats.cache_stats.current_size, stats.cache_stats.max_capacity
            );
            println!(
                "├── 💾 Memory: {} bytes",
                format_number_with_commas(stats.cache_stats.memory_usage as usize)
            );

            #[cfg(feature = "lru-debug")]
            {
                if !stats.cache_stats.top_prefixes.is_empty() {
                    println!("├── 🔍 Top Key Prefixes:");
                    for (i, (prefix, count)) in stats.cache_stats.top_prefixes.iter().enumerate() {
                        let is_last_prefix = i == stats.cache_stats.top_prefixes.len() - 1;
                        let prefix_symbol = if is_last_prefix {
                            "│   └──"
                        } else {
                            "│   ├──"
                        };
                        println!("{} {}: {} accesses", prefix_symbol, prefix, count);
                    }
                }
            }

            println!("└── 🗑️  Evictions: {}", stats.cache_stats.evictions);
        } else {
            println!("└── 😴 No cache activity");
        }
        println!();

        // GPU Pipeline Performance
        if stats.pipeline_stats.gpu_shards_executed > 0 || stats.pipeline_stats.wasm_fallback_shards > 0 {
            println!("🚀 GPU PIPELINE PERFORMANCE");
            
            // Calculate pipeline efficiency
            let total_messages = stats.pipeline_stats.gpu_messages_processed +
                               (stats.pipeline_stats.wasm_fallback_shards * 32); // Estimate WASM messages
            let efficiency = if total_messages > 0 {
                (stats.pipeline_stats.gpu_messages_processed as f64 / total_messages as f64) * 100.0
            } else {
                0.0
            };
            
            let efficiency_emoji = if efficiency >= 80.0 {
                "🎯"
            } else if efficiency >= 60.0 {
                "⚡"
            } else if efficiency > 0.0 {
                "⚠️"
            } else {
                "💻"
            };
            
            println!("├── {} Pipeline Efficiency: {:.1}%", efficiency_emoji, efficiency);
            println!("├── 🔥 GPU Shards: {} ({} messages)",
                     stats.pipeline_stats.gpu_shards_executed,
                     stats.pipeline_stats.gpu_messages_processed);
            
            if stats.pipeline_stats.wasm_fallback_shards > 0 {
                println!("├── 💻 WASM Fallback: {} shards", stats.pipeline_stats.wasm_fallback_shards);
            }
            
            if stats.pipeline_stats.gpu_execution_time_us > 0 {
                let gpu_time_ms = stats.pipeline_stats.gpu_execution_time_us as f64 / 1000.0;
                println!("├── ⏱️  GPU Time: {:.2} ms", gpu_time_ms);
            }
            
            if stats.pipeline_stats.gpu_memory_used_bytes > 0 {
                let gpu_memory_mb = stats.pipeline_stats.gpu_memory_used_bytes as f64 / (1024.0 * 1024.0);
                println!("├── 💾 GPU Memory: {:.1} MB", gpu_memory_mb);
            }
            
            if stats.pipeline_stats.clustering_passes > 0 {
                println!("├── 🔄 Clustering: {} passes, {} merges",
                         stats.pipeline_stats.clustering_passes,
                         stats.pipeline_stats.shard_merges);
            }
            
            if stats.pipeline_stats.conflicts_resolved > 0 {
                println!("├── ⚔️  Conflicts Resolved: {}", stats.pipeline_stats.conflicts_resolved);
            }
            
            if stats.pipeline_stats.gpu_shards_executed > 0 {
                let avg_shard_size = stats.pipeline_stats.gpu_messages_processed as f64 /
                                   stats.pipeline_stats.gpu_shards_executed as f64;
                println!("└── 📊 Avg Shard Size: {:.1} messages", avg_shard_size);
            } else {
                println!("└── 📊 No GPU shards executed");
            }
        } else {
            println!("🚀 GPU PIPELINE PERFORMANCE");
            println!("└── 💻 CPU-only execution (no GPU pipeline used)");
        }

        println!();
        println!("🏗️  ═══════════════════════════════════════════════════════════════");
        println!();
    }
}

/// Helper function to format numbers with commas
fn format_number_with_commas(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }

    result
}

#[cfg(target_arch = "wasm32")]
pub fn log_block_summary(block: &Block, height: u32) {
    log_block_summary_with_size(block, height, block.vfsize() as usize);
}

/// Log block summary with actual block data size
#[cfg(target_arch = "wasm32")]
pub fn log_block_summary_with_size(block: &Block, height: u32, block_size_bytes: usize) {
    // Update cache stats before logging
    let current_cache_stats = get_cache_stats();
    update_cache_stats(current_cache_stats);

    BLOCK_STATS.with(|stats| {
        if let Some(ref stats) = &*stats.borrow() {
            // Use println! to ensure block summaries are always visible regardless of logs feature
            println!();
            println!("🏗️  ═══════════════════════════════════════════════════════════════");
            println!("📦 BLOCK {} PROCESSING SUMMARY", height);
            println!("🏗️  ═══════════════════════════════════════════════════════════════");
            println!("🔗 Block Hash: {}", block.block_hash());
            println!(
                "📏 Block Size: {} bytes",
                format_number_with_commas(block_size_bytes)
            );
            println!();

            // Transaction & Outpoint Processing
            println!("💳 TRANSACTION PROCESSING");
            println!("├── 📊 Transactions: {}", stats.transactions_processed);
            println!("└── 🎯 Outpoints: {}", stats.outpoints_indexed);
            println!();

            // Protostone Execution
            println!("⚡ PROTOSTONE EXECUTION");
            println!("├── 🚀 Total Executed: {}", stats.protostones_run);
            println!(
                "└── 📦 With Cellpacks: {}",
                stats.protostones_with_cellpacks
            );
            println!();

            // New Alkanes Created
            if !stats.new_alkanes.is_empty() {
                println!("🧪 NEW ALKANES DEPLOYED ({})", stats.new_alkanes.len());

                let mut direct_init_count = 0;
                let mut predictable_count = 0;
                let mut factory_clone_count = 0;
                let mut factory_clone_predictable_count = 0;
                let mut total_wasm_size_kb = 0.0;

                for (i, alkane) in stats.new_alkanes.iter().enumerate() {
                    let is_last = i == stats.new_alkanes.len() - 1;
                    let prefix = if is_last { "└──" } else { "├──" };

                    match alkane.creation_method {
                        CreationMethod::DirectInit => {
                            direct_init_count += 1;
                            println!(
                                "{} 🆕 [2, {}]: {:.2} KB WASM (direct init [1, 0])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb
                            );
                        }
                        CreationMethod::PredictableAddress(n) => {
                            predictable_count += 1;
                            println!(
                                "{} 🎯 [4, {}]: {:.2} KB WASM (predictable [3, {}])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, n
                            );
                        }
                        CreationMethod::FactoryClone(source) => {
                            factory_clone_count += 1;
                            println!(
                                "{} 🏭 [2, {}]: {:.2} KB WASM (factory clone [5, {}])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx
                            );
                        }
                        CreationMethod::FactoryClonePredictable(source) => {
                            factory_clone_predictable_count += 1;
                            println!(
                                "{} 🎯🏭 [2, {}]: {:.2} KB WASM (factory clone [6, {}])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx
                            );
                        }
                    }
                    total_wasm_size_kb += alkane.wasm_size_kb;
                }

                println!();
                println!("📈 DEPLOYMENT BREAKDOWN:");
                println!("├── 🆕 Direct Init: {}", direct_init_count);
                println!("├── 🎯 Predictable: {}", predictable_count);
                println!("├── 🏭 Factory Clones: {}", factory_clone_count);
                println!(
                    "├── 🎯🏭 Factory Predictable: {}",
                    factory_clone_predictable_count
                );
                println!("└── 💾 Total WASM: {:.2} KB", total_wasm_size_kb);
            } else {
                println!("🧪 NEW ALKANES DEPLOYED");
                println!("└── ❌ None deployed this block");
            }
            println!();

            // Fuel Usage
            println!("⛽ FUEL CONSUMPTION");
            println!("├── 🔥 Total Consumed: {}", stats.total_fuel_consumed);
            println!("└── 💨 Excess Unused: {}", stats.excess_fuel_unused);
            println!();

            // Cache Performance
            println!("🗄️  CACHE PERFORMANCE");
            if stats.cache_stats.hits > 0 || stats.cache_stats.misses > 0 {
                let hit_rate = if stats.cache_stats.hits + stats.cache_stats.misses > 0 {
                    (stats.cache_stats.hits as f64
                        / (stats.cache_stats.hits + stats.cache_stats.misses) as f64)
                        * 100.0
                } else {
                    0.0
                };
                let hit_emoji = if hit_rate >= 80.0 {
                    "🎯"
                } else if hit_rate >= 60.0 {
                    "👍"
                } else {
                    "⚠️"
                };

                println!(
                    "├── {} Hit Rate: {:.1}% ({} hits, {} misses)",
                    hit_emoji, hit_rate, stats.cache_stats.hits, stats.cache_stats.misses
                );
                println!(
                    "├── 📊 Usage: {}/{} entries",
                    stats.cache_stats.current_size, stats.cache_stats.max_capacity
                );
                println!(
                    "├── 💾 Memory: {} bytes",
                    format_number_with_commas(stats.cache_stats.memory_usage as usize)
                );

                #[cfg(feature = "lru-debug")]
                {
                    if !stats.cache_stats.top_prefixes.is_empty() {
                        println!("├── 🔍 Top Key Prefixes:");
                        for (i, (prefix, count)) in
                            stats.cache_stats.top_prefixes.iter().enumerate()
                        {
                            let is_last_prefix = i == stats.cache_stats.top_prefixes.len() - 1;
                            let prefix_symbol = if is_last_prefix {
                                "│   └──"
                            } else {
                                "│   ├──"
                            };
                            println!("{} {}: {} accesses", prefix_symbol, prefix, count);
                        }
                    }
                }

                println!("└── 🗑️  Evictions: {}", stats.cache_stats.evictions);
            } else {
                println!("└── 😴 No cache activity");
            }
            println!();

            // GPU Pipeline Performance
            if stats.pipeline_stats.gpu_shards_executed > 0 || stats.pipeline_stats.wasm_fallback_shards > 0 {
                println!("🚀 GPU PIPELINE PERFORMANCE");
                
                // Calculate pipeline efficiency
                let total_messages = stats.pipeline_stats.gpu_messages_processed +
                                   (stats.pipeline_stats.wasm_fallback_shards * 32); // Estimate WASM messages
                let efficiency = if total_messages > 0 {
                    (stats.pipeline_stats.gpu_messages_processed as f64 / total_messages as f64) * 100.0
                } else {
                    0.0
                };
                
                let efficiency_emoji = if efficiency >= 80.0 {
                    "🎯"
                } else if efficiency >= 60.0 {
                    "⚡"
                } else if efficiency > 0.0 {
                    "⚠️"
                } else {
                    "💻"
                };
                
                println!("├── {} Pipeline Efficiency: {:.1}%", efficiency_emoji, efficiency);
                println!("├── 🔥 GPU Shards: {} ({} messages)",
                         stats.pipeline_stats.gpu_shards_executed,
                         stats.pipeline_stats.gpu_messages_processed);
                
                if stats.pipeline_stats.wasm_fallback_shards > 0 {
                    println!("├── 💻 WASM Fallback: {} shards", stats.pipeline_stats.wasm_fallback_shards);
                }
                
                if stats.pipeline_stats.gpu_execution_time_us > 0 {
                    let gpu_time_ms = stats.pipeline_stats.gpu_execution_time_us as f64 / 1000.0;
                    println!("├── ⏱️  GPU Time: {:.2} ms", gpu_time_ms);
                }
                
                if stats.pipeline_stats.gpu_memory_used_bytes > 0 {
                    let gpu_memory_mb = stats.pipeline_stats.gpu_memory_used_bytes as f64 / (1024.0 * 1024.0);
                    println!("├── 💾 GPU Memory: {:.1} MB", gpu_memory_mb);
                }
                
                if stats.pipeline_stats.clustering_passes > 0 {
                    println!("├── 🔄 Clustering: {} passes, {} merges",
                             stats.pipeline_stats.clustering_passes,
                             stats.pipeline_stats.shard_merges);
                }
                
                if stats.pipeline_stats.conflicts_resolved > 0 {
                    println!("├── ⚔️  Conflicts Resolved: {}", stats.pipeline_stats.conflicts_resolved);
                }
                
                if stats.pipeline_stats.gpu_shards_executed > 0 {
                    let avg_shard_size = stats.pipeline_stats.gpu_messages_processed as f64 /
                                       stats.pipeline_stats.gpu_shards_executed as f64;
                    println!("└── 📊 Avg Shard Size: {:.1} messages", avg_shard_size);
                } else {
                    println!("└── 📊 No GPU shards executed");
                }
            } else {
                println!("🚀 GPU PIPELINE PERFORMANCE");
                println!("└── 💻 CPU-only execution (no GPU pipeline used)");
            }

            println!();
            println!("🏗️  ═══════════════════════════════════════════════════════════════");
            println!();
        }
    });
}

/// Log function for individual alkanes (only active with --features logs)
#[macro_export]
macro_rules! alkane_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "logs")]
        {
            use metashrew_core::println;
            println!("🧪 [ALKANE] {}", format!($($arg)*));
        }
    };
}

/// Helper function to calculate WASM size in KB
pub fn calculate_wasm_size_kb(wasm_bytes: &[u8]) -> f64 {
    wasm_bytes.len() as f64 / 1024.0
}

/// Helper function to determine creation method from cellpack target
pub fn determine_creation_method(target: &AlkaneId, _resolved: &AlkaneId) -> CreationMethod {
    match (target.block, target.tx) {
        (1, 0) => CreationMethod::DirectInit,
        (3, n) => CreationMethod::PredictableAddress(n),
        (5, n) => CreationMethod::FactoryClone(AlkaneId { block: 2, tx: n }),
        (6, n) => CreationMethod::FactoryClonePredictable(AlkaneId { block: 4, tx: n }),
        _ => CreationMethod::DirectInit, // fallback
    }
}

/// Get current block stats (for testing/debugging)
#[cfg(not(target_arch = "wasm32"))]
pub fn get_block_stats() -> Option<BlockStats> {
    BLOCK_STATS.lock().unwrap().clone()
}

#[cfg(target_arch = "wasm32")]
pub fn get_block_stats() -> Option<BlockStats> {
    BLOCK_STATS.with(|stats| stats.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_stats_initialization() {
        init_block_stats();
        record_transaction();
        record_outpoints(5);

        let stats = get_block_stats();
        assert!(stats.is_some());
        let s = stats.unwrap();
        assert_eq!(s.transactions_processed, 1);
        assert_eq!(s.outpoints_indexed, 5);
    }

    #[test]
    fn test_creation_method_determination() {
        let target1 = AlkaneId { block: 1, tx: 0 };
        let resolved1 = AlkaneId { block: 2, tx: 1 };
        assert!(matches!(
            determine_creation_method(&target1, &resolved1),
            CreationMethod::DirectInit
        ));

        let target2 = AlkaneId {
            block: 3,
            tx: 12345,
        };
        let resolved2 = AlkaneId {
            block: 4,
            tx: 12345,
        };
        assert!(matches!(
            determine_creation_method(&target2, &resolved2),
            CreationMethod::PredictableAddress(12345)
        ));
    }

    #[test]
    fn test_wasm_size_calculation() {
        let wasm_bytes = vec![0u8; 2048]; // 2KB
        assert_eq!(calculate_wasm_size_kb(&wasm_bytes), 2.0);
    }

    #[test]
    fn test_pipeline_stats_recording() {
        init_block_stats();
        
        // Test GPU shard execution recording
        record_gpu_shard_execution(32, 1500, 1024 * 1024); // 32 messages, 1.5ms, 1MB
        record_gpu_shard_execution(28, 1200, 800 * 1024);  // 28 messages, 1.2ms, 800KB
        
        // Test WASM fallback recording
        record_wasm_fallback_shard();
        record_wasm_fallback_shard();
        
        // Test clustering stats
        record_clustering_stats(3, 5, 2); // 3 passes, 5 merges, 2 conflicts
        
        let stats = get_block_stats().unwrap();
        
        // Verify GPU execution stats
        assert_eq!(stats.pipeline_stats.gpu_shards_executed, 2);
        assert_eq!(stats.pipeline_stats.gpu_messages_processed, 60); // 32 + 28
        assert_eq!(stats.pipeline_stats.gpu_execution_time_us, 2700); // 1500 + 1200
        assert_eq!(stats.pipeline_stats.gpu_memory_used_bytes, 1024 * 1024); // Max of the two
        
        // Verify WASM fallback stats
        assert_eq!(stats.pipeline_stats.wasm_fallback_shards, 2);
        
        // Verify clustering stats
        assert_eq!(stats.pipeline_stats.clustering_passes, 3);
        assert_eq!(stats.pipeline_stats.shard_merges, 5);
        assert_eq!(stats.pipeline_stats.conflicts_resolved, 2);
    }

    #[test]
    fn test_pipeline_stats_update() {
        init_block_stats();
        
        let custom_stats = PipelineStats {
            gpu_shards_executed: 10,
            gpu_messages_processed: 320,
            wasm_fallback_shards: 3,
            gpu_execution_time_us: 5000,
            gpu_memory_used_bytes: 2 * 1024 * 1024,
            clustering_passes: 2,
            shard_merges: 4,
            conflicts_resolved: 1,
            avg_shard_size: 32.0,
            pipeline_efficiency: 91.4,
        };
        
        update_pipeline_stats(custom_stats.clone());
        
        let stats = get_block_stats().unwrap();
        assert_eq!(stats.pipeline_stats.gpu_shards_executed, 10);
        assert_eq!(stats.pipeline_stats.gpu_messages_processed, 320);
        assert_eq!(stats.pipeline_stats.wasm_fallback_shards, 3);
        assert_eq!(stats.pipeline_stats.gpu_execution_time_us, 5000);
        assert_eq!(stats.pipeline_stats.gpu_memory_used_bytes, 2 * 1024 * 1024);
        assert_eq!(stats.pipeline_stats.clustering_passes, 2);
        assert_eq!(stats.pipeline_stats.shard_merges, 4);
        assert_eq!(stats.pipeline_stats.conflicts_resolved, 1);
    }
}
