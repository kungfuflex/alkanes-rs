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
use alkanes_std_firevector::abi::{BPS_DENOM, MAX_TREASURY_BPS, unpack_weightmap, Weightmap};
use alkanes_std_firevector::vm::{eval, validate, Item, Mode, Qualifier};
use alkanes_support::id::AlkaneId;
use bitcoin::{Block, BlockHash, Txid};
use metashrew_core::index_pointer::IndexPointer;
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
/// DSIGIL — the token that may claim the 12:1 orbital.
///
/// Already deployed on chain, and separately holds the DIESEL treasury
/// permission. It is spent into `claim_vector_orbital` once and handed straight
/// back, so claiming does not cost it that role.
///
/// `0:0` would mean "no claim authority", leaving the orbital locked inside 12:0
/// forever — the fail-closed default this replaced. A wrong id here is the one
/// way to lose the emission-policy capability permanently, so it is asserted
/// rather than assumed.
pub const DSIGIL_ID: AlkaneId = AlkaneId {
    block: 2,
    tx: 69805,
};

/// The FIREVECTOR orbital at 12:1 — possession authorises `set_weightmap`.
pub const FV_ORBITAL_ID: AlkaneId = AlkaneId { block: 12, tx: 1 };

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

/// Per-block cumulative claimed weight under [`Mode::Rate`], keyed by height.
/// Clamping this at the weightmap's `rate_floor` is what bounds a block's total
/// payout; see [`rate_denominator`].
pub const RATE_SPENT_KEY: &[u8] = b"/rate_spent";

/// Per-transaction reservation under [`Mode::Rate`], keyed by height and txid.
/// Present so a second call for the same transaction — a view-path replay, a
/// trace re-render, a contract calling the precompile twice — returns the same
/// answer instead of consuming the budget again.
pub const RATE_CLAIM_KEY: &[u8] = b"/rate_claim";

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
    /// `Arc` rather than a bare value: `block_weights` is called once per mint,
    /// and the table now carries the whole block's scan index. Returning it by
    /// clone made every claim copy every mint item — turning the memo that fixed
    /// RATE's quadratic into a fresh quadratic for SPLIT (measured 0.074 ms/mint
    /// flat, regressed to 0.277 ms/mint and climbing at 400 mints).
    weights: Arc<BlockWeights>,
}

static WEIGHTS_CACHE: LazyLock<Arc<RwLock<Option<CachedWeights>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(None)));

/// The weightmap as it stood when the block started.
///
/// Without this, `load_weightmap` reads through the *currently executing
/// message's* atomic pointer — so a `set_weightmap` transaction at index 5 would
/// change emission for a block whose first mint is at index 200, but not for one
/// whose first mint is at index 3. The effective activation point of a governance
/// action would be "the first mint after the setter", which is an implicit,
/// unauditable boundary that a miner chooses by ordering the block.
///
/// Snapshotting once per block makes a weightmap set at height H take effect at
/// H+1, always. Left empty outside `index_block`, in which case the read falls
/// back to the message atomic — which is what unit tests want, since they write
/// the map and query it without running a block.
static WEIGHTMAP_SNAPSHOT: LazyLock<Arc<RwLock<Option<(BlockHash, Option<Weightmap>)>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(None)));

/// Drop the memo. Called once per block from `index_block`, next to
/// `clear_diesel_mints_cache`.
pub fn clear_weights_cache() {
    if let Ok(mut cache) = WEIGHTS_CACHE.try_write() {
        *cache = None;
    }
    if let Ok(mut snap) = WEIGHTMAP_SNAPSHOT.try_write() {
        *snap = None;
    }
}

/// Pin the active weightmap for the duration of this block.
///
/// Called from `index_block` after `setup_firevector`, so the deploy block sees
/// the seeded identity vector, and every block after that sees whatever
/// governance committed in a *previous* block.
pub fn snapshot_weightmap(block: &Block) {
    let wm = load_weightmap(&IndexPointer::default());
    if let Ok(mut snap) = WEIGHTMAP_SNAPSHOT.write() {
        *snap = Some((block.block_hash(), wm));
    }
}

/// The weightmap this block runs under.
///
/// Prefers the block-start snapshot; falls back to reading through the caller's
/// pointer when there is none for this block.
fn active_weightmap<T: KeyValuePointer>(block: &Block, atomic: &T) -> Option<Weightmap> {
    if let Ok(guard) = WEIGHTMAP_SNAPSHOT.read() {
        if let Some((hash, wm)) = guard.as_ref() {
            if *hash == block.block_hash() {
                return wm.clone();
            }
        }
    }
    load_weightmap(atomic)
}

/// Weights for one block, keyed by the transaction that may claim them.
/// One transaction's scan result, computed once per block.
///
/// RATE previously rebuilt all of this per claim — a `compute_txid()` sweep of
/// the block to find the caller, a full re-scan of its protostones, and a second
/// full sweep to count its mint rank. That is O(N) work inside an O(N) loop.
/// Measured 0.36 ms/mint at 50 mints rising to 2.23 ms/mint at 400 — quadratic,
/// extrapolating to ~90-140s for the 4-5k-mint blocks this chain actually sees,
/// which makes reindexing impossible.
#[derive(Clone, Debug)]
pub struct TxScan {
    pub txindex: u32,
    /// Rank among the block's mint-bearing transactions.
    pub mint_rank: u128,
    /// Qualified DIESEL-mint protostones only, by `pstone_index`.
    pub mint_items: Vec<(u128, Item)>,
}

#[derive(Clone, Debug, Default)]
pub struct BlockWeights {
    /// Every mint-bearing transaction in the block, keyed by txid. Built in the
    /// same single pass that computes the weights, so a RATE claim is a hash
    /// lookup rather than a rescan.
    pub by_txid: HashMap<Txid, TxScan>,
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
    /// Whether this protostone's header is well formed — `pointer` and `refund`
    /// both present and within `num_outputs + num_protostones`.
    ///
    /// Load-bearing, and the reason is not obvious. A malformed header is
    /// refunded and skipped by `process_message` *before* the message runs
    /// (`protorune/src/protostone.rs:107-125`), so it never reaches
    /// `handle_message` and never calls `FuelTank::drain_fuel`. It therefore
    /// costs nothing, executes nothing, and leaves the transaction's fuel intact
    /// for the mint that follows — while still parsing as a perfectly good
    /// cellpack here.
    ///
    /// Without this flag the qualifier could be satisfied by an action that
    /// provably never happened, for the price of one out-of-range integer in the
    /// OP_RETURN. It gates qualifier matching ONLY: `is_diesel_mint` and
    /// [`legacy_mint_count`] deliberately ignore it, because today's counter does
    /// too and changing that would move every historical payout.
    header_ok: bool,
    /// Whether this mint is preceded by an action matching the weightmap's
    /// qualifier. Always true when the weightmap sets no qualifier.
    ///
    /// The qualifier has to gate the item rather than merely supply
    /// `prior_inputs`, because a program is under no obligation to read them — a
    /// flat membership program like `PUSH 1` would otherwise pay every mint in
    /// the block whether or not it did the thing being subsidised.
    qualified: bool,
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
/// Counts `scan_transaction` calls so a test can assert the block is scanned
/// once per transaction rather than once per transaction per claim. The
/// quadratic this guards against is invisible to a correctness test — every
/// answer stays right, it just gets slower until reindexing is impossible.
#[cfg(test)]
pub static SCAN_CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(test)]
pub fn reset_scan_calls() {
    SCAN_CALLS.store(0, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
pub fn scan_calls() -> usize {
    SCAN_CALLS.load(std::sync::atomic::Ordering::SeqCst)
}

fn scan_transaction(tx: &bitcoin::Transaction, txindex: u32, height: u64) -> Vec<ScannedItem> {
    #[cfg(test)]
    SCAN_CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let mut out = Vec::new();

    let runestone = match Runestone::decipher(tx) {
        Some(Artifact::Runestone(r)) => r,
        _ => return out,
    };
    let protostones = match Protostone::from_runestone(&runestone) {
        Ok(p) => p,
        Err(_) => return out,
    };

    // Same bound `process_message` checks. Computed over ALL protostones, not
    // just tag-1 ones, because that is what protorune does.
    let bound = (tx.output.len() + protostones.len()) as u32;

    for (pstone_index, protostone) in protostones.into_iter().enumerate() {
        if protostone.protocol_tag != 1 {
            continue;
        }
        let header_ok = match (protostone.pointer, protostone.refund) {
            (Some(p), Some(r)) => p <= bound && r <= bound,
            _ => false,
        };
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
                // Filled in by `resolve_qualifier` once the whole protostone list
                // is known, since matching looks backwards from each mint.
                prior_inputs: Vec::new(),
                // Realized amounts exist only under `Mode::Rate`, where the item
                // is built while the claim executes. The pre-execution scan that
                // feeds `Mode::Split` cannot know them, which is why
                // `OP_INCOMING_AMOUNT` is a validation error in that mode.
                incoming: Vec::new(),
                txindex: txindex as u128,
                pstone_index: pstone_index as u128,
                height: height as u128,
                // Filled in by the caller once the block's mint order is known;
                // u128::MAX until then, i.e. "not a mint".
                mint_rank: u128::MAX,
            },
            is_diesel_mint,
            header_ok,
            // No qualifier configured means everything qualifies;
            // `resolve_qualifier` narrows this when one is.
            qualified: true,
        });
    }

    out
}

/// Point every mint at the action that qualifies it.
///
/// For each mint protostone, walk **backwards** and take the nearest preceding
/// protostone that matches the qualifier, copying its declared cellpack inputs
/// into the mint's `prior_inputs`. A mint with no match gets an empty vector and
/// so weighs nothing.
///
/// Two rules, both consensus-relevant:
///
/// - **Backwards only.** The loop bound is the whole trust boundary. A protostone
///   at or after the claiming mint has not executed when the mint runs, so its
///   declared inputs prove nothing; a forward match would let a caller promise an
///   action they never perform. Expressing this as a slice bound rather than as a
///   check inside an opcode means it cannot be forgotten in one place.
/// - **Header-malformed protostones are skipped.** See [`ScannedItem::header_ok`].
///
/// Nearest rather than first, because "the thing I just did" is what a subsidy
/// author means. Either is deterministic; the claimant controls the ordering
/// regardless, so the choice decides which accident is possible, not which
/// attack.
fn resolve_qualifier(scanned: &mut [ScannedItem], q: &Qualifier) {
    for i in 0..scanned.len() {
        if !scanned[i].is_diesel_mint {
            continue;
        }
        let matched = scanned[..i].iter().rev().find(|s| {
            s.header_ok
                && s.item.target_block == q.target_block
                && s.item.target_tx == q.target_tx
                && s.item.opcode == q.opcode
        });
        match matched {
            Some(s) => scanned[i].item.prior_inputs = s.item.inputs.clone(),
            None => {
                scanned[i].item.prior_inputs = Vec::new();
                scanned[i].qualified = false;
            }
        }
    }
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
fn storage_base<T: KeyValuePointer>(atomic: &T) -> T {
    atomic
        .keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(FIREVECTOR_ID))
        .keyword("/storage/")
}

fn load_weightmap<T: KeyValuePointer>(atomic: &T) -> Option<Weightmap> {
    let raw = storage_base(atomic).select(&WEIGHTMAP_KEY.to_vec()).get();
    unpack_weightmap(&raw)
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
    let mut weights = BlockWeights {
        by_txid: HashMap::new(),
        per_tx: HashMap::new(),
        total: 0,
        legacy_count: 0,
    };

    let wm = active_weightmap(block, atomic);
    // SPLIT pre-computes a block total; RATE settles per claim as it executes and
    // must not be evaluated here. Both still need the scan index built below, so
    // this only decides whether weights are scored, never whether we scan.
    let split_eval = match &wm {
        Some(w) if w.mode == Mode::Split => validate(&w.program, Mode::Split)
            .ok()
            .map(|steps| (w.clone(), steps.max(1))),
        _ => None,
    };

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
        // The legacy count is the number of mint-bearing TRANSACTIONS, which is
        // exactly what this loop already visits — computing it here rather than
        // in a separate `legacy_mint_count` pass removes a second full scan of
        // every block.
        weights.legacy_count += 1;

        for s in scanned.iter_mut() {
            s.item.mint_rank = mint_rank;
        }
        mint_rank += 1;

        // The qualifier comes from the active weightmap regardless of mode: RATE
        // needs `prior_inputs` and `qualified` resolved here too, since that is
        // now the only place the block is scanned.
        if let Some(q) = wm.as_ref().and_then(|w| w.qualifier) {
            resolve_qualifier(&mut scanned, &q);
        }

        let txid = tx.compute_txid();
        weights.by_txid.insert(
            txid,
            TxScan {
                txindex: txindex as u32,
                mint_rank: mint_rank - 1,
                mint_items: scanned
                    .iter()
                    .filter(|s| s.is_diesel_mint && s.qualified)
                    .map(|s| (s.item.pstone_index, s.item.clone()))
                    .collect(),
            },
        );

        let Some((wm_split, budget)) = split_eval.as_ref() else {
            continue;
        };
        let wm = wm_split;
        let budget = *budget;

        // Only mint protostones are weighed. Previously every protostone of a
        // mint-bearing transaction was evaluated, which was harmless while the
        // program itself filtered on target and opcode — but the qualifier now
        // does that filtering, so a curve-only program would happily score a
        // non-mint protostone. Under the identity vector this changes nothing:
        // non-mints scored 0 there anyway, so `max` picked the same value.
        let raw = scanned
            .iter()
            .filter(|s| s.is_diesel_mint && s.qualified)
            .map(|s| eval(&wm.program, &s.item, budget))
            .max()
            .unwrap_or(0);

        // SPLIT settles membership, not magnitude.
        //
        // The delivery channel pays `emission / denominator` for an integer
        // denominator, so the only split it can settle exactly is the uniform
        // one: `sum(floor(E/k))` over k equal claimants is `<= E`, whereas
        // `floor(E / floor(W/w_i))` rounds every payout UP and over-mints the
        // block — measured at 1.40x for weights [3,1,1] and 1.50x for [100,99].
        // Exact conservation needs `w_i | W` for every i, an Egyptian-fraction
        // condition that arbitrary weights do not satisfy.
        //
        // Clamping is therefore not a restriction imposed on the design, it is
        // the channel's real expressiveness made explicit. Magnitude lives in
        // RATE mode. It also caps the damage from a forged qualifier at one
        // share rather than at an arbitrary declared amount.
        let w = (raw != 0) as u128;
        if w == 0 {
            continue;
        }
        weights.total = weights.total.saturating_add(w);
        weights.per_tx.insert(txid, w);
    }

    weights
}

/// Get (or compute and memo) this block's weight table.
///
/// The memo only answers for the block it was computed from; anything else
/// recomputes rather than returning a plausible-looking wrong table.
fn block_weights<T: KeyValuePointer>(
    block: &Block,
    height: u64,
    atomic: &T,
) -> Arc<BlockWeights> {
    let block_hash = block.block_hash();

    if let Ok(guard) = WEIGHTS_CACHE.read() {
        if let Some(cached) = guard.as_ref() {
            if cached.block_hash == block_hash {
                return Arc::clone(&cached.weights);
            }
        }
    }
    let computed = Arc::new(compute_block_weights(block, height, atomic));
    if let Ok(mut guard) = WEIGHTS_CACHE.write() {
        *guard = Some(CachedWeights {
            block_hash,
            weights: Arc::clone(&computed),
        });
    }
    computed
}

/// Is FIREVECTOR weighting active at this height?
/// The DIESEL block reward at `height`.
///
/// A deliberate mirror of `ChainConfiguration::block_reward` on the genesis
/// alkane, which FIREVECTOR cannot call — the contract is wasm, this is the
/// indexer, and the whole design rests on not modifying the contract. The two
/// must agree exactly, because [`treasury_target`] converts a basis-point share
/// of this into the fee the contract will compute. `block_reward_matches_the_contract`
/// pins the values at known heights and halving boundaries.
pub fn block_reward(height: u64) -> u128 {
    #[cfg(any(feature = "mainnet", not(any(
        feature = "dogecoin",
        feature = "fractal",
        feature = "luckycoin",
        feature = "bellscoin"
    ))))]
    {
        (50e8 as u128) / (1u128 << ((height as u128) / 210_000u128))
    }
    #[cfg(feature = "dogecoin")]
    {
        let _ = height;
        1_000_000_000_000u128
    }
    #[cfg(feature = "fractal")]
    {
        (25e8 as u128) / (1u128 << ((height as u128) / 2_100_000u128))
    }
    #[cfg(any(feature = "luckycoin", feature = "bellscoin"))]
    {
        let _ = height;
        1_000_000_000u128
    }
}

/// What `800000000:3` should report so the contract credits `treasury_bps` of
/// the block reward to `/fees`.
///
/// The contract does:
///
/// ```text
///   total_tx_fee = max(0, reported - block_reward)
///   diesel_fee   = min(block_reward / 2, total_tx_fee)
///   minters get   (block_reward - diesel_fee) / denominator
/// ```
///
/// so reporting `block_reward + target` makes `diesel_fee == target`, as long as
/// `target <= block_reward / 2` — which [`MAX_TREASURY_BPS`] guarantees.
///
/// `max` with the true coinbase total is what keeps this a floor rather than a
/// replacement: a genuinely high-fee block still credits its real fees, exactly
/// as today. And at `treasury_bps == 0` the target is 0, so the result is
/// `max(true_total, block_reward)` — which yields the same `total_tx_fee` the
/// contract computes today for every input, high-fee or low. **Zero is a no-op.**
pub fn reported_miner_fee(true_coinbase_total: u128, height: u64, treasury_bps: u128) -> u128 {
    let reward = block_reward(height);
    let bps = core::cmp::min(treasury_bps, MAX_TREASURY_BPS);
    let target = reward.saturating_mul(bps) / BPS_DENOM;
    core::cmp::max(true_coinbase_total, reward.saturating_add(target))
}

/// The value `800000000:3` returns.
///
/// Inert below the fork and whenever no weightmap sets `treasury_bps`.
pub fn effective_miner_fee<T: KeyValuePointer>(
    block: &Block,
    height: u64,
    true_coinbase_total: u128,
    atomic: &T,
) -> u128 {
    if !is_active(height) {
        return true_coinbase_total;
    }
    let bps = active_weightmap(block, atomic)
        .map(|wm| wm.treasury_bps)
        .unwrap_or(0);
    if bps == 0 {
        return true_coinbase_total;
    }
    reported_miner_fee(true_coinbase_total, height, bps)
}

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
    caller_vout: u32,
    incoming: &[(u128, u128, u128)],
    atomic: &T,
) -> u128 {
    if !is_active(height) {
        return legacy_mint_count(block);
    }

    match active_weightmap(block, atomic) {
        Some(wm) if wm.mode == Mode::Rate => {
            rate_denominator(block, height, caller_txid, caller_vout, incoming, &wm, atomic)
        }
        _ => split_denominator(block, height, caller_txid, atomic),
    }
}

fn split_denominator<T: KeyValuePointer>(
    block: &Block,
    height: u64,
    caller_txid: Txid,
    atomic: &T,
) -> u128 {
    let weights = block_weights(block, height, atomic);

    // No weightmap configured, or it weighed nothing in this block: emission
    // behaves exactly as it does today. This is the intended default — if nobody
    // does anything the weightmap rewards, the communist mint is what happens.
    if weights.total == 0 {
        return weights.legacy_count;
    }

    match weights.per_tx.get(&caller_txid).copied() {
        // The caller qualified: W / w_i. Weights are clamped to 0/1, so this is
        // `N / 1 == N` — an exact, conserving equal split among qualifiers.
        Some(w) if w > 0 => (weights.total / w).max(1),
        // The caller carries a mint but earned no weight under the active
        // vector. It claims nothing.
        _ => DENOMINATOR_NO_CLAIM,
    }
}

/// `ceil(a / b)` without overflowing. `b == 0` yields 0.
fn ceil_div(a: u128, b: u128) -> u128 {
    if b == 0 {
        return 0;
    }
    a / b + u128::from(a % b != 0)
}

/// RATE settlement: pay a rate against a governance-set virtual denominator.
///
/// # Why this shape
///
/// RATE weights depend on what actually happened during execution, so no block
/// total can exist — transaction `j`'s weight is unknowable until `j` runs, and
/// the first mint would need it. Instead of a share of `W`, a claimant weighing
/// `w` is paid against a fixed `rate_floor`:
///
/// ```text
///   denominator = ceil(rate_floor / w)
/// ```
///
/// # Why it conserves
///
/// `ceil` is the whole trick. `d_i = ceil(rate_floor / w_i) >= rate_floor / w_i`,
/// so `1/d_i <= w_i / rate_floor`, and therefore
///
/// ```text
///   sum(payouts) = sum(floor(E / d_i)) <= E * sum(w_i) / rate_floor <= E
/// ```
///
/// provided cumulative weight is clamped at `rate_floor` — which is what the
/// spend counter below does. Rounding the denominator UP rounds the payout DOWN,
/// so a block can under-pay but never over-mint. That is the opposite of the
/// SPLIT `floor(W/w)` form, which rounds every payout up at once.
///
/// The cost is precision: the relative shortfall is at most `1/(d_i + 1)`, so a
/// claimant taking a large fraction of the block loses more to rounding than one
/// taking a small slice. Calibrate `rate_floor` so that typical weights are a
/// small fraction of it and the loss stays under a percent.
///
/// Note that no knowledge of the emission `E` is needed anywhere here. An earlier
/// design computed `E` indexer-side to invert the division, which would have
/// meant duplicating the block-reward and fee-carve-out arithmetic — per-chain,
/// consensus-critical, and impossible to keep in sync. `rate_floor` avoids it.
fn rate_denominator<T: KeyValuePointer>(
    block: &Block,
    height: u64,
    txid: Txid,
    vout: u32,
    incoming: &[(u128, u128, u128)],
    wm: &Weightmap,
    atomic: &T,
) -> u128 {
    if wm.rate_floor == 0 {
        return DENOMINATOR_NO_CLAIM;
    }
    let Ok(steps) = validate(&wm.program, Mode::Rate) else {
        // An unrunnable weightmap falls back to today's emission, never to an
        // error and never to a silent zero for everybody.
        return block_weights(block, height, atomic).legacy_count;
    };

    let base = storage_base(atomic);
    let mut claim_key = RATE_CLAIM_KEY.to_vec();
    claim_key.extend_from_slice(&height.to_le_bytes());
    claim_key.extend_from_slice(txid.as_ref());
    let claim_ptr = base.select(&claim_key);

    // Idempotence. `effective_denominator` is reachable more than once for the
    // same claim — a view-path replay, a trace re-render, a contract that
    // extcalls the precompile twice — and a bare running total would consume the
    // budget again each time. The symptom would be silently wrong emission
    // rather than a crash, which is the failure class the block-hash keying on
    // WEIGHTS_CACHE already exists to prevent.
    let existing = claim_ptr.get();
    if existing.len() == 16 {
        return u128::from_le_bytes(existing[..16].try_into().expect("len checked"));
    }

    // The block scan is memoized, so this is a hash lookup rather than the
    // O(N) rebuild it used to be. `tx_output_len` is needed to invert the shadow
    // vout and is the only thing still read off the transaction itself.
    let weights = block_weights(block, height, atomic);
    let Some(tx_output_len) = weights
        .by_txid
        .get(&txid)
        .and_then(|scan| block.txdata.get(scan.txindex as usize))
        .map(|tx| tx.output.len())
    else {
        return DENOMINATOR_NO_CLAIM;
    };
    let Some(item) = rate_item(&weights, txid, vout, tx_output_len, incoming) else {
        return DENOMINATOR_NO_CLAIM;
    };
    let w = eval(&wm.program, &item, steps.max(1));
    if w == 0 {
        // Earns nothing, and reserves nothing — a zero-weight claim must not
        // consume budget that a real claimant could have used.
        return DENOMINATOR_NO_CLAIM;
    }

    let mut spent_key = RATE_SPENT_KEY.to_vec();
    spent_key.extend_from_slice(&height.to_le_bytes());
    let mut spent_ptr = base.select(&spent_key);
    let raw = spent_ptr.get();
    let spent = if raw.len() == 16 {
        u128::from_le_bytes(raw[..16].try_into().expect("len checked"))
    } else {
        0
    };

    let w_eff = core::cmp::min(w, wm.rate_floor.saturating_sub(spent));
    if w_eff == 0 {
        // The block's rate budget is exhausted. Later claimants earn nothing
        // rather than the block over-minting.
        return DENOMINATOR_NO_CLAIM;
    }

    let denominator = ceil_div(wm.rate_floor, w_eff).max(1);
    spent_ptr.set(Arc::new(spent.saturating_add(w_eff).to_le_bytes().to_vec()));
    base.select(&claim_key)
        .set(Arc::new(denominator.to_le_bytes().to_vec()));
    denominator
}

/// Build the item for a RATE claim: this protostone, with its realized incoming
/// alkanes and its qualifying prior action.
fn rate_item(
    weights: &BlockWeights,
    txid: Txid,
    vout: u32,
    tx_output_len: usize,
    incoming: &[(u128, u128, u128)],
) -> Option<Item> {
    let scan = weights.by_txid.get(&txid)?;

    // Invert `shadow_vout = pstone_index + tx.output.len() + 1`.
    let pstone_index = (vout as usize).checked_sub(tx_output_len + 1)? as u128;

    let mut item = scan
        .mint_items
        .iter()
        .find(|(idx, _)| *idx == pstone_index)
        .map(|(_, it)| it.clone())?;

    item.mint_rank = scan.mint_rank;
    item.incoming = incoming.to_vec();
    Some(item)
}
