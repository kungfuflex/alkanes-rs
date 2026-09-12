use crate::envelope::RawEnvelope;
use bitcoin::blockdata::opcodes;
use bitcoin::blockdata::script::{Instruction, Script};
use bitcoin::blockdata::transaction::Transaction;
use bitcoin::blockdata::witness::Witness;
use bitcoin::hashes::{sha256, Hash};
use std::collections::HashMap;

/// gzip stream magic. Alkanes payloads are always gzip-compressed before chunking,
/// so this disambiguates a real hashlock/HTLC spend from a data envelope.
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Upper bound on hashlock gates parsed from a single leaf (DoS guard).
const HASHLOCK_MAX_GATES: usize = 1 << 20;

/// Extract the `i`-th contract payload carried in `tx`'s witnesses.
///
/// Recognises, in order:
///   1. the legacy ord-style `OP_FALSE OP_IF <"BIN"> ... OP_ENDIF` envelope
///      (grandfathered; may become unminable if BIP-110 activates), then
///   2. the BIP-110-resistant hashlock envelope (see docs/OPERATION-HASHLOCK).
///
/// Both return the still-gzip-compressed payload; callers decompress. Back-compatible:
/// when only legacy envelopes are present this returns exactly what it always did.
pub fn find_witness_payload(tx: &Transaction, i: usize) -> Option<Vec<u8>> {
    let mut payloads: Vec<Vec<u8>> = RawEnvelope::from_transaction(tx)
        .into_iter()
        .map(|envelope| {
            envelope
                .payload
                .into_iter()
                .skip(1)
                .flatten()
                .collect::<Vec<u8>>()
        })
        .collect();
    for input in &tx.input {
        if let Some(payload) = parse_hashlock_witness(&input.witness) {
            payloads.push(payload);
        }
    }
    payloads.into_iter().nth(i)
}

/// Parse a single input's witness as a hashlock envelope, returning the reassembled
/// (still-compressed) payload, or `None` if it is not a hashlock data carrier.
fn parse_hashlock_witness(witness: &Witness) -> Option<Vec<u8>> {
    let leaf = witness.tapscript()?;
    let gates = match_hashlock_leaf(leaf)?;
    // Map sha256(element) -> element over every witness stack element. The signature,
    // leaf script and control block simply never match a gate hash and are ignored,
    // which makes recovery independent of witness ordering, the annex, and the sig.
    let mut by_hash: HashMap<[u8; 32], Vec<u8>> = HashMap::new();
    for element in witness.iter() {
        by_hash.insert(sha256::Hash::hash(element).to_byte_array(), element.to_vec());
    }
    let mut payload = Vec::new();
    for gate in &gates {
        payload.extend_from_slice(by_hash.get(gate)?);
    }
    if payload.len() >= 2 && payload[0..2] == GZIP_MAGIC {
        Some(payload)
    } else {
        None
    }
}

/// Match `(OP_SHA256 <32-byte> OP_EQUALVERIFY){n>=1} <32|33-byte> OP_CHECKSIG` and
/// return the ordered 32-byte hash gates. Any deviation yields `None`.
fn match_hashlock_leaf(script: &Script) -> Option<Vec<[u8; 32]>> {
    let mut gates: Vec<[u8; 32]> = Vec::new();
    let mut instructions = script.instructions();
    loop {
        match instructions.next() {
            Some(Ok(Instruction::Op(op))) if op == opcodes::all::OP_SHA256 => {
                let push = match instructions.next() {
                    Some(Ok(Instruction::PushBytes(bytes))) if bytes.len() == 32 => bytes,
                    _ => return None,
                };
                match instructions.next() {
                    Some(Ok(Instruction::Op(op))) if op == opcodes::all::OP_EQUALVERIFY => {}
                    _ => return None,
                }
                let mut gate = [0u8; 32];
                gate.copy_from_slice(push.as_bytes());
                gates.push(gate);
                if gates.len() > HASHLOCK_MAX_GATES {
                    return None;
                }
            }
            Some(Ok(Instruction::PushBytes(bytes))) if bytes.len() == 32 || bytes.len() == 33 => {
                // terminal `<pubkey> OP_CHECKSIG`
                match instructions.next() {
                    Some(Ok(Instruction::Op(op))) if op == opcodes::all::OP_CHECKSIG => {}
                    _ => return None,
                }
                if instructions.next().is_some() {
                    return None;
                }
                return if gates.is_empty() { None } else { Some(gates) };
            }
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{RawEnvelope, NUMS_INTERNAL_KEY};
    use crate::gz::decompress;
    use bitcoin::absolute::LockTime;
    use bitcoin::secp256k1::XOnlyPublicKey;
    use bitcoin::transaction::Version;
    use bitcoin::{OutPoint, ScriptBuf, Sequence, Transaction, TxIn, Witness};

    fn tx_with_witness(witness: Witness) -> Transaction {
        Transaction {
            version: Version(2),
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness,
            }],
            output: vec![],
        }
    }

    #[test]
    fn hashlock_roundtrip_multi_chunk() {
        // > 256 bytes so the payload spans several preimages.
        let raw: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        let witness = RawEnvelope::hashlock_witness(raw.clone(), true).unwrap();
        let tx = tx_with_witness(witness);
        let payload = find_witness_payload(&tx, 0).expect("hashlock payload");
        assert_eq!(&payload[0..2], &GZIP_MAGIC);
        assert_eq!(decompress(payload).unwrap(), raw);
    }

    #[test]
    fn hashlock_roundtrip_chunk_boundaries() {
        for len in [0usize, 1, 255, 256, 257, 512, 513] {
            let raw: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            let witness = RawEnvelope::hashlock_witness(raw.clone(), true).unwrap();
            let tx = tx_with_witness(witness);
            let payload =
                find_witness_payload(&tx, 0).unwrap_or_else(|| panic!("len {len} not found"));
            assert_eq!(decompress(payload).unwrap(), raw, "len {len}");
        }
    }

    #[test]
    fn hashlock_is_bip110_clean() {
        let raw = vec![7u8; 1500];
        let witness = RawEnvelope::hashlock_witness(raw, true).unwrap();
        let elements: Vec<Vec<u8>> = witness.iter().map(|e| e.to_vec()).collect();
        // layout: [sig, preimages.., leaf, control_block]
        let leaf = Script::from_bytes(&elements[elements.len() - 2]);
        for instruction in leaf.instructions() {
            match instruction.unwrap() {
                Instruction::Op(op) => {
                    assert_ne!(op, opcodes::all::OP_IF);
                    assert_ne!(op, opcodes::all::OP_NOTIF);
                }
                Instruction::PushBytes(bytes) => assert!(bytes.len() <= 256),
            }
        }
        // every data stack element (everything but the leaf + control block) <= 256 bytes
        for element in &elements[..elements.len() - 2] {
            assert!(element.len() <= 256);
        }
    }

    #[test]
    fn real_htlc_is_not_recognized() {
        // A genuine 1-hop hashlock whose preimage is NOT a gzip stream must not be
        // mistaken for a data envelope.
        let preimage = [0xABu8; 32];
        let reveal_key = XOnlyPublicKey::from_slice(&NUMS_INTERNAL_KEY).unwrap();
        let (leaf, _) = RawEnvelope::hashlock_reveal_script(&preimage, &reveal_key);
        let mut witness = Witness::new();
        witness.push([]);
        witness.push(preimage);
        witness.push(leaf.as_bytes());
        witness.push([0u8; 33]); // dummy control block
        let tx = tx_with_witness(witness);
        assert!(find_witness_payload(&tx, 0).is_none());
    }

    #[test]
    fn legacy_envelope_still_recognized() {
        let raw: Vec<u8> = (0..600u32).map(|i| (i % 251) as u8).collect();
        let witness = RawEnvelope::from(raw.clone()).to_witness(true);
        let tx = tx_with_witness(witness);
        let payload = find_witness_payload(&tx, 0).expect("legacy payload");
        assert_eq!(decompress(payload).unwrap(), raw);
    }
}
