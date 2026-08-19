//! Indexer-side FIREVECTOR tests.
//!
//! The claim under test is narrow and load-bearing: with the identity vector
//! loaded, `800000000:2` returns exactly what it returns today, so activation
//! changes nothing — not the payout, and not the fuel, because the DIESEL
//! contract is untouched and the reply stays 16 bytes.
//!
//! Everything else here is about the ways that claim could quietly stop being
//! true.

use crate::firevector::{DSIGIL_ID, FV_ORBITAL_ID, 
    clear_weights_cache, compute_block_weights, effective_denominator, legacy_mint_count,
    DENOMINATOR_NO_CLAIM, DIESEL_ID, DIESEL_MINT_OPCODE, FIREVECTOR_ID, WEIGHTMAP_KEY,
};
use crate::tests::helpers::{self as alkane_helpers, clear};
use alkanes_std_firevector::abi::{pack_weightmap, Weightmap};
use alkanes_std_firevector::programs;
use alkanes_std_firevector::vm::*;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use bitcoin::hashes::Hash;
use bitcoin::{Block, OutPoint, ScriptBuf, Transaction, Txid, Witness};
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::test_helpers::create_block_with_coinbase_tx;
use std::sync::Arc;
use wasm_bindgen_test::wasm_bindgen_test;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn mint_cellpack() -> Cellpack {
    Cellpack {
        target: DIESEL_ID,
        inputs: vec![DIESEL_MINT_OPCODE],
    }
}

/// A transaction carrying `cellpacks` as consecutive tag-1 protostones.
///
/// `nonce` varies the spent outpoint so each transaction gets a distinct txid.
/// That matters: weights are keyed by txid, and a fixture of byte-identical
/// transactions would collapse them into one entry — which real blocks cannot do
/// anyway, since BIP30 forbids duplicate txids.
fn tx_with(cellpacks: Vec<Cellpack>, nonce: u32) -> Transaction {
    let previous_output = OutPoint {
        txid: Txid::from_byte_array([0u8; 32]),
        vout: nonce,
    };
    alkane_helpers::create_multiple_cellpack_with_witness_and_scriptsig_and_in(
        Witness::new(),
        ScriptBuf::new(),
        cellpacks,
        previous_output,
        false,
    )
}

/// As [`tx_with`], but protostone 0 gets a `pointer` far past
/// `num_outputs + num_protostones`.
///
/// Such a protostone is refunded and skipped by `process_message` before its
/// message runs, so it never executes and never drains fuel — yet it still
/// decodes here as a well-formed cellpack. It is the cheapest way to fake a
/// qualifying action, which is why the qualifier must refuse to match it.
fn tx_with_malformed_first_protostone(cellpacks: Vec<Cellpack>, nonce: u32) -> Transaction {
    use ordinals::Runestone;
    use protorune_support::protostone::{Protostone, Protostones};

    let n = cellpacks.len();
    let protostones: Vec<Protostone> = cellpacks
        .into_iter()
        .enumerate()
        .map(|(i, cellpack)| Protostone {
            message: cellpack.encipher(),
            pointer: Some(if i == 0 { 100 } else { 0 }),
            refund: Some(0),
            edicts: vec![],
            from: None,
            burn: None,
            protocol_tag: 1u128,
        })
        .collect();
    assert!(n >= 1);

    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: Some(0),
        edicts: Vec::new(),
        mint: None,
        protocol: protostones.encipher().ok(),
    })
    .encipher();

    let mut tx = tx_with(vec![mint_cellpack()], nonce);
    // Swap in our hand-built OP_RETURN, keeping the distinct prevout so the txid
    // stays unique.
    let last = tx.output.len() - 1;
    tx.output[last] = bitcoin::TxOut {
        value: bitcoin::Amount::from_sat(0),
        script_pubkey: runestone,
    };
    tx
}

/// Commit the header to the transactions it contains.
///
/// `create_block_with_coinbase_tx` fixes the header at construction, so pushing
/// transactions afterwards leaves the merkle root stale and every fixture block
/// hashes identically. A real block cannot do that, and the weight memo is keyed
/// by block hash — so without this, one fixture's table gets served to the next.
fn finalize(mut block: Block) -> Block {
    if let Some(root) = block.compute_merkle_root() {
        block.header.merkle_root = root;
    }
    block
}

/// A block of `n` independent DIESEL mint transactions.
fn block_of_mints(n: usize) -> Block {
    let mut block = create_block_with_coinbase_tx(880_000);
    for i in 0..n {
        block.txdata.push(tx_with(vec![mint_cellpack()], i as u32));
    }
    finalize(block)
}

/// Write a weightmap into FIREVECTOR's contract storage, exactly where
/// `set_weightmap` on 12:0 puts it.
///
/// Defaults to SPLIT with no qualifier, which is what the equivalence tests need:
/// every DIESEL mint qualifies and the payout is the plain equal split.
fn set_weightmap(program: &[u128]) {
    set_weightmap_full(&Weightmap {
        mode: Mode::Split,
        qualifier: None,
        rate_floor: 0,
        treasury_bps: 0,
        program: program.to_vec(),
    });
}

fn set_weightmap_full(wm: &Weightmap) {
    set_weightmap_raw(pack_weightmap(wm));
}

fn set_weightmap_raw(bytes: Vec<u8>) {
    write_weightmap_bytes(bytes);
    clear_weights_cache();
}

/// Write the storage slot WITHOUT dropping the per-block caches.
///
/// Models a `set_weightmap` landing partway through a block: the bytes change,
/// but the block-start snapshot taken by `index_block` does not.
fn write_weightmap_bytes(bytes: Vec<u8>) {
    IndexPointer::default()
        .keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(FIREVECTOR_ID))
        .keyword("/storage/")
        .select(&WEIGHTMAP_KEY.to_vec())
        .set(Arc::new(bytes));
}

/// The denominator the precompile hands transaction `txindex`.
///
/// `vout` and `incoming` are the RATE-mode inputs; SPLIT ignores both, so the
/// equivalence tests pass a protostone vout and no alkanes.
fn denominator_for(block: &Block, txindex: usize) -> u128 {
    denominator_for_claim(block, txindex, 0, &[])
}

fn denominator_for_claim(
    block: &Block,
    txindex: usize,
    pstone: u32,
    incoming: &[(u128, u128, u128)],
) -> u128 {
    let tx = &block.txdata[txindex];
    let vout = tx.output.len() as u32 + 1 + pstone;
    effective_denominator(
        block,
        880_000,
        tx.compute_txid(),
        vout,
        incoming,
        &IndexPointer::default(),
    )
}

/// The weight a program assigns a transaction, via the same path the precompile
/// uses.
fn weights_of(block: &Block) -> crate::firevector::BlockWeights {
    compute_block_weights(block, 880_000, &IndexPointer::default())
}

// ---------------------------------------------------------------------------
// Equivalence: the whole point of the design
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn identity_vector_returns_exactly_the_legacy_count() {
    clear();
    for n in [1usize, 2, 5, 50] {
        let block = block_of_mints(n);
        let expected = legacy_mint_count(&block);
        assert_eq!(expected, n as u128, "fixture sanity: {n} mints");

        set_weightmap(&programs::identity());
        for i in 1..=n {
            assert_eq!(
                denominator_for(&block, i),
                expected,
                "mint {i} of {n}: identity vector must return the legacy count"
            );
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn with_no_weightmap_configured_nothing_changes() {
    // The state a chain is in between shipping the indexer and seeding the
    // weightmap slot. It must behave exactly as today.
    clear();
    let block = block_of_mints(7);
    assert_eq!(denominator_for(&block, 1), legacy_mint_count(&block));
    assert_eq!(denominator_for(&block, 1), 7);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_reply_is_sixteen_bytes() {
    // `returndatacopy` charges 2 fuel per byte of reply. A wider answer would
    // reintroduce exactly the fuel divergence this design exists to avoid, so
    // the width is part of the contract, not an implementation detail.
    clear();
    let block = block_of_mints(3);
    set_weightmap(&programs::identity());
    assert_eq!(denominator_for(&block, 1).to_le_bytes().len(), 16);
}

// ---------------------------------------------------------------------------
// Fallbacks — every degenerate case degrades to today's behaviour
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_malformed_weightmap_falls_back_to_the_legacy_count() {
    clear();
    let block = block_of_mints(4);

    // Not a multiple of 16 bytes, so not a whole number of u128 words.
    set_weightmap_raw(vec![0u8; 17]);
    assert_eq!(denominator_for(&block, 1), 4);

    // Empty.
    set_weightmap_raw(vec![]);
    assert_eq!(denominator_for(&block, 1), 4);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn an_invalid_program_falls_back_to_the_legacy_count() {
    clear();
    let block = block_of_mints(4);
    // ADD with nothing on the stack — fails validation.
    set_weightmap(&[OP_ADD]);
    assert_eq!(denominator_for(&block, 1), 4);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_program_reading_unavailable_facts_falls_back() {
    clear();
    let block = block_of_mints(4);
    // Realized incoming amounts are not knowable during SPLIT's pre-execution
    // scan; the program is rejected for that mode rather than silently reading
    // zero, and the rejection degrades to today's count.
    set_weightmap(&[OP_INCOMING_AMOUNT, 32, 0]);
    assert_eq!(denominator_for(&block, 1), 4);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_vector_that_weights_nothing_falls_back_to_the_communist_mint() {
    // "If nobody does anything the weightmap rewards, what happens today keeps
    // happening" — the intended default, not an accident.
    clear();
    let block = block_of_mints(6);
    // Well-formed, but matches no transaction in this block.
    let other_app = vec![
        OP_TARGET_BLOCK, OP_PUSH, 4, OP_EQ,
        OP_TARGET_TX, OP_PUSH, 9999, OP_EQ, OP_AND,
    ];
    assert!(validate(&other_app, Mode::Split).is_ok());
    set_weightmap(&other_app);
    assert_eq!(denominator_for(&block, 1), 6);
}

// ---------------------------------------------------------------------------
// The two policies this system exists to express
// ---------------------------------------------------------------------------

/// A pool contract a weightmap can subsidise.
const POOL: (u128, u128) = (4, 1778);
/// Its swap opcode.
const SWAP_OP: u128 = 3;

fn swap_cellpack(amount_in: u128) -> Cellpack {
    Cellpack {
        target: AlkaneId {
            block: POOL.0,
            tx: POOL.1,
        },
        inputs: vec![SWAP_OP, amount_in],
    }
}

fn pool_qualifier() -> Qualifier {
    Qualifier {
        target_block: POOL.0,
        target_tx: POOL.1,
        opcode: SWAP_OP,
    }
}

/// A block where `swappers` transactions do `[swap(amount), mint]` and
/// `plain` transactions do `[mint]` alone.
fn block_of_swaps(amounts: &[u128], plain: usize) -> Block {
    let mut block = create_block_with_coinbase_tx(880_000);
    for (i, amt) in amounts.iter().enumerate() {
        block
            .txdata
            .push(tx_with(vec![swap_cellpack(*amt), mint_cellpack()], i as u32));
    }
    for j in 0..plain {
        block
            .txdata
            .push(tx_with(vec![mint_cellpack()], (amounts.len() + j) as u32));
    }
    finalize(block)
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn scenario_equal_split_among_callers_of_an_opcode() {
    // "Split the block equally among everyone who called this opcode."
    //
    // The program is a bare membership constant; all the selectivity lives in
    // the header's qualifier, which the indexer resolves by looking backwards
    // from each mint. A mint with no preceding swap is not qualified and earns
    // nothing, even though the program would happily return 1 for it — which is
    // exactly why the qualifier has to gate the item rather than just hand it
    // `prior_inputs`.
    clear();
    let block = block_of_swaps(&[100, 200, 300], 2); // 3 swappers, 2 plain mints
    set_weightmap_full(&Weightmap {
        mode: Mode::Split,
        qualifier: Some(pool_qualifier()),
        rate_floor: 0,
        treasury_bps: 0,
        program: vec![OP_PUSH, 1],
    });

    let w = weights_of(&block);
    assert_eq!(w.total, 3, "only the three swappers qualify");

    for i in 1..=3 {
        assert_eq!(
            denominator_for(&block, i),
            3,
            "swapper {i} takes an equal third"
        );
    }
    for i in 4..=5 {
        assert_eq!(
            denominator_for(&block, i),
            DENOMINATOR_NO_CLAIM,
            "a mint with no preceding swap earns nothing"
        );
    }

    // And the block still pays out exactly once over.
    let paid: u128 = (1..=3).map(|i| EMISSION / denominator_for(&block, i)).sum();
    assert!(paid <= EMISSION, "paid {paid} against emission {EMISSION}");
    assert!(paid + 3 >= EMISSION, "at most one sub-unit of dust per claimant");
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn scenario_linear_in_the_amount_swapped() {
    // "Pay linearly in the amount that was swapped."
    //
    // RATE mode, because a proportional payout cannot be settled through the
    // `emission / k` channel as a share: only the uniform split lands exactly on
    // that ladder. Against a fixed `rate_floor` it can, because `ceil` rounds the
    // denominator up and so rounds every payout DOWN.
    //
    // The amount is read from the swap's own declared cellpack input. That is
    // trustworthy for a reason outside this VM: a reverting protostone drains the
    // transaction's fuel, so a caller who overstates their swap makes the swap
    // revert and their own mint dies with it.
    clear();
    let rate_floor = 1_000u128;
    let block = block_of_swaps(&[500, 300, 100], 1);
    set_weightmap_full(&Weightmap {
        mode: Mode::Rate,
        qualifier: Some(pool_qualifier()),
        rate_floor,
        treasury_bps: 0,
        // weight = the declared amount_in, scaled 1:1
        program: programs::linear_on_prior(0, 1, 1),
    });

    // Weights 500 / 300 / 100 against a floor of 1000 -> denominators
    // ceil(1000/500)=2, ceil(1000/300)=4, ceil(1000/100)=10.
    let d1 = denominator_for_claim(&block, 1, 1, &[]);
    let d2 = denominator_for_claim(&block, 2, 1, &[]);
    let d3 = denominator_for_claim(&block, 3, 1, &[]);
    assert_eq!((d1, d2, d3), (2, 4, 10));

    // Bigger swap, bigger payout, and monotone in the declared amount.
    let (p1, p2, p3) = (EMISSION / d1, EMISSION / d2, EMISSION / d3);
    assert!(p1 > p2 && p2 > p3, "payout must increase with size: {p1} {p2} {p3}");

    // The mint with no swap in front of it earns nothing.
    assert_eq!(
        denominator_for_claim(&block, 4, 0, &[]),
        DENOMINATOR_NO_CLAIM
    );

    // Conservation. sum(1/d_i) <= sum(w_i)/rate_floor <= 1, so the block cannot
    // over-mint no matter what the curve returns.
    let paid = p1 + p2 + p3;
    assert!(
        paid <= EMISSION,
        "rate mode paid {paid} against emission {EMISSION}"
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn rate_mode_never_overmints_however_large_the_weights() {
    // The conservation guarantee has to hold when demand exceeds the floor, which
    // is the case the clamp exists for: cumulative weight is capped at
    // `rate_floor`, so claimants past the cap earn nothing rather than the block
    // paying out twice over.
    clear();
    let rate_floor = 1_000u128;
    // Four swappers each declaring 400 -> 1600 of demand against a floor of 1000.
    let block = block_of_swaps(&[400, 400, 400, 400], 0);
    set_weightmap_full(&Weightmap {
        mode: Mode::Rate,
        qualifier: Some(pool_qualifier()),
        rate_floor,
        treasury_bps: 0,
        program: programs::linear_on_prior(0, 1, 1),
    });

    let paid: u128 = (1..=4)
        .map(|i| {
            let d = denominator_for_claim(&block, i, 1, &[]);
            if d == DENOMINATOR_NO_CLAIM || d == 0 {
                0
            } else {
                EMISSION / d
            }
        })
        .sum();
    assert!(
        paid <= EMISSION,
        "over-subscribed rate block paid {paid} against emission {EMISSION}"
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn rate_reservations_are_idempotent() {
    // `effective_denominator` is reachable more than once for the same claim --
    // a view-path replay, a trace re-render, a contract calling the precompile
    // twice. A bare running total would consume the budget again each time and
    // hand the same claimant a worse denominator on every repeat.
    clear();
    let block = block_of_swaps(&[500, 500], 0);
    set_weightmap_full(&Weightmap {
        mode: Mode::Rate,
        qualifier: Some(pool_qualifier()),
        rate_floor: 1_000,
        treasury_bps: 0,
        program: programs::linear_on_prior(0, 1, 1),
    });

    let first = denominator_for_claim(&block, 1, 1, &[]);
    for _ in 0..5 {
        assert_eq!(
            denominator_for_claim(&block, 1, 1, &[]),
            first,
            "a repeated query must not consume the rate budget again"
        );
    }
    // The second claimant still gets its full share.
    assert_eq!(denominator_for_claim(&block, 2, 1, &[]), 2);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_forged_qualifier_earns_nothing() {
    // H1: a protostone whose header is malformed is refunded and skipped BEFORE
    // its message runs, so it never drains fuel and the mint after it executes
    // normally -- while still parsing here as a perfectly good swap cellpack.
    //
    // Without the header check in `resolve_qualifier`, one out-of-range integer
    // in the OP_RETURN would buy a qualifying action that provably never
    // happened. `crates/alkanes/src/tests/firevector_ordering.rs` proves the
    // fuel half of this; here we prove the weightmap refuses to pay for it.
    clear();
    let mut block = create_block_with_coinbase_tx(880_000);
    block
        .txdata
        .push(tx_with(vec![swap_cellpack(100), mint_cellpack()], 0));
    block.txdata.push(tx_with_malformed_first_protostone(
        vec![swap_cellpack(u128::MAX), mint_cellpack()],
        1,
    ));
    let block = finalize(block);

    // Sanity, or the whole test passes vacuously: BOTH transactions must parse
    // as carrying a DIESEL mint. If the malformed fixture simply failed to
    // decode, tx 2 would earn nothing for an entirely uninteresting reason.
    assert_eq!(
        legacy_mint_count(&block),
        2,
        "both fixtures must decode as mint-bearing transactions"
    );

    set_weightmap_full(&Weightmap {
        mode: Mode::Split,
        qualifier: Some(pool_qualifier()),
        rate_floor: 0,
        treasury_bps: 0,
        program: vec![OP_PUSH, 1],
    });

    let w = weights_of(&block);
    assert_eq!(
        w.total, 1,
        "only the honest swap qualifies; the malformed one is not evidence of anything"
    );
    assert_eq!(denominator_for(&block, 1), 1, "honest swapper takes the block");
    assert_eq!(
        denominator_for(&block, 2),
        DENOMINATOR_NO_CLAIM,
        "a skipped protostone must not qualify the mint behind it"
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_weightmap_set_mid_block_does_not_take_effect_until_the_next_block() {
    // Emission policy must not change underneath a block that is already
    // running. Reading the map through the executing message's atomic pointer
    // would make a governance write visible to every mint ordered after it and
    // invisible to every mint before it — so the effective activation point
    // would be "the first mint following the setter", a boundary chosen by
    // whoever orders the block rather than by governance.
    clear();
    let block = block_of_mints(3);

    // Block-start state: the plain equal split.
    set_weightmap(&programs::identity());
    crate::firevector::snapshot_weightmap(&block);

    // Governance lands a map partway through the block that pays ONLY tx 3.
    // It has to produce a genuinely different answer from the identity vector,
    // or the test proves nothing: a map that merely qualifies nobody falls back
    // to the legacy count, which is 3 — the same number the equal split gives.
    write_weightmap_bytes(pack_weightmap(&Weightmap {
        mode: Mode::Split,
        qualifier: None,
        rate_floor: 0,
        treasury_bps: 0,
        program: qualify_from_txindex(3),
    }));

    // This block still settles under the map it started with: three equal shares.
    for i in 1..=3 {
        assert_eq!(
            denominator_for(&block, i),
            3,
            "mint {i} must settle under the block-start weightmap, not the one \
             written while the block was running"
        );
    }

    // A later block picks the new map up, and it pays differently: only the
    // transaction at txindex 3 qualifies, so it takes the whole block.
    let next = block_of_mints(3);
    clear_weights_cache();
    crate::firevector::snapshot_weightmap(&next);
    assert_eq!(denominator_for(&next, 1), DENOMINATOR_NO_CLAIM);
    assert_eq!(denominator_for(&next, 3), 1);
}

// ---------------------------------------------------------------------------
// Non-uniform weights, and the cache bug this design had to avoid
// ---------------------------------------------------------------------------

/// Weight a mint by its protostone index, so that transactions in the same block
/// get deliberately different raw weights.
///
/// Under SPLIT the clamp flattens all of these to 1; the fixture is still useful
/// for exercising the memo, since the *table* is still computed per transaction.
fn weight_by_txindex() -> Vec<u128> {
    // qualify as a DIESEL mint, then weight = txindex
    vec![
        OP_TARGET_BLOCK, OP_PUSH, 2, OP_EQ,
        OP_TARGET_TX, OP_PUSH, 0, OP_EQ, OP_AND,
        OP_OPCODE, OP_PUSH, 77, OP_EQ, OP_AND,
        OP_TXINDEX,
        OP_MUL,
    ]
}

/// Qualify only the mints at or after `from`, so a block contains both claimants
/// and non-claimants.
fn qualify_from_txindex(from: u128) -> Vec<u128> {
    vec![
        OP_TARGET_BLOCK, OP_PUSH, 2, OP_EQ,
        OP_TARGET_TX, OP_PUSH, 0, OP_EQ, OP_AND,
        OP_OPCODE, OP_PUSH, 77, OP_EQ, OP_AND,
        OP_TXINDEX, OP_PUSH, from, OP_GE, OP_AND,
    ]
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn different_transactions_get_different_denominators() {
    // THE regression test for the cache bug. The old precompile memoized the
    // reply *bytes*, which is fine while the answer is block-global and
    // catastrophic once it is per-transaction: the first mint would populate the
    // cache and every later mint would be handed the first one's denominator.
    //
    // SPLIT clamps weights to 0/1, so two *qualifying* transactions now share a
    // denominator by design — that is the equal split working, not a cache hit.
    // The distinguishing case is therefore qualifier vs non-qualifier, which is
    // what this fixture builds: tx 1 earns nothing, tx 2 and tx 3 split the block.
    clear();
    let block = block_of_mints(3); // txindex 1, 2, 3
    set_weightmap(&qualify_from_txindex(2));

    let w = weights_of(&block);
    assert_eq!(w.total, 2, "two qualifiers, each weighing exactly 1");

    let d1 = denominator_for(&block, 1);
    let d2 = denominator_for(&block, 2);
    let d3 = denominator_for(&block, 3);

    assert_eq!(d1, DENOMINATOR_NO_CLAIM, "tx 1 does not qualify");
    assert_eq!(d2, 2);
    assert_eq!(d3, 2);
    assert!(
        d1 != d2,
        "per-transaction answers must not collapse to one cached value: {d1} {d2} {d3}"
    );
}

// ---------------------------------------------------------------------------
// Conservation: the sum of what everybody is paid must not exceed the emission
// ---------------------------------------------------------------------------

/// The block reward at height 880_000: `50e8 / 2^(880000/210000)` = `50e8 / 16`.
///
/// The real emission is `block_reward - diesel_fee`, but the fee carve-out is a
/// constant across every mint in a block and so cancels out of the ratio these
/// tests assert. Using the reward directly keeps them independent of the
/// coinbase fixture.
const EMISSION: u128 = 312_500_000;

/// Weight a qualifying mint `hi` if its txindex is below `split_at`, else `lo`.
///
/// Exists because the only non-uniform fixture in this file (`weight_by_txindex`)
/// produces `[1,2,3]`, where `W = 6` is divisible by every weight — the one shape
/// that conserves by accident. Real weightmaps have no such property.
fn weight_hi_lo(hi: u128, lo: u128, split_at: u128) -> Vec<u128> {
    vec![
        OP_TARGET_BLOCK, OP_PUSH, 2, OP_EQ,
        OP_TARGET_TX, OP_PUSH, 0, OP_EQ, OP_AND,
        OP_OPCODE, OP_PUSH, 77, OP_EQ, OP_AND,
        // hi - (hi - lo) * (txindex >= split_at)
        OP_PUSH, hi,
        OP_TXINDEX, OP_PUSH, split_at, OP_GE,
        OP_PUSH, hi - lo, OP_MUL,
        OP_SUB,
        OP_MUL,
    ]
}

/// Total paid out across the block, as the DIESEL contract would compute it:
/// each mint independently gets `emission / denominator`, with no cross-mint
/// residual accounting and no block-level cap.
fn total_paid(block: &Block, n_mints: usize) -> u128 {
    (1..=n_mints)
        .map(|i| {
            let d = denominator_for(block, i);
            if d == 0 { 0 } else { EMISSION / d }
        })
        .sum()
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn split_conserves_emission() {
    // A normalized split must divide the emission, never multiply it. The
    // contract computes `emission / denominator` per mint and calls
    // `increase_total_supply` each time, so paying out more than `emission`
    // inflates DIESEL past its halving schedule.
    //
    // Flooring the DENOMINATOR rounds the PAYOUT up: `floor(W/w_i) <= W/w_i`,
    // so `emission / floor(W/w_i) >= emission * w_i / W` for every claimant at
    // once. Exact conservation holds iff `w_i` divides `W` for all `i`.
    clear();
    let block = block_of_mints(3); // txindex 1, 2, 3

    // The program returns raw weights [3, 1, 1]. Before the clamp that gave
    // W = 5 and denominators 5/3=1, 5/1=5, 5/1=5, i.e. the first claimant took
    // the WHOLE block and the other two took a fifth each -- 1.40x emission.
    // The clamp flattens it to [1, 1, 1], so W = 3 and everyone gets E/3.
    set_weightmap(&weight_hi_lo(3, 1, 2));
    assert_eq!(
        weights_of(&block).total,
        3,
        "SPLIT clamps every non-zero weight to 1, so W is the qualifier count"
    );

    let paid = total_paid(&block, 3);
    assert!(
        paid <= EMISSION,
        "block paid out {} against an emission of {} ({:.2}x) -- a normalized \
         split must never mint more than the block reward",
        paid,
        EMISSION,
        paid as f64 / EMISSION as f64
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn split_conserves_emission_across_weight_shapes() {
    // The failure is systematic, not a quirk of one fixture. Each case is a
    // (hi, lo, split_at, n_mints) whose weight multiset is stated in the
    // comment; none of them is exotic.
    clear();
    for (hi, lo, split_at, n) in [
        (3u128, 1u128, 2u128, 3usize), // [3,1,1]   W=5
        (2, 1, 2, 2),                  // [2,1]     W=3
        (2, 1, 2, 4),                  // [2,1,1,1] W=5
        (100, 99, 2, 2),               // [100,99]  W=199
    ] {
        let block = block_of_mints(n);
        set_weightmap(&weight_hi_lo(hi, lo, split_at));

        let paid = total_paid(&block, n);
        assert!(
            paid <= EMISSION,
            "hi={} lo={} n={}: paid {} against emission {} ({:.2}x)",
            hi, lo, n, paid, EMISSION,
            paid as f64 / EMISSION as f64
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn uniform_weights_conserve_exactly() {
    // The control. Every qualifier weighing the same is the one shape the
    // `emission / k` delivery channel can settle exactly, and it is what the
    // identity vector produces -- which is why activation is payout-neutral.
    clear();
    for n in 1..=6usize {
        let block = block_of_mints(n);
        set_weightmap(&programs::identity());

        let paid = total_paid(&block, n);
        assert!(paid <= EMISSION, "n={}: paid {} > emission {}", n, paid, EMISSION);
        // Dust is at most one sub-unit per claimant, from the single floor.
        assert!(
            paid + (n as u128) >= EMISSION,
            "n={}: paid {} leaves more than {} dust unminted",
            n, paid, n
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_memo_survives_repeated_queries_without_drifting() {
    clear();
    let block = block_of_mints(3);
    set_weightmap(&weight_by_txindex());

    let first: Vec<u128> = (1..=3).map(|i| denominator_for(&block, i)).collect();
    for _ in 0..5 {
        let again: Vec<u128> = (1..=3).map(|i| denominator_for(&block, i)).collect();
        assert_eq!(first, again, "memoized table must be stable within a block");
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_mint_with_zero_weight_claims_nothing() {
    clear();
    // txindex 1 and 2; weight = txindex - 1, so the first mint weighs 0.
    let block = block_of_mints(2);
    let p = vec![
        OP_TARGET_BLOCK, OP_PUSH, 2, OP_EQ,
        OP_TARGET_TX, OP_PUSH, 0, OP_EQ, OP_AND,
        OP_TXINDEX, OP_PUSH, 1, OP_SUB,
        OP_MUL,
    ];
    set_weightmap(&p);

    assert_eq!(
        denominator_for(&block, 1),
        DENOMINATOR_NO_CLAIM,
        "a zero-weight mint must divide the emission to nothing"
    );
    // emission / u128::MAX == 0 for any realistic emission
    assert_eq!(312_500_000u128 / DENOMINATOR_NO_CLAIM, 0);

    // The one that did qualify takes the whole block.
    assert_eq!(denominator_for(&block, 2), 1);
}

// ---------------------------------------------------------------------------
// The scan must match `_get_number_diesel_mints` exactly
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_transaction_with_two_mint_protostones_still_counts_once() {
    // `_get_number_diesel_mints` breaks out of the protostone loop on its first
    // match, so a double-mint transaction counts once today. Weights are taken as
    // the MAX across a transaction's protostones rather than the sum precisely so
    // that stays true — summing would let such a transaction weigh 2 and dilute
    // every honest minter in the block.
    clear();
    let mut block = create_block_with_coinbase_tx(880_000);
    block.txdata.push(tx_with(vec![mint_cellpack(), mint_cellpack()], 0));
    block.txdata.push(tx_with(vec![mint_cellpack()], 1));

    let block = finalize(block);
    assert_eq!(legacy_mint_count(&block), 2, "two transactions, not three mints");

    set_weightmap(&programs::identity());
    let w = weights_of(&block);
    assert_eq!(w.total, 2);
    assert_eq!(denominator_for(&block, 1), 2);
    assert_eq!(denominator_for(&block, 2), 2);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn non_mint_transactions_do_not_contribute_to_the_denominator() {
    // A transaction that cannot claim must not be counted, or its share of the
    // emission would simply be burned.
    clear();
    let mut block = create_block_with_coinbase_tx(880_000);
    block.txdata.push(tx_with(vec![mint_cellpack()], 0));
    block.txdata.push(tx_with(
        vec![Cellpack {
            target: AlkaneId { block: 4, tx: 1778 },
            inputs: vec![1],
        }],
        1,
    ));
    block.txdata.push(tx_with(vec![mint_cellpack()], 2));

    let block = finalize(block);
    assert_eq!(legacy_mint_count(&block), 2);
    set_weightmap(&programs::identity());
    assert_eq!(weights_of(&block).total, 2);
    assert_eq!(denominator_for(&block, 1), 2);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_transaction_with_no_runestone_is_ignored() {
    clear();
    let mut block = create_block_with_coinbase_tx(880_000);
    block.txdata.push(tx_with(vec![mint_cellpack()], 0));
    let block = finalize(block);
    // The coinbase carries no runestone and must not be scanned as a mint.
    assert_eq!(legacy_mint_count(&block), 1);
    set_weightmap(&programs::identity());
    assert_eq!(denominator_for(&block, 1), 1);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn an_empty_block_yields_a_zero_count_without_panicking() {
    clear();
    let block = finalize(create_block_with_coinbase_tx(880_000));
    assert_eq!(legacy_mint_count(&block), 0);
    set_weightmap(&programs::identity());
    // No claimant, so nothing to divide; must not panic or divide by zero.
    let txid = block.txdata[0].compute_txid();
    assert_eq!(
        effective_denominator(&block, 880_000, txid, 0, &[], &IndexPointer::default()),
        0
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn an_unknown_caller_claims_nothing() {
    // A transaction that is not in this block's table at all — e.g. some other
    // contract calling the precompile out of curiosity.
    clear();
    let block = block_of_mints(3);
    set_weightmap(&programs::identity());
    let stranger = create_block_with_coinbase_tx(999_999).txdata[0].compute_txid();
    assert_eq!(
        effective_denominator(&block, 880_000, stranger, 0, &[], &IndexPointer::default()),
        DENOMINATOR_NO_CLAIM
    );
}

// ---------------------------------------------------------------------------
// Totality
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn adversarial_weightmaps_never_panic() {
    clear();
    let block = block_of_mints(3);

    let cases: Vec<Vec<u8>> = vec![
        vec![],
        vec![0u8; 1],
        vec![0u8; 15],
        vec![0u8; 16],
        vec![0xff; 16],
        vec![0xff; 160],
        [u128::MAX.to_le_bytes(), 0u128.to_le_bytes()].concat(),
    ];
    for bytes in cases {
        set_weightmap_raw(bytes);
        let _ = denominator_for(&block, 1);
    }

    // Blobs that survive the magic check but are internally inconsistent: a
    // declared program length that does not match the words present, an unknown
    // mode, an out-of-range qualifier flag.
    let mut header = vec![
        alkanes_std_firevector::abi::WEIGHTMAP_MAGIC,
        alkanes_std_firevector::abi::WEIGHTMAP_VERSION,
        0, 0, 0, 0, 0, 0, 99,
    ];
    for (i, v) in [(2u128, 7u128), (3, 5), (8, u128::MAX)] {
        let mut words = header.clone();
        words[i as usize] = v;
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        set_weightmap_raw(bytes);
        let _ = denominator_for(&block, 1);
    }
    header[8] = 0;
    let bytes: Vec<u8> = header.iter().flat_map(|w| w.to_le_bytes()).collect();
    set_weightmap_raw(bytes);
    let _ = denominator_for(&block, 1);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_mode_selects_which_facts_are_legal() {
    clear();
    let block = block_of_mints(3);

    // A program reading realized incoming amounts is legal in RATE and rejected
    // in SPLIT, because SPLIT's weights come from a scan that runs before any
    // message executes. Rejection is deliberate: a vector reading a fact that
    // cannot exist is a bug in the vector, and governance should learn that at
    // the setter rather than discover it as silently-zero weights a block later.
    let p = vec![OP_INCOMING_AMOUNT, 32, 0];
    assert!(validate(&p, Mode::Rate).is_ok());
    assert!(validate(&p, Mode::Split).is_err());

    // Rejected in SPLIT -> falls back to the communist mint.
    set_weightmap(&p);
    assert_eq!(denominator_for(&block, 1), 3);
}


#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_memo_is_keyed_by_block_and_never_serves_a_stale_table() {
    // Regression test for the memo. It was originally unkeyed and relied on
    // `index_block` clearing it, which is true in production and not structural —
    // a reorg, `simulate_block`, or a view-path re-entry would all serve the
    // previous block's weights, and the symptom would be silently wrong emission
    // rather than a crash.
    //
    // This is also the failure the fixture originally masked: a block whose
    // header does not commit to its transactions hashes the same as any other.
    clear();
    set_weightmap(&programs::identity());

    let small = block_of_mints(3);
    let large = block_of_mints(9);
    assert_ne!(
        small.block_hash(),
        large.block_hash(),
        "fixture blocks must differ, or this test proves nothing"
    );

    // Query the small block first so its table is the one memoized.
    assert_eq!(denominator_for(&small, 1), 3);
    // The large block must NOT be answered from it.
    assert_eq!(denominator_for(&large, 1), 9);
    // And going back must not return the large block's table either.
    assert_eq!(denominator_for(&small, 1), 3);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_legacy_rule_is_expressible_end_to_end() {
    // The pre-upgrade DIESEL rule: first mint in the block takes everything,
    // everyone else gets nothing. Only expressible because the scan assigns
    // MINT_RANK in transaction order.
    clear();
    let block = block_of_mints(5);
    set_weightmap(&programs::legacy_winner_takes_all());

    let w = weights_of(&block);
    assert_eq!(w.total, 1, "exactly one winner");

    // The winner divides the emission by 1 — it takes the whole block.
    assert_eq!(denominator_for(&block, 1), 1);
    // Everyone else claims nothing.
    for i in 2..=5 {
        assert_eq!(
            denominator_for(&block, i),
            DENOMINATOR_NO_CLAIM,
            "mint {i} must not claim"
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn mint_rank_follows_transaction_order_and_skips_non_minters() {
    // Rank counts mint-bearing transactions only, so an unrelated transaction
    // sitting between two mints must not consume a rank.
    clear();
    let mut block = create_block_with_coinbase_tx(880_000);
    block.txdata.push(tx_with(vec![mint_cellpack()], 0)); // rank 0
    block.txdata.push(tx_with(
        vec![Cellpack {
            target: AlkaneId { block: 4, tx: 1778 },
            inputs: vec![1],
        }],
        1,
    )); // not a mint, no rank
    block.txdata.push(tx_with(vec![mint_cellpack()], 2)); // rank 1
    let block = finalize(block);

    set_weightmap(&programs::first_n_mints(2));
    let w = weights_of(&block);
    assert_eq!(w.total, 2, "both mints are within the first two");

    // Now only the first mint qualifies; the second must still be rank 1.
    set_weightmap(&programs::legacy_winner_takes_all());
    assert_eq!(weights_of(&block).total, 1);
    assert_eq!(denominator_for(&block, 1), 1);
    assert_eq!(denominator_for(&block, 3), DENOMINATOR_NO_CLAIM);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn first_n_mints_splits_between_exactly_n_winners() {
    clear();
    let block = block_of_mints(6);
    set_weightmap(&programs::first_n_mints(3));

    assert_eq!(weights_of(&block).total, 3);
    for i in 1..=3 {
        assert_eq!(denominator_for(&block, i), 3, "winner {i} splits three ways");
    }
    for i in 4..=6 {
        assert_eq!(denominator_for(&block, i), DENOMINATOR_NO_CLAIM);
    }
}

// ---------------------------------------------------------------------------
// Genesis deployment: 12:0 the contract, 12:1 the capability
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn genesis_deploys_the_vm_and_the_sigil() {
    clear();
    let block = finalize(create_block_with_coinbase_tx(880_000));
    crate::network::setup_firevector(&block, 880_000).expect("setup must succeed");

    for id in [FIREVECTOR_ID, AlkaneId { block: 12, tx: 1 }] {
        let code = IndexPointer::from_keyword("/alkanes/")
            .select(&<AlkaneId as Into<Vec<u8>>>::into(id))
            .get();
        assert!(!code.is_empty(), "{id:?} must have code deployed");
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_vm_is_a_contract_and_never_a_token() {
    // 12:0 must never have units. It is addressable so wallets can `simulate`
    // and anyone can `disassemble` the active policy — not because it is an
    // asset. `setup_firevector` hard-errors if initialize hands anything back,
    // so reaching here at all is part of the assertion; this pins the storage
    // side too.
    clear();
    let block = finalize(create_block_with_coinbase_tx(880_000));
    crate::network::setup_firevector(&block, 880_000).expect("setup must succeed");

    let supply = IndexPointer::from_keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(FIREVECTOR_ID))
        .keyword("/storage/")
        .select(&b"/totalsupply".to_vec())
        .get();
    assert!(
        supply.is_empty() || supply.iter().all(|b| *b == 0),
        "12:0 must have no supply, found {supply:?}"
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_sigil_has_exactly_one_unit_and_no_way_to_make_another() {
    clear();
    let block = finalize(create_block_with_coinbase_tx(880_000));
    crate::network::setup_firevector(&block, 880_000).expect("setup must succeed");

    let dsigil = AlkaneId { block: 12, tx: 1 };
    let supply = IndexPointer::from_keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(dsigil))
        .keyword("/storage/")
        .select(&b"/totalsupply".to_vec())
        .get();
    assert!(!supply.is_empty(), "DSIGIL must have recorded its supply");
    assert_eq!(
        u128::from_le_bytes(supply[0..16].try_into().unwrap()),
        1,
        "exactly one DSIGIL"
    );

    // And `initialize` is the only mint path, guarded by observe_initialization —
    // there is no `authenticate` opcode to inflate it, which is the whole reason
    // this contract exists rather than reusing the canonical auth token.
    let initialized = IndexPointer::from_keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(dsigil))
        .keyword("/storage/")
        .select(&b"/initialized".to_vec())
        .get();
    assert!(!initialized.is_empty(), "initialize must be latched");
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn genesis_seeds_the_identity_vector_so_emission_is_unchanged_from_block_one() {
    // The VM must never be live with a sigil but no policy. Seeding the identity
    // vector at init means the first block after deployment behaves exactly like
    // the last block before it.
    clear();
    let block = finalize(create_block_with_coinbase_tx(880_000));
    crate::network::setup_firevector(&block, 880_000).expect("setup must succeed");

    let stored = IndexPointer::from_keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(FIREVECTOR_ID))
        .keyword("/storage/")
        .select(&WEIGHTMAP_KEY.to_vec())
        .get();
    // The slot holds a fully packed weightmap, not a bare program. Decoding it
    // rather than comparing raw bytes is what makes this test notice a STALE
    // PRECOMPILED BLOB: `firevector_build.rs` is generated by hand from a wasm
    // build, nothing keeps it in sync with the contract source, and it was in
    // fact stale — genesis was seeding a pre-header weightmap while the source
    // had moved on. A byte comparison against the bare program passed against
    // the old blob and would have kept passing.
    let wm = alkanes_std_firevector::abi::unpack_weightmap(stored.as_ref())
        .expect("seeded weightmap must decode — if this fails, regenerate firevector_build.rs");
    assert_eq!(wm.mode, Mode::Split, "seed must be SPLIT");
    assert_eq!(wm.qualifier, None, "seed must qualify every mint");
    assert_eq!(wm.treasury_bps, 0, "seed must divert nothing to the treasury");
    assert_eq!(wm.program, programs::identity(), "seed must be the identity vector");

    // And it actually drives emission: a block of mints divides by the count.
    clear_weights_cache();
    let mints = block_of_mints(5);
    assert_eq!(denominator_for(&mints, 1), 5);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn setup_is_idempotent() {
    // index_block calls this every block; it must be a no-op after the first.
    clear();
    let block = finalize(create_block_with_coinbase_tx(880_000));
    crate::network::setup_firevector(&block, 880_000).expect("first setup");
    crate::network::setup_firevector(&block, 880_000).expect("second setup must be a no-op");
    crate::network::setup_firevector(&block, 880_000).expect("third setup must be a no-op");

    let dsigil = AlkaneId { block: 12, tx: 1 };
    let supply = IndexPointer::from_keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(dsigil))
        .keyword("/storage/")
        .select(&b"/totalsupply".to_vec())
        .get();
    assert_eq!(
        u128::from_le_bytes(supply[0..16].try_into().unwrap()),
        1,
        "repeated setup must not mint a second DSIGIL"
    );
}

// ---------------------------------------------------------------------------
// Scaling
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_block_is_scanned_once_per_transaction_not_once_per_claim() {
    // RATE used to rebuild the whole block per claim — a `compute_txid()` sweep
    // to find the caller, a re-scan of its protostones, and a second sweep for
    // its mint rank. Correctness tests could not see it: every answer stayed
    // right, it just went quadratic. Measured 0.36 ms/mint at 50 mints and
    // 2.23 ms/mint at 400, extrapolating to ~90-140s for the 4-5k-mint blocks
    // this chain actually sees, which makes reindexing impossible.
    //
    // Asserting the scan count rather than a wall-clock time keeps this
    // deterministic.
    use crate::firevector::{reset_scan_calls, scan_calls};

    let q = Qualifier { target_block: 4, target_tx: 1778, opcode: 1 };
    for mode in [Mode::Split, Mode::Rate] {
        for n in [10usize, 40, 160] {
            clear();
            let block = block_of_mints(n);
            set_weightmap_full(&Weightmap {
                mode,
                qualifier: Some(q),
                rate_floor: 1_000_000,
                treasury_bps: 0,
                program: programs::identity(),
            });
            clear_weights_cache();
            reset_scan_calls();

            for i in 1..=n {
                let _ = denominator_for(&block, i);
            }

            // One pass over the block: coinbase + n mint transactions.
            let calls = scan_calls();
            assert!(
                calls <= block.txdata.len(),
                "{mode:?} with {n} mints scanned {calls} times for {} transactions \
                 — the per-block memo is not being hit",
                block.txdata.len()
            );
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_memoized_legacy_count_agrees_with_a_naive_recount() {
    // `legacy_mint_count` is now a second, deliberately naive implementation
    // kept as an oracle: the memoized count is accumulated inside the scan loop,
    // and the two must not drift.
    clear();
    set_weightmap(&programs::identity());
    for n in [0usize, 1, 5, 30] {
        let block = block_of_mints(n);
        clear_weights_cache();
        let memoized = crate::firevector::compute_block_weights(
            &block,
            880_000,
            &IndexPointer::default(),
        )
        .legacy_count;
        assert_eq!(memoized, legacy_mint_count(&block), "n={n}");
        assert_eq!(memoized, n as u128, "n={n}");
    }
}

// ---------------------------------------------------------------------------
// Treasury routing: "communist mint, but far less of it"
// ---------------------------------------------------------------------------
// The excess does not vanish and does not accumulate — it is minted to the
// DIESEL treasury via the mechanism that already exists. The contract computes
// `diesel_fee = min(block_reward/2, reported - block_reward)` from what
// `800000000:3` reports, credits it to `/fees` in `observe_upgraded_mint`, and
// `collect_fees` (opcode 78) drains it. FIREVECTOR only sets `reported`.

use crate::firevector::{block_reward, effective_miner_fee, reported_miner_fee};

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn block_reward_matches_the_contract() {
    // A deliberate mirror of ChainConfiguration::block_reward, which the indexer
    // cannot call. If these drift, every treasury target is computed against the
    // wrong base.
    for (h, want) in [
        (0u64, 5_000_000_000u128),
        (209_999, 5_000_000_000),
        (210_000, 2_500_000_000),
        (839_999, 625_000_000),
        (840_000, 312_500_000),
        (1_049_999, 312_500_000),
        (1_050_000, 156_250_000),
    ] {
        assert_eq!(block_reward(h), want, "height {h}");
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn zero_treasury_bps_reports_exactly_what_is_reported_today() {
    // The no-op property, over the whole interesting range of coinbase totals:
    // below the subsidy, at it, and far above it. `max(true, reward + 0)` must
    // yield the same `total_tx_fee` the contract derives today for every input.
    let h = 880_000u64;
    let reward = block_reward(h);
    for true_total in [0u128, 1, reward - 1, reward, reward + 1, reward * 2, reward * 100] {
        let reported = reported_miner_fee(true_total, h, 0);
        let today_fee = true_total.saturating_sub(reward);
        let new_fee = reported.saturating_sub(reward);
        assert_eq!(
            today_fee.min(reward / 2),
            new_fee.min(reward / 2),
            "true_total={true_total}: diesel_fee must be unchanged at 0 bps"
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn treasury_bps_sets_the_fee_the_contract_will_compute() {
    let h = 880_000u64;
    let reward = block_reward(h);
    for bps in [1u128, 100, 2500, 5000] {
        let reported = reported_miner_fee(0, h, bps);
        let derived_fee = reported.saturating_sub(reward).min(reward / 2);
        assert_eq!(
            derived_fee,
            reward * bps / 10_000,
            "bps={bps} must land exactly on its share of the reward"
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_treasury_share_is_a_floor_not_a_replacement() {
    // A genuinely high-fee block must still credit its real fees. Overriding
    // downward would quietly take revenue away from the protocol on exactly the
    // blocks where there is most of it.
    let h = 880_000u64;
    let reward = block_reward(h);
    let rich = reward * 10;
    assert_eq!(reported_miner_fee(rich, h, 0), rich);
    assert_eq!(reported_miner_fee(rich, h, 2500), rich);
    // And below the subsidy the floor is what applies.
    assert_eq!(
        reported_miner_fee(0, h, 2500),
        reward + reward * 2500 / 10_000
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_treasury_share_can_never_exceed_half_the_reward() {
    // The contract clamps at block_reward/2 regardless, so the setter rejects
    // anything above 5000 bps rather than letting a voter write 8000 and quietly
    // get 50%.
    use alkanes_std_firevector::abi::{pack_weightmap, unpack_weightmap, MAX_TREASURY_BPS};
    let mut wm = Weightmap {
        mode: Mode::Split,
        qualifier: None,
        rate_floor: 0,
        treasury_bps: MAX_TREASURY_BPS,
        program: programs::identity(),
    };
    assert!(unpack_weightmap(&pack_weightmap(&wm)).is_some());
    wm.treasury_bps = MAX_TREASURY_BPS + 1;
    assert!(
        unpack_weightmap(&pack_weightmap(&wm)).is_none(),
        "a weightmap above the contract's own cap must not decode"
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn treasury_routing_is_inert_without_a_weightmap_and_below_the_fork() {
    clear();
    let block = block_of_mints(3);
    let h = 880_000u64;
    let true_total = block_reward(h) * 3;

    // No weightmap at all.
    assert_eq!(
        effective_miner_fee(&block, h, true_total, &IndexPointer::default()),
        true_total
    );

    // A weightmap that sets no treasury share.
    set_weightmap(&programs::identity());
    assert_eq!(
        effective_miner_fee(&block, h, true_total, &IndexPointer::default()),
        true_total
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn treasury_routing_leaves_the_split_among_minters_untouched() {
    // Diverting to the treasury changes the NUMERATOR the contract divides, not
    // the denominator FIREVECTOR returns. Minters still split whatever is left
    // equally, so the two mechanisms compose without interfering.
    clear();
    let block = block_of_mints(4);
    set_weightmap_full(&Weightmap {
        mode: Mode::Split,
        qualifier: None,
        rate_floor: 0,
        treasury_bps: 5000,
        program: programs::identity(),
    });
    for i in 1..=4 {
        assert_eq!(denominator_for(&block, i), 4, "still an equal 4-way split");
    }
}

// ---------------------------------------------------------------------------
// The two capabilities: DSIGIL claims the orbital, the orbital sets the vector
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_orbital_is_held_by_the_vm_not_by_an_outpoint() {
    // Genesis mints 12:1 into 12:0's own balance rather than onto the genesis
    // outpoint. Two reasons: the outpoint already holds the DIESEL premine and
    // rewriting that chunk is what once dropped it, and an alkane balance is
    // lazily materialized so nothing needs reindexing. It also gives the
    // capability a defined release path instead of belonging to whoever spends a
    // historic UTXO.
    clear();
    let block = finalize(create_block_with_coinbase_tx(880_000));
    crate::network::setup_firevector(&block, 880_000).expect("setup");

    let held = IndexPointer::from_keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(FV_ORBITAL_ID))
        .keyword("/balances/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(FIREVECTOR_ID))
        .get_value::<u128>();
    assert_eq!(held, 1, "12:0 must hold exactly one 12:1 orbital");
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_vm_records_both_authorities_separately() {
    // The emission-policy key (12:1) and the key that can retrieve it (DSIGIL)
    // are different tokens on purpose. DSIGIL keeps the DIESEL treasury
    // permission it already has; the orbital only ever governs the vector.
    clear();
    let block = finalize(create_block_with_coinbase_tx(880_000));
    crate::network::setup_firevector(&block, 880_000).expect("setup");

    let read = |key: &[u8]| {
        IndexPointer::from_keyword("/alkanes/")
            .select(&<AlkaneId as Into<Vec<u8>>>::into(FIREVECTOR_ID))
            .keyword("/storage/")
            .select(&key.to_vec())
            .get()
            .as_ref()
            .clone()
    };
    let id_of = |b: Vec<u8>| AlkaneId {
        block: u128::from_le_bytes(b[0..16].try_into().unwrap()),
        tx: u128::from_le_bytes(b[16..32].try_into().unwrap()),
    };

    assert_eq!(id_of(read(b"/sigil")), FV_ORBITAL_ID, "vector authority is 12:1");
    assert_eq!(
        id_of(read(b"/claim_authority")),
        DSIGIL_ID,
        "claim authority is DSIGIL"
    );
    assert_ne!(
        FV_ORBITAL_ID, DSIGIL_ID,
        "the two capabilities must not be the same token"
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn an_unset_dsigil_leaves_the_orbital_locked() {
    // DSIGIL_ID is 0:0 until its real on-chain id is filled in. That must mean
    // "nobody can claim", not "anybody can" — shipping unconfigured has to fail
    // closed, because the thing being handed out is the emission-policy key.
    assert_eq!(
        DSIGIL_ID,
        AlkaneId { block: 0, tx: 0 },
        "if this has been set, update this test and the deployment checklist"
    );
}

// ---------------------------------------------------------------------------
// Retroactive equivalence from the DIESEL v2 upgrade
// ---------------------------------------------------------------------------

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_legacy_mint_path_never_consults_firevector() {
    // The reason activating retroactively is safe. `create_mint_transfer` — the
    // pre-upgrade winner-takes-all path — uses only `observe_mint()` and
    // `current_block_reward()`. It asks for neither the mint count nor the miner
    // fee, so below the v2 upgrade the contract does not consult FIREVECTOR at
    // all and nothing FIREVECTOR does can change historical emission.
    //
    // Asserted against the contract source so it fails if that path ever starts
    // calling a precompile.
    let src = include_str!(
        "../../../../alkanes/alkanes-std-genesis-alkane-upgraded-eoa/src/lib.rs"
    );
    let start = src
        .find("pub fn create_mint_transfer")
        .expect("legacy mint path must exist");
    let body = &src[start..start + 900];
    let end = body.find("\n    }").expect("function must terminate");
    let body = &body[..end];

    assert!(
        !body.contains("number_diesel_mints"),
        "legacy path must not consult 800000000:2"
    );
    assert!(
        !body.contains("total_miner_fee"),
        "legacy path must not consult 800000000:3"
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn activation_sits_at_the_v2_upgrade_and_both_precompiles_are_inert_below_it() {
    // Deploy and fork are both the v2 upgrade height, and deploy must not be
    // later than fork or the vector would be consulted before it exists.
    assert!(
        crate::network::genesis::FIREVECTOR_DEPLOY_HEIGHT
            <= crate::network::genesis::FIREVECTOR_FORK_HEIGHT,
        "code must exist before the denominator changes meaning"
    );

    clear();
    let block = block_of_mints(5);
    set_weightmap(&programs::identity());
    clear_weights_cache();

    let below = crate::network::genesis::FIREVECTOR_FORK_HEIGHT as u64;
    if below > 0 {
        let h = below - 1;
        let txid = block.txdata[1].compute_txid();
        // Below the fork the denominator is the plain mint count...
        assert_eq!(
            effective_denominator(&block, h, txid, 0, &[], &IndexPointer::default()),
            legacy_mint_count(&block)
        );
        // ...and the reported miner fee is the true coinbase total, untouched.
        assert_eq!(
            effective_miner_fee(&block, h, 123_456, &IndexPointer::default()),
            123_456
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_identity_vector_is_idempotent_across_the_activation_boundary() {
    // "Equivalent for every transaction since the v2 upgrade": at and above the
    // fork, with the seeded identity vector, the denominator is the same mint
    // count the old precompile returned — so every payout, and therefore every
    // fuel charge in the unmodified contract, is unchanged.
    clear();
    set_weightmap(&programs::identity());
    let fork = crate::network::genesis::FIREVECTOR_FORK_HEIGHT as u64;

    for n in [1usize, 3, 20] {
        let block = block_of_mints(n);
        for h in [fork, fork + 1, fork + 5_000] {
            clear_weights_cache();
            for i in 1..=n {
                let txid = block.txdata[i].compute_txid();
                assert_eq!(
                    effective_denominator(&block, h, txid, 0, &[], &IndexPointer::default()),
                    n as u128,
                    "n={n} h={h}"
                );
            }
            assert_eq!(
                effective_miner_fee(&block, h, 999, &IndexPointer::default()),
                999,
                "identity vector must not divert anything to the treasury"
            );
        }
    }
}
