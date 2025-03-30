# ALKANES-RS Active Context

## Current Work Focus

The ALKANES-RS project is a Rust implementation of the ALKANES metaprotocol for Bitcoin-based decentralized finance (DeFi). The project provides a framework for creating and executing smart contracts on the Bitcoin blockchain, leveraging the METASHREW indexer stack.

### Current Development Status

The project is structured as a Rust workspace with multiple crates, each serving a specific purpose in the overall system:

1. **Top-level crate (alkanes)**: Main indexer implementation for the METASHREW environment
2. **Support Crates**: Shared utilities for METASHREW integration, protorunes compatibility, and ALKANES protocol
3. **Runtime Crates**: Smart contract runtime environment
4. **Standard Library Crates**: Multiple `alkanes-std-*` crates implementing standard smart contracts

### Recent Changes

Based on the repository structure and documentation, the project appears to have implemented:

- Core indexer functionality for processing Bitcoin blocks and transactions
- WebAssembly-based smart contract execution environment
- Standard library contracts including auth-token, genesis-alkane, merkle-distributor, proxy, and upgradeable contracts
- Support for multiple Bitcoin-based networks including mainnet, testnet, regtest, dogecoin, luckycoin, and bellscoin

## Active Decisions and Considerations

### Architecture Decisions

1. **WASM-based Smart Contracts**: Smart contracts are compiled to WebAssembly for portability and security, with a runtime environment for execution and fuel metering to prevent DoS attacks.

2. **Modular Architecture**: The system is designed with a separation of concerns between indexing, execution, and state management, with support crates for shared functionality and a standard library for common contract patterns.

3. **Protocol Extensions**: ALKANES is built on top of the protorunes protocol, compatible with Bitcoin's transaction model, and leverages ordinals for additional functionality.

### Technical Considerations

1. **Cross-Network Support**: The system must support multiple Bitcoin-based networks with different address formats, block structures, activation heights, and network-specific parameters.

2. **Performance Optimization**: The indexer must efficiently process blockchain data, handle large blocks and transactions, maintain state consistency, provide responsive queries, and scale with blockchain growth.

3. **Security**: The system employs fuel metering to prevent DoS attacks, sandboxed execution to isolate contract execution from the host system, input validation to ensure only valid transactions are processed, and error handling to gracefully handle invalid inputs and execution failures.

## Next Steps

Based on the current state of the project, potential next steps could include:

1. **Expanded Standard Library**: Growing the set of standard contracts for common DeFi patterns.

2. **Improved Tooling**: Enhancing development, testing, and deployment tools.

3. **Cross-Protocol Interoperability**: Better integration with other Bitcoin layer 2 solutions.

4. **Performance Optimization**: Continued improvements to execution efficiency and state management.

5. **Community Governance**: Potential for community-driven protocol evolution.

6. **Documentation and Examples**: Expanding documentation and providing more examples to improve developer experience.

7. **Testing and Validation**: Comprehensive testing across different networks and use cases.