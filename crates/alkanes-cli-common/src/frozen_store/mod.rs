//! Persistent record of outpoints the operator has taken off the table.
//!
//! Background — the gap this closes:
//!
//!   `UtxoInfo` has carried a `frozen: bool` and a `freeze_reason` since the
//!   beginning, and four separate selectors already honour it:
//!   `check_utxo_eligibility` (→ `UtxoSkipReason::Frozen`), `select_coins`,
//!   `transaction.rs`'s largest-first selector, and the brc20-prog selector.
//!   Nothing ever set it. `get_utxos` hard-coded `frozen: false` at both
//!   construction sites, and `freeze_utxo`/`unfreeze_utxo` were
//!   `unimplemented!()` panics behind a CLI handler that printed
//!   "❄️  UTXO {utxo} frozen successfully" — so the one path that told the
//!   user their coin was protected was a panic on a good day and a lie on a
//!   bad one.
//!
//! Freezing is a **safety control**, not a cache. That single fact drives
//! every design decision here:
//!
//!   * **It is durable and it has its own file.** The store does NOT live
//!     under `ALKANES_CACHE_DIR`. A user who clears a cache to reclaim disk
//!     must not thereby unfreeze the UTXO holding their inscription. See
//!     [`sqlite::default_frozen_db_path`].
//!   * **It fails closed.** Callers must not paper over an unopenable store
//!     by substituting an empty in-memory one — that reads as "nothing is
//!     frozen", which is the most dangerous possible wrong answer. The
//!     cache does exactly that fallback, correctly, because a cache miss is
//!     harmless. A freeze miss spends someone's ordinal.
//!   * **A frozen UTXO is still reported.** `get_utxos` returns it with
//!     `frozen: true` rather than filtering it out, so the coin never simply
//!     vanishes from the user's balance with no explanation. Selection skips
//!     it; display shows it with its reason.
//!
//! Implementations, one per platform, all behind [`FrozenStore`]:
//!
//!   * [`memory::MemoryFrozenStore`] — `Arc<Mutex<BTreeMap<..>>>`. The
//!     harness driver: exercises every behaviour with no side effects on the
//!     developer's own `~/.alkanes` state.
//!   * [`sqlite::SqliteFrozenStore`] — the native CLI, at
//!     `~/.alkanes/frozen.sqlite3` (feature `frozen-sqlite`).
//!   * `IndexedDbFrozenStore` — the browser, in `alkanes-web-sys`, so a
//!     freeze survives a page reload.
//!
//! Keys are canonical `txid:vout` strings. Every entry point normalizes
//! through [`normalize_outpoint`], which parses the txid as a real
//! `bitcoin::Txid` rather than trusting the string: a mistyped outpoint that
//! silently froze nothing would leave the user believing a coin was
//! protected when it was not.

#[cfg(feature = "std")]
use std::collections::BTreeMap;
#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap;

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec, format};

use async_trait::async_trait;
use anyhow::{anyhow, Result};

pub mod memory;

#[cfg(all(feature = "std", feature = "frozen-sqlite"))]
pub mod sqlite;

pub use memory::MemoryFrozenStore;

/// One frozen outpoint, as recorded.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FrozenRecord {
    /// Canonical `txid:vout`.
    pub outpoint: String,
    /// Operator-supplied note. `None` when frozen without one.
    pub reason: Option<String>,
    /// Unix seconds at which the freeze was recorded. `0` when the platform
    /// has no clock (no_std) — treated as "unknown", never as 1970.
    pub frozen_at: u64,
}

/// A durable set of outpoints excluded from coin selection.
///
/// All methods take `&self` so the store can live behind an `Arc` and be
/// shared across the provider, the selectors and async tasks; interior
/// mutation is each implementation's concern. This mirrors
/// [`crate::pending_tx_store::PendingTxStore`], the sibling store that
/// established the pattern.
#[async_trait(?Send)]
pub trait FrozenStore {
    /// Record `outpoint` as frozen, with an optional reason. Idempotent:
    /// freezing an already-frozen outpoint overwrites the reason and leaves
    /// one entry, rather than erroring — re-freezing with a better note is a
    /// reasonable thing to do and must not fail.
    async fn freeze(&self, outpoint: &str, reason: Option<&str>) -> Result<()>;

    /// Remove `outpoint` from the frozen set. Returns whether it was
    /// actually present, so the CLI can say "wasn't frozen" instead of
    /// reporting success for a no-op — the failure mode this whole module
    /// exists to end.
    async fn unfreeze(&self, outpoint: &str) -> Result<bool>;

    /// The record for one outpoint, or `None` if it is not frozen.
    async fn get(&self, outpoint: &str) -> Result<Option<FrozenRecord>>;

    /// Every frozen record. `get_utxos` calls this ONCE per listing and
    /// builds a lookup from it, rather than calling [`Self::get`] per UTXO —
    /// a wallet with hundreds of UTXOs would otherwise issue hundreds of
    /// queries to answer a question the whole table can answer at once.
    async fn list(&self) -> Result<Vec<FrozenRecord>>;

    /// Drop every entry. Test/maintenance only, on the trait so callers
    /// don't have to downcast.
    async fn clear(&self) -> Result<()>;
}

/// Canonicalize an `outpoint` string of the form `txid:vout`.
///
/// The txid is parsed as a real [`bitcoin::Txid`] and re-rendered, so the
/// stored key is always lowercase, exactly 64 hex chars, and round-trips
/// against what `get_utxos` builds from an `OutPoint`. Garbage in — a
/// truncated txid, a non-numeric vout, a missing colon — is an error rather
/// than a key that would quietly never match anything.
pub fn normalize_outpoint(outpoint: &str) -> Result<String> {
    let trimmed = outpoint.trim();
    let (txid_part, vout_part) = trimmed
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("invalid outpoint {trimmed:?}: expected txid:vout"))?;

    let txid: bitcoin::Txid = txid_part
        .parse()
        .map_err(|e| anyhow!("invalid txid in outpoint {trimmed:?}: {e}"))?;
    let vout: u32 = vout_part
        .parse()
        .map_err(|e| anyhow!("invalid vout in outpoint {trimmed:?}: {e}"))?;

    Ok(format!("{txid}:{vout}"))
}

/// Unix seconds, or `0` where no clock is available.
pub(crate) fn now_secs() -> u64 {
    #[cfg(feature = "std")]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    #[cfg(not(feature = "std"))]
    {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TXID: &str = "c5520bb64d1a742a6bd62999267f683e1f0756481220ff2155d2be841a3d7b92";

    #[test]
    fn normalizes_a_well_formed_outpoint_unchanged() {
        let s = format!("{TXID}:1");
        assert_eq!(normalize_outpoint(&s).unwrap(), s);
    }

    #[test]
    fn uppercase_txid_normalizes_to_lowercase() {
        // The same coin typed two ways must produce the same key, or a
        // freeze recorded from a copy-pasted explorer string would never
        // match the UTXO the wallet builds from an OutPoint.
        let upper = format!("{}:0", TXID.to_uppercase());
        assert_eq!(normalize_outpoint(&upper).unwrap(), format!("{TXID}:0"));
    }

    #[test]
    fn surrounding_whitespace_is_tolerated() {
        let padded = format!("  {TXID}:2  ");
        assert_eq!(normalize_outpoint(&padded).unwrap(), format!("{TXID}:2"));
    }

    #[test]
    fn a_missing_vout_is_an_error_not_a_silent_key() {
        assert!(normalize_outpoint(TXID).is_err());
    }

    #[test]
    fn a_truncated_txid_is_an_error() {
        // Silently accepting this would record a freeze that can never match
        // a real UTXO — the user would be told their coin is protected when
        // it is not.
        assert!(normalize_outpoint("deadbeef:0").is_err());
    }

    #[test]
    fn a_non_numeric_vout_is_an_error() {
        assert!(normalize_outpoint(&format!("{TXID}:first")).is_err());
    }

    #[test]
    fn a_negative_vout_is_an_error() {
        assert!(normalize_outpoint(&format!("{TXID}:-1")).is_err());
    }
}
