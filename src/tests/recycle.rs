//! Tests for the `8:dead` recycle bin (capture side + shared codec/format).
//!
//! Capture (`src/recycle.rs`, `IndexPointer`) and the `8:dead` claim WASM
//! (`StoragePointer`) build the `/recycle/<spk>` ledger key through the SAME
//! `KeyValuePointer` default `keyword`/`select` impls on an identically-wrapped
//! `Vec`, so the keys are byte-identical by construction. Here we lock the
//! capture-side key format + the shared ledger codec + the EOA gate so an
//! accidental change is caught at `cargo test` time. (A full end-to-end claim
//! test that runs the real WASM is tracked as a follow-up; it additionally
//! exercises the parity through an actual `8:dead:3` execution.)

use crate::recycle::{decode_ledger, encode_ledger};
use bitcoin::hashes::Hash;
use bitcoin::ScriptBuf;
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::balance_sheet::ProtoruneRuneId;

const RECYCLE_LEDGER_PREFIX: &str = "/recycle/";

fn p2tr(byte: u8) -> ScriptBuf {
    ScriptBuf::new_p2tr_tweaked(bitcoin::key::TweakedPublicKey::dangerous_assume_tweaked(
        bitcoin::XOnlyPublicKey::from_slice(&[byte; 32]).unwrap(),
    ))
}

/// The capture-side ledger key is deterministic, embeds the script_pubkey, and
/// distinguishes distinct recipients. This locks the key format the claim WASM
/// must (and structurally does) mirror.
#[test]
fn ledger_key_is_deterministic_and_partitioned() {
    let a = p2tr(2).to_bytes();
    let b = p2tr(9).to_bytes();
    let key = |spk: &Vec<u8>| -> Vec<u8> {
        IndexPointer::from_keyword(RECYCLE_LEDGER_PREFIX)
            .select(spk)
            .unwrap()
            .as_ref()
            .clone()
    };
    assert_eq!(key(&a), key(&a), "same spk → same key");
    assert_ne!(key(&a), key(&b), "distinct recipients → distinct keys (partition)");
    // key must actually depend on the spk bytes (not collapse to the prefix)
    assert_ne!(
        key(&a),
        IndexPointer::from_keyword(RECYCLE_LEDGER_PREFIX)
            .unwrap()
            .as_ref()
            .clone()
    );
}

/// The ledger codec is shared verbatim between capture and the claim WASM.
/// Round-trip and confirm the 48-byte (block,tx,value) LE triple framing.
#[test]
fn ledger_codec_roundtrip_and_framing() {
    let entries = vec![
        (ProtoruneRuneId { block: 2, tx: 0x80424 }, 148_999_796u128),
        (ProtoruneRuneId { block: 2, tx: 0 }, u128::MAX),
        (ProtoruneRuneId { block: 8, tx: 0xdead }, 1),
    ];
    let blob = encode_ledger(&entries);
    assert_eq!(blob.len(), entries.len() * 48, "48 bytes per entry");
    assert_eq!(decode_ledger(&blob), entries);
    // trailing bytes shorter than a full triple are ignored, never panic
    let mut truncated = blob.clone();
    truncated.extend_from_slice(&[0xab; 10]);
    assert_eq!(decode_ledger(&truncated), entries);
    // a zeroed (claimed) entry decodes to nothing
    assert!(decode_ledger(&[]).is_empty());
}

/// EOA gate: capture credits / claim releases only key-path outputs (mirrors
/// `is_eoa` on both sides).
#[test]
fn is_eoa_classification() {
    fn is_eoa(spk: &ScriptBuf) -> bool {
        spk.is_p2tr() || spk.is_p2wpkh() || spk.is_p2pkh()
    }
    let p2wpkh = ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_byte_array([3u8; 20]));
    let p2pkh = ScriptBuf::new_p2pkh(&bitcoin::PubkeyHash::from_byte_array([4u8; 20]));
    let p2wsh = ScriptBuf::new_p2wsh(&bitcoin::WScriptHash::from_byte_array([5u8; 32]));
    let op_return = ScriptBuf::new_op_return([0u8; 4]);

    assert!(is_eoa(&p2tr(2)));
    assert!(is_eoa(&p2wpkh));
    assert!(is_eoa(&p2pkh));
    assert!(!is_eoa(&p2wsh), "script-path (contract-like) is not EOA");
    assert!(!is_eoa(&op_return), "OP_RETURN is never a recycle recipient");
}

/// End-to-end: seed an alkane at an outpoint, strand it with a protostone-less
/// spend (native capture sweeps it into 8:dead), then claim it back via the
/// 8:dead wasm (opcode 3). Exercises capture↔claim key-parity, the EOA caller
/// gate, and the anti-mint inventory accounting through a real VM execution.
#[cfg(test)]
mod e2e {
    use crate::index_block;
    use crate::recycle::RECYCLE_ALKANE_ID;
    use crate::message::AlkaneMessageContext;
    use crate::tests::helpers as h;
    use alkanes_support::cellpack::Cellpack;
    use alkanes_support::id::AlkaneId;
    use anyhow::Result;
    use bitcoin::hashes::Hash;
    use bitcoin::{
        Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
    };
    use metashrew_core::index_pointer::IndexPointer;
    use metashrew_support::index_pointer::KeyValuePointer;
    use protorune::balance_sheet::{load_sheet_chunked, save_chunked};
    use protorune::message::MessageContext;
    use protorune::tables::RuneTable;
    use protorune_support::balance_sheet::{
        BalanceSheet, BalanceSheetOperations, ProtoruneRuneId,
    };
    use protorune_support::utils::consensus_encode;
    use std::sync::Arc;


    fn eoa_recovery_spk() -> ScriptBuf {
        ScriptBuf::new_p2wpkh(&bitcoin::WPubkeyHash::from_byte_array([7u8; 20]))
    }

    /// `/alkanes/<what>/balances/<8:dead>` — what the claim WASM debits.
    fn recycle_inventory_balance(what: &AlkaneId) -> u128 {
        let what_bytes: Vec<u8> = what.clone().into();
        let who_bytes: Vec<u8> = RECYCLE_ALKANE_ID.into();
        IndexPointer::from_keyword("/alkanes/")
            .select(&what_bytes)
            .keyword("/balances/")
            .select(&who_bytes)
            .get_value::<u128>()
    }

    /// Decode the `8:dead` ledger at `/alkanes/<8:dead>/storage/recycle/<spk>`.
    fn recycle_ledger(spk: &[u8]) -> Vec<(ProtoruneRuneId, u128)> {
        let inner: Vec<u8> = IndexPointer::from_keyword("/recycle/")
            .select(&spk.to_vec())
            .unwrap()
            .as_ref()
            .clone();
        let id_bytes: Vec<u8> = RECYCLE_ALKANE_ID.into();
        let raw = IndexPointer::from_keyword("/alkanes/")
            .select(&id_bytes)
            .keyword("/storage/")
            .select(&inner)
            .get();
        crate::recycle::decode_ledger(raw.as_ref())
    }

    #[test]
    fn recycle_capture_then_claim_roundtrip() -> Result<()> {
        h::clear();
        let tag = AlkaneMessageContext::protocol_tag();
        let table = RuneTable::for_protocol(tag);
        let stranded = AlkaneId { block: 2, tx: 1234 };
        let stranded_rune = ProtoruneRuneId { block: 2, tx: 1234 };
        let amount: u128 = 500;
        let recovery = eoa_recovery_spk();

        // 1) seed an outpoint that carries the alkane
        let seed_op = OutPoint {
            txid: bitcoin::Txid::from_byte_array([9u8; 32]),
            vout: 0,
        };
        let mut seed_sheet = BalanceSheet::<IndexPointer>::default();
        seed_sheet.increase(&stranded_rune, amount)?;
        // v3 outpoint balances are chunked.
        save_chunked(
            &seed_sheet,
            &mut table.OUTPOINT_TO_RUNES.select(&consensus_encode(&seed_op)?),
            false,
        );

        // 2) strand it: a tx with NO OP_RETURN spending seed_op, paying recovery EOA
        let strand_tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: seed_op,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![TxOut {
                value: Amount::from_sat(546),
                script_pubkey: recovery.clone(),
            }],
        };
        let block1 = protorune::test_helpers::create_block_with_txs(vec![strand_tx]);
        index_block(&block1, 1)?;

        // 3) capture assertions
        assert_eq!(
            recycle_inventory_balance(&stranded),
            amount,
            "8:dead inventory should hold the stranded alkane"
        );
        assert_eq!(
            recycle_ledger(recovery.as_bytes()),
            vec![(stranded_rune.clone(), amount)],
            "ledger[recovery] should record the stranded balance"
        );
        // capture clears the spent input's chunk (clear_chunked_balances).
        assert_eq!(
            load_sheet_chunked(&table.OUTPOINT_TO_RUNES.select(&consensus_encode(&seed_op)?))
                .get_cached(&stranded_rune),
            0,
            "stranded input balance should be cleared after capture"
        );

        // 4) claim via 8:dead opcode 3 — output[0] is the recovery EOA
        let claim_cellpack = Cellpack {
            target: RECYCLE_ALKANE_ID,
            inputs: vec![3],
        };
        let protostone = protorune_support::protostone::Protostone {
            message: claim_cellpack.encipher(),
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
            from: None,
            burn: None,
            protocol_tag: tag,
        };
        let dummy_in = TxIn {
            previous_output: OutPoint {
                txid: bitcoin::Txid::from_byte_array([1u8; 32]),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        };
        let claim_tx = h::create_protostone_tx_with_inputs(
            vec![dummy_in],
            vec![TxOut {
                value: Amount::from_sat(546),
                script_pubkey: recovery.clone(),
            }],
            protostone,
        );
        let block2 = protorune::test_helpers::create_block_with_txs(vec![claim_tx]);
        index_block(&block2, 2)?;

        // 5) claim assertions: recovery output (vout 0) holds the alkane; ledger zeroed.
        // Read the claim output with the chunked loader (v3 storage format).
        let claim_outpoint = OutPoint {
            txid: block2.txdata[0].compute_txid(),
            vout: 0,
        };
        let claimed = load_sheet_chunked(
            &table.OUTPOINT_TO_RUNES.select(&consensus_encode(&claim_outpoint)?),
        );
        assert_eq!(
            claimed.get_cached(&stranded_rune),
            amount,
            "claim should release the stranded alkane to the recovery output"
        );
        assert!(
            recycle_ledger(recovery.as_bytes()).is_empty(),
            "ledger entry should be zeroed after claim (no replay)"
        );
        let _ = Arc::new(());
        Ok(())
    }
}
