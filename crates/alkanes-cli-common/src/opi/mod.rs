/// OPI (Open Protocol Indexer) module
/// Provides functionality to interact with BRC-20 and other meta-protocol indexers
/// Based on https://github.com/bestinslot-xyz/OPI

pub mod types;
#[cfg(feature = "std")]
pub mod client;
#[cfg(feature = "std")]
pub mod commands;

pub use types::*;
#[cfg(feature = "std")]
pub use client::OpiClient;
#[cfg(feature = "std")]
pub use commands::*;
