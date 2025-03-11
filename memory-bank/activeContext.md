# ALKANES-RS Active Context

## Current Work Focus

The ALKANES-RS project is focused on implementing a metaprotocol for DeFi operations on Bitcoin. The current work centers around:

1. **Core Protocol Implementation**: Developing and refining the ALKANES metaprotocol as a subprotocol of runes that is compatible with protorunes.
2. **Smart Contract Runtime**: Enhancing the WebAssembly-based execution environment for smart contracts.
3. **Standard Library Development**: Expanding the set of standard contracts for common DeFi patterns.
4. **Cross-Network Support**: Ensuring compatibility across multiple Bitcoin-based networks.

## Recent Changes

Based on the project structure and documentation, recent development appears to have focused on:

1. **Standard Contract Implementation**: Development of various standard contracts including:
   - Authentication token contracts
   - Owned token contracts
   - Genesis contracts for different networks
   - Proxy contracts for upgradeable functionality
   - AMM (Automated Market Maker) contracts

2. **Message Dispatch Framework**: Implementation of a framework for contract ABI exposure and message handling.

3. **Multi-Network Support**: Adding support for various Bitcoin-based networks including mainnet, testnet, regtest, dogecoin, luckycoin, and bellscoin.

## Next Steps

The following areas have been identified as priorities for continued development:

1. **Expanded Standard Library**: Continue developing standard contracts for common DeFi patterns.
2. **Improved Tooling**: Enhance development, testing, and deployment tools.
3. **Performance Optimization**: Improve execution efficiency and state management.
4. **Documentation**: Expand documentation for developers building on ALKANES.
5. **Testing Infrastructure**: Enhance testing capabilities for contracts and protocol features.

## Active Decisions and Considerations

Several key decisions and considerations are currently guiding development:

1. **Trait-Based Abstraction**: Using Rust traits to define interfaces and behavior for contracts and protocol components.
2. **WASM-Based Execution**: Continuing to refine the WebAssembly execution environment for smart contracts.
3. **Table Relationship Management**: Ensuring proper relationships between database tables for consistent state management.
4. **Fuel Metering**: Refining the fuel system to prevent DoS attacks while allowing efficient contract execution.
5. **Cross-Network Compatibility**: Balancing network-specific features with a consistent core protocol.
6. **Message Context Pattern**: Using a message context pattern to encapsulate transaction data and execution environment.

## Integration Points

Key integration points in the current development include:

1. **METASHREW Indexer Stack**: Integration with the underlying infrastructure for processing blockchain data.
2. **Protorunes Protocol**: Building on and extending the protorunes protocol for token functionality.
3. **Bitcoin Blockchain**: Ensuring compatibility with Bitcoin's transaction model and consensus rules.
4. **WebAssembly Runtime**: Integration with the wasmi interpreter for contract execution.

## Current Challenges

The development team is addressing several challenges:

1. **Bitcoin Compatibility**: Working within the constraints of Bitcoin's limited scripting capabilities.
2. **WebAssembly Limitations**: Managing the constraints of the WebAssembly execution environment.
3. **Indexer Performance**: Ensuring efficient processing of blockchain data and state management.
4. **Cross-Network Support**: Handling differences between various Bitcoin-based networks.
5. **Contract Security**: Ensuring secure execution and proper isolation of smart contracts.