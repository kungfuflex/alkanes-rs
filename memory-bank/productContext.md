# ALKANES-RS Product Context

## Purpose and Problem Statement

ALKANES-RS is designed to address the need for decentralized finance (DeFi) capabilities on the Bitcoin blockchain. While Bitcoin has strong security and widespread adoption, it lacks the native smart contract functionality that has enabled complex financial applications on other blockchains like Ethereum.

The key problems ALKANES-RS solves include:

1. **Limited Programmability**: Bitcoin's scripting language is intentionally limited, making complex financial applications difficult to implement natively.
2. **DeFi Gap**: Bitcoin has lacked the rich DeFi ecosystem that exists on other blockchains, despite having the largest market capitalization.
3. **Protocol Compatibility**: Implementing DeFi on Bitcoin requires maintaining compatibility with Bitcoin's consensus model and transaction structure.
4. **Execution Environment**: Smart contracts need a secure, metered execution environment to prevent DoS attacks and ensure reliable operation.

## Solution Approach

ALKANES-RS provides a metaprotocol layer that enables DeFi functionality on Bitcoin without requiring changes to the Bitcoin protocol itself. It achieves this by:

1. **Building on Protorunes**: ALKANES is implemented as a subprotocol of runes that is compatible with protorunes, leveraging existing token standards.
2. **WebAssembly Execution**: Smart contracts are compiled to WebAssembly (WASM) for secure, sandboxed execution.
3. **METASHREW Integration**: The system integrates with the METASHREW indexer stack for efficient blockchain data processing and state management.
4. **Fuel Metering**: Computation is metered using a fuel system to prevent DoS attacks, similar to gas on other blockchains.

## User Experience Goals

ALKANES-RS aims to provide:

1. **Developer-Friendly Environment**: A Rust-based development environment for writing smart contracts with familiar tools and patterns.
2. **Familiar DeFi Primitives**: Support for common DeFi operations like token creation, transfers, AMM pools, and more.
3. **Cross-Network Compatibility**: Support for multiple Bitcoin-based networks including mainnet, testnet, regtest, dogecoin, luckycoin, and bellscoin.
4. **Efficient State Management**: Optimized state handling for smart contract execution and data persistence.
5. **Security and Reliability**: Protection against common attack vectors through metered execution and proper isolation.

## Target Users

ALKANES-RS targets several user groups:

1. **Bitcoin Developers**: Developers looking to build DeFi applications on Bitcoin without moving to other blockchains.
2. **DeFi Projects**: Projects wanting to expand their offerings to the Bitcoin ecosystem.
3. **Bitcoin Holders**: Users who want to participate in DeFi activities while keeping their assets on Bitcoin-based chains.
4. **Cross-Chain Applications**: Applications that want to provide consistent functionality across multiple blockchain ecosystems.

## Competitive Landscape

ALKANES-RS exists in an ecosystem with other Bitcoin extension protocols:

1. **Ordinals and Inscriptions**: Provide NFT-like functionality but lack the programmability for complex DeFi.
2. **RGB Protocol**: Another smart contract system for Bitcoin with different design choices.
3. **Stacks**: A separate blockchain with Bitcoin anchoring that enables smart contracts.
4. **Liquid Network**: A Bitcoin sidechain with some additional scripting capabilities.

ALKANES-RS differentiates itself through its direct integration with Bitcoin's transaction model, WASM-based execution environment, and focus on DeFi primitives.

## Success Metrics

The success of ALKANES-RS can be measured by:

1. **Adoption**: Number of projects building on the ALKANES protocol.
2. **Transaction Volume**: Amount of Bitcoin value flowing through ALKANES contracts.
3. **Contract Diversity**: Variety of DeFi applications implemented using ALKANES.
4. **Developer Experience**: Ease of development and deployment for smart contracts.
5. **Network Support**: Successful operation across multiple Bitcoin-based networks.

## Future Vision

The long-term vision for ALKANES-RS includes:

1. **Expanded Standard Library**: Growing the set of standard contracts for common DeFi patterns.
2. **Improved Tooling**: Enhanced development, testing, and deployment tools.
3. **Cross-Protocol Interoperability**: Better integration with other Bitcoin layer 2 solutions.
4. **Performance Optimization**: Continued improvements to execution efficiency and state management.
5. **Community Governance**: Potential for community-driven protocol evolution.