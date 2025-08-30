# Stateful Views in Metashrew Runtime

## Overview

Stateful views are now **enabled by default** in Metashrew, allowing WASM memory to persist between view function calls. This enables stateful operations where data can be maintained across multiple view invocations, improving performance and enabling more sophisticated indexer patterns.

## Default Behavior

As of this update, stateful views are automatically enabled when creating a `MetashrewRuntime` instance:

- **Automatic Initialization**: Stateful view runtime is created during `MetashrewRuntime::load()` and `MetashrewRuntime::new()`
- **Memory Persistence**: WASM memory state persists between view function calls
- **Performance Benefits**: Eliminates the overhead of creating fresh WASM instances for each view call
- **Backward Compatibility**: Existing code continues to work without changes

## How It Works

When stateful views are enabled (default):

1. **Persistent WASM Instance**: A single WASM instance is created and reused for all view calls
2. **Memory Retention**: WASM linear memory persists between view function invocations
3. **Context Updates**: Database context is updated for each view call, but WASM memory state is preserved
4. **Thread Safety**: The stateful runtime is protected by `Arc<tokio::sync::Mutex<_>>` for concurrent access

## Memory Management

The stateful view system includes comprehensive memory management:

- **LRU Cache Integration**: Uses the three-tier cache system with height partitioning
- **Memory Limits**: 1GB total memory limit with automatic LRU eviction
- **Cache Allocation Modes**: 
  - Indexer mode: All memory to main LRU cache
  - View mode: Memory split between height-partitioned and API caches
- **Height Isolation**: Cache is partitioned by height to ensure proper historical queries

## API Reference

### Check Status
```rust
// Check if stateful views are enabled (should be true by default)
if runtime.is_stateful_views_enabled() {
    println!("Stateful views are active (default)");
}
```

### Manual Control (Optional)
```rust
// Disable stateful views if needed (not recommended)
runtime.disable_stateful_views();

// Re-enable stateful views
runtime.enable_stateful_views().await?;

// Synchronous enable (used internally during initialization)
runtime.enable_stateful_views_sync()?;
```

## Use Cases

Stateful views are beneficial for:

- **Complex Computations**: Maintaining computed state across multiple queries
- **Caching**: WASM-level caching of expensive operations
- **Session State**: Maintaining user session data across view calls
- **Progressive Loading**: Building up complex data structures over multiple calls
- **Performance Optimization**: Avoiding repeated initialization costs

## Migration Notes

### For Existing Code
- **No Changes Required**: Existing code will automatically benefit from stateful views
- **Performance Improvement**: View calls will be faster due to memory persistence
- **Behavior Consistency**: View functions will work the same way, just with persistent memory

### For New Code
- **Design for Persistence**: WASM modules can now rely on memory persisting between view calls
- **Memory Management**: WASM authors are responsible for managing memory within their modules
- **State Isolation**: Remember that cache is still partitioned by height for historical queries

## Implementation Details

### Initialization Process
1. `MetashrewRuntime::load()` or `MetashrewRuntime::new()` is called
2. Basic runtime is created with `stateful_view_runtime: None`
3. `enable_stateful_views_sync()` is automatically called
4. `StatefulViewRuntime` is created and assigned
5. All subsequent view calls use the persistent runtime

### Fallback Behavior
If stateful view initialization fails:
- A warning is logged
- Runtime continues with `stateful_view_runtime: None`
- View calls fall back to creating fresh WASM instances (old behavior)
- No functionality is lost, only performance benefits

### Thread Safety
- Stateful runtime uses `Arc<tokio::sync::Mutex<wasmtime::Store<State>>>`
- Multiple concurrent view calls are safely serialized
- Context is updated per view call while preserving WASM memory

## Performance Impact

### Benefits
- **Reduced Overhead**: No WASM instance creation per view call
- **Memory Reuse**: Persistent allocations across calls
- **Initialization Savings**: One-time setup costs amortized across calls
- **Cache Efficiency**: Better cache locality with persistent memory

### Considerations
- **Memory Usage**: WASM memory persists between calls (managed by LRU cache)
- **Initialization Cost**: Slight increase in runtime creation time
- **Memory Responsibility**: WASM authors must manage their memory usage

## Troubleshooting

### If Stateful Views Fail to Initialize
Check logs for warnings like:
```
WARN Failed to enable stateful views by default: <error>
```

The runtime will continue to work with non-stateful views as fallback.

### Memory Issues
- Monitor memory usage with `lru_cache_memory_usage()`
- Check cache statistics with `lru_cache_stats()`
- LRU eviction will automatically manage memory pressure

### Debugging
- Use `is_stateful_views_enabled()` to verify status
- Check logs for stateful view initialization messages
- View function execution logs will indicate "stateful mode" when active

## Conclusion

Stateful views are now the default behavior in Metashrew, providing better performance and enabling more sophisticated indexer patterns while maintaining full backward compatibility. The system automatically handles memory management and provides fallback behavior if initialization fails.