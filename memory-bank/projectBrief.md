# ALKANES-RS Project Brief

## Project Overview

ALKANES-RS is a Rust implementation of the ALKANES metaprotocol, designed for Bitcoin-based decentralized finance (DeFi). The project provides a framework for creating and executing smart contracts on the Bitcoin blockchain, leveraging the METASHREW indexer stack. ALKANES is built as a subprotocol of runes that is compatible with protorunes, enabling DeFi functionality within Bitcoin's consensus model.

The project's primary goals are:
- Provide a metaprotocol for DeFi operations on Bitcoin
- Support smart contract execution through WebAssembly (WASM)
- Enable token creation, transfers, and complex financial operations
- Maintain compatibility with the Bitcoin consensus model

The genesis block for ALKANES is 880000, and the project is designed to work across multiple networks including mainnet, testnet, regtest, dogecoin, luckycoin, and bellscoin.

## Technical Context

### Programming Languages and Technologies
- **Primary Language**: Rust
- **Compilation Target**: WebAssembly (wasm32-unknown-unknown)
- **Runtime Environment**: METASHREW indexer stack
- **Testing Framework**: wasm-bindgen-test-runner
- **Build System**: Cargo (Rust's package manager)

### Dependencies
Major dependencies include:
- **bitcoin**: Bitcoin data structures and utilities
- **metashrew**: Indexer stack for blockchain data
- **protorune**: Implementation of the protorunes protocol
- **wasmi**: WebAssembly interpreter for smart contract execution
- **protobuf**: Protocol Buffers for data serialization
- **wasm-bindgen**: WebAssembly bindings for Rust
- **anyhow**: Error handling utilities
- **flate2**: Compression utilities

### System Architecture

The ALKANES-RS project is structured as a Rust workspace with multiple crates:

1. **Top-level crate (alkanes)**: 
   - Main indexer implementation for the METASHREW environment
   - Handles block processing, transaction validation, and state management

2. **Support Crates**:
   - **metashrew-support**: Shared utilities for METASHREW integration
   - **protorune-support**: Shared utilities for protorunes compatibility
   - **alkanes-support**: Core utilities for the ALKANES protocol

3. **Runtime Crates**:
   - **alkanes-runtime**: Smart contract runtime environment
   - Provides the execution context for ALKANES smart contracts
   - Handles storage, calls between contracts, and state management

4. **Standard Library Crates**:
   - Multiple `alkanes-std-*` crates implementing standard smart contracts:
     - **auth-token**: Authentication token implementation
     - **genesis-alkane**: Genesis contract for different networks
     - **merkle-distributor**: Token distribution mechanism
     - **proxy**: Contract proxy functionality
     - **upgradeable**: Upgradeable contract implementation
     - **amm-pool/amm-factory**: Automated Market Maker functionality
     - **orbital**: Additional protocol functionality

5. **Protocol Implementation Crates**:
   - **protorune**: Implementation of the protorunes protocol
   - **ordinals**: Support for Bitcoin ordinals

### Key Components

1. **Indexer**:
   - Processes Bitcoin blocks and transactions
   - Extracts and validates ALKANES protocol messages
   - Updates the state of the ALKANES ecosystem

2. **Virtual Machine (VM)**:
   - Executes WebAssembly smart contracts
   - Manages gas/fuel for computation
   - Provides isolation and security for contract execution

3. **Message System**:
   - Defines the format for inter-contract communication
   - Handles the transfer of tokens between contracts
   - Provides context for contract execution

4. **Storage System**:
   - Persistent storage for contract state
   - Key-value based storage interface
   - Efficient state management across contract calls

5. **Network Configuration**:
   - Support for multiple Bitcoin-based networks
   - Network-specific parameters and genesis configurations
   - Feature flags for different network targets

### Design Patterns

1. **Trait-based Abstraction**:
   - `AlkaneResponder` trait for smart contract implementation
   - `Extcall` trait for external contract calls
   - `Token` trait for token standard implementation

2. **WASM-based Smart Contracts**:
   - Contracts compiled to WebAssembly for portability and security
   - Runtime environment for contract execution
   - Fuel metering to prevent DoS attacks

3. **Modular Architecture**:
   - Separation of concerns between indexing, execution, and state management
   - Support crates for shared functionality
   - Standard library for common contract patterns

4. **Protocol Extensions**:
   - Built on top of the protorunes protocol
   - Compatible with Bitcoin's transaction model
   - Leverages ordinals for additional functionality

## Source Code Modules

### Main Modules

1. **src/lib.rs**:
   - Entry point for the ALKANES indexer
   - Exports functions for the METASHREW environment
   - Configures the network and initializes the system

2. **src/indexer.rs**:
   - Implements block indexing functionality
   - Configures network parameters
   - Processes blocks and updates state

3. **src/block.rs**:
   - Block processing logic
   - Extracts ALKANES protocol messages from transactions
   - Validates block structure

4. **src/message.rs**:
   - Defines message formats for the ALKANES protocol
   - Implements message parsing and validation
   - Handles context for message execution

5. **src/network.rs**:
   - Network-specific configurations
   - Genesis block and outpoint definitions
   - Network activation logic

6. **src/view.rs**:
   - Query interface for the ALKANES state
   - Implements RPC methods for external access
   - Provides simulation capabilities for transactions

7. **src/vm/**:
   - Virtual machine implementation for contract execution
   - Fuel/gas metering for computation
   - WebAssembly execution environment

### Support Modules

1. **crates/alkanes-support/**:
   - Core utilities for the ALKANES protocol
   - Shared data structures and functions
   - Protocol-specific helpers

2. **crates/alkanes-runtime/**:
   - Smart contract runtime environment
   - Storage and context management
   - Inter-contract communication

3. **crates/protorune/**:
   - Implementation of the protorunes protocol
   - Balance sheet management
   - Rune creation and transfer logic

4. **crates/metashrew/**:
   - Integration with the METASHREW indexer stack
   - Blockchain data access
   - State persistence

### Standard Library Modules

1. **crates/alkanes-std-auth-token/**:
   - Authentication token implementation
   - Access control for contracts
   - Permission management

2. **crates/alkanes-std-genesis-alkane/**:
   - Genesis contract implementation
   - Network-specific initialization
   - Protocol bootstrapping

3. **crates/alkanes-std-amm-pool/**:
   - Automated Market Maker pool implementation
   - Liquidity provision and swapping
   - Price discovery mechanism

4. **crates/alkanes-std-proxy/**:
   - Contract proxy functionality
   - Delegation of calls
   - Upgradeable contract support

## Additional Context

### Testing Strategy

The project employs multiple testing approaches:
- **Integration Tests**: End-to-end tests using the compiled WASM
- **Unit Tests**: Native Rust tests for individual components
- **Test Fixtures**: Simulated blockchain environments for testing
- **Test Helpers**: Utilities for creating test scenarios

Tests can be run using:
```
cargo test --all
```

### Deployment Procedure

The ALKANES indexer is built as a WASM binary that can be used with the METASHREW indexer stack:

1. Build the ALKANES indexer:
   ```
   cargo build --release --features all,<network>
   ```
   Where `<network>` is one of: mainnet, testnet, regtest, dogecoin, luckycoin, bellscoin

2. The build produces:
   - `alkanes.wasm`: Main indexer binary
   - Standard library contract WASMs in `target/alkanes/wasm32-unknown-unknown/release/`

3. Run with METASHREW:
   ```
   metashrew-keydb --redis <redis-url> --rpc-url <bitcoin-rpc> --auth <auth> --indexer <path-to-alkanes.wasm>
   ```

### Documentation

The ALKANES specification is hosted in the project wiki:
- [https://github.com/kungfuflex/alkanes-rs/wiki](https://github.com/kungfuflex/alkanes-rs/wiki)

Additional documentation for protorunes is available at:
- [https://github.com/kungfuflex/protorune/wiki](https://github.com/kungfuflex/protorune/wiki)

The project is designed to be used with the METASHREW indexer stack, documented at:
- [https://github.com/sandshrewmetaprotocols/metashrew](https://github.com/sandshrewmetaprotocols/metashrew)

### License

The project is licensed under the MIT License.

### Authors

- flex
- v16
- butenprks
- clothic
- m3 