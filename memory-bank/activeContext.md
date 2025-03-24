# ALKANES-RS Active Context

## Current Work Focus

The ALKANES-RS project is focused on implementing a metaprotocol for DeFi operations on Bitcoin and compatible blockchains. The system is built as a Rust workspace with multiple crates, each serving a specific purpose in the overall architecture.

### Current Development Priorities

1. **Core Protocol Implementation**: Ensuring the ALKANES metaprotocol correctly processes Bitcoin transactions and maintains state.
2. **Smart Contract Runtime**: Refining the WebAssembly execution environment for smart contracts.
3. **Standard Library Contracts**: Expanding and improving the set of standard contracts for common DeFi patterns.
4. **Cross-Network Support**: Maintaining compatibility across multiple Bitcoin-based networks.

## Recent Changes

Based on the project structure and documentation, recent development appears to have focused on:

1. **Standard Library Expansion**: Implementation of various standard contracts including:
   - Authentication token contract
   - Owned token contract
   - Genesis contract for different networks
   - Proxy contract for upgradeable functionality
   - AMM pool and factory contracts

2. **Message Dispatch Framework**: Development of a framework for contract ABI definition and message handling.

3. **Testing Infrastructure**: Implementation of comprehensive testing strategies for both the indexer and smart contracts.

## Next Steps

The following areas have been identified as next steps for the project:

1. **BalanceSheet Refactoring**: Refactoring the BalanceSheet handling in protorune to support lazy loading of balances instead of loading the entire set of assets into memory, which is critical for scaling the ALKANES protocol with a large number of assets.

2. **Documentation Enhancement**: Improving documentation for developers building on the ALKANES protocol.

3. **Tooling Development**: Creating better tools for contract development, testing, and deployment.

4. **Performance Optimization**: Improving the efficiency of the indexer and smart contract execution.

5. **Feature Expansion**: Adding more DeFi primitives to the standard library.

6. **Community Engagement**: Encouraging adoption and contributions from the wider blockchain community.

## Active Decisions and Considerations

### Technical Decisions

1. **WebAssembly for Smart Contracts**: The decision to use WebAssembly for smart contract execution provides a good balance of security, performance, and portability.

2. **Trait-Based Abstraction**: The use of Rust traits for defining interfaces allows for clean separation of concerns and extensibility.

3. **Fuel Metering**: The implementation of a fuel system for metering computation prevents DoS attacks and ensures fair resource usage.

4. **Protocol Extension**: Building on top of the protorunes protocol leverages existing infrastructure while adding specialized functionality.

### Design Considerations

1. **Balance Between Flexibility and Security**: The system must provide enough flexibility for complex DeFi applications while maintaining the security guarantees expected from Bitcoin.

2. **Cross-Network Compatibility**: Supporting multiple Bitcoin-based networks requires careful handling of network-specific parameters and activation logic.

3. **Developer Experience**: Making the system accessible to developers requires good documentation, tooling, and familiar patterns.

4. **State Management**: Efficient state management is crucial for performance and scalability as the system grows.

### Open Questions

1. **Governance Model**: How will protocol upgrades and changes be managed as the system evolves?

2. **Interoperability**: How can ALKANES best interact with other Bitcoin layer 2 solutions and DeFi ecosystems?

3. **Scalability**: What optimizations will be needed as adoption grows and state size increases?
   - A critical scalability issue has been identified with the BalanceSheet object in the protorune codebase. Currently, it loads the entire set of protorune assets for the subprotocol into memory, which doesn't scale for protocols like ALKANES with potentially large asset sets.
   - The solution is to refactor this to implement lazy loading of balances (the mapping of AlkaneId => u128) only on demand, rather than loading everything into memory at once.
   - This refactoring is essential to eliminate what appears to be the last major scaling bottleneck for supporting a very large set of assets within a given subprotocol.

4. **User Interfaces**: What tools and interfaces will be developed to make ALKANES accessible to end users?

## Integration Points

The ALKANES-RS system integrates with several external components:

1. **Bitcoin Node**: For accessing blockchain data through the METASHREW indexer stack.

2. **METASHREW Indexer**: For efficient processing and indexing of blockchain data.

3. **Redis**: For state storage and caching.

4. **RPC Interface**: For external access to the system's functionality.

These integration points are critical for the system's operation and must be maintained as the project evolves.