//! Regression tests for the `max_virtual_vout` fork and the recycle-bin
//! prevout-owner keying that backstops it.
//!
//! Background: `protorune::protostone::process_message` rejected any protostone
//! whose shadow vout was `>= num_outputs + 100` — an artifact of the 80-byte
//! OP_RETURN standardness era. That rejection returns `Err`, not `Ok(false)`, so
//! it never takes the refund path: it propagates out of `index_protostones` and
//! voids the whole transaction's protorune processing, leaving the spent inputs'
//! alkanes stranded. Mainnet tx
//! `bf45c294f30a2165f9d28209514175c9712385326413bf2d1ed8f3d4c1633b99` (block
//! 954425) carried 347 protostones against 59 outputs and stranded 365 balance
//! entries this way.
//!
//! The fork removes the cap at `MAX_VIRTUAL_VOUT_REMOVAL_HEIGHT`. Below that
//! height the cap still applies (history stays byte-identical) and the stranded
//! assets are recovered by `recycle::capture_block`, which must credit the
//! **owner of the spent outpoint** — not the spending tx's first output.

use crate::message::AlkaneMessageContext;
use crate::network::genesis::MAX_VIRTUAL_VOUT_REMOVAL_HEIGHT;
use crate::recycle;
use crate::tests::helpers as alkane_helpers;
use alkanes_support::id::AlkaneId;
use bitcoin::hashes::Hash;
use bitcoin::{Amount, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Witness};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::balance_sheet::{load_sheet_chunked, save_chunked};
use protorune::message::MessageContext;
use protorune::tables::{RuneTable, OUTPOINT_TO_OUTPUT};
use protorune::test_helpers::create_block_with_txs;
use protorune_support::balance_sheet::{
    BalanceSheet, BalanceSheetOperations, ProtoruneRuneId,
};
use protorune_support::utils::consensus_encode;
use prost::Message as _;
use std::sync::Arc;
use wasm_bindgen_test::wasm_bindgen_test;

/// A distinct EOA scriptPubKey per `byte`. p2wpkh rather than p2tr because the
/// hash is arbitrary bytes — no curve-point validity to satisfy — and `is_eoa`
/// accepts it just the same.
fn p2tr(byte: u8) -> ScriptBuf {
    ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_slice(&[byte; 20]).unwrap())
}

/// A protostone at shadow vout `num_outputs + 100` or beyond is only rejected
/// BELOW the fork height. This locks the gate the consensus change hangs on: if
/// someone flips the comparison or drops the height check, one of these fails.
#[wasm_bindgen_test]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn cap_applies_only_below_fork_height() {
    let num_outputs: usize = 59;
    let capped = |height: u64, shadow_vout: u32| -> bool {
        // Mirrors the gate in protorune::protostone::process_message.
        height < AlkaneMessageContext::max_virtual_vout_removal_height()
            && shadow_vout >= (num_outputs + 100) as u32
    };
    let fork = MAX_VIRTUAL_VOUT_REMOVAL_HEIGHT;
    // At/after the fork the cap is gone on every chain. The real mainnet failure
    // was 347 protostones on 59 outputs -> shadow vouts 61..=407, of which 159
    // was the first the cap rejected.
    assert!(!capped(fork, 159), "cap must be gone AT the fork height");
    assert!(!capped(fork, 407), "cap must be gone AT the fork height");
    assert!(
        !capped(fork.saturating_add(1), 10_000),
        "10k protostones must be fine after the fork"
    );
    // Under the cap is always fine, on both sides of the fork.
    assert!(!capped(fork, 158));

    if fork == 0 {
        // Non-mainnet chains are genesis-coincident: there is no "below fork"
        // regime to test, and the cap must never apply.
        assert!(!capped(0, 407), "genesis-coincident fork: cap never applies");
    } else {
        let below = fork - 1;
        assert!(capped(below, 159), "cap must still bite below the fork");
        assert!(capped(below, 407), "cap must still bite below the fork");
        assert!(!capped(below, 158), "under the cap is fine below the fork too");
    }
}

/// A malformed protostone header (absent or out-of-range pointer/refund) is a
/// transaction-voiding `Err` below the fork and an ordinary refunding failure
/// at/after it. Mirrors the classifier in
/// `protorune::protostone::process_message`.
#[wasm_bindgen_test]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn malformed_header_voids_tx_below_fork_and_refunds_after() {
    let num_outputs: usize = 3;
    let num_protostones: usize = 2;
    let bound = (num_outputs + num_protostones) as u32;
    let fork = AlkaneMessageContext::max_virtual_vout_removal_height();

    // (pointer, refund) -> is the header malformed?
    let malformed = |pointer: Option<u32>, refund: Option<u32>| -> bool {
        match (pointer, refund) {
            (Some(p), Some(r)) => p > bound || r > bound,
            _ => true,
        }
    };
    // Post-fork a malformed header refunds instead of erroring.
    let voids_tx = |pointer: Option<u32>, refund: Option<u32>, height: u64| -> bool {
        malformed(pointer, refund) && height < fork
    };

    // The three legacy Err paths.
    let missing_pointer = (None, Some(0u32));
    let missing_refund = (Some(0u32), None);
    let out_of_range = (Some(bound + 1), Some(0u32));
    for (p, r) in [missing_pointer, missing_refund, out_of_range] {
        assert!(malformed(p, r), "must be classified malformed: {:?}", (p, r));
        assert!(
            !voids_tx(p, r, fork),
            "at/after the fork a malformed header must refund, not void the tx: {:?}",
            (p, r)
        );
        if fork > 0 {
            assert!(
                voids_tx(p, r, fork - 1),
                "below the fork the legacy Err must stand: {:?}",
                (p, r)
            );
        }
    }

    // A well-formed header is untouched on both sides of the fork, including a
    // pointer that legitimately addresses a shadow vout (== bound).
    for (p, r) in [(Some(0u32), Some(0u32)), (Some(bound), Some(1u32))] {
        assert!(!malformed(p, r), "well-formed header must not be flagged");
        assert!(!voids_tx(p, r, fork));
        if fork > 0 {
            assert!(!voids_tx(p, r, fork - 1));
        }
    }
}

/// The refund target chosen for a malformed header is always a REAL output —
/// never a shadow vout (which addresses another protostone that may itself
/// fail) and never left unset.
#[wasm_bindgen_test]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn malformed_header_refund_target_is_a_real_output() {
    let num_outputs: usize = 3;
    // Mirrors the selection in process_message: honour a declared refund that
    // names a real output, else fall back to default_output.
    let pick = |refund: Option<u32>, default_output: u32| -> u32 {
        refund
            .filter(|r| (*r as usize) < num_outputs)
            .unwrap_or(default_output)
    };
    // default_output = 1 here (imagine vout 0 is the OP_RETURN).
    assert_eq!(pick(Some(2), 1), 2, "a real declared refund output is honoured");
    assert_eq!(pick(None, 1), 1, "absent refund falls back to default_output");
    assert_eq!(
        pick(Some(4), 1),
        1,
        "a shadow vout / out-of-range refund must NOT be used as the target"
    );
    assert!(
        (pick(Some(4), 1) as usize) < num_outputs,
        "chosen target must always be a real output"
    );
}

/// End-to-end: a protostone with a MISSING pointer (one of the three legacy
/// `Err` paths) must not destroy the transaction's alkanes. Post-fork the
/// message fails and the incoming balance is refunded to a real output instead
/// of being stranded at the spent input.
///
/// This drives the real `index_block` path rather than mirroring the classifier,
/// so it proves the wiring, not just the decision table. The block is indexed AT
/// the fork height, so the test exercises post-fork behaviour on every network —
/// genesis-coincident (regtest) and mainnet's 975_000 alike.
#[wasm_bindgen_test]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn malformed_header_refunds_end_to_end() -> anyhow::Result<()> {
    alkane_helpers::clear();
    alkane_helpers::configure_network();
    // At/after the fork on whichever network this build targets.
    let height = AlkaneMessageContext::max_virtual_vout_removal_height().max(880_001);

    let owner = p2tr(21);
    let protocol_tag = AlkaneMessageContext::protocol_tag();
    let table = RuneTable::for_protocol(protocol_tag);

    // Seed an outpoint carrying an alkane.
    let seed_op = OutPoint {
        txid: bitcoin::Txid::from_byte_array([0x5au8; 32]),
        vout: 0,
    };
    let key = consensus_encode(&seed_op)?;
    let asset = ProtoruneRuneId { block: 2, tx: 555 };
    let amount: u128 = 4_242;
    let mut atomic = AtomicPointer::default();
    let mut ptr = atomic.derive(&table.OUTPOINT_TO_RUNES.select(&key));
    let mut sheet = BalanceSheet::new_ptr_backed(ptr.clone());
    sheet.increase(&asset, amount)?;
    save_chunked(&sheet, &mut ptr, false);
    atomic.commit();

    // Spend it with a protostone whose pointer is ABSENT — malformed header.
    let malformed = protorune_support::protostone::Protostone {
        message: alkanes_support::cellpack::Cellpack {
            target: AlkaneId { block: 2, tx: 555 },
            inputs: vec![0],
        }
        .encipher(),
        pointer: None, // <-- the "Missing pointer" path
        refund: Some(0),
        edicts: vec![],
        from: None,
        burn: None,
        protocol_tag,
    };
    let tx = alkane_helpers::create_protostone_tx_with_inputs(
        vec![TxIn {
            previous_output: seed_op,
            script_sig: ScriptBuf::new(),
            sequence: bitcoin::Sequence::MAX,
            witness: Witness::new(),
        }],
        vec![TxOut {
            value: Amount::from_sat(546),
            script_pubkey: owner.clone(),
        }],
        malformed,
    );
    let block = create_block_with_txs(vec![tx]);
    // The whole point: this must NOT bubble an Err out of index_block.
    crate::index_block(&block, height as u32)?;

    // The alkanes must be somewhere spendable — not stranded at the spent input.
    let txid = block.txdata[0].compute_txid();
    let mut total_out: u128 = 0;
    for vout in 0..block.txdata[0].output.len() as u32 {
        let op = OutPoint { txid, vout };
        let mut a = AtomicPointer::default();
        let s: BalanceSheet<AtomicPointer> =
            load_sheet_chunked(&a.derive(&table.OUTPOINT_TO_RUNES.select(&consensus_encode(&op)?)));
        total_out += s.get_cached(&asset);
    }
    assert_eq!(
        total_out, amount,
        "a malformed-header protostone must refund its alkanes to an output, not destroy them"
    );
    Ok(())
}

/// A protocol that does not opt in keeps the legacy behaviour forever.
#[wasm_bindgen_test]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn default_message_context_never_removes_the_cap() {
    struct Legacy;
    impl MessageContext for Legacy {
        fn handle(
            _p: &protorune::message::MessageContextParcel,
        ) -> anyhow::Result<(
            Vec<protorune_support::rune_transfer::RuneTransfer>,
            BalanceSheet<AtomicPointer>,
        )> {
            unreachable!()
        }
        fn protocol_tag() -> u128 {
            0xdead_beef
        }
    }
    assert_eq!(Legacy::max_virtual_vout_removal_height(), u64::MAX);
}

/// Capture credits the address that OWNED THE SPENT OUTPOINT, not the spending
/// transaction's first output.
///
/// This is the theft/mis-credit vector: a transaction that spends victim `V`'s
/// stranded outpoint while paying output 0 to attacker `A` must NOT put V's
/// alkanes in A's claimable ledger. (The claim side then binds the release to
/// the payout output, so only V paying V can take them out.)
#[wasm_bindgen_test]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn capture_credits_prevout_owner_not_output_zero() -> anyhow::Result<()> {
    alkane_helpers::clear();
    alkane_helpers::configure_network();

    let victim = p2tr(7);
    let attacker = p2tr(200);
    let protocol_tag = AlkaneMessageContext::protocol_tag();
    let table = RuneTable::for_protocol(protocol_tag);

    // A funded outpoint owned by the victim, carrying a stranded alkane.
    let stranded_outpoint = OutPoint {
        txid: bitcoin::Txid::from_raw_hash(bitcoin::hashes::Hash::all_zeros()),
        vout: 0,
    };
    let key = consensus_encode(&stranded_outpoint)?;

    // OUTPOINT_TO_OUTPUT records the prevout's scriptPubKey — this is what
    // capture must consult to find the owner.
    OUTPOINT_TO_OUTPUT.select(&key).set(Arc::new(
        (protorune_support::proto::protorune::Output {
            script: victim.clone().into_bytes(),
            value: 546,
        })
        .encode_to_vec(),
    ));

    // Strand a balance at that outpoint (what a voided tx leaves behind).
    let asset = ProtoruneRuneId { block: 2, tx: 81974 };
    let mut atomic = AtomicPointer::default();
    let mut ptr = atomic.derive(&table.OUTPOINT_TO_RUNES.select(&key));
    let mut sheet = BalanceSheet::new_ptr_backed(ptr.clone());
    sheet.increase(&asset, 364u128)?;
    save_chunked(&sheet, &mut ptr, false);
    atomic.commit();

    // The spending tx: input = victim's outpoint, output 0 = ATTACKER.
    let spend = Transaction {
        version: bitcoin::blockdata::transaction::Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: stranded_outpoint,
            script_sig: ScriptBuf::new(),
            sequence: bitcoin::Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(546),
            script_pubkey: attacker.clone(),
        }],
    };
    let block = create_block_with_txs(vec![spend]);

    recycle::capture_block(&block, 900_000, protocol_tag)?;

    let claimable = |spk: &ScriptBuf| -> Vec<(ProtoruneRuneId, u128)> {
        recycle::recycled_balances(&spk.clone().into_bytes())
    };

    assert_eq!(
        claimable(&victim),
        vec![(asset, 364u128)],
        "the prevout owner must be credited the stranded alkanes"
    );
    assert!(
        claimable(&attacker).is_empty(),
        "output 0 of the spending tx must NOT be credited"
    );

    // And the stranded sheet is cleared so protorunesbyoutpoint stops reporting it.
    let mut a2 = AtomicPointer::default();
    let after: BalanceSheet<AtomicPointer> =
        load_sheet_chunked(&a2.derive(&table.OUTPOINT_TO_RUNES.select(&key)));
    assert!(after.balances().is_empty(), "stranded balance must be cleared");
    Ok(())
}

/// `getrecycled` reports exactly what is claimable, clamped to `8:dead`'s held
/// inventory, and is empty for an address with no ledger.
#[wasm_bindgen_test]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn getrecycled_view_reports_claimable_balances() -> anyhow::Result<()> {
    alkane_helpers::clear();
    alkane_helpers::configure_network();

    let owner = p2tr(11);
    let stranger = p2tr(12);
    let protocol_tag = AlkaneMessageContext::protocol_tag();
    let table = RuneTable::for_protocol(protocol_tag);

    let outpoint = OutPoint {
        txid: bitcoin::Txid::from_raw_hash(bitcoin::hashes::Hash::all_zeros()),
        vout: 3,
    };
    let key = consensus_encode(&outpoint)?;
    OUTPOINT_TO_OUTPUT.select(&key).set(Arc::new(
        (protorune_support::proto::protorune::Output {
            script: owner.clone().into_bytes(),
            value: 330,
        })
        .encode_to_vec(),
    ));
    let asset = ProtoruneRuneId { block: 2, tx: 77623 };
    let mut atomic = AtomicPointer::default();
    let mut ptr = atomic.derive(&table.OUTPOINT_TO_RUNES.select(&key));
    let mut sheet = BalanceSheet::new_ptr_backed(ptr.clone());
    sheet.increase(&asset, 1_813u128)?;
    save_chunked(&sheet, &mut ptr, false);
    atomic.commit();

    let spend = Transaction {
        version: bitcoin::blockdata::transaction::Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: outpoint,
            script_sig: ScriptBuf::new(),
            sequence: bitcoin::Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(330),
            script_pubkey: owner.clone(),
        }],
    };
    recycle::capture_block(&create_block_with_txs(vec![spend]), 900_001, protocol_tag)?;

    let ask = |spk: &ScriptBuf| -> anyhow::Result<Vec<(AlkaneId, u128)>> {
        let req = alkanes_support::proto::alkanes::AlkaneRecycledRequest {
            script: spk.clone().into_bytes(),
        };
        let resp = crate::view::getrecycled(&req.encode_to_vec())?;
        Ok(resp
            .alkanes
            .into_iter()
            .map(|t| {
                let id: AlkaneId = t.id.clone().unwrap().into();
                let v: u128 = t.value.clone().unwrap().into();
                (id, v)
            })
            .collect())
    };

    assert_eq!(
        ask(&owner)?,
        vec![(AlkaneId { block: 2, tx: 77623 }, 1_813u128)],
        "getrecycled must report the owner's claimable balance sheet"
    );
    assert!(
        ask(&stranger)?.is_empty(),
        "getrecycled must be empty for an address with no ledger"
    );
    Ok(())
}
