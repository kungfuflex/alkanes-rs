# Alkanes-RS Beautiful Logging System Demo

## Example Block Processing Summary Output

```
🏗️  ═══════════════════════════════════════════════════════════════
📦 BLOCK 850123 PROCESSING SUMMARY
🏗️  ═══════════════════════════════════════════════════════════════
🔗 Block Hash: 00000000000000000002a7c4c1e48d76c5a37902165a270156b7a8d72728a054
📏 Block Size: 1,234,567 bytes

💳 TRANSACTION PROCESSING
├── 📊 Transactions: 2,847
└── 🎯 Outpoints: 8,521

⚡ PROTOSTONE EXECUTION
├── 🚀 Total Executed: 156
└── 📦 With Cellpacks: 42

🧪 NEW ALKANES DEPLOYED (8)
├── 🆕 [2, 1]: 45.2 KB WASM (direct init [1, 0])
├── 🎯 [4, 12345]: 23.8 KB WASM (predictable [3, 12345])
├── 🏭 [2, 2]: 12.1 KB WASM (factory clone [5, 100])
├── 🎯🏭 [2, 3]: 8.7 KB WASM (factory clone [6, 200])
├── 🆕 [2, 4]: 67.3 KB WASM (direct init [1, 0])
├── 🏭 [2, 5]: 15.4 KB WASM (factory clone [5, 150])
├── 🎯 [4, 54321]: 31.2 KB WASM (predictable [3, 54321])
└── 🆕 [2, 6]: 89.1 KB WASM (direct init [1, 0])

📈 DEPLOYMENT BREAKDOWN:
├── 🆕 Direct Init: 3
├── 🎯 Predictable: 2
├── 🏭 Factory Clones: 2
├── 🎯🏭 Factory Predictable: 1
└── 💾 Total WASM: 292.8 KB

⛽ FUEL CONSUMPTION
├── 🔥 Total Consumed: 15,847,293
└── 💨 Excess Unused: 2,156,847

🗄️  CACHE PERFORMANCE
├── 🎯 Hit Rate: 87.3% (12,847 hits, 1,876 misses)
├── 📊 Usage: 8,234/10,000 entries
└── 🗑️  Evictions: 156

🏗️  ═══════════════════════════════════════════════════════════════
```

## Individual Alkane Logging (with --features logs)

```
🧪 [ALKANE] Initializing alkane [2, 1] with 45.2 KB WASM
🧪 [ALKANE] Processing transfer: 1000 units from [2, 1] to [2, 2]
🧪 [ALKANE] Balance updated: [2, 1] now has 9000 units
🧪 [ALKANE] Factory deployment: cloning [2, 1] to create [2, 3]
🧪 [ALKANE] Predictable address resolved: [3, 12345] -> [4, 12345]
```

## Key Features

### 🎨 Visual Design
- **Tree Structure**: Clear hierarchical display using Unicode box-drawing characters
- **Emoji Icons**: Intuitive visual indicators for different types of operations
- **Color Coding**: Different sections clearly separated with borders
- **Consistent Formatting**: Aligned numbers and consistent spacing

### 📊 Comprehensive Metrics
1. **Transaction Processing**: Count of transactions and outpoints indexed
2. **Protostone Execution**: Total executions and cellpack attachments
3. **Alkane Deployments**: Detailed breakdown by creation method with WASM sizes
4. **Fuel Usage**: Total consumption and excess unused fuel
5. **Cache Performance**: Hit rates, usage statistics, and eviction counts

### 🔧 Technical Implementation
- **Cross-Platform**: Works on both native and WASM targets using conditional compilation
- **Thread-Safe**: Uses appropriate synchronization primitives for each target
- **Real-Time Stats**: Integrates with metashrew-support LRU cache for live statistics
- **Feature-Gated**: Individual alkane logs only activate with `--features logs`

### 🚀 Performance Benefits
- **Consolidated Logging**: Replaces scattered debug statements with organized summaries
- **Minimal Overhead**: Statistics collection has negligible performance impact
- **Memory Efficient**: Uses existing cache infrastructure without additional allocations
- **Debugging Ready**: Detailed individual logs available when needed for troubleshooting