# LRU Debug Feature for ALKANES-RS

This document describes the LRU cache debugging functionality integrated into the ALKANES metaprotocol indexer.

## Overview

The `lru-debug` feature provides detailed insights into LRU cache usage patterns during block processing, helping developers and operators understand cache performance and optimize key access patterns.

## Features

### 1. Enhanced Block Processing Reports

When the `lru-debug` feature is enabled, block processing summaries include:

- **Memory Usage**: Actual memory consumption in bytes
- **Top Key Prefixes**: Most frequently accessed cache key patterns
- **Readable Key Formats**: Human-readable key representations instead of raw hex
- **Access Pattern Analysis**: Distribution of cache hits across different data types

### 2. Intelligent Key Parsing

The system automatically parses cache keys to show readable formats:

```
Raw Key: 2f626c6f636b686173682f62796865696768742f01000000
Parsed:  /blockhash/byheight/01000000
```

### 3. Prefix Pattern Recognition

Identifies common blockchain data patterns:
- Block data: `/blockhash/`, `/blockdata/`
- Transaction data: `/tx/`, `/outpoint/`
- Protorune data: `/protorune/`, `/rune/`
- Alkane data: `/alkane/`, `/wasm/`

## Usage

### Enabling the Feature

Add the `lru-debug` feature when building:

```bash
# Build with LRU debugging
cargo build --features lru-debug

# Run tests with debugging
cargo test --features lru-debug

# Run example
cargo run --example lru_debug_example --features lru-debug
```

### Feature Flag in Cargo.toml

```toml
[features]
lru-debug = []
```

### Integration in Code

The feature is automatically integrated into the block processing pipeline:

```rust
// In alkanes_indexer function (lib.rs)
#[cfg(feature = "lru-debug")]
logging::enable_lru_debug_mode();
```

## Block Summary Output

### Without LRU Debug

```
🗄️  CACHE PERFORMANCE
├── 🎯 Hit Rate: 85.2% (1234 hits, 215 misses)
├── 📊 Usage: 512/1048576 entries
├── 💾 Memory: 2,048,576 bytes
└── 🗑️  Evictions: 23
```

### With LRU Debug

```
🗄️  CACHE PERFORMANCE
├── 🎯 Hit Rate: 85.2% (1234 hits, 215 misses)
├── 📊 Usage: 512/1048576 entries
├── 💾 Memory: 2,048,576 bytes
├── 🔍 Top Key Prefixes:
│   ├── /blockhash/byheight: 456 accesses
│   ├── /protorune/balance: 234 accesses
│   ├── /alkane/bytecode: 123 accesses
│   ├── /outpoint/spent: 89 accesses
│   └── /tx/outputs: 67 accesses
└── 🗑️  Evictions: 23
```

## API Reference

### Core Functions

```rust
// Enable/disable debug mode (feature-gated)
#[cfg(feature = "lru-debug")]
pub fn enable_lru_debug_mode();

#[cfg(feature = "lru-debug")]
pub fn disable_lru_debug_mode();

// Generate detailed debug report
#[cfg(feature = "lru-debug")]
pub fn generate_lru_debug_report() -> String;

// Enhanced cache statistics
pub fn get_cache_stats() -> CacheStats;
```

### CacheStats Structure

```rust
#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub current_size: u64,
    pub max_capacity: u64,
    pub evictions: u64,
    pub memory_usage: u64,
    
    // Only available with lru-debug feature
    #[cfg(feature = "lru-debug")]
    pub top_prefixes: Vec<(String, u64)>,
}
```

## Performance Impact

### Runtime Overhead

- **Without feature**: Zero overhead (feature-gated compilation)
- **With feature**: Minimal overhead (~1-2% during block processing)
- **Memory usage**: Additional ~100KB for prefix tracking

### Compilation Impact

- **Binary size**: Increases by ~50KB when feature is enabled
- **Compile time**: Negligible increase

## Use Cases

### 1. Performance Optimization

Identify hot cache paths and optimize data structures:

```bash
# Monitor cache patterns during sync
cargo run --features lru-debug --bin alkanes-indexer
```

### 2. Debugging Cache Issues

Analyze cache misses and eviction patterns:

```rust
#[cfg(feature = "lru-debug")]
{
    let report = logging::generate_lru_debug_report();
    println!("Cache analysis: {}", report);
}
```

### 3. Capacity Planning

Understand memory usage patterns for deployment sizing:

```
💾 Memory: 2,048,576 bytes
📊 Usage: 512/1048576 entries
```

## Integration with Existing Features

### Compatibility

- ✅ Works with all existing alkanes-rs features
- ✅ Compatible with `cache` feature
- ✅ Works in both WASM and native builds
- ✅ Thread-safe for concurrent access

### Feature Combinations

```toml
# Common combinations
features = ["lru-debug", "cache", "logs"]
features = ["lru-debug", "mainnet"]
features = ["lru-debug", "testnet", "debug-log"]
```

## Example Output

### Complete Block Summary with LRU Debug

```
🏗️  ═══════════════════════════════════════════════════════════════
📦 BLOCK 850000 PROCESSING SUMMARY
🏗️  ═══════════════════════════════════════════════════════════════
🔗 Block Hash: 0000000000000000000308a8f1097a8f1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8
📏 Block Size: 1,048,576 bytes

💳 TRANSACTION PROCESSING
├── 📊 Transactions: 2,847
└── 🎯 Outpoints: 6,234

⚡ PROTOSTONE EXECUTION
├── 🚀 Total Executed: 156
└── 📦 With Cellpacks: 23

🧪 NEW ALKANES DEPLOYED (3)
├── 🆕 [2, 1234]: 45.2 KB WASM (direct init [1, 0])
├── 🏭 [2, 1235]: 32.1 KB WASM (factory clone [5, 567])
└── 🎯 [4, 1236]: 67.8 KB WASM (predictable [3, 9876])

📈 DEPLOYMENT BREAKDOWN:
├── 🆕 Direct Init: 1
├── 🎯 Predictable: 1
├── 🏭 Factory Clones: 1
├── 🎯🏭 Factory Predictable: 0
└── 💾 Total WASM: 145.1 KB

⛽ FUEL CONSUMPTION
├── 🔥 Total Consumed: 2,456,789
└── 💨 Excess Unused: 123,456

🗄️  CACHE PERFORMANCE
├── 🎯 Hit Rate: 87.3% (4,567 hits, 665 misses)
├── 📊 Usage: 8,192/1,048,576 entries
├── 💾 Memory: 16,777,216 bytes
├── 🔍 Top Key Prefixes:
│   ├── /blockhash/byheight: 1,234 accesses
│   ├── /protorune/balance/addr: 987 accesses
│   ├── /alkane/bytecode: 456 accesses
│   ├── /outpoint/spent: 234 accesses
│   ├── /tx/outputs: 123 accesses
│   ├── /rune/supply: 89 accesses
│   ├── /blockdata: 67 accesses
│   ├── /protorune/mint: 45 accesses
│   ├── /alkane/storage: 34 accesses
│   └── /sequence: 23 accesses
└── 🗑️  Evictions: 45

🏗️  ═══════════════════════════════════════════════════════════════
```

## Troubleshooting

### Common Issues

1. **Feature not working**: Ensure compilation with `--features lru-debug`
2. **No prefix data**: Cache may be empty or recently cleared
3. **Performance impact**: Consider disabling for production if not needed

### Debug Commands

```bash
# Verify feature is compiled in
cargo run --features lru-debug --example lru_debug_example

# Check cache state
cargo test --features lru-debug test_lru_debug_functions
```

## Future Enhancements

- [ ] Cache warming strategies based on prefix analysis
- [ ] Automatic cache size recommendations
- [ ] Integration with monitoring systems
- [ ] Historical cache pattern analysis
- [ ] Cache efficiency scoring

## Related Documentation

- [LRU_CACHE_DEBUGGING.md](../../LRU_CACHE_DEBUGGING.md) - Core LRU cache implementation
- [LRU_CACHE_IMPLEMENTATION.md](../../LRU_CACHE_IMPLEMENTATION.md) - Technical details
- [examples/lru_debug_example.rs](examples/lru_debug_example.rs) - Usage example