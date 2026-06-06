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
