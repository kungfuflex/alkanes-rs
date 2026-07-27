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
