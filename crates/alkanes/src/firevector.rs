//! FIREVECTOR weight table — the indexer side of the emission VM.
//!
//! # Why this lives behind the `800000000:2` precompile
//!
//! `handle_extcall` short-circuits precompiles *before* fuel is computed:
//!
//! ```text
//!   if cellpack.target.block == 800000000 { return _handle_special_extcall(..) }  // :762
//!   ...
//!   let total_fuel = compute_extcall_fuel(storage_map_len, height)?;              // :815
//! ```
//!
//! so a precompile call never reaches `compute_extcall_fuel` and never pays
//! `FUEL_EXTCALL`; the callee side sets `fuel_used = 0` besides. The boundary is
//! free in both directions, which means arbitrary work can happen in here at
//! zero consensus fuel.
//!
//! That is what lets FIREVECTOR activate without touching the DIESEL contract at
//! all. The genesis alkane keeps computing
//!
//! ```text
//!   value_per_mint = (block_reward - diesel_fee) / total_mints
//! ```
//!
//! byte for byte. All we change is what `total_mints` *means*: below the fork it
//! is the mint count, at and above it is an effective denominator `W / w_i`
//! derived from the governance weightmap. With the identity vector every weight
//! is 1, so `W / w_i == N` and the returned value is bit-identical to today.
//!
//! Consequences worth stating plainly, because they are the whole point:
//!
//! - the contract wasm is unchanged, so its metered instruction count is
//!   unchanged, so `refuel_block` hands later transactions in the block exactly
//!   the same fuel they get today;
//! - the response stays **16 bytes**, so `returndatacopy` (2 fuel/byte,
//!   `host_functions.rs:280`) charges exactly what it charges today. Widening the
//!   reply would silently reintroduce the fuel divergence this design exists to
//!   avoid.
//!
//! # Totality
//!
//! Every failure path falls back to today's mint count. A missing weightmap, a
//! malformed one, an invalid program, a block where nothing qualifies — all
//! degrade to the communist mint rather than to an error. At mainnet h=953281 a
//! single malformed protostone made this precompile's ancestor `?` out, so it
//! never returned a count, so every DIESEL mint in the block spun on a failed
//! extcall until per-tx fuel ran out and an indexer wedged at tip 953280. Nothing
//! in here may return `Err`.

use crate::network::genesis;
use alkanes_std_firevector::vm::{eval, validate, Item, Source};
use alkanes_support::id::AlkaneId;
use bitcoin::{Block, BlockHash, Txid};
use metashrew_support::index_pointer::KeyValuePointer;
use ordinals::{Artifact, Runestone};
use protorune_support::protostone::Protostone;
use protorune_support::utils::decode_varint_list;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, LazyLock, RwLock};

use alkanes_support::cellpack::Cellpack;

/// DIESEL.
pub const DIESEL_ID: AlkaneId = AlkaneId { block: 2, tx: 0 };
/// The FIREVECTOR alkane, which owns the weightmap.
///
/// The weightmap lives here rather than in DIESEL's storage on purpose: it means
/// DIESEL never needs a governance setter, so the DIESEL contract is never
/// modified, so there is never a second fuel-affecting height gate. The whole
/// system reaches steady state with the deployed genesis alkane untouched.
pub const FIREVECTOR_ID: AlkaneId = AlkaneId { block: 12, tx: 0 };
/// DIESEL's mint opcode.
pub const DIESEL_MINT_OPCODE: u128 = 77;

/// Contract-storage key holding the active weightmap, as written by
/// `set_weightmap` on 12:0. Full index key is
/// `/alkanes/<12:0>/storage//weightmap`.
pub const WEIGHTMAP_KEY: &[u8] = b"/weightmap";

/// Storage key holding the weightmap's [`Source`] discriminant.
pub const WEIGHTMAP_SOURCE_KEY: &[u8] = b"/weightmap_source";

/// Returned to a caller that earns nothing. The contract computes
/// `emission / denominator`, so `u128::MAX` pays out zero without the contract
/// needing to know that a zero-weight case exists.
pub const DENOMINATOR_NO_CLAIM: u128 = u128::MAX;

/// Per-block memo, **keyed by block hash**.
///
/// Two things this gets right that the older cache did not.
///
/// It memoizes the *table*, not the reply bytes. Caching reply bytes was correct
/// while the answer was block-global and becomes a serious bug the moment the
/// answer is per-transaction: the first mint in a block would populate it and
/// every later mint would be handed the first one's denominator.
///
/// And it is keyed by block hash rather than trusting the caller to have cleared
/// it. `index_block` does clear it, but nothing structural stops a stale table
/// being served to a different block — a reorg, `simulate_block`, or a view-path
/// re-entry would all do it, and the symptom would be silently wrong emission
/// rather than a crash. Keying makes staleness impossible instead of merely
/// unlikely.
struct CachedWeights {
    block_hash: BlockHash,
    weights: BlockWeights,
}

static WEIGHTS_CACHE: LazyLock<Arc<RwLock<Option<CachedWeights>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(None)));

/// Drop the memo. Called once per block from `index_block`, next to
/// `clear_diesel_mints_cache`.
pub fn clear_weights_cache() {
    if let Ok(mut cache) = WEIGHTS_CACHE.try_write() {
        *cache = None;
    }
}

/// Weights for one block, keyed by the transaction that may claim them.
#[derive(Clone, Debug, Default)]
pub struct BlockWeights {
    /// Weight per claiming transaction. Only transactions carrying a DIESEL mint
    /// appear — a transaction that cannot claim must not contribute to `total`,
    /// or its share of the emission would simply be burned.
    pub per_tx: HashMap<Txid, u128>,
    /// Sum of `per_tx`, saturating.
    pub total: u128,
    /// Today's mint count, always computed, used as the fallback denominator.
    pub legacy_count: u128,
}

/// One tag-1 protostone, decoded far enough to weigh.
struct ScannedItem {
    item: Item,
    is_diesel_mint: bool,
}

/// Decode every tag-1 protostone of a transaction into weighable items.
///
/// The skip rules replicate `_get_number_diesel_mints` exactly, because any
/// divergence in what counts as a mint changes `N` and therefore every payout in
/// the block:
///
/// - `Runestone::decipher` yields no runestone → no items
/// - `Protostone::from_runestone` errors → no items (the 04c9baa5 fix for the
///   h=953281 wedge; a malformed protostone is not a mint)
/// - `protocol_tag != 1` → skip that protostone
/// - empty calldata → skip
/// - `decode_varint_list` errors → skip
/// - fewer than 2 varints → skip
/// - `TryInto<Cellpack>` fails → skip
fn scan_transaction(tx: &bitcoin::Transaction, txindex: u32, height: u64) -> Vec<ScannedItem> {
    let mut out = Vec::new();

    let runestone = match Runestone::decipher(tx) {
        Some(Artifact::Runestone(r)) => r,
        _ => return out,
    };
    let protostones = match Protostone::from_runestone(&runestone) {
        Ok(p) => p,
        Err(_) => return out,
    };

    for (pstone_index, protostone) in protostones.into_iter().enumerate() {
        if protostone.protocol_tag != 1 {
            continue;
        }
        let calldata: Vec<u8> = protostone
            .message
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        if calldata.is_empty() {
            continue;
        }
        let varint_list = match decode_varint_list(&mut Cursor::new(calldata)) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if varint_list.len() < 2 {
            continue;
        }
        let cellpack: Cellpack = match varint_list.try_into() {
            Ok(c) => c,
            Err(_) => continue,
        };

        let opcode = cellpack.inputs.first().copied().unwrap_or(0);
        let is_diesel_mint = cellpack.target == DIESEL_ID
            && !cellpack.inputs.is_empty()
            && opcode == DIESEL_MINT_OPCODE;

        out.push(ScannedItem {
            item: Item {
                target_block: cellpack.target.block,
                target_tx: cellpack.target.tx,
                opcode,
                inputs: cellpack.inputs.iter().skip(1).copied().collect(),
                // Pre-execution scan: success and outgoing amounts are not
                // knowable, which is why a SameBlock weightmap may not read them.
                status: 0,
                incoming: Vec::new(),
                outgoing: Vec::new(),
                txindex: txindex as u128,
                pstone_index: pstone_index as u128,
                height: height as u128,
                // Filled in by the caller once the block's mint order is known;
                // u128::MAX until then, i.e. "not a mint".
                mint_rank: u128::MAX,
            },
            is_diesel_mint,
        });
    }

    out
}

/// Today's mint count: transactions carrying at least one DIESEL mint protostone.
///
/// Counted per **transaction**, not per protostone — `_get_number_diesel_mints`
/// breaks out of the protostone loop on its first match
/// (`host_functions.rs:700`), so a transaction with two mint protostones counts
/// once.
pub fn legacy_mint_count(block: &Block) -> u128 {
    let mut counter: u128 = 0;
    for (txindex, tx) in block.txdata.iter().enumerate() {
        if scan_transaction(tx, txindex as u32, 0)
            .iter()
            .any(|s| s.is_diesel_mint)
        {
            counter += 1;
        }
    }
    counter
}

/// Read the active weightmap out of DIESEL's contract storage.
///
/// Stored as little-endian `u128` words, so a length that is not a multiple of
/// 16 is malformed and yields nothing — which falls back to today's behaviour
/// rather than to a partially-decoded program.
fn load_weightmap<T: KeyValuePointer>(atomic: &T) -> Option<(Vec<u128>, Source)> {
    let base = atomic
        .keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(FIREVECTOR_ID))
        .keyword("/storage/");

    let raw = base.select(&WEIGHTMAP_KEY.to_vec()).get();
    if raw.is_empty() || raw.len() % 16 != 0 {
        return None;
    }
    let program: Vec<u128> = raw
        .chunks_exact(16)
        .map(|c| u128::from_le_bytes(c.try_into().expect("chunks_exact(16)")))
        .collect();

    let src_raw = base.select(&WEIGHTMAP_SOURCE_KEY.to_vec()).get();
    let source = match src_raw.first().copied() {
        Some(1) => Source::PrevBlock,
        // Default and explicit 0 both mean same-block, which is what the identity
        // vector needs: today's count is a parse-only pre-pass.
        _ => Source::SameBlock,
    };

    Some((program, source))
}

/// Compute the block's weight table.
///
/// A transaction's weight is the **maximum** weight across its qualifying
/// protostones, not the sum. Three reasons, in order of importance:
///
/// 1. It preserves equivalence exactly. Under the identity vector every mint
///    protostone weighs 1, and `max(1, 1) == 1`, so a transaction carrying two
///    mint protostones still weighs 1 — matching the `break` in today's counter.
///    Summing would let such a transaction weigh 2 and dilute every honest
///    minter in the block.
/// 2. It is order-independent, so the result cannot depend on protostone
///    ordering within a transaction.
/// 3. It stops anyone splitting one action across many protostones to inflate a
///    weight cheaply.
pub(crate) fn compute_block_weights<T: KeyValuePointer>(block: &Block, height: u64, atomic: &T) -> BlockWeights {
    let legacy_count = legacy_mint_count(block);

    let mut weights = BlockWeights {
        per_tx: HashMap::new(),
        total: 0,
        legacy_count,
    };

    let Some((program, source)) = load_weightmap(atomic) else {
        return weights;
    };
    let Ok(steps) = validate(&program, source) else {
        return weights;
    };
    let budget = steps.max(1);

    // Rank among mint-bearing transactions, assigned in transaction order. This
    // is the one fact a program can read that is not local to its own item, and
    // it exists because "am I the first mint in this block" is otherwise
    // inexpressible — which is what the pre-upgrade winner-takes-all rule needs.
    let mut mint_rank: u128 = 0;

    for (txindex, tx) in block.txdata.iter().enumerate() {
        let mut scanned = scan_transaction(tx, txindex as u32, height);
        if !scanned.iter().any(|s| s.is_diesel_mint) {
            // Cannot claim, so must not contribute to the denominator.
            continue;
        }
        for s in scanned.iter_mut() {
            s.item.mint_rank = mint_rank;
        }
        mint_rank += 1;

        let w = scanned
            .iter()
            .map(|s| eval(&program, &s.item, budget))
            .max()
            .unwrap_or(0);
        if w == 0 {
            continue;
        }
        weights.total = weights.total.saturating_add(w);
        weights.per_tx.insert(tx.compute_txid(), w);
    }

    weights
}

/// Get (or compute and memo) this block's weight table.
///
/// The memo only answers for the block it was computed from; anything else
/// recomputes rather than returning a plausible-looking wrong table.
fn block_weights<T: KeyValuePointer>(block: &Block, height: u64, atomic: &T) -> BlockWeights {
    let block_hash = block.block_hash();

    if let Ok(guard) = WEIGHTS_CACHE.read() {
        if let Some(cached) = guard.as_ref() {
            if cached.block_hash == block_hash {
                return cached.weights.clone();
            }
        }
    }
    let computed = compute_block_weights(block, height, atomic);
    if let Ok(mut guard) = WEIGHTS_CACHE.write() {
        *guard = Some(CachedWeights {
            block_hash,
            weights: computed.clone(),
        });
    }
    computed
}

/// Is FIREVECTOR weighting active at this height?
pub fn is_active(height: u64) -> bool {
    height >= genesis::FIREVECTOR_FORK_HEIGHT as u64
}

/// The value `800000000:2` returns: the denominator the DIESEL contract divides
/// the block's emission by.
///
/// Below the fork, and on every fallback path, this is today's mint count. At and
/// above the fork with a valid weightmap it is `total_weight / caller_weight`,
/// which for the identity vector is `N / 1 == N` — bit-identical to today.
///
/// Infallible. Every degenerate case resolves to the legacy count or to
/// [`DENOMINATOR_NO_CLAIM`]; none returns an error.
pub fn effective_denominator<T: KeyValuePointer>(
    block: &Block,
    height: u64,
    caller_txid: Txid,
    atomic: &T,
) -> u128 {
    if !is_active(height) {
        return legacy_mint_count(block);
    }

    let weights = block_weights(block, height, atomic);

    // No weightmap configured, or it weighed nothing in this block: emission
    // behaves exactly as it does today. This is the intended default — if nobody
    // does anything the weightmap rewards, the communist mint is what happens.
    if weights.total == 0 {
        return weights.legacy_count;
    }

    match weights.per_tx.get(&caller_txid).copied() {
        // The caller qualified: W / w_i.
        Some(w) if w > 0 => (weights.total / w).max(1),
        // The caller carries a mint but earned no weight under the active
        // vector. It claims nothing.
        _ => DENOMINATOR_NO_CLAIM,
    }
}
