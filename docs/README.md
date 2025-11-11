# Alkanes-RS Documentation

Welcome to the comprehensive documentation for alkanes-rs, a complete Bitcoin metaprotocol implementation with DeFi capabilities.

## Documentation Structure

### Getting Started
- [Quick Start Guide](./quickstart.md) - Get up and running in minutes
- [Installation](./installation.md) - Detailed setup instructions
- [CLI Usage](./cli-usage.md) - Command-line interface guide

### Architecture
- [System Overview](./architecture/overview.md) - High-level system design
- [Indexer Stack](./architecture/indexer-stack.md) - How the indexer works
- [Runtime System](./architecture/runtime.md) - WASM runtime and execution
- [Storage Layer](./architecture/storage.md) - State management and persistence

### CLI Tools
- [Alkanes CLI](./cli/alkanes.md) - Main CLI tool for alkanes operations
- [BRC20-Prog CLI](./cli/brc20-prog.md) - BRC20 programmable contracts
- [Wallet Management](./cli/wallet.md) - Wallet operations and key management
- [Transaction Building](./cli/transactions.md) - Creating and signing transactions

### Protocol Features
- [Wrap-BTC](./features/wrap-btc.md) - Wrapping BTC to frBTC
- [AMM & DeFi](./features/amm.md) - Automated market maker functionality
- [Protostones](./features/protostones.md) - Transaction composition
- [Smart Contracts](./features/smart-contracts.md) - Alkane contract development
- [External Signing](./features/external-signing.md) - Address-only mode and external key signing
- [Transaction Broadcasting](./features/transaction-broadcasting.md) - All broadcast options (Slipstream, Rebar, etc.)
- [Rebar Shield](./features/rebar-shield.md) - Private relay with MEV protection

### Crates Reference
- [alkanes](./crates/alkanes.md) - Core indexer implementation
- [alkanes-cli](./crates/alkanes-cli.md) - CLI binary crate
- [alkanes-cli-common](./crates/alkanes-cli-common.md) - Shared CLI functionality
- [alkanes-runtime](./crates/alkanes-runtime.md) - Smart contract runtime
- [metashrew-*](./crates/metashrew.md) - Indexer runtime stack
- [protorune](./crates/protorune.md) - Protorune protocol implementation

### Developer Guides
- [Building Alkanes](./dev/building-alkanes.md) - Creating alkane smart contracts
- [Testing](./dev/testing.md) - Running tests and debugging
- [Contributing](./dev/contributing.md) - How to contribute to the project
- [RPC API](./dev/rpc-api.md) - Metashrew RPC interface

### Examples
- [Basic Operations](./examples/basic.md) - Simple usage examples
- [Contract Deployment](./examples/deploy.md) - Deploying smart contracts
- [DeFi Integration](./examples/defi.md) - Using AMM and liquidity pools
- [Advanced Patterns](./examples/advanced.md) - Complex transaction patterns

## Quick Links

- [GitHub Repository](https://github.com/kungfuflex/alkanes-rs)
- [Alkanes Wiki](https://github.com/kungfuflex/alkanes-rs/wiki)
- [Protorune Spec](https://github.com/kungfuflex/protorune/wiki)
- [Metashrew](https://github.com/sandshrewmetaprotocols/metashrew)

## Getting Help

- Join the SANDSHREW サンド Discord
- Open an issue on GitHub
- Check the [FAQ](./faq.md)

## License

MIT
