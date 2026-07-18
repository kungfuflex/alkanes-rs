//! Fork-mode WASM indexer runtime with read-through upstream.
//!
//! This crate is a slim sibling of the RocksDB-backed
//! `qubitcoin-indexer` runtime. It serves the **server-side mempool /
//! pending-state projection** use case where:
//!
//!   * Storage is in-memory (HashMap-backed [`MemStorage`]) — no
//!     RocksDB snapshot is required.
//!   * Reads that miss the in-memory layer fall through to a
//!     caller-supplied async [`ForkUpstream`] (typically a JSON-RPC
//!     `metashrew_view "getstorageat"` against an upstream confirmed
//!     indexer).
//!   * Upstream hits are written back into the overlay so subsequent
//!     same-block reads short-circuit without another network round trip.
//!
//! The runtime mirrors the metashrew-compatible ABI exposed by
//! `qubitcoin-indexer/src/runtime.rs`:
//!
//!   * `__host_len`, `__load_input`
//!   * `__get`, `__get_len` — augmented with upstream fall-through
//!   * `__flush`            — stages KV pairs in `pending_flush`
//!   * `__log`, `abort`
//!
//! Block processing uses an async-fiber `_start()` invocation (matches
//! metashrew); view calls have both sync and async flavors.
//!
//! # Wiring sketch
//!
//! ```ignore
//! use qubitcoin_indexer_fork::{ForkRuntime, ForkUpstream, MemStorage};
//! use std::sync::Arc;
//!
//! struct MyUpstream;
//! #[async_trait::async_trait]
//! impl ForkUpstream for MyUpstream {
//!     async fn fetch(&self, key: &[u8]) -> Result<Option<Vec<u8>>, String> {
//!         /* call your JSON-RPC */
//!         Ok(None)
//!     }
//! }
//!
//! let runtime = ForkRuntime::new(&wasm_bytes)?;
//! let storage = MemStorage::new(Arc::new(MyUpstream), /* projected_height */ 100);
//! let pairs   = runtime.run_block(input, storage.clone(), "alkanes")?;
//! // pairs are the kv writes the WASM produced; apply them as needed.
//! # Ok::<(), String>(())
//! ```

pub mod runtime;
pub mod state;
pub mod storage;
pub mod upstream;

pub use runtime::ForkRuntime;
pub use state::ForkWasmState;
pub use storage::MemStorage;
pub use upstream::ForkUpstream;

#[cfg(feature = "http-upstream")]
pub use upstream::HttpForkUpstream;
