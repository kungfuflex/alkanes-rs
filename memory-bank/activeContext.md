# ALKANES-RS Active Context

## Current Work Focus

The ALKANES-RS project is currently focused on implementing and refining the core components of the ALKANES metaprotocol for Bitcoin-based decentralized finance. The system is structured as a Rust workspace with multiple crates, each serving a specific purpose in the overall architecture.

## Recent Changes

Based on the existing codebase, the project appears to have established:

1. **Core Protocol Implementation**: The foundational components of the ALKANES metaprotocol have been implemented, including:
   - Block processing and indexing
   - Message extraction and validation
   - WebAssembly-based smart contract execution
   - Storage and state management

2. **Standard Library Contracts**: Several standard contracts have been implemented:
   - Authentication token (alkanes-std-auth-token)
   - Genesis contracts for different networks (alkanes-std-genesis-alkane)
   - Owned token implementation (alkanes-std-owned-token)
   - Proxy contract functionality (alkanes-std-proxy)
   - Upgradeable contract support (alkanes-std-upgradeable)
   - Merkle distributor for token distribution (alkanes-std-merkle-distributor)
   - Orbital functionality (alkanes-std-orbital)

3. **Multi-Network Support**: The system supports multiple Bitcoin-based networks through feature flags:
   - Bitcoin mainnet, testnet, and regtest
   - Dogecoin
   - Luckycoin
   - Bellscoin
   - Fractal

4. **Fuel Management Improvements**: The fuel system has been updated to correctly handle fuel refunding and consumption:
   - Fixed the `consume_fuel` method to properly track remaining fuel and provide clearer error messages
   - Updated the `drain_fuel` method to avoid incorrect fuel deductions in error cases
   - Ensured that only the actual remaining fuel (not the initially allocated amount) is refunded to the block
   - Added explicit fuel consumption checks to prevent "all fuel consumed by WebAssembly" errors
   - Enhanced logging throughout the fuel management system to provide detailed diagnostic information:
     - Added comprehensive logging to `consume_fuel` with detailed error information
     - Added execution tracking logs to `run_after_special` to monitor fuel usage during WebAssembly execution
     - Added allocation tracking logs to `fuel_transaction` to monitor initial fuel allocation
     - Added refunding tracking logs to `refuel_block` to monitor fuel refunding process
     - Added detailed logging to all host functions that consume fuel:
       - Storage operations: `request_storage`, `load_storage`
       - Context operations: `request_context`, `load_context`
       - Block and transaction operations: `request_block`, `load_block`, `request_transaction`, `load_transaction`
       - Utility operations: `sequence`, `fuel`, `height`, `balance`, `returndatacopy`
       - Contract operations: `extcall` (including deployment fuel)
     - Added transaction-level cellpack logging:
       - Logs detailed information about the contract being called at the start of each transaction
       - Shows target contract address, input count, and first opcode (operation being performed)
       - Logs resolved contract addresses after address resolution
       - Provides enhanced error reporting with contract-specific context for fuel-related errors
   - Implemented fuel benchmarking in the test suite:
     - Added a benchmarking framework to `src/tests/genesis.rs`
     - Created utilities for tracking and displaying fuel consumption metrics
     - Added detailed fuel usage reporting for different operations (genesis block processing, mint operations)
     - Implemented percentage-based fuel consumption analysis
   - Optimized fuel costs for large data operations:
     - Replaced variable fuel costs with fixed costs for block and transaction loading operations
     - Added `FUEL_LOAD_BLOCK` and `FUEL_LOAD_TRANSACTION` constants for fixed fuel costs
     - Modified host functions to use fixed costs instead of scaling with data size
     - Added detailed logging for block and transaction loading operations
     - Confirmed effectiveness with real transaction logs:
       - Loading a 1.5MB block now costs only 1,000 fuel units (fixed)
       - Previously would have cost ~3,000,000 fuel units (2 units per byte)
       - Significant savings that prevent "all fuel consumed" errors

## Next Steps

Based on the project structure and documentation, potential next steps could include:

1. **Enhanced DeFi Primitives**: Expanding the standard library with additional DeFi components:
   - Lending and borrowing protocols
   - Staking and yield farming
   - Derivatives and synthetic assets
   - Governance mechanisms

2. **Developer Tooling**: Improving the developer experience:
   - CLI tools for contract deployment and interaction
   - Testing frameworks and simulation environments
   - Documentation and examples
   - Client libraries for different languages

3. **Performance Optimization**: Enhancing system performance:
   - Optimizing state access patterns
   - Improving WASM execution efficiency
   - Reducing memory usage
   - Enhancing indexing speed

4. **Integration Testing**: Comprehensive testing across networks:
   - End-to-end testing with real blockchain data
   - Stress testing with high transaction volumes
   - Security audits and vulnerability testing
   - Cross-network compatibility testing

## Active Decisions and Considerations

1. **Message Dispatch Framework**: The project has implemented a message dispatch framework using Rust enums and traits to simplify contract development and ABI generation. This approach provides a clean interface for defining contract methods and handling parameters.

2. **Storage Abstraction**: The system uses a key-value storage abstraction for contract state, providing a consistent interface across different storage backends. This allows for efficient state caching and batching.

3. **Fuel Metering**: Computation is metered using a fuel system to prevent DoS attacks, with block-level fuel allocation and transaction-level fuel tracking. This ensures that contracts cannot consume excessive resources.

4. **Cross-Network Compatibility**: The system supports multiple Bitcoin-based networks through feature flags and configuration, allowing for network-specific parameters and conditional compilation for different targets.

5. **Error Handling Strategy**: The project uses Rust's `anyhow` for error handling, providing rich error context, propagation of errors across boundaries, and consistent error reporting.

6. **Testing Approach**: The system employs multiple testing strategies, including unit tests, integration tests, end-to-end tests, and WASM tests. This comprehensive approach ensures correctness at all levels of the system.

## Current Development Environment

The project is built using:
- Rust toolchain with wasm32-unknown-unknown target
- Cargo as the build system
- wasm-bindgen-test-runner for testing WebAssembly code
- Protocol Buffers for data serialization

The development workflow involves:
1. Writing smart contracts in Rust
2. Compiling to WebAssembly
3. Testing with the wasm-bindgen-test-runner
4. Deploying with the METASHREW indexer stack

## Integration Points

The ALKANES-RS project integrates with several external systems:

1. **Bitcoin Blockchain**: The primary data source and transaction medium
2. **METASHREW Indexer Stack**: The underlying infrastructure for processing blockchain data
3. **Protorunes Protocol**: The token standard that ALKANES extends
4. **WebAssembly Runtime**: The execution environment for smart contracts