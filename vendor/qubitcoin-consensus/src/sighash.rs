//! Signature hash computation.
//! Maps to: src/script/interpreter.cpp (SignatureHash, etc.)
//!
//! Implements all three signature hash algorithms used in Bitcoin:
//! - Legacy (pre-segwit) signature hash
//! - BIP143 segwit v0 signature hash
//! - BIP341 taproot signature hash

use crate::transaction::{Transaction, TxOut};
use qubitcoin_crypto::hash::{hash256, sha256_hash, tagged_hash};
use qubitcoin_primitives::Uint256;
use qubitcoin_script::{Opcode, Script};
use qubitcoin_serialize::Encodable;

/// `SIGHASH_ALL` (0x01): sign all inputs and all outputs. Default sighash type.
pub const SIGHASH_ALL: u32 = 1;
/// `SIGHASH_NONE` (0x02): sign all inputs but no outputs (anyone can redirect the funds).
pub const SIGHASH_NONE: u32 = 2;
/// `SIGHASH_SINGLE` (0x03): sign all inputs and only the output at the same index.
pub const SIGHASH_SINGLE: u32 = 3;
/// `SIGHASH_ANYONECANPAY` (0x80): modifier flag; sign only the current input.
/// Combined with a base type (e.g., `SIGHASH_ALL | SIGHASH_ANYONECANPAY`).
pub const SIGHASH_ANYONECANPAY: u32 = 0x80;

/// Mask for the base sighash type (lower 5 bits).
pub const SIGHASH_OUTPUT_MASK: u32 = 0x1f;

/// Precomputed transaction data for efficient signature hashing.
/// Maps to: PrecomputedTransactionData in Bitcoin Core.
///
/// Caches expensive hash computations that are shared across multiple
/// signature verifications in the same transaction.
#[derive(Debug, Clone)]
pub struct PrecomputedTransactionData {
    /// SHA256d of all input outpoints (for BIP143)
    pub hash_prevouts: Uint256,
    /// SHA256d of all input sequences (for BIP143)
    pub hash_sequence: Uint256,
    /// SHA256d of all outputs (for BIP143)
    pub hash_outputs: Uint256,
    /// SHA256 of all input outpoints (for BIP341)
    pub sha_prevouts: [u8; 32],
    /// SHA256 of all input amounts (for BIP341)
    pub sha_amounts: [u8; 32],
    /// SHA256 of all input scriptPubKeys (for BIP341)
    pub sha_script_pubkeys: [u8; 32],
    /// SHA256 of all input sequences (for BIP341)
    pub sha_sequences: [u8; 32],
    /// SHA256 of all outputs (for BIP341)
    pub sha_outputs: [u8; 32],
    /// Previous outputs (for BIP341 key spend path)
    pub spent_outputs: Vec<TxOut>,
    /// Whether the precomputed data is fully initialized
    pub ready: bool,
}

impl PrecomputedTransactionData {
    /// Compute all hashes for a transaction.
    ///
    /// `spent_outputs` should contain the TxOut being spent by each input,
    /// in the same order as `tx.vin`. For BIP143-only usage, this can be empty.
    pub fn new(tx: &Transaction, spent_outputs: &[TxOut]) -> Self {
        // Compute hash_prevouts: SHA256d(prevout_1 || prevout_2 || ...)
        let mut prevouts_buf = Vec::new();
        for input in &tx.vin {
            input.prevout.encode(&mut prevouts_buf).unwrap();
        }
        let hash_prevouts = Uint256::from_bytes(hash256(&prevouts_buf));

        // Compute hash_sequence: SHA256d(sequence_1 || sequence_2 || ...)
        let mut sequence_buf = Vec::new();
        for input in &tx.vin {
            input.sequence.encode(&mut sequence_buf).unwrap();
        }
        let hash_sequence = Uint256::from_bytes(hash256(&sequence_buf));

        // Compute hash_outputs: SHA256d(output_1 || output_2 || ...)
        let mut outputs_buf = Vec::new();
        for output in &tx.vout {
            output.encode(&mut outputs_buf).unwrap();
        }
        let hash_outputs = Uint256::from_bytes(hash256(&outputs_buf));

        // SHA256 variants for BIP341
        let sha_prevouts = sha256_hash(&prevouts_buf);
        let sha_sequences = sha256_hash(&sequence_buf);
        let sha_outputs = sha256_hash(&outputs_buf);

        // Compute sha_amounts: SHA256(amount_1 || amount_2 || ...)
        let mut amounts_buf = Vec::new();
        for so in spent_outputs {
            so.value.to_sat().encode(&mut amounts_buf).unwrap();
        }
        let sha_amounts = sha256_hash(&amounts_buf);

        // Compute sha_script_pubkeys: SHA256(scriptPubKey_1 || scriptPubKey_2 || ...)
        let mut spk_buf = Vec::new();
        for so in spent_outputs {
            so.script_pubkey.encode(&mut spk_buf).unwrap();
        }
        let sha_script_pubkeys = sha256_hash(&spk_buf);

        PrecomputedTransactionData {
            hash_prevouts,
            hash_sequence,
            hash_outputs,
            sha_prevouts,
            sha_amounts,
            sha_script_pubkeys,
            sha_sequences,
            sha_outputs,
            spent_outputs: spent_outputs.to_vec(),
            ready: true,
        }
    }

    /// Create an empty/uninitialized instance.
    pub fn empty() -> Self {
        PrecomputedTransactionData {
            hash_prevouts: Uint256::ZERO,
            hash_sequence: Uint256::ZERO,
            hash_outputs: Uint256::ZERO,
            sha_prevouts: [0u8; 32],
            sha_amounts: [0u8; 32],
            sha_script_pubkeys: [0u8; 32],
            sha_sequences: [0u8; 32],
            sha_outputs: [0u8; 32],
            spent_outputs: Vec::new(),
            ready: false,
        }
    }
}

/// Compute the legacy sighash (pre-segwit).
/// Maps to: SignatureHash() in interpreter.cpp with SIGVERSION_BASE.
///
/// This serializes a modified copy of the transaction and returns
/// SHA256d of the result.
pub fn signature_hash(
    script_code: &Script,
    tx: &Transaction,
    input_index: usize,
    hash_type: u32,
) -> Uint256 {
    // Special case: input_index out of bounds -> return uint256(1)
    // This matches Bitcoin Core's behavior for the historical OP_CODESEPARATOR bug.
    if input_index >= tx.vin.len() {
        return Uint256::ONE;
    }

    // Remove OP_CODESEPARATOR from the script code
    let script_code = remove_codeseparators(script_code);

    let base_type = hash_type & SIGHASH_OUTPUT_MASK;

    // Special case: SIGHASH_SINGLE with input_index >= outputs
    if base_type == SIGHASH_SINGLE && input_index >= tx.vout.len() {
        return Uint256::ONE;
    }

    let mut buf = Vec::new();

    // Serialize version
    tx.version.encode(&mut buf).unwrap();

    // Serialize inputs
    let anyone_can_pay = (hash_type & SIGHASH_ANYONECANPAY) != 0;
    let n_inputs = if anyone_can_pay { 1 } else { tx.vin.len() };

    qubitcoin_serialize::write_compact_size(&mut buf, n_inputs as u64).unwrap();

    for i in 0..tx.vin.len() {
        if anyone_can_pay && i != input_index {
            continue;
        }

        // Serialize outpoint
        tx.vin[i].prevout.encode(&mut buf).unwrap();

        // Serialize scriptSig: use script_code for input_index, empty for others
        if i == input_index {
            script_code.encode(&mut buf).unwrap();
        } else {
            Script::new().encode(&mut buf).unwrap();
        }

        // Serialize sequence: for NONE and SINGLE, set other inputs' sequence to 0
        if i != input_index && (base_type == SIGHASH_NONE || base_type == SIGHASH_SINGLE) {
            0u32.encode(&mut buf).unwrap();
        } else {
            tx.vin[i].sequence.encode(&mut buf).unwrap();
        }
    }

    // Serialize outputs
    match base_type {
        SIGHASH_NONE => {
            // No outputs
            qubitcoin_serialize::write_compact_size(&mut buf, 0u64).unwrap();
        }
        SIGHASH_SINGLE => {
            // Outputs up to and including input_index
            let n_outputs = input_index + 1;
            qubitcoin_serialize::write_compact_size(&mut buf, n_outputs as u64).unwrap();
            for i in 0..n_outputs {
                if i == input_index {
                    tx.vout[i].encode(&mut buf).unwrap();
                } else {
                    // Blank output: value = -1, empty script
                    TxOut::null().encode(&mut buf).unwrap();
                }
            }
        }
        _ => {
            // SIGHASH_ALL: all outputs
            qubitcoin_serialize::write_compact_size(&mut buf, tx.vout.len() as u64).unwrap();
            for output in &tx.vout {
                output.encode(&mut buf).unwrap();
            }
        }
    }

    // Serialize locktime
    tx.lock_time.encode(&mut buf).unwrap();

    // Append hash_type as LE u32
    hash_type.encode(&mut buf).unwrap();

    // Return SHA256d(serialized)
    Uint256::from_bytes(hash256(&buf))
}

/// Compute BIP143 segwit v0 sighash.
/// Maps to: SignatureHash() with SIGVERSION_WITNESS_V0.
///
/// Uses the precomputed hashes for efficiency when verifying
/// multiple inputs in the same transaction.
pub fn witness_v0_signature_hash(
    script_code: &Script,
    tx: &Transaction,
    input_index: usize,
    hash_type: u32,
    amount: i64,
    precomputed: &PrecomputedTransactionData,
) -> Uint256 {
    let base_type = hash_type & SIGHASH_OUTPUT_MASK;
    let anyone_can_pay = (hash_type & SIGHASH_ANYONECANPAY) != 0;

    let mut buf = Vec::new();

    // 1. nVersion (4 bytes LE)
    tx.version.encode(&mut buf).unwrap();

    // 2. hashPrevouts (32 bytes)
    if !anyone_can_pay {
        buf.extend_from_slice(precomputed.hash_prevouts.as_bytes());
    } else {
        buf.extend_from_slice(&[0u8; 32]);
    }

    // 3. hashSequence (32 bytes)
    if !anyone_can_pay && base_type != SIGHASH_SINGLE && base_type != SIGHASH_NONE {
        buf.extend_from_slice(precomputed.hash_sequence.as_bytes());
    } else {
        buf.extend_from_slice(&[0u8; 32]);
    }

    // 4. outpoint (32+4 = 36 bytes)
    tx.vin[input_index].prevout.encode(&mut buf).unwrap();

    // 5. scriptCode (varint + script bytes)
    script_code.encode(&mut buf).unwrap();

    // 6. amount (8 bytes LE)
    amount.encode(&mut buf).unwrap();

    // 7. nSequence (4 bytes LE)
    tx.vin[input_index].sequence.encode(&mut buf).unwrap();

    // 8. hashOutputs (32 bytes)
    if base_type != SIGHASH_SINGLE && base_type != SIGHASH_NONE {
        buf.extend_from_slice(precomputed.hash_outputs.as_bytes());
    } else if base_type == SIGHASH_SINGLE && input_index < tx.vout.len() {
        // Hash only the output at input_index
        let mut output_buf = Vec::new();
        tx.vout[input_index].encode(&mut output_buf).unwrap();
        let output_hash = hash256(&output_buf);
        buf.extend_from_slice(&output_hash);
    } else {
        buf.extend_from_slice(&[0u8; 32]);
    }

    // 9. nLockTime (4 bytes LE)
    tx.lock_time.encode(&mut buf).unwrap();

    // 10. nHashType (4 bytes LE)
    hash_type.encode(&mut buf).unwrap();

    // SHA256d of the above
    Uint256::from_bytes(hash256(&buf))
}

/// Compute BIP341 taproot sighash (signature hash for key-path spending).
/// Maps to: SignatureHash() with SIGVERSION_TAPROOT / SIGVERSION_TAPSCRIPT.
///
/// Uses tagged hashing ("TapSighash") per BIP341.
/// Returns `None` if hash_type is invalid or SIGHASH_SINGLE with input_index >= vout.len().
pub fn taproot_signature_hash(
    tx: &Transaction,
    input_index: usize,
    hash_type: u32,
    precomputed: &PrecomputedTransactionData,
    ext_flag: u8,
    tapleaf_hash: Option<&[u8; 32]>,
    codesep_pos: Option<u32>,
    annex_hash: Option<&[u8; 32]>,
) -> Option<Uint256> {
    // Validate hash_type (matches Bitcoin Core: hash_type <= 0x03 || (0x81..=0x83))
    if !(hash_type <= 0x03 || (hash_type >= 0x81 && hash_type <= 0x83)) {
        return None;
    }

    let base_type = if hash_type == 0 {
        SIGHASH_ALL
    } else {
        hash_type & SIGHASH_OUTPUT_MASK
    };
    let anyone_can_pay = (hash_type & SIGHASH_ANYONECANPAY) != 0;
    let is_none = base_type == SIGHASH_NONE;
    let is_single = base_type == SIGHASH_SINGLE;

    let mut buf = Vec::new();

    // Epoch (0x00)
    buf.push(0x00);

    // hash_type (1 byte)
    buf.push(hash_type as u8);

    // version (4 bytes LE)
    tx.version.encode(&mut buf).unwrap();

    // locktime (4 bytes LE)
    tx.lock_time.encode(&mut buf).unwrap();

    // sha_prevouts, sha_amounts, sha_scriptpubkeys, sha_sequences (unless ANYONECANPAY)
    if !anyone_can_pay {
        buf.extend_from_slice(&precomputed.sha_prevouts);
        buf.extend_from_slice(&precomputed.sha_amounts);
        buf.extend_from_slice(&precomputed.sha_script_pubkeys);
        buf.extend_from_slice(&precomputed.sha_sequences);
    }

    // sha_outputs (unless NONE or SINGLE)
    if !is_none && !is_single {
        buf.extend_from_slice(&precomputed.sha_outputs);
    }

    // spend_type: 2 * ext_flag + annex_present
    let annex_present = if annex_hash.is_some() { 1u8 } else { 0u8 };
    let spend_type = ext_flag * 2 + annex_present;
    buf.push(spend_type);

    // If ANYONECANPAY: individual input data
    if anyone_can_pay {
        // outpoint
        tx.vin[input_index].prevout.encode(&mut buf).unwrap();
        // amount
        if input_index < precomputed.spent_outputs.len() {
            precomputed.spent_outputs[input_index]
                .value
                .to_sat()
                .encode(&mut buf)
                .unwrap();
        } else {
            0i64.encode(&mut buf).unwrap();
        }
        // scriptPubKey
        if input_index < precomputed.spent_outputs.len() {
            precomputed.spent_outputs[input_index]
                .script_pubkey
                .encode(&mut buf)
                .unwrap();
        } else {
            Script::new().encode(&mut buf).unwrap();
        }
        // sequence
        tx.vin[input_index].sequence.encode(&mut buf).unwrap();
    } else {
        // input_index (4 bytes LE)
        (input_index as u32).encode(&mut buf).unwrap();
    }

    // If annex is present, include sha256(annex)
    if let Some(ah) = annex_hash {
        buf.extend_from_slice(ah);
    }

    // If SINGLE: this output's hash (must exist, or return None)
    if is_single {
        if input_index >= tx.vout.len() {
            return None;
        }
        let mut output_buf = Vec::new();
        tx.vout[input_index].encode(&mut output_buf).unwrap();
        let output_hash = sha256_hash(&output_buf);
        buf.extend_from_slice(&output_hash);
    }

    // If tapscript (ext_flag == 1): tapleaf_hash, key_version, codesep_pos
    if ext_flag == 1 {
        if let Some(hash) = tapleaf_hash {
            buf.extend_from_slice(hash);
        }
        // key_version (0x00 for BIP342)
        buf.push(0x00);
        // codeseparator_pos
        let pos = codesep_pos.unwrap_or(0xFFFFFFFF);
        pos.encode(&mut buf).unwrap();
    }

    // tagged_hash("TapSighash", ...)
    let result = tagged_hash(b"TapSighash", &buf);
    Some(Uint256::from_bytes(result))
}

/// Remove OP_CODESEPARATOR from script code (for legacy sighash).
///
/// Creates a new script with all OP_CODESEPARATOR opcodes removed.
/// This is necessary for legacy signature hash computation.
pub fn remove_codeseparators(script: &Script) -> Script {
    let mut result = Vec::new();
    let mut pos = 0;

    while let Some((opcode, _data, new_pos)) = script.get_op(pos) {
        if opcode == Opcode::OpCodeSeparator as u8 {
            // Skip OP_CODESEPARATOR
            pos = new_pos;
            continue;
        }
        // Copy the raw bytes from the original script for this opcode
        result.extend_from_slice(&script.as_bytes()[pos..new_pos]);
        pos = new_pos;
    }

    Script::from_bytes(result)
}

/// Validate a sighash type for legacy/segwit signatures.
/// Returns true if the hash_type is valid.
pub fn is_valid_signature_hash_type(hash_type: u32) -> bool {
    let base = hash_type & SIGHASH_OUTPUT_MASK;
    base >= SIGHASH_ALL && base <= SIGHASH_SINGLE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{OutPoint, TxIn, SEQUENCE_FINAL};
    use qubitcoin_primitives::{Amount, Txid};

    /// Helper: build a simple 1-in 1-out transaction.
    fn make_simple_tx() -> Transaction {
        Transaction::new(
            1,
            vec![TxIn::new(
                OutPoint::new(Txid::from_bytes([0xaa; 32]), 0),
                Script::from_bytes(vec![0x00]),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(
                Amount::from_sat(50_000),
                Script::from_bytes(vec![0x76, 0xa9]),
            )],
            0,
        )
    }

    /// Helper: build a 2-in 2-out transaction.
    fn make_multi_tx() -> Transaction {
        Transaction::new(
            2,
            vec![
                TxIn::new(
                    OutPoint::new(Txid::from_bytes([0xaa; 32]), 0),
                    Script::new(),
                    0xfffffffe,
                ),
                TxIn::new(
                    OutPoint::new(Txid::from_bytes([0xbb; 32]), 1),
                    Script::new(),
                    SEQUENCE_FINAL,
                ),
            ],
            vec![
                TxOut::new(
                    Amount::from_sat(10_000),
                    Script::from_bytes(vec![0x76, 0xa9, 0x14]),
                ),
                TxOut::new(
                    Amount::from_sat(20_000),
                    Script::from_bytes(vec![0xa9, 0x14]),
                ),
            ],
            500_000,
        )
    }

    #[test]
    fn test_precomputed_transaction_data() {
        let tx = make_multi_tx();
        let spent_outputs = vec![
            TxOut::new(Amount::from_sat(100_000), Script::from_bytes(vec![0x76])),
            TxOut::new(Amount::from_sat(200_000), Script::from_bytes(vec![0xa9])),
        ];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);

        // Verify that the data is marked as ready
        assert!(precomputed.ready);

        // Verify hash_prevouts is not zero (we have inputs)
        assert_ne!(precomputed.hash_prevouts, Uint256::ZERO);

        // Verify hash_sequence is not zero
        assert_ne!(precomputed.hash_sequence, Uint256::ZERO);

        // Verify hash_outputs is not zero
        assert_ne!(precomputed.hash_outputs, Uint256::ZERO);

        // Verify SHA256 variants are not zero
        assert_ne!(precomputed.sha_prevouts, [0u8; 32]);
        assert_ne!(precomputed.sha_sequences, [0u8; 32]);
        assert_ne!(precomputed.sha_outputs, [0u8; 32]);
        assert_ne!(precomputed.sha_amounts, [0u8; 32]);
        assert_ne!(precomputed.sha_script_pubkeys, [0u8; 32]);

        // Verify spent outputs are stored
        assert_eq!(precomputed.spent_outputs.len(), 2);
    }

    #[test]
    fn test_precomputed_deterministic() {
        let tx = make_multi_tx();
        let spent_outputs = vec![
            TxOut::new(Amount::from_sat(100_000), Script::from_bytes(vec![0x76])),
            TxOut::new(Amount::from_sat(200_000), Script::from_bytes(vec![0xa9])),
        ];

        let p1 = PrecomputedTransactionData::new(&tx, &spent_outputs);
        let p2 = PrecomputedTransactionData::new(&tx, &spent_outputs);

        assert_eq!(p1.hash_prevouts, p2.hash_prevouts);
        assert_eq!(p1.hash_sequence, p2.hash_sequence);
        assert_eq!(p1.hash_outputs, p2.hash_outputs);
        assert_eq!(p1.sha_prevouts, p2.sha_prevouts);
        assert_eq!(p1.sha_amounts, p2.sha_amounts);
        assert_eq!(p1.sha_script_pubkeys, p2.sha_script_pubkeys);
        assert_eq!(p1.sha_sequences, p2.sha_sequences);
        assert_eq!(p1.sha_outputs, p2.sha_outputs);
    }

    #[test]
    fn test_legacy_sighash_out_of_bounds() {
        let tx = make_simple_tx();
        let script_code = Script::from_bytes(vec![0x76, 0xa9]);

        // input_index >= tx.vin.len() should return Uint256::ONE
        let hash = signature_hash(&script_code, &tx, 5, SIGHASH_ALL);
        assert_eq!(hash, Uint256::ONE);
    }

    #[test]
    fn test_legacy_sighash_all() {
        let tx = make_simple_tx();
        let script_code = Script::from_bytes(vec![0x76, 0xa9]);

        let hash = signature_hash(&script_code, &tx, 0, SIGHASH_ALL);
        // Hash should be deterministic and non-zero
        assert_ne!(hash, Uint256::ZERO);
        assert_ne!(hash, Uint256::ONE);

        // Same inputs should produce same output
        let hash2 = signature_hash(&script_code, &tx, 0, SIGHASH_ALL);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_legacy_sighash_none() {
        let tx = make_multi_tx();
        let script_code = Script::from_bytes(vec![0x76, 0xa9]);

        let hash_all = signature_hash(&script_code, &tx, 0, SIGHASH_ALL);
        let hash_none = signature_hash(&script_code, &tx, 0, SIGHASH_NONE);

        // Different hash types should produce different hashes
        assert_ne!(hash_all, hash_none);
        assert_ne!(hash_none, Uint256::ZERO);
    }

    #[test]
    fn test_legacy_sighash_single() {
        let tx = make_multi_tx();
        let script_code = Script::from_bytes(vec![0x76, 0xa9]);

        let hash_all = signature_hash(&script_code, &tx, 0, SIGHASH_ALL);
        let hash_single = signature_hash(&script_code, &tx, 0, SIGHASH_SINGLE);

        assert_ne!(hash_all, hash_single);
        assert_ne!(hash_single, Uint256::ZERO);
    }

    #[test]
    fn test_legacy_sighash_single_out_of_range() {
        // When SIGHASH_SINGLE and input_index >= vout.len(), return ONE
        let tx = Transaction::new(
            1,
            vec![
                TxIn::new(
                    OutPoint::new(Txid::from_bytes([0xaa; 32]), 0),
                    Script::new(),
                    SEQUENCE_FINAL,
                ),
                TxIn::new(
                    OutPoint::new(Txid::from_bytes([0xbb; 32]), 1),
                    Script::new(),
                    SEQUENCE_FINAL,
                ),
            ],
            vec![TxOut::new(
                Amount::from_sat(50_000),
                Script::from_bytes(vec![0x76]),
            )],
            0,
        );
        let script_code = Script::from_bytes(vec![0x76]);

        // input_index=1 but only 1 output
        let hash = signature_hash(&script_code, &tx, 1, SIGHASH_SINGLE);
        assert_eq!(hash, Uint256::ONE);
    }

    #[test]
    fn test_legacy_sighash_anyonecanpay() {
        let tx = make_multi_tx();
        let script_code = Script::from_bytes(vec![0x76, 0xa9]);

        let hash_all = signature_hash(&script_code, &tx, 0, SIGHASH_ALL);
        let hash_acp = signature_hash(&script_code, &tx, 0, SIGHASH_ALL | SIGHASH_ANYONECANPAY);

        assert_ne!(hash_all, hash_acp);
        assert_ne!(hash_acp, Uint256::ZERO);
    }

    #[test]
    fn test_legacy_sighash_different_inputs() {
        let tx = make_multi_tx();
        let script_code = Script::from_bytes(vec![0x76, 0xa9]);

        let hash0 = signature_hash(&script_code, &tx, 0, SIGHASH_ALL);
        let hash1 = signature_hash(&script_code, &tx, 1, SIGHASH_ALL);

        // Different input indices should produce different hashes
        assert_ne!(hash0, hash1);
    }

    #[test]
    fn test_witness_v0_sighash_all() {
        let tx = make_multi_tx();
        let spent_outputs = vec![
            TxOut::new(Amount::from_sat(100_000), Script::from_bytes(vec![0x76])),
            TxOut::new(Amount::from_sat(200_000), Script::from_bytes(vec![0xa9])),
        ];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);
        let script_code = Script::from_bytes(vec![0x76, 0xa9, 0x14]);

        let hash =
            witness_v0_signature_hash(&script_code, &tx, 0, SIGHASH_ALL, 100_000, &precomputed);

        assert_ne!(hash, Uint256::ZERO);

        // Deterministic
        let hash2 =
            witness_v0_signature_hash(&script_code, &tx, 0, SIGHASH_ALL, 100_000, &precomputed);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_witness_v0_sighash_none() {
        let tx = make_multi_tx();
        let spent_outputs = vec![
            TxOut::new(Amount::from_sat(100_000), Script::from_bytes(vec![0x76])),
            TxOut::new(Amount::from_sat(200_000), Script::from_bytes(vec![0xa9])),
        ];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);
        let script_code = Script::from_bytes(vec![0x76, 0xa9, 0x14]);

        let hash_all =
            witness_v0_signature_hash(&script_code, &tx, 0, SIGHASH_ALL, 100_000, &precomputed);
        let hash_none =
            witness_v0_signature_hash(&script_code, &tx, 0, SIGHASH_NONE, 100_000, &precomputed);

        assert_ne!(hash_all, hash_none);
    }

    #[test]
    fn test_witness_v0_sighash_single() {
        let tx = make_multi_tx();
        let spent_outputs = vec![
            TxOut::new(Amount::from_sat(100_000), Script::from_bytes(vec![0x76])),
            TxOut::new(Amount::from_sat(200_000), Script::from_bytes(vec![0xa9])),
        ];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);
        let script_code = Script::from_bytes(vec![0x76, 0xa9, 0x14]);

        let hash_all =
            witness_v0_signature_hash(&script_code, &tx, 0, SIGHASH_ALL, 100_000, &precomputed);
        let hash_single =
            witness_v0_signature_hash(&script_code, &tx, 0, SIGHASH_SINGLE, 100_000, &precomputed);

        assert_ne!(hash_all, hash_single);
    }

    #[test]
    fn test_witness_v0_sighash_anyonecanpay() {
        let tx = make_multi_tx();
        let spent_outputs = vec![
            TxOut::new(Amount::from_sat(100_000), Script::from_bytes(vec![0x76])),
            TxOut::new(Amount::from_sat(200_000), Script::from_bytes(vec![0xa9])),
        ];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);
        let script_code = Script::from_bytes(vec![0x76, 0xa9, 0x14]);

        let hash_all =
            witness_v0_signature_hash(&script_code, &tx, 0, SIGHASH_ALL, 100_000, &precomputed);
        let hash_acp = witness_v0_signature_hash(
            &script_code,
            &tx,
            0,
            SIGHASH_ALL | SIGHASH_ANYONECANPAY,
            100_000,
            &precomputed,
        );

        assert_ne!(hash_all, hash_acp);
    }

    #[test]
    fn test_witness_v0_different_amounts() {
        let tx = make_multi_tx();
        let spent_outputs = vec![
            TxOut::new(Amount::from_sat(100_000), Script::from_bytes(vec![0x76])),
            TxOut::new(Amount::from_sat(200_000), Script::from_bytes(vec![0xa9])),
        ];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);
        let script_code = Script::from_bytes(vec![0x76, 0xa9, 0x14]);

        let hash1 =
            witness_v0_signature_hash(&script_code, &tx, 0, SIGHASH_ALL, 100_000, &precomputed);
        let hash2 =
            witness_v0_signature_hash(&script_code, &tx, 0, SIGHASH_ALL, 200_000, &precomputed);

        // Different amounts should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_remove_codeseparators_empty() {
        let script = Script::new();
        let result = remove_codeseparators(&script);
        assert_eq!(result.as_bytes(), script.as_bytes());
    }

    #[test]
    fn test_remove_codeseparators_no_codesep() {
        // P2PKH script has no OP_CODESEPARATOR
        let script = Script::from_bytes(vec![
            Opcode::OpDup as u8,
            Opcode::OpHash160 as u8,
            0x14, // push 20 bytes
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            Opcode::OpEqualVerify as u8,
            Opcode::OpCheckSig as u8,
        ]);
        let result = remove_codeseparators(&script);
        assert_eq!(result.as_bytes(), script.as_bytes());
    }

    #[test]
    fn test_remove_codeseparators_with_codesep() {
        // Script with OP_CODESEPARATOR in the middle
        let script = Script::from_bytes(vec![
            Opcode::OpDup as u8,
            Opcode::OpCodeSeparator as u8,
            Opcode::OpHash160 as u8,
        ]);
        let result = remove_codeseparators(&script);
        assert_eq!(
            result.as_bytes(),
            &[Opcode::OpDup as u8, Opcode::OpHash160 as u8]
        );
    }

    #[test]
    fn test_remove_codeseparators_multiple() {
        // Script with multiple OP_CODESEPARATOR
        let script = Script::from_bytes(vec![
            Opcode::OpCodeSeparator as u8,
            Opcode::OpDup as u8,
            Opcode::OpCodeSeparator as u8,
            Opcode::OpHash160 as u8,
            Opcode::OpCodeSeparator as u8,
        ]);
        let result = remove_codeseparators(&script);
        assert_eq!(
            result.as_bytes(),
            &[Opcode::OpDup as u8, Opcode::OpHash160 as u8]
        );
    }

    #[test]
    fn test_remove_codeseparators_with_data_push() {
        // OP_CODESEPARATOR opcode byte (0xab) inside a data push should NOT be removed
        let script = Script::from_bytes(vec![
            0x02, // push 2 bytes
            0xab,
            0xcd,                          // data (0xab is OP_CODESEPARATOR but here it's data)
            Opcode::OpCodeSeparator as u8, // this one should be removed
            Opcode::OpDup as u8,
        ]);
        let result = remove_codeseparators(&script);
        assert_eq!(result.as_bytes(), &[0x02, 0xab, 0xcd, Opcode::OpDup as u8]);
    }

    #[test]
    fn test_taproot_sighash_basic() {
        let tx = make_multi_tx();
        let spent_outputs = vec![
            TxOut::new(
                Amount::from_sat(100_000),
                Script::from_bytes(vec![0x51, 0x20]),
            ),
            TxOut::new(
                Amount::from_sat(200_000),
                Script::from_bytes(vec![0x51, 0x20]),
            ),
        ];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);

        let hash = taproot_signature_hash(
            &tx,
            0,
            0, // 0 means default = SIGHASH_ALL
            &precomputed,
            0,
            None,
            None,
            None,
        )
        .expect("valid hash_type");

        assert_ne!(hash, Uint256::ZERO);

        // Deterministic
        let hash2 = taproot_signature_hash(&tx, 0, 0, &precomputed, 0, None, None, None)
            .expect("valid hash_type");
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_taproot_sighash_tapscript() {
        let tx = make_multi_tx();
        let spent_outputs = vec![
            TxOut::new(
                Amount::from_sat(100_000),
                Script::from_bytes(vec![0x51, 0x20]),
            ),
            TxOut::new(
                Amount::from_sat(200_000),
                Script::from_bytes(vec![0x51, 0x20]),
            ),
        ];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);

        let tapleaf_hash = [0x42u8; 32];

        // Key path (ext_flag=0) vs tapscript (ext_flag=1) should differ
        let hash_key = taproot_signature_hash(&tx, 0, 0, &precomputed, 0, None, None, None)
            .expect("valid hash_type");
        let hash_script = taproot_signature_hash(
            &tx,
            0,
            0,
            &precomputed,
            1,
            Some(&tapleaf_hash),
            Some(0xFFFFFFFF),
            None,
        )
        .expect("valid hash_type");

        assert_ne!(hash_key, hash_script);
    }

    #[test]
    fn test_is_valid_signature_hash_type() {
        assert!(is_valid_signature_hash_type(SIGHASH_ALL));
        assert!(is_valid_signature_hash_type(SIGHASH_NONE));
        assert!(is_valid_signature_hash_type(SIGHASH_SINGLE));
        assert!(is_valid_signature_hash_type(
            SIGHASH_ALL | SIGHASH_ANYONECANPAY
        ));
        assert!(is_valid_signature_hash_type(
            SIGHASH_NONE | SIGHASH_ANYONECANPAY
        ));
        assert!(is_valid_signature_hash_type(
            SIGHASH_SINGLE | SIGHASH_ANYONECANPAY
        ));
        assert!(!is_valid_signature_hash_type(0)); // 0 is not valid
        assert!(!is_valid_signature_hash_type(4)); // 4 is not a valid base type
    }

    #[test]
    fn test_sighash_all_vs_none_vs_single_all_different() {
        let tx = make_multi_tx();
        let script_code = Script::from_bytes(vec![0x76, 0xa9]);

        let h_all = signature_hash(&script_code, &tx, 0, SIGHASH_ALL);
        let h_none = signature_hash(&script_code, &tx, 0, SIGHASH_NONE);
        let h_single = signature_hash(&script_code, &tx, 0, SIGHASH_SINGLE);
        let h_acp = signature_hash(&script_code, &tx, 0, SIGHASH_ALL | SIGHASH_ANYONECANPAY);

        // All four should be different
        let hashes = [h_all, h_none, h_single, h_acp];
        for i in 0..hashes.len() {
            for j in (i + 1)..hashes.len() {
                assert_ne!(hashes[i], hashes[j], "Hash {} and {} should differ", i, j);
            }
        }
    }

    #[test]
    fn test_witness_v0_sighash_single_no_matching_output() {
        // When SIGHASH_SINGLE and input_index >= vout.len(), should get zeros for hashOutputs
        let tx = Transaction::new(
            2,
            vec![
                TxIn::new(
                    OutPoint::new(Txid::from_bytes([0xaa; 32]), 0),
                    Script::new(),
                    SEQUENCE_FINAL,
                ),
                TxIn::new(
                    OutPoint::new(Txid::from_bytes([0xbb; 32]), 1),
                    Script::new(),
                    SEQUENCE_FINAL,
                ),
            ],
            vec![TxOut::new(
                Amount::from_sat(50_000),
                Script::from_bytes(vec![0x76]),
            )],
            0,
        );
        let spent_outputs = vec![
            TxOut::new(Amount::from_sat(100_000), Script::from_bytes(vec![0x76])),
            TxOut::new(Amount::from_sat(200_000), Script::from_bytes(vec![0xa9])),
        ];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);
        let script_code = Script::from_bytes(vec![0x76, 0xa9, 0x14]);

        // input_index=1 but only 1 output
        let hash =
            witness_v0_signature_hash(&script_code, &tx, 1, SIGHASH_SINGLE, 200_000, &precomputed);

        // Should still produce a valid (non-zero) hash
        assert_ne!(hash, Uint256::ZERO);
    }
}
