//! Test: verify BIP-341 taproot sighash computation matches rust-bitcoin.
//!
//! This tests the exact same computation that frusd-build-mint does in WASM,
//! using the reference rust-bitcoin SighashCache as ground truth.

use bitcoin::{
    absolute::LockTime,
    transaction::Version,
    Address, Amount, Network, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness,
    sighash::{Prevouts, SighashCache, TapSighashType},
};
use bitcoin::hashes::Hash;
use sha2::{Sha256, Digest};
use std::str::FromStr;

/// Tagged hash per BIP-340/341.
fn tagged_hash(tag: &[u8], data: &[u8]) -> [u8; 32] {
    let tag_hash = Sha256::digest(tag);
    let mut hasher = Sha256::new();
    hasher.update(&tag_hash);
    hasher.update(&tag_hash);
    hasher.update(data);
    hasher.finalize().into()
}

/// Manual BIP-341 sighash computation (same as frusd-build-mint WASM).
fn manual_taproot_sighash(
    tx: &Transaction,
    input_index: u32,
    prev_amounts: &[u64],
    prev_scriptpubkeys: &[ScriptBuf],
) -> [u8; 32] {
    let num_inputs = tx.input.len();

    // sha_prevouts
    let mut prevouts_data = Vec::new();
    for input in &tx.input {
        prevouts_data.extend_from_slice(&input.previous_output.txid.to_byte_array());
        prevouts_data.extend_from_slice(&input.previous_output.vout.to_le_bytes());
    }
    let sha_prevouts = Sha256::digest(&prevouts_data);

    // sha_amounts
    let mut amounts_data = Vec::new();
    for &amt in prev_amounts {
        amounts_data.extend_from_slice(&amt.to_le_bytes());
    }
    let sha_amounts = Sha256::digest(&amounts_data);

    // sha_scriptpubkeys (compact size + script)
    let mut scripts_data = Vec::new();
    for script in prev_scriptpubkeys {
        let script_bytes = script.as_bytes();
        if script_bytes.len() < 0xfd {
            scripts_data.push(script_bytes.len() as u8);
        } else {
            scripts_data.push(0xfd);
            scripts_data.extend_from_slice(&(script_bytes.len() as u16).to_le_bytes());
        }
        scripts_data.extend_from_slice(script_bytes);
    }
    let sha_scriptpubkeys = Sha256::digest(&scripts_data);

    // sha_sequences
    let mut sequences_data = Vec::new();
    for input in &tx.input {
        sequences_data.extend_from_slice(&input.sequence.0.to_le_bytes());
    }
    let sha_sequences = Sha256::digest(&sequences_data);

    // sha_outputs
    let mut outputs_data = Vec::new();
    for output in &tx.output {
        outputs_data.extend_from_slice(&output.value.to_sat().to_le_bytes());
        let script = output.script_pubkey.as_bytes();
        if script.len() < 0xfd {
            outputs_data.push(script.len() as u8);
        } else {
            outputs_data.push(0xfd);
            outputs_data.extend_from_slice(&(script.len() as u16).to_le_bytes());
        }
        outputs_data.extend_from_slice(script);
    }
    let sha_outputs = Sha256::digest(&outputs_data);

    // Build SigMsg
    let mut sigmsg = Vec::new();
    sigmsg.push(0x00); // epoch
    sigmsg.push(0x00); // hash_type = SIGHASH_DEFAULT
    sigmsg.extend_from_slice(&tx.version.0.to_le_bytes());
    sigmsg.extend_from_slice(&tx.lock_time.to_consensus_u32().to_le_bytes());
    sigmsg.extend_from_slice(&sha_prevouts);
    sigmsg.extend_from_slice(&sha_amounts);
    sigmsg.extend_from_slice(&sha_scriptpubkeys);
    sigmsg.extend_from_slice(&sha_sequences);
    sigmsg.extend_from_slice(&sha_outputs);
    sigmsg.push(0x00); // spend_type = 0 (key path, no annex)
    sigmsg.extend_from_slice(&input_index.to_le_bytes());

    tagged_hash(b"TapSighash", &sigmsg)
}

#[test]
fn manual_sighash_matches_rust_bitcoin() {
    // Build a simple TX with 2 P2TR inputs and 2 outputs
    let xonly_hex = "36f5063cfc8a7e841f331c618b9108f4ef1cfecf2f9aaa554b031ea12aa97edf";
    let xonly_bytes = hex::decode(xonly_hex).unwrap();
    let mut script_pubkey = vec![0x51, 0x20]; // OP_1 PUSH32
    script_pubkey.extend_from_slice(&xonly_bytes);
    let script = ScriptBuf::from(script_pubkey.clone());

    let txid1 = Txid::from_str("d23cfff710b0c92ec2458feff0db3cdaa744bd7b65dfe725c2c27a4ee09c4ffb").unwrap();
    let txid2 = Txid::from_str("31e1f4f30d532a94592a4fae30369c75997835554eec2854b374af18dfda6b49").unwrap();

    let tx = Transaction {
        version: Version(2),
        lock_time: LockTime::ZERO,
        input: vec![
            TxIn {
                previous_output: OutPoint { txid: txid1, vout: 0 },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            },
            TxIn {
                previous_output: OutPoint { txid: txid2, vout: 0 },
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            },
        ],
        output: vec![
            TxOut {
                value: Amount::from_sat(546),
                script_pubkey: script.clone(),
            },
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: ScriptBuf::from(vec![0x6a, 0x24, 0x00]), // OP_RETURN
            },
            TxOut {
                value: Amount::from_sat(100_000_000),
                script_pubkey: script.clone(),
            },
        ],
    };

    let prev_amounts = vec![546u64, 156_250_000u64];
    let prev_txouts: Vec<TxOut> = prev_amounts.iter().map(|&a| TxOut {
        value: Amount::from_sat(a),
        script_pubkey: script.clone(),
    }).collect();
    let prev_scriptpubkeys: Vec<ScriptBuf> = prev_txouts.iter().map(|t| t.script_pubkey.clone()).collect();

    // Compute sighash with rust-bitcoin (reference)
    let mut sighash_cache = SighashCache::new(&tx);
    let prevouts = Prevouts::All(&prev_txouts);
    let reference_sighash = sighash_cache
        .taproot_key_spend_signature_hash(0, &prevouts, TapSighashType::Default)
        .expect("sighash computation failed");

    // Compute sighash with manual method (same as WASM build_mint)
    let manual_sighash = manual_taproot_sighash(&tx, 0, &prev_amounts, &prev_scriptpubkeys);

    println!("Reference sighash: {}", hex::encode(reference_sighash.as_byte_array()));
    println!("Manual sighash:    {}", hex::encode(manual_sighash));

    assert_eq!(
        reference_sighash.as_byte_array(),
        &manual_sighash,
        "Manual sighash must match rust-bitcoin reference"
    );
}
