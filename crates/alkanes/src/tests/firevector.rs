//! Indexer-side FIREVECTOR tests.
//!
//! The claim under test is narrow and load-bearing: with the identity vector
//! loaded, `800000000:2` returns exactly what it returns today, so activation
//! changes nothing — not the payout, and not the fuel, because the DIESEL
//! contract is untouched and the reply stays 16 bytes.
//!
//! Everything else here is about the ways that claim could quietly stop being
//! true.

use crate::firevector::{
    clear_weights_cache, compute_block_weights, effective_denominator, legacy_mint_count,
    DENOMINATOR_NO_CLAIM, DIESEL_ID, DIESEL_MINT_OPCODE, WEIGHTMAP_KEY, WEIGHTMAP_SOURCE_KEY,
};
use crate::tests::helpers::{self as alkane_helpers, clear};
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

/// Write a weightmap into DIESEL's contract storage, exactly where a governance
/// setter on 2:0 would put it.
fn set_weightmap(program: &[u128], source_byte: u8) {
    let mut bytes = Vec::with_capacity(program.len() * 16);
    for w in program {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    set_weightmap_raw(bytes, source_byte);
}

fn set_weightmap_raw(bytes: Vec<u8>, source_byte: u8) {
    let base = IndexPointer::default()
        .keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(DIESEL_ID))
        .keyword("/storage/");
    base.select(&WEIGHTMAP_KEY.to_vec()).set(Arc::new(bytes));
    base.select(&WEIGHTMAP_SOURCE_KEY.to_vec())
        .set(Arc::new(vec![source_byte]));
    clear_weights_cache();
}

fn denominator_for(block: &Block, txindex: usize) -> u128 {
    let txid = block.txdata[txindex].compute_txid();
    effective_denominator(block, 880_000, txid, &IndexPointer::default())
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

        set_weightmap(&programs::identity(), 0);
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
    set_weightmap(&programs::identity(), 0);
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
    set_weightmap_raw(vec![0u8; 17], 0);
    assert_eq!(denominator_for(&block, 1), 4);

    // Empty.
    set_weightmap_raw(vec![], 0);
    assert_eq!(denominator_for(&block, 1), 4);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn an_invalid_program_falls_back_to_the_legacy_count() {
    clear();
    let block = block_of_mints(4);
    // ADD with nothing on the stack — fails validation.
    set_weightmap(&[OP_ADD], 0);
    assert_eq!(denominator_for(&block, 1), 4);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn a_program_reading_unavailable_facts_falls_back() {
    clear();
    let block = block_of_mints(4);
    // STATUS is not knowable in a same-block scan; the program is rejected for
    // this source rather than silently reading zero.
    set_weightmap(&[OP_STATUS], 0);
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
    assert!(validate(&other_app, Source::SameBlock).is_ok());
    set_weightmap(&other_app, 0);
    assert_eq!(denominator_for(&block, 1), 6);
}

// ---------------------------------------------------------------------------
// Non-uniform weights, and the cache bug this design had to avoid
// ---------------------------------------------------------------------------

/// Weight a mint by its protostone index, so that transactions in the same block
/// get deliberately different weights.
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

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn different_transactions_get_different_denominators() {
    // THE regression test for the cache bug. The old precompile memoized the
    // reply *bytes*, which is fine while the answer is block-global and
    // catastrophic once it is per-transaction: the first mint would populate the
    // cache and every later mint would be handed the first one's denominator.
    clear();
    let block = block_of_mints(3); // txindex 1, 2, 3
    set_weightmap(&weight_by_txindex(), 0);

    let w = weights_of(&block);
    assert_eq!(w.total, 1 + 2 + 3);

    let d1 = denominator_for(&block, 1);
    let d2 = denominator_for(&block, 2);
    let d3 = denominator_for(&block, 3);

    // W / w_i
    assert_eq!(d1, 6 / 1);
    assert_eq!(d2, 6 / 2);
    assert_eq!(d3, 6 / 3);
    assert!(
        d1 != d2 && d2 != d3,
        "per-transaction answers must not collapse to one cached value: {d1} {d2} {d3}"
    );
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_memo_survives_repeated_queries_without_drifting() {
    clear();
    let block = block_of_mints(3);
    set_weightmap(&weight_by_txindex(), 0);

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
    set_weightmap(&p, 0);

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

    set_weightmap(&programs::identity(), 0);
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
    set_weightmap(&programs::identity(), 0);
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
    set_weightmap(&programs::identity(), 0);
    assert_eq!(denominator_for(&block, 1), 1);
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn an_empty_block_yields_a_zero_count_without_panicking() {
    clear();
    let block = finalize(create_block_with_coinbase_tx(880_000));
    assert_eq!(legacy_mint_count(&block), 0);
    set_weightmap(&programs::identity(), 0);
    // No claimant, so nothing to divide; must not panic or divide by zero.
    let txid = block.txdata[0].compute_txid();
    assert_eq!(
        effective_denominator(&block, 880_000, txid, &IndexPointer::default()),
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
    set_weightmap(&programs::identity(), 0);
    let stranger = create_block_with_coinbase_tx(999_999).txdata[0].compute_txid();
    assert_eq!(
        effective_denominator(&block, 880_000, stranger, &IndexPointer::default()),
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
        set_weightmap_raw(bytes.clone(), 0);
        let _ = denominator_for(&block, 1);
        set_weightmap_raw(bytes, 1);
        let _ = denominator_for(&block, 1);
    }
}

#[cfg_attr(not(target_arch = "wasm32"), test)]
#[wasm_bindgen_test]
fn the_source_byte_selects_which_facts_are_legal() {
    clear();
    let block = block_of_mints(3);

    // A prev-block program reading STATUS is legal for source=1 but rejected for
    // source=0. It weighs nothing here either way (the scan cannot set status),
    // so both end up at the legacy count — the point is that neither panics and
    // neither silently mis-evaluates.
    let p = vec![
        OP_TARGET_BLOCK, OP_PUSH, 2, OP_EQ,
        OP_STATUS, OP_AND,
    ];
    assert!(validate(&p, Source::PrevBlock).is_ok());
    assert!(validate(&p, Source::SameBlock).is_err());

    set_weightmap(&p, 0);
    assert_eq!(denominator_for(&block, 1), 3);
    set_weightmap(&p, 1);
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
    set_weightmap(&programs::identity(), 0);

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
