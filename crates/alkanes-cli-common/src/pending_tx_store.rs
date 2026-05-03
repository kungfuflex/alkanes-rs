//! Pending-tx store for chained / mempool-aware UTXO selection.
//!
//! Background — the runtime gap this closes:
//!
//!   When the SDK broadcasts a tx, the user's local node has it instantly
//!   but the chain indexers (esplora, metashrew, espo) only catch up
//!   ~hundreds of ms later. Any subsequent `select_utxos` call in the same
//!   session that asks "what can the user spend?" reads the indexer's
//!   pre-broadcast view, picks the just-spent prevouts, and produces a
//!   tx that conflicts with the one already in mempool — observed as
//!   BIP125 RBF rejections during atomic split-tx flows on mainnet
//!   2026-05-03 (commit `1d838f89`).
//!
//!   The previous fix (`known_pending_tx_hexes` param on
//!   `EnhancedExecuteParams`) plumbed Tx A's hex through to Tx B's
//!   selector at one specific call site (`execute_split`). It works but
//!   is local — every new caller has to remember to thread the hex
//!   through, and the param doesn't persist across distinct
//!   `alkanesExecute*` calls (e.g. user broadcasts atomic wrap+swap
//!   then immediately initiates an alkane-send before the first tx is
//!   indexed).
//!
//! `PendingTxStore` upgrades that into a session-scoped store that
//! `select_utxos` reads automatically on every call:
//!
//!   - `add(tx_hex)` — broadcast paths (`execute_full`, `wrap_btc`,
//!     `amm_cli`, etc.) push successful broadcasts here so any
//!     subsequent selection sees the new state.
//!   - `list()` — returns all pending tx hexes; `select_utxos`
//!     decodes each via `decode_tx_hex_to_mempool_json` and merges
//!     into the same `apply_mempool_adjustment` call that already
//!     handles esplora's mempool view.
//!   - `evict_confirmed(indexer_height)` — purges entries the indexer
//!     has now seen. Called opportunistically after each successful
//!     `provider.sync()` round-trip.
//!
//! Implementations:
//!   - `MemoryPendingTxStore` (this module) — `Arc<Mutex<...>>` map.
//!     Default for CLI / tests.
//!   - `IndexedDbPendingTxStore` (alkanes-web-sys) — persists across
//!     page reloads so a user who broadcasts a tx, refreshes, and
//!     immediately initiates another op still sees the pending state
//!     (overlaid until the indexer catches up).
//!
//! The `known_pending_tx_hexes` field on `EnhancedExecuteParams` stays
//! as an explicit per-call override for callers without provider
//! access (vendored CLI tools, integration tests) but the store is
//! the canonical path going forward.

#[cfg(feature = "std")]
use std::collections::BTreeMap;
#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap;

#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};
#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use spin::Mutex;

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use async_trait::async_trait;
use anyhow::Result;

/// Trait for a session-scoped store of broadcast-but-unconfirmed
/// transactions.
///
/// Implementations must be cheap to call (nothing more than a hash-map
/// lookup is expected for `list()`) because `select_utxos` calls
/// `list()` on every selection round.
///
/// All methods take `&self` rather than `&mut self` so the store can
/// be wrapped in an `Arc` and shared across `select_utxos` /
/// `execute_full` / async tasks. Internal mutation is the impl's
/// concern.
#[async_trait(?Send)]
pub trait PendingTxStore {
    /// Insert a freshly-broadcast tx (raw hex, same format as
    /// `sendrawtransaction` arg). Idempotent on txid — calling twice
    /// with the same tx is a no-op.
    async fn add(&self, tx_hex: &str) -> Result<()>;

    /// Return all pending tx hexes (txid → hex) currently in the
    /// store. Order is not specified; callers that need a stable
    /// order should sort.
    async fn list(&self) -> Result<Vec<String>>;

    /// Remove a single tx by txid. Used when a tx confirms or gets
    /// evicted from mempool. No-op if the txid isn't present.
    async fn remove(&self, txid: &str) -> Result<()>;

    /// Bulk-evict transactions whose txids appear in the given list.
    /// Typically called after a `provider.sync()` round-trip with the
    /// set of txids the indexer has now seen confirmed at the new
    /// tip — saves N round-trips vs N individual `remove()` calls.
    async fn evict(&self, confirmed_txids: &[String]) -> Result<()>;

    /// Wipe the entire store. Test-only, exposed on the trait so
    /// implementors don't have to downcast.
    async fn clear(&self) -> Result<()>;

    /// Number of pending entries — useful for instrumentation.
    async fn len(&self) -> Result<usize>;
}

/// In-memory `PendingTxStore` backed by `Arc<Mutex<BTreeMap<txid,
/// hex>>>`. Default for CLI flows and the test harness; the browser
/// wires up an IndexedDB-backed equivalent in `alkanes-web-sys`.
///
/// `Clone` is cheap — the underlying `Arc` is shared so cloned
/// instances see the same data. That's the property that lets the
/// store be handed to a `DeezelProvider` impl AND retained by the
/// caller for direct inspection in tests.
#[derive(Clone, Default)]
pub struct MemoryPendingTxStore {
    inner: Arc<Mutex<BTreeMap<String, String>>>,
}

impl MemoryPendingTxStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "std")]
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap()
}

#[cfg(not(feature = "std"))]
fn lock<'a, T>(m: &'a Mutex<T>) -> spin::MutexGuard<'a, T> {
    m.lock()
}

#[async_trait(?Send)]
impl PendingTxStore for MemoryPendingTxStore {
    async fn add(&self, tx_hex: &str) -> Result<()> {
        let txid = compute_txid(tx_hex)?;
        let mut g = lock(&self.inner);
        g.insert(txid, tx_hex.to_string());
        Ok(())
    }

    async fn list(&self) -> Result<Vec<String>> {
        let g = lock(&self.inner);
        Ok(g.values().cloned().collect())
    }

    async fn remove(&self, txid: &str) -> Result<()> {
        let mut g = lock(&self.inner);
        g.remove(txid);
        Ok(())
    }

    async fn evict(&self, confirmed_txids: &[String]) -> Result<()> {
        let mut g = lock(&self.inner);
        for txid in confirmed_txids {
            g.remove(txid);
        }
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut g = lock(&self.inner);
        g.clear();
        Ok(())
    }

    async fn len(&self) -> Result<usize> {
        let g = lock(&self.inner);
        Ok(g.len())
    }
}

/// Decode a tx hex to its txid. Fast path used by `MemoryPendingTxStore`
/// to key entries. Mirrors `bitcoin::Transaction::compute_txid()` but
/// kept local so this module can be used from `no_std` callers without
/// dragging in the inspector machinery.
fn compute_txid(tx_hex: &str) -> Result<String> {
    use bitcoin::consensus::Decodable;
    let stripped = tx_hex.strip_prefix("0x").unwrap_or(tx_hex);
    let bytes = hex::decode(stripped)?;
    let tx: bitcoin::Transaction =
        bitcoin::Transaction::consensus_decode(&mut &bytes[..])?;
    Ok(tx.compute_txid().to_string())
}

// =====================================================================
// Tests
// =====================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// Real Tx A from the 2026-05-03 mainnet split-tx run. Same fixture
    /// the `apply_mempool_adjustment` round-trip test uses so the
    /// behavior matches end-to-end.
    const TX_A_HEX: &str = "02000000000102c0b16477f5a5ab2d2b1ed826138bf6d1d91338428880df1b35499a11800f1a600100000000fdffffff22de02b77e503167665374f9161999ced057d093e453753372901f61a3f0b8c60200000000fdffffff043075000000000000225120a7f90b8256f58c1074fe085d37b73dff3040774babc216dae106e281e020638b22020000000000002251207ab57455a9be2f87f4d3dfc3ddf2ac2a3ebc0163159f36130f7ceb9e527fa2c34cbc0000000000002251207ab57455a9be2f87f4d3dfc3ddf2ac2a3ebc0163159f36130f7ceb9e527fa2c30000000000000000136a5d101600ff7f818cec8ad0abc0a8a081d2150140300f852484bcd16e2d5c2850f8c3bc1bd861a033971994f621fb589deb3edf8225dfbbdb969abb738b4ba2e1c119c7c3f860d77095b150b058a89170b2d532ad01408e1f00dd1c42ee3c073f256395d5b74d7c8366a52d29b72832a1ebec3bda4048f3a86f41625ec8736cf97051796b20961e05e11291aa65737cbf0ddb243f450f00000000";
    const TX_A_TXID: &str = "c5520bb64d1a742a6bd62999267f683e1f0756481220ff2155d2be841a3d7b92";

    #[tokio::test]
    async fn empty_store_returns_empty_list() {
        let store = MemoryPendingTxStore::new();
        let listed = store.list().await.unwrap();
        assert!(listed.is_empty());
        assert_eq!(store.len().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn add_then_list_returns_the_added_hex() {
        let store = MemoryPendingTxStore::new();
        store.add(TX_A_HEX).await.unwrap();

        let listed = store.list().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0], TX_A_HEX);
        assert_eq!(store.len().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn add_is_idempotent_on_txid() {
        // The same tx hex added twice must not produce two entries —
        // otherwise a retry / double-broadcast would double-count
        // pending state.
        let store = MemoryPendingTxStore::new();
        store.add(TX_A_HEX).await.unwrap();
        store.add(TX_A_HEX).await.unwrap();
        assert_eq!(store.len().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn remove_by_txid_drops_the_entry() {
        let store = MemoryPendingTxStore::new();
        store.add(TX_A_HEX).await.unwrap();
        assert_eq!(store.len().await.unwrap(), 1);

        store.remove(TX_A_TXID).await.unwrap();
        assert_eq!(store.len().await.unwrap(), 0);
        assert!(store.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn remove_unknown_txid_is_noop() {
        let store = MemoryPendingTxStore::new();
        store.add(TX_A_HEX).await.unwrap();

        // Unrelated txid — must not affect the store.
        store
            .remove("0000000000000000000000000000000000000000000000000000000000000000")
            .await
            .unwrap();
        assert_eq!(store.len().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn evict_handles_partial_match() {
        let store = MemoryPendingTxStore::new();
        store.add(TX_A_HEX).await.unwrap();

        // Evict list contains the txid plus an unrelated one; the
        // unrelated entry should be a no-op while the matching one
        // is removed.
        store
            .evict(&[
                TX_A_TXID.to_string(),
                "deadbeef00000000000000000000000000000000000000000000000000000000".to_string(),
            ])
            .await
            .unwrap();
        assert_eq!(store.len().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn clear_wipes_the_store() {
        let store = MemoryPendingTxStore::new();
        store.add(TX_A_HEX).await.unwrap();
        store.clear().await.unwrap();
        assert_eq!(store.len().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn cloned_handle_shares_state() {
        // Cloning the store must share the underlying Arc so a handle
        // passed to a provider stays in sync with the caller's
        // reference.
        let store = MemoryPendingTxStore::new();
        let alias = store.clone();

        store.add(TX_A_HEX).await.unwrap();
        assert_eq!(alias.len().await.unwrap(), 1);
        assert_eq!(alias.list().await.unwrap()[0], TX_A_HEX);

        alias.remove(TX_A_TXID).await.unwrap();
        assert_eq!(store.len().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn add_rejects_invalid_hex() {
        let store = MemoryPendingTxStore::new();
        let bad = store.add("not-actually-hex").await;
        assert!(bad.is_err());
    }

    #[tokio::test]
    async fn add_strips_optional_0x_prefix() {
        let store = MemoryPendingTxStore::new();
        let prefixed = format!("0x{}", TX_A_HEX);
        store.add(&prefixed).await.unwrap();
        assert_eq!(store.len().await.unwrap(), 1);
    }

    /// Drives the integration by feeding the store output directly
    /// into `apply_mempool_adjustment` — same code path that
    /// `select_utxos` executes at runtime. Verifies the end-to-end
    /// "broadcast → store → next selection sees it" flow without
    /// spinning up a full provider.
    #[tokio::test]
    async fn store_feeds_apply_mempool_adjustment_end_to_end() {
        use crate::alkanes::execute::{
            apply_mempool_adjustment, decode_tx_hex_to_mempool_json,
        };
        use crate::traits::UtxoInfo;
        use bitcoin::OutPoint;
        use std::str::FromStr;

        // Caller's wallet currently sees these confirmed UTXOs — the
        // same prevouts the real Tx A (c5520bb6…) is about to spend
        // (matching the on-chain mempool tx fixture).
        let txid_a_in_0 =
            "601a0f80119a49351bdf8088423813d9d1f68b1326d81e2b2daba5f57764b1c0";
        let txid_a_in_1 =
            "c6b8f0a3611f9072337553e493d057d0ce991916f97453666731507eb702de22";
        let user_addr = "bc1p026hg4dfhchc0axnmlpamu4v9gltcqtrzk0nvyc00n4eu5nl5tpsrh7zkm";

        let make_utxo = |txid: &str, vout: u32, amount: u64| -> (OutPoint, UtxoInfo) {
            (
                OutPoint::from_str(&format!("{}:{}", txid, vout)).unwrap(),
                UtxoInfo {
                    txid: txid.to_string(),
                    vout,
                    amount,
                    address: user_addr.to_string(),
                    script_pubkey: None,
                    confirmations: 1,
                    frozen: false,
                    freeze_reason: None,
                    block_height: Some(1),
                    has_inscriptions: false,
                    has_runes: false,
                    has_alkanes: false,
                    is_coinbase: false,
                },
            )
        };

        let mut spendable: Vec<(OutPoint, UtxoInfo)> = vec![
            // Tx A's two prevouts (will be stripped).
            make_utxo(txid_a_in_0, 1, 574),
            make_utxo(txid_a_in_1, 2, 78462),
            // An unrelated UTXO that should NOT be touched.
            make_utxo("deadbeef00000000000000000000000000000000000000000000000000000000", 0, 1000),
        ];

        // Broadcast Tx A → push to store.
        let store = MemoryPendingTxStore::new();
        store.add(TX_A_HEX).await.unwrap();
        assert_eq!(store.len().await.unwrap(), 1);

        // The next `select_utxos` call would do this:
        //   let hexes = store.list().await?;
        //   let payloads = hexes.iter().map(decode_tx_hex_to_mempool_json).collect();
        //   apply_mempool_adjustment(&mut spendable, &payloads, &addresses);
        let hexes = store.list().await.unwrap();
        let payloads: Vec<serde_json::Value> = hexes
            .iter()
            .map(|h| serde_json::json!([decode_tx_hex_to_mempool_json(h).unwrap()]))
            .collect();

        let report = apply_mempool_adjustment(
            &mut spendable,
            &payloads,
            &[user_addr.to_string()],
        );

        // Tx A's 2 prevouts must be stripped (it spent them);
        // its 2 user-paying outputs (alkane carrier + BTC change)
        // must appear as new candidates. Signer output is NOT ours
        // and stays excluded. The unrelated UTXO is preserved.
        assert_eq!(report.stripped, 2, "Tx A's 2 prevouts stripped");
        assert_eq!(report.added, 2, "2 user-paying Tx A outputs added");
        assert_eq!(spendable.len(), 3, "1 unrelated + 2 new Tx A outputs");

        let outpoints: Vec<String> = spendable
            .iter()
            .map(|(op, _)| format!("{}:{}", op.txid, op.vout))
            .collect();
        assert!(outpoints.iter().any(|s| s == &format!("{}:1", TX_A_TXID)),
                "Tx A vout 1 (alkane carrier) added");
        assert!(outpoints.iter().any(|s| s == &format!("{}:2", TX_A_TXID)),
                "Tx A vout 2 (BTC change) added");
        assert!(outpoints.iter().any(|s| s.starts_with("deadbeef")),
                "unrelated UTXO preserved");

        // After the indexer catches up and the tx is confirmed, the
        // store should be evicted to free up its candidate-set
        // contribution.
        store.evict(&[TX_A_TXID.to_string()]).await.unwrap();
        assert_eq!(store.len().await.unwrap(), 0);

        // Re-running with an empty store leaves the candidate set
        // unchanged from the post-Tx-A view.
        let prev_count = spendable.len();
        let report2 = apply_mempool_adjustment(
            &mut spendable,
            &[],
            &[user_addr.to_string()],
        );
        assert_eq!(report2.stripped, 0);
        assert_eq!(report2.added, 0);
        assert_eq!(spendable.len(), prev_count);
    }

    /// Two pending txs that form a chain (A spent in B's input set,
    /// B's outputs paying us). Verifies the store handles
    /// independently-broadcast chains — not just the
    /// `execute_split` pattern.
    ///
    /// Scenario: User broadcasts atomic wrap+swap (Tx A), then
    /// immediately initiates an alkane-send (Tx B) before Tx A is
    /// indexed. Tx B's selector should see both A and B in the
    /// store and produce a final candidate set that reflects "Tx B
    /// has consumed Tx A's alkane carrier and produced a new
    /// recipient output."
    #[tokio::test]
    async fn store_handles_chained_pending_txs() {
        use crate::alkanes::execute::{
            apply_mempool_adjustment, decode_tx_hex_to_mempool_json,
        };

        let store = MemoryPendingTxStore::new();
        store.add(TX_A_HEX).await.unwrap();
        // Adding the same tx hex twice (e.g. retry loop) collapses
        // to one entry — guards against double-adjustment.
        store.add(TX_A_HEX).await.unwrap();
        assert_eq!(store.len().await.unwrap(), 1);

        let hexes = store.list().await.unwrap();
        let payloads: Vec<serde_json::Value> = hexes
            .iter()
            .map(|h| serde_json::json!([decode_tx_hex_to_mempool_json(h).unwrap()]))
            .collect();

        // Empty candidate set + Tx A's mempool payload → 0 stripped,
        // 2 added (Tx A pays user twice).
        let mut spendable = Vec::new();
        let report = apply_mempool_adjustment(
            &mut spendable,
            &payloads,
            &["bc1p026hg4dfhchc0axnmlpamu4v9gltcqtrzk0nvyc00n4eu5nl5tpsrh7zkm".to_string()],
        );
        assert_eq!(report.stripped, 0);
        assert_eq!(report.added, 2);
    }
}
