# ALKANES-RS Progress

## What Works

Based on the project structure and documentation, the following components appear to be implemented and functional:

### Core Infrastructure

1. **Indexer Implementation**: The main indexer for processing Bitcoin blocks and extracting ALKANES protocol messages.
2. **WebAssembly Runtime**: The execution environment for smart contracts with fuel metering.
3. **Storage System**: Key-value storage for contract state persistence.
4. **Message System**: Inter-contract communication and token transfer mechanisms.

### Standard Library Contracts

1. **Auth Token Contract**: Implementation of authentication and access control mechanisms.
2. **Genesis Contract**: Network-specific initialization and protocol bootstrapping.
3. **Owned Token Contract**: Token implementation with ownership verification.
4. **Proxy Contract**: Contract delegation and upgradeability support.
5. **Merkle Distributor**: Token distribution mechanism.

### Development Tools

1. **Build System**: Cargo-based build system with custom scripts for WASM compilation.
2. **Testing Framework**: Comprehensive testing infrastructure for both native and WASM code.
3. **Message Dispatch Framework**: System for contract ABI definition and message handling.

### Network Support

1. **Multiple Networks**: Support for various Bitcoin-based networks including:
   - Bitcoin mainnet
   - Bitcoin testnet
   - Bitcoin regtest
   - Dogecoin
   - Luckycoin
   - Bellscoin
   - Fractal

## What's Left to Build

Based on the project documentation and future vision, the following areas may still need development:

### Standard Library Expansion

1. **Advanced DeFi Primitives**: More sophisticated financial instruments and mechanisms.
2. **Governance Contracts**: Implementations for decentralized governance.
3. **Cross-Chain Bridges**: Contracts for interoperability with other blockchains.

### Tooling Improvements

1. **Developer SDK**: Comprehensive SDK for building applications on ALKANES.
2. **Contract Templates**: More templates and examples for common use cases.
3. **Deployment Tools**: Better tools for contract deployment and management.
4. **Monitoring Tools**: Systems for monitoring contract execution and state.

### Performance Optimizations

1. **Indexer Efficiency**: Optimizations for faster block processing.
2. **State Management**: More efficient state storage and retrieval.
3. **Contract Execution**: Optimizations for WASM execution.

### User Interfaces

1. **Web Interface**: User-friendly web interface for interacting with ALKANES contracts.
2. **Wallet Integration**: Integration with popular Bitcoin wallets.
3. **Block Explorer**: Specialized block explorer for ALKANES transactions and contracts.

## Current Status

The project appears to be in an active development state with a functional core system and several standard contracts implemented. The codebase is organized as a Rust workspace with multiple crates, each serving a specific purpose in the overall architecture.

### Development Status

1. **Core Protocol**: Implemented and functional.
2. **Runtime Environment**: Implemented with fuel metering and state management.
3. **Standard Library**: Several key contracts implemented, with room for expansion.
4. **Testing**: Comprehensive testing infrastructure in place.
5. **Documentation**: Basic documentation available, could be expanded.

### Deployment Status

The system can be deployed using the METASHREW indexer stack with:

```sh
metashrew-keydb --redis <redis-url> --rpc-url <bitcoin-rpc> --auth <auth> --indexer <path-to-alkanes.wasm>
```

Different network targets can be built using feature flags:

```sh
cargo build --release --features all,<network>
```

## Known Issues

Based on the documentation, the following areas may present challenges or known issues:

### Technical Issues

1. **BalanceSheet Scaling**: In the protorune codebase, the BalanceSheet object currently loads the entire set of protorune assets for the subprotocol into memory. This approach doesn't scale for protocols like ALKANES that may have a very large set of assets. This needs to be refactored to implement lazy loading of balances (the mapping of AlkaneId => u128) only on demand.

2. **Double Indexing**: Double indexing (calling `index_block` multiple times for the same block) leads to inconsistent state and should be avoided, especially in tests.

3. **Table Dependencies**: View functions depend on specific tables being populated correctly. If these tables are not populated as expected, queries may return incomplete or incorrect results.

4. **Cross-Network Compatibility**: Supporting multiple networks with different parameters requires careful handling of network-specific logic.

### Development Challenges

1. **WebAssembly Limitations**: Smart contracts must operate within WebAssembly constraints, which may limit certain types of operations.

2. **Bitcoin Compatibility**: The system must work within Bitcoin's transaction model, which imposes limitations on data storage and execution.

3. **State Growth**: As the system is used, the state will grow, potentially leading to performance challenges.

### Documentation Gaps

1. **Developer Onboarding**: More comprehensive documentation for new developers may be needed.

2. **Contract Patterns**: Better documentation of common patterns and best practices for contract development.

3. **Error Handling**: More detailed documentation on error handling and debugging.

## Next Milestones

Based on the project's current state and future vision, the following milestones may be considered:

1. **BalanceSheet Refactoring**: Refactoring the BalanceSheet handling in the protorune codebase to implement lazy loading of balances instead of loading the entire set of assets into memory. This is critical for scaling the ALKANES protocol to support a large number of assets and is likely the last major scaling bottleneck.

2. **Expanded Standard Library**: Implementing more DeFi primitives and contract templates.

3. **Improved Developer Tools**: Creating better tools for contract development and deployment.

4. **Performance Optimization**: Enhancing the efficiency of the indexer and contract execution.

5. **Community Building**: Encouraging adoption and contributions from the wider blockchain community.

6. **Integration with Other Systems**: Improving interoperability with other Bitcoin layer 2 solutions.