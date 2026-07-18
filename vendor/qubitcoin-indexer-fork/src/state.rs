//! WASM host state for the fork-mode runtime.
//!
//! Mirrors `WasmState` in the slim runtime (vendor/qubitcoin-indexer/
//! src/runtime.rs:18-32) but swaps the RocksDB-backed `IndexerStorage`
//! for a [`crate::storage::MemStorage`] and carries an optional
//! [`crate::upstream::ForkUpstream`] for read-through misses.

use std::collections::HashMap;
use wasmtime::StoreLimits;

use crate::storage::MemStorage;

/// Host state threaded through the wasmtime `Store`. The runtime's
/// host functions read/write through this struct via
/// `Caller::data{,_mut}`.
pub struct ForkWasmState {
    pub input_data: Vec<u8>,
    pub pending_flush: Option<Vec<(Vec<u8>, Vec<u8>)>>,
    pub storage: MemStorage,
    pub had_failure: bool,
    pub completed: bool,
    pub label: String,
    pub view_mode: bool,
    pub limits: StoreLimits,
    /// Write-through cache for same-block __flush writes. Mirrors the
    /// slim runtime's behavior so subsequent __get/__get_len reads
    /// within the same _start() see the staged values.
    pub write_cache: HashMap<Vec<u8>, Vec<u8>>,
}

impl ForkWasmState {
    pub fn new(
        input_data: Vec<u8>,
        storage: MemStorage,
        label: &str,
        view_mode: bool,
    ) -> Self {
        let limits = wasmtime::StoreLimitsBuilder::new()
            .memories(usize::MAX)
            .tables(usize::MAX)
            .instances(usize::MAX)
            .build();
        Self {
            input_data,
            pending_flush: None,
            storage,
            had_failure: false,
            completed: false,
            label: label.to_string(),
            view_mode,
            limits,
            write_cache: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upstream::testing::StubUpstream;
    use std::collections::HashMap as Map;
    use std::sync::Arc;

    #[test]
    fn new_state_initializes_fields() {
        let storage = MemStorage::new(Arc::new(StubUpstream::new(Map::new())), 5);
        let s = ForkWasmState::new(b"in".to_vec(), storage, "alkanes", false);
        assert_eq!(s.input_data, b"in");
        assert!(s.pending_flush.is_none());
        assert!(!s.had_failure);
        assert!(!s.completed);
        assert_eq!(s.label, "alkanes");
        assert!(!s.view_mode);
        assert!(s.write_cache.is_empty());
    }
}
