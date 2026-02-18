// OP_NET module for interacting with opshrew-indexed OP_NET state
// via metashrew_view JSON-RPC calls.
//
// View functions match the opshrew WASM exports and correspond
// to canonical OP_NET JSON-RPC methods (btc_*).

pub mod client;
pub mod types;

pub use client::OpnetClient;
#[cfg(feature = "std")]
pub use client::NativeOpnetClient;
pub use types::*;
