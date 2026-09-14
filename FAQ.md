# Alkanes FAQ

## What is Alkanes?

Alkanes is a metaprotocol that brings smart contracts to Bitcoin L1 by inscribing WASM binaries into the witness data of transactions — the same space that Ordinals uses to inscribe images and other media. Instead of pictures, Alkanes inscribes programs.

WASM (WebAssembly) is a more modern and widely supported program format than EVM bytecode. It is a compilation target for Rust, C, C++, AssemblyScript, and many other languages, meaning developers can write Alkanes smart contracts in familiar toolchains rather than learning a domain-specific language like Solidity.

## How is Alkanes different from an L2?

Traditional L2s and rollups rely on a centralized sequencer or a permissioned set of PoA/PoS validators to batch transactions and submit them back to L1. This introduces trust assumptions, liveness dependencies, and additional infrastructure beyond Bitcoin itself.

Alkanes has none of that. There is no sequencer, no separate validator set, and no off-chain execution environment. Every state change settles per-transaction directly on Bitcoin L1. The protocol is a light extension of Bitcoin itself — anyone running an Alkanes-aware indexer can independently derive the full state from the Bitcoin blockchain alone.

## How does it compare to Ordinals?

Ordinals demonstrated that Bitcoin's witness space can carry arbitrary data — images, text, video — and that the community will adopt protocols built on this capability. Alkanes takes the same insight and applies it to computation: instead of inscribing a JPEG, you inscribe a WASM binary that can be called, composed, and executed.

Where Ordinals gives Bitcoin NFTs and fungible tokens (BRC-20), Alkanes gives Bitcoin programmable smart contracts with on-chain state.

## What makes it a "sovereign rollup"?

A sovereign rollup is one where the canonical state is defined purely as a deterministic transformation of L1 data. Alkanes fits this definition:

- **Data availability is implicit.** All transaction data lives on Bitcoin L1 — there is no separate DA layer to trust or pay for.
- **State is a pure function of L1 data.** Any node can replay the Bitcoin blockchain through the Alkanes indexer and arrive at the exact same state. There is no external input or consensus mechanism beyond Bitcoin's own.
- **Settlement is per-transaction.** Each Bitcoin transaction that contains an Alkanes operation settles immediately in the block it is mined in. There are no batching delays or finality windows beyond Bitcoin's own confirmation model.

The result is a system that inherits Bitcoin's security and decentralization properties while adding programmability — without the trust assumptions that come with traditional L2 architectures.
