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

use alkanes_support::id::AlkaneId;
use bitcoin::Block;
use crate::vm::fuel::VirtualFuelBytes;

// Conditional compilation for different targets
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[allow(unused_imports)]
use metashrew_core::{println, stdio::{stdout, Write}};

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

/// Get current cache statistics from metashrew-support
pub fn get_cache_stats() -> CacheStats {
    // Get actual cache stats from metashrew-support LRU cache
    let metashrew_stats = metashrew_support::lru_cache::get_cache_stats();
    
    CacheStats {
        hits: metashrew_stats.hits,
        misses: metashrew_stats.misses,
        current_size: metashrew_stats.items as u64,
        max_capacity: 1024 * 1024 * 1024 / 1024, // Approximate max items (1GB / 1KB avg)
        evictions: metashrew_stats.evictions,
    }
}

/// Log block summary at the end of block processing
#[cfg(not(target_arch = "wasm32"))]
pub fn log_block_summary(block: &Block, height: u32) {
    // Update cache stats before logging
    let current_cache_stats = get_cache_stats();
    update_cache_stats(current_cache_stats);
    
    let stats = {
        let stats_guard = BLOCK_STATS.lock().unwrap();
        stats_guard.clone()
    };

    if let Some(stats) = stats {
        println!();
        println!("🏗️  ═══════════════════════════════════════════════════════════════");
        println!("📦 BLOCK {} PROCESSING SUMMARY", height);
        println!("🏗️  ═══════════════════════════════════════════════════════════════");
        println!("🔗 Block Hash: {}", block.block_hash());
        println!("📏 Block Size: {} bytes", block.vfsize());
        println!();
        
        // Transaction & Outpoint Processing
        println!("💳 TRANSACTION PROCESSING");
        println!("├── 📊 Transactions: {}", stats.transactions_processed);
        println!("└── 🎯 Outpoints: {}", stats.outpoints_indexed);
        println!();
        
        // Protostone Execution
        println!("⚡ PROTOSTONE EXECUTION");
        println!("├── 🚀 Total Executed: {}", stats.protostones_run);
        println!("└── 📦 With Cellpacks: {}", stats.protostones_with_cellpacks);
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
                        println!("{} 🆕 [2, {}]: {:.2} KB WASM (direct init [1, 0])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb);
                    },
                    CreationMethod::PredictableAddress(n) => {
                        predictable_count += 1;
                        println!("{} 🎯 [4, {}]: {:.2} KB WASM (predictable [3, {}])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, n);
                    },
                    CreationMethod::FactoryClone(source) => {
                        factory_clone_count += 1;
                        println!("{} 🏭 [2, {}]: {:.2} KB WASM (factory clone [5, {}])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx);
                    },
                    CreationMethod::FactoryClonePredictable(source) => {
                        factory_clone_predictable_count += 1;
                        println!("{} 🎯🏭 [2, {}]: {:.2} KB WASM (factory clone [6, {}])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx);
                    },
                }
                total_wasm_size_kb += alkane.wasm_size_kb;
            }
            
            println!();
            println!("📈 DEPLOYMENT BREAKDOWN:");
            println!("├── 🆕 Direct Init: {}", direct_init_count);
            println!("├── 🎯 Predictable: {}", predictable_count);
            println!("├── 🏭 Factory Clones: {}", factory_clone_count);
            println!("├── 🎯🏭 Factory Predictable: {}", factory_clone_predictable_count);
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
                (stats.cache_stats.hits as f64 / (stats.cache_stats.hits + stats.cache_stats.misses) as f64) * 100.0
            } else {
                0.0
            };
            let hit_emoji = if hit_rate >= 80.0 { "🎯" } else if hit_rate >= 60.0 { "👍" } else { "⚠️" };
            
            println!("├── {} Hit Rate: {:.1}% ({} hits, {} misses)", hit_emoji, hit_rate, stats.cache_stats.hits, stats.cache_stats.misses);
            println!("├── 📊 Usage: {}/{} entries", stats.cache_stats.current_size, stats.cache_stats.max_capacity);
            println!("└── 🗑️  Evictions: {}", stats.cache_stats.evictions);
        } else {
            println!("└── 😴 No cache activity");
        }
        
        println!();
        println!("🏗️  ═══════════════════════════════════════════════════════════════");
        println!();
    }
}

#[cfg(target_arch = "wasm32")]
pub fn log_block_summary(block: &Block, height: u32) {
    // Update cache stats before logging
    let current_cache_stats = get_cache_stats();
    update_cache_stats(current_cache_stats);
    
    BLOCK_STATS.with(|stats| {
        if let Some(ref stats) = &*stats.borrow() {
            println!();
            println!("🏗️  ═══════════════════════════════════════════════════════════════");
            println!("📦 BLOCK {} PROCESSING SUMMARY", height);
            println!("🏗️  ═══════════════════════════════════════════════════════════════");
            println!("🔗 Block Hash: {}", block.block_hash());
            println!("📏 Block Size: {} bytes", block.vfsize());
            println!();
            
            // Transaction & Outpoint Processing
            println!("💳 TRANSACTION PROCESSING");
            println!("├── 📊 Transactions: {}", stats.transactions_processed);
            println!("└── 🎯 Outpoints: {}", stats.outpoints_indexed);
            println!();
            
            // Protostone Execution
            println!("⚡ PROTOSTONE EXECUTION");
            println!("├── 🚀 Total Executed: {}", stats.protostones_run);
            println!("└── 📦 With Cellpacks: {}", stats.protostones_with_cellpacks);
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
                            println!("{} 🆕 [2, {}]: {:.2} KB WASM (direct init [1, 0])",
                                    prefix, alkane.alkane_id.tx, alkane.wasm_size_kb);
                        },
                        CreationMethod::PredictableAddress(n) => {
                            predictable_count += 1;
                            println!("{} 🎯 [4, {}]: {:.2} KB WASM (predictable [3, {}])",
                                    prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, n);
                        },
                        CreationMethod::FactoryClone(source) => {
                            factory_clone_count += 1;
                            println!("{} 🏭 [2, {}]: {:.2} KB WASM (factory clone [5, {}])",
                                    prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx);
                        },
                        CreationMethod::FactoryClonePredictable(source) => {
                            factory_clone_predictable_count += 1;
                            println!("{} 🎯🏭 [2, {}]: {:.2} KB WASM (factory clone [6, {}])",
                                    prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx);
                        },
                    }
                    total_wasm_size_kb += alkane.wasm_size_kb;
                }
                
                println!();
                println!("📈 DEPLOYMENT BREAKDOWN:");
                println!("├── 🆕 Direct Init: {}", direct_init_count);
                println!("├── 🎯 Predictable: {}", predictable_count);
                println!("├── 🏭 Factory Clones: {}", factory_clone_count);
                println!("├── 🎯🏭 Factory Predictable: {}", factory_clone_predictable_count);
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
                    (stats.cache_stats.hits as f64 / (stats.cache_stats.hits + stats.cache_stats.misses) as f64) * 100.0
                } else {
                    0.0
                };
                let hit_emoji = if hit_rate >= 80.0 { "🎯" } else if hit_rate >= 60.0 { "👍" } else { "⚠️" };
                
                println!("├── {} Hit Rate: {:.1}% ({} hits, {} misses)", hit_emoji, hit_rate, stats.cache_stats.hits, stats.cache_stats.misses);
                println!("├── 📊 Usage: {}/{} entries", stats.cache_stats.current_size, stats.cache_stats.max_capacity);
                println!("└── 🗑️  Evictions: {}", stats.cache_stats.evictions);
            } else {
                println!("└── 😴 No cache activity");
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
        assert!(matches!(determine_creation_method(&target1, &resolved1), CreationMethod::DirectInit));

        let target2 = AlkaneId { block: 3, tx: 12345 };
        let resolved2 = AlkaneId { block: 4, tx: 12345 };
        assert!(matches!(determine_creation_method(&target2, &resolved2), CreationMethod::PredictableAddress(12345)));
    }

    #[test]
    fn test_wasm_size_calculation() {
        let wasm_bytes = vec![0u8; 2048]; // 2KB
        assert_eq!(calculate_wasm_size_kb(&wasm_bytes), 2.0);
    }
}