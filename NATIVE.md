# ALKANES Native Indexer

This document explains how to build and run the ALKANES indexer as a native standalone binary for better performance.

## Overview

The ALKANES indexer can now be built as a native standalone binary, which runs without the overhead of a WebAssembly virtual machine. This results in better performance while maintaining the same functionality as the WASM version.

## Building the Native Indexer

### Prerequisites

- Rust toolchain (stable)
- Bitcoin Core node (for indexing)

### Build Commands

To build the native indexer with default settings (regtest):

```bash
cargo build --bin alkanes-native --features native --release
```

To build for specific networks, add the appropriate feature flag:

```bash
# For mainnet
cargo build --bin alkanes-native --features "native mainnet" --release

# For testnet
cargo build --bin alkanes-native --features "native testnet" --release

# For dogecoin
cargo build --bin alkanes-native --features "native dogecoin" --release

# For luckycoin
cargo build --bin alkanes-native --features "native luckycoin" --release

# For bellscoin
cargo build --bin alkanes-native --features "native bellscoin" --release
```

You can also combine with other feature flags:

```bash
# For mainnet with cache
cargo build --bin alkanes-native --features "native mainnet cache" --release

# For mainnet with all features
cargo build --bin alkanes-native --features "native mainnet all" --release
```

## Running the Native Indexer

The native indexer has the same command-line interface as `rockshrew-mono`, but without the need to specify a WASM file:

```bash
./target/release/alkanes-native \
  --daemon-rpc-url http://localhost:8332 \
  --auth bitcoinrpc:password \
  --db-path ~/.metashrew \
  --host 0.0.0.0 \
  --port 8080
```

### Command-Line Options

- `--daemon-rpc-url`: URL of the Bitcoin Core RPC server
- `--auth`: Authentication credentials for the Bitcoin Core RPC server (username:password)
- `--db-path`: Path to the database directory
- `--start-block`: Starting block height (optional)
- `--label`: Database label (optional)
- `--exit-at`: Exit at block height (optional)
- `--host`: Host to bind the JSON-RPC server to (default: 127.0.0.1)
- `--port`: Port to bind the JSON-RPC server to (default: 8080)
- `--cors`: CORS allowed origins (optional)
- `--pipeline-size`: Pipeline size for block processing (default: 5)

## Performance Comparison

The native indexer typically provides better performance than the WASM version:

- Faster block processing
- Lower memory usage
- Better CPU utilization
- No WASM VM overhead

## Compatibility

The native indexer is fully compatible with the WASM version. It:

- Uses the same database format
- Provides the same JSON-RPC API
- Supports the same view functions
- Handles chain reorganizations in the same way

## Troubleshooting

### Common Issues

1. **Database Compatibility**: The native indexer uses the same database format as the WASM version, so you can switch between them without issues.

2. **Memory Usage**: The native indexer may use more memory initially but should be more efficient over time.

3. **Network Configuration**: Make sure to build with the correct network feature flag for your use case.

### Logs

The native indexer uses the same logging system as the WASM version. You can control the log level using the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug ./target/release/alkanes-native ...
```

## Development

When developing with the native indexer, you can use the same codebase for both WASM and native targets. The `native` feature flag controls which version is built.

### Adding New View Functions

When adding new view functions, make sure to:

1. Implement them in the ALKANES library
2. Add the corresponding `ProtoViewFunction` implementation in `alkanes-native.rs`
3. Register them in the `view_functions` method of the `NativeIndexer` implementation

## License

MIT