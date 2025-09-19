// ALKANES-RS Consolidated Logging System
//
// This module provides block-level summary logging with specific metrics:
// 1) Number of transactions + outpoints indexed
// 2) Number of protostones run
// 3) Number of protostones with message payloads (cellpacks) attached
// 4) New alkanes created (each alkaneid as [2, n] or [4, n] printed alongside bytesize in kb of each WASM added, and how many of those were factoried with [5, n] or [6, n] vs how many were initialized with [1, 0] or [3, n])
// 5) Total fuel used by all execution for the block / excess fuel unused by transactions (under minimum_fuel)
// 6) Cache stats (placeholder)
// 7) Individual alkane __log statements are only activated with --features logs
//
// Sourced from `./reference/alkanes-rs/src/logging.rs`

use crate::id::AlkaneId;
use bitcoin::Block;
use metashrew_support::environment::RuntimeEnvironment;
use std::cell::RefCell;


// Conditional compilation for different targets
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]




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
    /// Memory usage in bytes
    pub memory_usage: u64,
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

/// Log block summary at the end of block processing
#[cfg(not(target_arch = "wasm32"))]
pub fn log_block_summary<E: RuntimeEnvironment>(
    env: &mut E,
    block: &Block,
    height: u32,
    block_size_bytes: usize,
) {
    // Update cache stats before logging
    update_cache_stats(CacheStats::default());

    let stats = {
        let stats_guard = BLOCK_STATS.lock().unwrap();
        stats_guard.clone()
    };

    if let Some(stats) = stats {
        // Use println! to ensure block summaries are always visible regardless of logs feature
        env.log("");
        env.log("🏗️  ═══════════════════════════════════════════════════════════════");
        env.log(&format!("📦 BLOCK {} PROCESSING SUMMARY", height));
        env.log("🏗️  ═══════════════════════════════════════════════════════════════");
        env.log(&format!("🔗 Block Hash: {}", block.block_hash()));
        env.log(&format!(
            "📏 Block Size: {} bytes",
            format_number_with_commas(block_size_bytes)
        ));
        env.log("");

        // Transaction & Outpoint Processing
        env.log("💳 TRANSACTION PROCESSING");
        env.log(&format!("├── 📊 Transactions: {}", stats.transactions_processed));
        env.log(&format!("└── 🎯 Outpoints: {}", stats.outpoints_indexed));
        env.log("");

        // Protostone Execution
        env.log("⚡ PROTOSTONE EXECUTION");
        env.log(&format!("├── 🚀 Total Executed: {}", stats.protostones_run));
        env.log(&format!(
            "└── 📦 With Cellpacks: {}",
            stats.protostones_with_cellpacks
        ));
        env.log("");

        // New Alkanes Created
        if !stats.new_alkanes.is_empty() {
            env.log(&format!("🧪 NEW ALKANES DEPLOYED ({})", stats.new_alkanes.len()));

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
                        env.log(&format!(
                            "{} 🆕 [2, {}]: {:.2} KB WASM (direct init [1, 0])",
                            prefix, alkane.alkane_id.tx, alkane.wasm_size_kb
                        ));
                    }
                    CreationMethod::PredictableAddress(n) => {
                        predictable_count += 1;
                        env.log(&format!(
                            "{} 🎯 [4, {}]: {:.2} KB WASM (predictable [3, {}])",
                            prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, n
                        ));
                    }
                    CreationMethod::FactoryClone(source) => {
                        factory_clone_count += 1;
                        env.log(&format!(
                            "{} 🏭 [2, {}]: {:.2} KB WASM (factory clone [5, {}])",
                            prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx
                        ));
                    }
                    CreationMethod::FactoryClonePredictable(source) => {
                        factory_clone_predictable_count += 1;
                        env.log(&format!(
                            "{} 🎯🏭 [2, {}]: {:.2} KB WASM (factory clone [6, {}])",
                            prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx
                        ));
                    }
                }
                total_wasm_size_kb += alkane.wasm_size_kb;
            }

            env.log("");
            env.log("📈 DEPLOYMENT BREAKDOWN:");
            env.log(&format!("├── 🆕 Direct Init: {}", direct_init_count));
            env.log(&format!("├── 🎯 Predictable: {}", predictable_count));
            env.log(&format!("├── 🏭 Factory Clones: {}", factory_clone_count));
            env.log(&format!(
                "├── 🎯🏭 Factory Predictable: {}",
                factory_clone_predictable_count
            ));
            env.log(&format!("└── 💾 Total WASM: {:.2} KB", total_wasm_size_kb));
        } else {
            env.log("🧪 NEW ALKANES DEPLOYED");
            env.log("└── ❌ None deployed this block");
        }
        env.log("");

        // Fuel Usage
        env.log("⛽ FUEL CONSUMPTION");
        env.log(&format!("├── 🔥 Total Consumed: {}", stats.total_fuel_consumed));
        env.log(&format!("└── 💨 Excess Unused: {}", stats.excess_fuel_unused));
        env.log("");

        // Cache Performance
        env.log("🗄️  CACHE PERFORMANCE");
        env.log("└── 😴 No cache activity");

        env.log("");
        env.log("🏗️  ═══════════════════════════════════════════════════════════════");
        env.log("");
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
pub fn log_block_summary<E: RuntimeEnvironment>(
    env: &mut E,
    block: &Block,
    height: u32,
    block_size_bytes: usize,
) {
    // Update cache stats before logging
    update_cache_stats(CacheStats::default());

    BLOCK_STATS.with(|stats| {
        if let Some(ref stats) = &*stats.borrow() {
            // Use E::log to ensure block summaries are always visible regardless of logs feature
            env.log("");
            env.log("🏗️  ═══════════════════════════════════════════════════════════════");
            env.log(&format!("📦 BLOCK {} PROCESSING SUMMARY", height));
            env.log("🏗️  ═══════════════════════════════════════════════════════════════");
            env.log(&format!("🔗 Block Hash: {}", block.block_hash()));
            env.log(&format!(
                "📏 Block Size: {} bytes",
                format_number_with_commas(block_size_bytes)
            ));
            env.log("");

            // Transaction & Outpoint Processing
            env.log("💳 TRANSACTION PROCESSING");
            env.log(&format!("├── 📊 Transactions: {}", stats.transactions_processed));
            env.log(&format!("└── 🎯 Outpoints: {}", stats.outpoints_indexed));
            env.log("");

            // Protostone Execution
            env.log("⚡ PROTOSTONE EXECUTION");
            env.log(&format!("├── 🚀 Total Executed: {}", stats.protostones_run));
            env.log(&format!(
                "└── 📦 With Cellpacks: {}",
                stats.protostones_with_cellpacks
            ));
            env.log("");

            // New Alkanes Created
            if !stats.new_alkanes.is_empty() {
                env.log(&format!("🧪 NEW ALKANES DEPLOYED ({})", stats.new_alkanes.len()));

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
                            env.log(&format!(
                                "{} 🆕 [2, {}]: {:.2} KB WASM (direct init [1, 0])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb
                            ));
                        }
                        CreationMethod::PredictableAddress(n) => {
                            predictable_count += 1;
                            env.log(&format!(
                                "{} 🎯 [4, {}]: {:.2} KB WASM (predictable [3, {}])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, n
                            ));
                        }
                        CreationMethod::FactoryClone(source) => {
                            factory_clone_count += 1;
                            env.log(&format!(
                                "{} 🏭 [2, {}]: {:.2} KB WASM (factory clone [5, {}])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx
                            ));
                        }
                        CreationMethod::FactoryClonePredictable(source) => {
                            factory_clone_predictable_count += 1;
                            env.log(&format!(
                                "{} 🎯🏭 [2, {}]: {:.2} KB WASM (factory clone [6, {}])",
                                prefix, alkane.alkane_id.tx, alkane.wasm_size_kb, source.tx
                            ));
                        }
                    }
                    total_wasm_size_kb += alkane.wasm_size_kb;
                }

                env.log("");
                env.log("📈 DEPLOYMENT BREAKDOWN:");
                env.log(&format!("├── 🆕 Direct Init: {}", direct_init_count));
                env.log(&format!("├── 🎯 Predictable: {}", predictable_count));
                env.log(&format!("├── 🏭 Factory Clones: {}", factory_clone_count));
                env.log(&format!(
                    "├── 🎯🏭 Factory Predictable: {}",
                    factory_clone_predictable_count
                ));
                env.log(&format!("└── 💾 Total WASM: {:.2} KB", total_wasm_size_kb));
            } else {
                env.log("🧪 NEW ALKANES DEPLOYED");
                env.log("└── ❌ None deployed this block");
            }
            env.log("");

            // Fuel Usage
            env.log("⛽ FUEL CONSUMPTION");
            env.log(&format!("├── 🔥 Total Consumed: {}", stats.total_fuel_consumed));
            env.log(&format!("└── 💨 Excess Unused: {}", stats.excess_fuel_unused));
            env.log("");

            // Cache Performance
            env.log("🗄️  CACHE PERFORMANCE");
            env.log("└── 😴 No cache activity");

            env.log("");
            env.log("🏗️  ═══════════════════════════════════════════════════════════════");
            env.log("");
        }
    });
}



thread_local! {
    static LOGGER: RefCell<Option<Box<dyn Fn(&str)>>> = RefCell::new(None);
}

pub fn with_logger<F, R>(logger: Box<dyn Fn(&str)>, f: F) -> R
where
    F: FnOnce() -> R,
{
    LOGGER.with(|l| *l.borrow_mut() = Some(logger));
    let result = f();
    LOGGER.with(|l| *l.borrow_mut() = None);
    result
}

/// Log function for individual alkanes (only active with --features logs)
#[macro_export]
macro_rules! alkane_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "logs")]
        {
            LOGGER.with(|l| {
                if let Some(logger) = &*l.borrow() {
                    logger(&format!("[ALKANE] {}", format!($($arg)*)));
                }
            });
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
}