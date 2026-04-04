//! Transaction signature verification.
//! Maps to: TransactionSignatureChecker in interpreter.cpp
//!
//! Provides the `TransactionSignatureChecker` which implements the
//! `SignatureChecker` trait from qubitcoin-script, enabling actual
//! ECDSA and Schnorr signature verification during script execution.

use crate::sighash::{
    signature_hash, taproot_signature_hash, witness_v0_signature_hash, PrecomputedTransactionData,
    SIGHASH_OUTPUT_MASK,
};
use crate::transaction::Transaction;
use qubitcoin_primitives::Uint256;
use qubitcoin_script::{
    Script, ScriptError, ScriptExecutionData, ScriptNum, SigVersion, SignatureChecker,
};

/// Checker that verifies signatures against a specific transaction input.
///
/// This is the core signature verification component used during script
/// evaluation. It computes the appropriate sighash for the signature version
/// and verifies the cryptographic signature.
pub struct TransactionSignatureChecker<'a> {
    tx: &'a Transaction,
    input_index: usize,
    amount: i64,
    precomputed: &'a PrecomputedTransactionData,
}

impl<'a> TransactionSignatureChecker<'a> {
    /// Create a new checker for a specific transaction input.
    ///
    /// # Arguments
    /// * `tx` - The transaction being verified
    /// * `input_index` - The index of the input being verified
    /// * `amount` - The value of the output being spent (needed for segwit sighash)
    /// * `precomputed` - Precomputed hash data for the transaction
    pub fn new(
        tx: &'a Transaction,
        input_index: usize,
        amount: i64,
        precomputed: &'a PrecomputedTransactionData,
    ) -> Self {
        TransactionSignatureChecker {
            tx,
            input_index,
            amount,
            precomputed,
        }
    }
}

impl<'a> SignatureChecker for TransactionSignatureChecker<'a> {
    fn check_ecdsa_signature(
        &self,
        sig: &[u8],
        pubkey: &[u8],
        script_code: &Script,
        sigversion: SigVersion,
    ) -> bool {
        if sig.is_empty() || pubkey.is_empty() {
            return false;
        }

        // Extract hash_type from last byte of signature
        let hash_type = *sig.last().unwrap() as u32;
        let sig_data = &sig[..sig.len() - 1]; // DER signature without hash_type byte

        // Compute sighash based on sig version
        let sighash = match sigversion {
            SigVersion::WitnessV0 => witness_v0_signature_hash(
                script_code,
                self.tx,
                self.input_index,
                hash_type,
                self.amount,
                self.precomputed,
            ),
            _ => {
                // Legacy (SigVersion::Base)
                signature_hash(script_code, self.tx, self.input_index, hash_type)
            }
        };

        // Verify ECDSA signature
        verify_ecdsa_signature(sig_data, pubkey, &sighash)
    }

    fn check_schnorr_signature(
        &self,
        sig: &[u8],
        pubkey: &[u8],
        sigversion: SigVersion,
        exec_data: &ScriptExecutionData,
        error: &mut ScriptError,
    ) -> bool {
        // Schnorr pubkeys must be 32 bytes (x-only)
        if pubkey.len() != 32 {
            *error = ScriptError::SchnorrSig;
            return false;
        }

        // Extract hash_type and signature data
        let (hash_type, sig_data) = if sig.len() == 64 {
            // 64-byte signature implies SIGHASH_DEFAULT (= SIGHASH_ALL)
            (0u32, &sig[..64])
        } else if sig.len() == 65 {
            // 65-byte signature has explicit hash_type as last byte
            let ht = sig[64] as u32;
            if ht == 0 {
                // hash_type 0 is not valid with explicit byte
                *error = ScriptError::SchnorrSigHashtype;
                return false;
            }
            (ht, &sig[..64])
        } else {
            *error = ScriptError::SchnorrSigSize;
            return false;
        };

        // Validate hash type for taproot
        if hash_type != 0 {
            let base = hash_type & SIGHASH_OUTPUT_MASK;
            if base < 1 || base > 3 {
                *error = ScriptError::SchnorrSigHashtype;
                return false;
            }
        }

        let ext_flag = match sigversion {
            SigVersion::Taproot => 0,
            SigVersion::Tapscript => 1,
            _ => {
                *error = ScriptError::SchnorrSig;
                return false;
            }
        };

        let tapleaf_hash = if exec_data.tapleaf_hash_init {
            Some(&exec_data.tapleaf_hash)
        } else {
            None
        };

        let codesep_pos = if exec_data.codeseparator_pos_init {
            Some(exec_data.codeseparator_pos)
        } else {
            None
        };

        let annex_hash = if exec_data.annex_init && exec_data.annex_present {
            Some(&exec_data.annex_hash)
        } else {
            None
        };

        let sighash = match taproot_signature_hash(
            self.tx,
            self.input_index,
            hash_type,
            self.precomputed,
            ext_flag,
            tapleaf_hash,
            codesep_pos,
            annex_hash,
        ) {
            Some(h) => h,
            None => {
                *error = ScriptError::SchnorrSigHashtype;
                return false;
            }
        };

        if !verify_schnorr_signature(sig_data, pubkey, &sighash) {
            *error = ScriptError::SchnorrSig;
            return false;
        }

        true
    }

    fn check_lock_time(&self, lock_time: &ScriptNum) -> bool {
        // BIP65: OP_CHECKLOCKTIMEVERIFY
        let lock_time_val = lock_time.get_i64();

        // Reject negative locktime
        if lock_time_val < 0 {
            return false;
        }

        let tx_lock_time = self.tx.lock_time as i64;

        // Both must be same type (height < 500_000_000 vs time >= 500_000_000)
        if (tx_lock_time < 500_000_000 && lock_time_val >= 500_000_000)
            || (tx_lock_time >= 500_000_000 && lock_time_val < 500_000_000)
        {
            return false;
        }

        // The lock time on the stack must not exceed the transaction's lock time
        if lock_time_val > tx_lock_time {
            return false;
        }

        // The input must not be finalized (sequence must not be 0xFFFFFFFF)
        self.tx.vin[self.input_index].sequence != 0xFFFFFFFF
    }

    fn check_sequence(&self, sequence: &ScriptNum) -> bool {
        // BIP112: OP_CHECKSEQUENCEVERIFY
        let seq_val = sequence.get_i64();

        // Reject negative sequence values
        if seq_val < 0 {
            return false;
        }

        let seq_val = seq_val as u32;

        // If the disable flag is set, the lock-time is disabled, so pass
        if seq_val & (1 << 31) != 0 {
            return true;
        }

        // Transaction version must be >= 2 for BIP68/BIP112
        if self.tx.version < 2 {
            return false;
        }

        let tx_seq = self.tx.vin[self.input_index].sequence;

        // If the transaction's sequence has the disable flag, fail
        if tx_seq & (1 << 31) != 0 {
            return false;
        }

        // Both must be same type (time-based vs height-based)
        if (seq_val & (1 << 22)) != (tx_seq & (1 << 22)) {
            return false;
        }

        // The sequence value on the stack must not exceed the transaction input's sequence
        (seq_val & 0xffff) <= (tx_seq & 0xffff)
    }
}

/// Verify an ECDSA signature against a public key and message hash.
fn verify_ecdsa_signature(sig: &[u8], pubkey: &[u8], hash: &Uint256) -> bool {
    use secp256k1::{ecdsa::Signature, Message, PublicKey, Secp256k1};

    let secp = Secp256k1::verification_only();
    let msg = match Message::from_digest_slice(hash.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let signature = match Signature::from_der(sig) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let pk = match PublicKey::from_slice(pubkey) {
        Ok(p) => p,
        Err(_) => return false,
    };
    secp.verify_ecdsa(&msg, &signature, &pk).is_ok()
}

/// Verify a Schnorr signature against an x-only public key and message hash.
fn verify_schnorr_signature(sig: &[u8], pubkey: &[u8], hash: &Uint256) -> bool {
    use secp256k1::{schnorr::Signature, Message, Secp256k1, XOnlyPublicKey};

    let secp = Secp256k1::verification_only();
    let msg = match Message::from_digest_slice(hash.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let sig_bytes: [u8; 64] = match sig.try_into() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let signature = match Signature::from_slice(&sig_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let pk = match XOnlyPublicKey::from_slice(pubkey) {
        Ok(p) => p,
        Err(_) => return false,
    };
    secp.verify_schnorr(&signature, &msg, &pk).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sighash::{
        signature_hash, witness_v0_signature_hash, PrecomputedTransactionData, SIGHASH_ALL,
        SIGHASH_ANYONECANPAY, SIGHASH_NONE, SIGHASH_SINGLE,
    };
    use crate::transaction::{OutPoint, TxIn, TxOut, SEQUENCE_FINAL};
    use qubitcoin_primitives::{Amount, Txid};
    use qubitcoin_script::Script;

    /// Helper: build a simple 1-in 1-out transaction.
    fn make_simple_tx(lock_time: u32, sequence: u32) -> Transaction {
        Transaction::new(
            2,
            vec![TxIn::new(
                OutPoint::new(Txid::from_bytes([0xaa; 32]), 0),
                Script::new(),
                sequence,
            )],
            vec![TxOut::new(
                Amount::from_sat(50_000),
                Script::from_bytes(vec![0x76, 0xa9]),
            )],
            lock_time,
        )
    }

    /// Helper: build a multi-input transaction for sequence tests.
    fn make_versioned_tx(version: u32, lock_time: u32, sequences: &[u32]) -> Transaction {
        let inputs: Vec<TxIn> = sequences
            .iter()
            .enumerate()
            .map(|(i, &seq)| {
                TxIn::new(
                    OutPoint::new(Txid::from_bytes([(i as u8) + 1; 32]), i as u32),
                    Script::new(),
                    seq,
                )
            })
            .collect();
        Transaction::new(
            version,
            inputs,
            vec![TxOut::new(
                Amount::from_sat(50_000),
                Script::from_bytes(vec![0x76, 0xa9]),
            )],
            lock_time,
        )
    }

    // ---------------------------------------------------------------
    // check_lock_time tests (BIP65: CHECKLOCKTIMEVERIFY)
    // ---------------------------------------------------------------

    #[test]
    fn test_check_lock_time_negative() {
        let tx = make_simple_tx(100, 0xfffffffe);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        // Negative lock time should fail
        assert!(!checker.check_lock_time(&ScriptNum::new(-1)));
    }

    #[test]
    fn test_check_lock_time_height_satisfied() {
        // tx.lock_time = 100 (block height), script lock_time = 50
        let tx = make_simple_tx(100, 0xfffffffe);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        // lock_time_val (50) <= tx_lock_time (100): should pass
        assert!(checker.check_lock_time(&ScriptNum::new(50)));
        assert!(checker.check_lock_time(&ScriptNum::new(100)));
    }

    #[test]
    fn test_check_lock_time_height_unsatisfied() {
        // tx.lock_time = 100, script lock_time = 200
        let tx = make_simple_tx(100, 0xfffffffe);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        // lock_time_val (200) > tx_lock_time (100): should fail
        assert!(!checker.check_lock_time(&ScriptNum::new(200)));
    }

    #[test]
    fn test_check_lock_time_time_satisfied() {
        // tx.lock_time = 1_600_000_000 (timestamp), script lock_time = 1_500_000_000
        let tx = make_simple_tx(1_600_000_000, 0xfffffffe);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        assert!(checker.check_lock_time(&ScriptNum::new(1_500_000_000)));
        assert!(checker.check_lock_time(&ScriptNum::new(1_600_000_000)));
    }

    #[test]
    fn test_check_lock_time_type_mismatch() {
        // tx.lock_time is height (100), script lock_time is time (500_000_001)
        let tx = make_simple_tx(100, 0xfffffffe);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        // Type mismatch: should fail
        assert!(!checker.check_lock_time(&ScriptNum::new(500_000_001)));
    }

    #[test]
    fn test_check_lock_time_final_sequence() {
        // Sequence = 0xFFFFFFFF means input is final -> should fail
        let tx = make_simple_tx(100, SEQUENCE_FINAL);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        assert!(!checker.check_lock_time(&ScriptNum::new(50)));
    }

    #[test]
    fn test_check_lock_time_zero() {
        let tx = make_simple_tx(100, 0xfffffffe);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        // Zero lock time should work (0 <= 100)
        assert!(checker.check_lock_time(&ScriptNum::new(0)));
    }

    // ---------------------------------------------------------------
    // check_sequence tests (BIP112: CHECKSEQUENCEVERIFY)
    // ---------------------------------------------------------------

    #[test]
    fn test_check_sequence_negative() {
        let tx = make_versioned_tx(2, 0, &[100]);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        assert!(!checker.check_sequence(&ScriptNum::new(-1)));
    }

    #[test]
    fn test_check_sequence_disabled() {
        // When the disable flag is set on the script sequence, it should always pass
        let tx = make_versioned_tx(2, 0, &[100]);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        // 1 << 31 = disable flag set -> should always pass
        assert!(checker.check_sequence(&ScriptNum::new(1 << 31)));
    }

    #[test]
    fn test_check_sequence_version1_fails() {
        // Transaction version < 2 should fail
        let tx = make_versioned_tx(1, 0, &[100]);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        assert!(!checker.check_sequence(&ScriptNum::new(50)));
    }

    #[test]
    fn test_check_sequence_height_satisfied() {
        // tx input sequence = 100 (height-based), script sequence = 50
        let tx = make_versioned_tx(2, 0, &[100]);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        // 50 <= 100: should pass
        assert!(checker.check_sequence(&ScriptNum::new(50)));
        assert!(checker.check_sequence(&ScriptNum::new(100)));
    }

    #[test]
    fn test_check_sequence_height_unsatisfied() {
        // tx input sequence = 50, script sequence = 100
        let tx = make_versioned_tx(2, 0, &[50]);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        // 100 > 50: should fail
        assert!(!checker.check_sequence(&ScriptNum::new(100)));
    }

    #[test]
    fn test_check_sequence_type_mismatch() {
        // Script uses time-based (bit 22 set), tx uses height-based (bit 22 clear)
        let tx = make_versioned_tx(2, 0, &[100]); // height-based
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        // Set time-based flag (1 << 22 = 0x400000)
        let time_seq = 50 | (1 << 22);
        assert!(!checker.check_sequence(&ScriptNum::new(time_seq)));
    }

    #[test]
    fn test_check_sequence_time_satisfied() {
        // Both tx and script use time-based relative locktime
        let time_flag = 1u32 << 22;
        let tx = make_versioned_tx(2, 0, &[time_flag | 100]);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        // Both have time flag set, 50 <= 100: should pass
        let script_seq = (time_flag | 50) as i64;
        assert!(checker.check_sequence(&ScriptNum::new(script_seq)));
    }

    #[test]
    fn test_check_sequence_tx_disabled() {
        // When the tx input has disable flag set, should fail
        let tx = make_versioned_tx(2, 0, &[1u32 << 31 | 100]);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        assert!(!checker.check_sequence(&ScriptNum::new(50)));
    }

    // ---------------------------------------------------------------
    // ECDSA signature verification tests
    // ---------------------------------------------------------------

    #[test]
    fn test_check_ecdsa_empty_sig() {
        let tx = make_simple_tx(0, SEQUENCE_FINAL);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        let script_code = Script::from_bytes(vec![0x76, 0xa9]);
        assert!(!checker.check_ecdsa_signature(&[], &[0x02; 33], &script_code, SigVersion::Base));
    }

    #[test]
    fn test_check_ecdsa_empty_pubkey() {
        let tx = make_simple_tx(0, SEQUENCE_FINAL);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        let script_code = Script::from_bytes(vec![0x76, 0xa9]);
        assert!(!checker.check_ecdsa_signature(&[0x30, 0x01], &[], &script_code, SigVersion::Base));
    }

    #[test]
    fn test_check_ecdsa_invalid_sig() {
        let tx = make_simple_tx(0, SEQUENCE_FINAL);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        let script_code = Script::from_bytes(vec![0x76, 0xa9]);
        // Garbage signature and pubkey - should fail gracefully
        let garbage_sig = vec![
            0x30,
            0x06,
            0x02,
            0x01,
            0x01,
            0x02,
            0x01,
            0x01,
            SIGHASH_ALL as u8,
        ];
        let garbage_pubkey = vec![0x02; 33];
        assert!(!checker.check_ecdsa_signature(
            &garbage_sig,
            &garbage_pubkey,
            &script_code,
            SigVersion::Base,
        ));
    }

    #[test]
    fn test_check_schnorr_wrong_pubkey_len() {
        let tx = make_simple_tx(0, SEQUENCE_FINAL);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        let exec_data = ScriptExecutionData::default();
        let mut error = ScriptError::Ok;

        // 33-byte pubkey (compressed) is wrong for Schnorr (needs 32)
        assert!(!checker.check_schnorr_signature(
            &[0u8; 64],
            &[0x02; 33],
            SigVersion::Taproot,
            &exec_data,
            &mut error,
        ));
        assert_eq!(error, ScriptError::SchnorrSig);
    }

    #[test]
    fn test_check_schnorr_wrong_sig_len() {
        let tx = make_simple_tx(0, SEQUENCE_FINAL);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        let exec_data = ScriptExecutionData::default();
        let mut error = ScriptError::Ok;

        // 63-byte signature is invalid
        assert!(!checker.check_schnorr_signature(
            &[0u8; 63],
            &[0x01; 32],
            SigVersion::Taproot,
            &exec_data,
            &mut error,
        ));
        assert_eq!(error, ScriptError::SchnorrSigSize);
    }

    #[test]
    fn test_check_schnorr_explicit_zero_hashtype() {
        let tx = make_simple_tx(0, SEQUENCE_FINAL);
        let spent = vec![TxOut::new(
            Amount::from_sat(50_000),
            Script::from_bytes(vec![0x51, 0x20]),
        )];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent);
        let checker = TransactionSignatureChecker::new(&tx, 0, 50_000, &precomputed);

        let exec_data = ScriptExecutionData::default();
        let mut error = ScriptError::Ok;

        // 65-byte sig with explicit hash_type=0 is invalid
        let mut sig = [0u8; 65];
        sig[64] = 0; // hash_type = 0 explicit
        assert!(!checker.check_schnorr_signature(
            &sig,
            &[0x01; 32],
            SigVersion::Taproot,
            &exec_data,
            &mut error,
        ));
        assert_eq!(error, ScriptError::SchnorrSigHashtype);
    }

    #[test]
    fn test_check_schnorr_invalid_base_hashtype() {
        let tx = make_simple_tx(0, SEQUENCE_FINAL);
        let spent = vec![TxOut::new(
            Amount::from_sat(50_000),
            Script::from_bytes(vec![0x51, 0x20]),
        )];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent);
        let checker = TransactionSignatureChecker::new(&tx, 0, 50_000, &precomputed);

        let exec_data = ScriptExecutionData::default();
        let mut error = ScriptError::Ok;

        // 65-byte sig with invalid base hash_type = 4
        let mut sig = [0u8; 65];
        sig[64] = 4; // base type 4 is invalid
        assert!(!checker.check_schnorr_signature(
            &sig,
            &[0x01; 32],
            SigVersion::Taproot,
            &exec_data,
            &mut error,
        ));
        assert_eq!(error, ScriptError::SchnorrSigHashtype);
    }

    #[test]
    fn test_check_schnorr_wrong_sigversion() {
        let tx = make_simple_tx(0, SEQUENCE_FINAL);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);

        let exec_data = ScriptExecutionData::default();
        let mut error = ScriptError::Ok;

        // SigVersion::Base is not valid for Schnorr
        assert!(!checker.check_schnorr_signature(
            &[0u8; 64],
            &[0x01; 32],
            SigVersion::Base,
            &exec_data,
            &mut error,
        ));
        assert_eq!(error, ScriptError::SchnorrSig);
    }

    // ---------------------------------------------------------------
    // End-to-end: sign and verify with real keys
    // ---------------------------------------------------------------

    #[test]
    fn test_ecdsa_sign_and_verify() {
        use secp256k1::{Message, Secp256k1, SecretKey};

        let secp = Secp256k1::new();

        // Generate a keypair
        let sk_bytes = [
            0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
            0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
            0x01, 0x01, 0x01, 0x02,
        ];
        let sk = SecretKey::from_slice(&sk_bytes).unwrap();
        let pk = secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let pk_bytes = pk.serialize(); // compressed 33 bytes

        // Build a transaction
        let tx = Transaction::new(
            1,
            vec![TxIn::new(
                OutPoint::new(Txid::from_bytes([0xcc; 32]), 0),
                Script::new(),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(
                Amount::from_sat(40_000),
                Script::from_bytes(vec![0x76, 0xa9]),
            )],
            0,
        );

        let script_code = Script::from_bytes(vec![0x76, 0xa9, 0x14]);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);

        // Compute the sighash
        let sighash = signature_hash(&script_code, &tx, 0, SIGHASH_ALL);

        // Sign
        let msg = Message::from_digest_slice(sighash.as_bytes()).unwrap();
        let ecdsa_sig = secp.sign_ecdsa(&msg, &sk);

        // Build the full signature (DER + hash_type byte)
        let mut sig_bytes = ecdsa_sig.serialize_der().to_vec();
        sig_bytes.push(SIGHASH_ALL as u8);

        // Verify using TransactionSignatureChecker
        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);
        assert!(checker.check_ecdsa_signature(
            &sig_bytes,
            &pk_bytes,
            &script_code,
            SigVersion::Base,
        ));

        // Verify that a wrong pubkey fails
        let wrong_pk = vec![0x02; 33];
        assert!(!checker.check_ecdsa_signature(
            &sig_bytes,
            &wrong_pk,
            &script_code,
            SigVersion::Base,
        ));

        // Verify that a tampered signature fails
        let mut tampered_sig = sig_bytes.clone();
        tampered_sig[5] ^= 0x01;
        assert!(!checker.check_ecdsa_signature(
            &tampered_sig,
            &pk_bytes,
            &script_code,
            SigVersion::Base,
        ));
    }

    #[test]
    fn test_ecdsa_sign_and_verify_witness_v0() {
        use secp256k1::{Message, Secp256k1, SecretKey};

        let secp = Secp256k1::new();

        let sk_bytes = [
            0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02,
            0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02,
            0x02, 0x02, 0x02, 0x03,
        ];
        let sk = SecretKey::from_slice(&sk_bytes).unwrap();
        let pk = secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let pk_bytes = pk.serialize();

        let amount = 100_000i64;
        let tx = Transaction::new(
            2,
            vec![TxIn::new(
                OutPoint::new(Txid::from_bytes([0xdd; 32]), 0),
                Script::new(),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(
                Amount::from_sat(90_000),
                Script::from_bytes(vec![0x00, 0x14]),
            )],
            0,
        );

        let spent_outputs = vec![TxOut::new(
            Amount::from_sat(amount),
            Script::from_bytes(vec![0x00, 0x14]),
        )];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);

        // Use witness v0 script code format for P2WPKH
        let script_code = Script::from_bytes(vec![
            0x76, 0xa9, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x88, 0xac,
        ]);

        let sighash =
            witness_v0_signature_hash(&script_code, &tx, 0, SIGHASH_ALL, amount, &precomputed);

        let msg = Message::from_digest_slice(sighash.as_bytes()).unwrap();
        let ecdsa_sig = secp.sign_ecdsa(&msg, &sk);

        let mut sig_bytes = ecdsa_sig.serialize_der().to_vec();
        sig_bytes.push(SIGHASH_ALL as u8);

        let checker = TransactionSignatureChecker::new(&tx, 0, amount, &precomputed);
        assert!(checker.check_ecdsa_signature(
            &sig_bytes,
            &pk_bytes,
            &script_code,
            SigVersion::WitnessV0,
        ));

        // Wrong amount should produce wrong hash and fail
        let _wrong_checker = TransactionSignatureChecker::new(&tx, 0, 50_000, &precomputed);
        // The sighash computed by the checker uses self.amount, but for WitnessV0 it matters
        let wrong_sighash =
            witness_v0_signature_hash(&script_code, &tx, 0, SIGHASH_ALL, 50_000, &precomputed);
        assert_ne!(sighash, wrong_sighash);
    }

    #[test]
    fn test_ecdsa_sign_sighash_none() {
        use secp256k1::{Message, Secp256k1, SecretKey};

        let secp = Secp256k1::new();

        let sk_bytes = [
            0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03,
            0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03, 0x03,
            0x03, 0x03, 0x03, 0x04,
        ];
        let sk = SecretKey::from_slice(&sk_bytes).unwrap();
        let pk = secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let pk_bytes = pk.serialize();

        let tx = Transaction::new(
            1,
            vec![TxIn::new(
                OutPoint::new(Txid::from_bytes([0xee; 32]), 0),
                Script::new(),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(
                Amount::from_sat(40_000),
                Script::from_bytes(vec![0x76, 0xa9]),
            )],
            0,
        );

        let script_code = Script::from_bytes(vec![0x76, 0xa9, 0x14]);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);

        let sighash = signature_hash(&script_code, &tx, 0, SIGHASH_NONE);
        let msg = Message::from_digest_slice(sighash.as_bytes()).unwrap();
        let ecdsa_sig = secp.sign_ecdsa(&msg, &sk);

        let mut sig_bytes = ecdsa_sig.serialize_der().to_vec();
        sig_bytes.push(SIGHASH_NONE as u8);

        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);
        assert!(checker.check_ecdsa_signature(
            &sig_bytes,
            &pk_bytes,
            &script_code,
            SigVersion::Base,
        ));
    }

    #[test]
    fn test_ecdsa_sign_sighash_single() {
        use secp256k1::{Message, Secp256k1, SecretKey};

        let secp = Secp256k1::new();

        let sk_bytes = [
            0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
            0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
            0x04, 0x04, 0x04, 0x05,
        ];
        let sk = SecretKey::from_slice(&sk_bytes).unwrap();
        let pk = secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let pk_bytes = pk.serialize();

        let tx = Transaction::new(
            1,
            vec![TxIn::new(
                OutPoint::new(Txid::from_bytes([0xff; 32]), 0),
                Script::new(),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(
                Amount::from_sat(40_000),
                Script::from_bytes(vec![0x76, 0xa9]),
            )],
            0,
        );

        let script_code = Script::from_bytes(vec![0x76, 0xa9, 0x14]);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);

        let sighash = signature_hash(&script_code, &tx, 0, SIGHASH_SINGLE);
        let msg = Message::from_digest_slice(sighash.as_bytes()).unwrap();
        let ecdsa_sig = secp.sign_ecdsa(&msg, &sk);

        let mut sig_bytes = ecdsa_sig.serialize_der().to_vec();
        sig_bytes.push(SIGHASH_SINGLE as u8);

        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);
        assert!(checker.check_ecdsa_signature(
            &sig_bytes,
            &pk_bytes,
            &script_code,
            SigVersion::Base,
        ));
    }

    #[test]
    fn test_ecdsa_sign_sighash_anyonecanpay() {
        use secp256k1::{Message, Secp256k1, SecretKey};

        let secp = Secp256k1::new();

        let sk_bytes = [
            0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05,
            0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05,
            0x05, 0x05, 0x05, 0x06,
        ];
        let sk = SecretKey::from_slice(&sk_bytes).unwrap();
        let pk = secp256k1::PublicKey::from_secret_key(&secp, &sk);
        let pk_bytes = pk.serialize();

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
                Amount::from_sat(80_000),
                Script::from_bytes(vec![0x76, 0xa9]),
            )],
            0,
        );

        let script_code = Script::from_bytes(vec![0x76, 0xa9, 0x14]);
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);

        let hash_type = SIGHASH_ALL | SIGHASH_ANYONECANPAY;
        let sighash = signature_hash(&script_code, &tx, 0, hash_type);
        let msg = Message::from_digest_slice(sighash.as_bytes()).unwrap();
        let ecdsa_sig = secp.sign_ecdsa(&msg, &sk);

        let mut sig_bytes = ecdsa_sig.serialize_der().to_vec();
        sig_bytes.push(hash_type as u8);

        let checker = TransactionSignatureChecker::new(&tx, 0, 0, &precomputed);
        assert!(checker.check_ecdsa_signature(
            &sig_bytes,
            &pk_bytes,
            &script_code,
            SigVersion::Base,
        ));
    }

    #[test]
    fn test_schnorr_sign_and_verify() {
        use secp256k1::{Keypair, Message, Secp256k1, XOnlyPublicKey};

        let secp = Secp256k1::new();

        let sk_bytes = [
            0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06,
            0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06, 0x06,
            0x06, 0x06, 0x06, 0x07,
        ];
        let keypair = Keypair::from_seckey_slice(&secp, &sk_bytes).unwrap();
        let (xonly_pk, _parity) = XOnlyPublicKey::from_keypair(&keypair);
        let pk_bytes = xonly_pk.serialize(); // 32 bytes

        let tx = Transaction::new(
            2,
            vec![TxIn::new(
                OutPoint::new(Txid::from_bytes([0xdd; 32]), 0),
                Script::new(),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(
                Amount::from_sat(40_000),
                Script::from_bytes(vec![0x51, 0x20]),
            )],
            0,
        );

        let amount = 50_000i64;
        let spent_outputs = vec![TxOut::new(
            Amount::from_sat(amount),
            Script::from_bytes(vec![0x51, 0x20]),
        )];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);

        // Compute the taproot sighash (default = SIGHASH_ALL with hash_type=0)
        let sighash = taproot_signature_hash(&tx, 0, 0, &precomputed, 0, None, None, None)
            .expect("valid hash_type");

        let msg = Message::from_digest_slice(sighash.as_bytes()).unwrap();
        let schnorr_sig = secp.sign_schnorr(&msg, &keypair);
        let sig_bytes = schnorr_sig.as_ref().to_vec(); // 64 bytes = SIGHASH_DEFAULT

        // Verify using TransactionSignatureChecker
        let checker = TransactionSignatureChecker::new(&tx, 0, amount, &precomputed);
        let exec_data = ScriptExecutionData::default();
        let mut error = ScriptError::Ok;
        assert!(checker.check_schnorr_signature(
            &sig_bytes,
            &pk_bytes,
            SigVersion::Taproot,
            &exec_data,
            &mut error,
        ));
        assert_eq!(error, ScriptError::Ok);

        // Wrong pubkey should fail
        let wrong_pk = [0x01; 32];
        let mut error2 = ScriptError::Ok;
        assert!(!checker.check_schnorr_signature(
            &sig_bytes,
            &wrong_pk,
            SigVersion::Taproot,
            &exec_data,
            &mut error2,
        ));
    }

    #[test]
    fn test_schnorr_sign_explicit_hashtype() {
        use secp256k1::{Keypair, Message, Secp256k1, XOnlyPublicKey};

        let secp = Secp256k1::new();

        let sk_bytes = [
            0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07,
            0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07, 0x07,
            0x07, 0x07, 0x07, 0x08,
        ];
        let keypair = Keypair::from_seckey_slice(&secp, &sk_bytes).unwrap();
        let (xonly_pk, _parity) = XOnlyPublicKey::from_keypair(&keypair);
        let pk_bytes = xonly_pk.serialize();

        let tx = Transaction::new(
            2,
            vec![TxIn::new(
                OutPoint::new(Txid::from_bytes([0xee; 32]), 0),
                Script::new(),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(
                Amount::from_sat(40_000),
                Script::from_bytes(vec![0x51, 0x20]),
            )],
            0,
        );

        let amount = 50_000i64;
        let spent_outputs = vec![TxOut::new(
            Amount::from_sat(amount),
            Script::from_bytes(vec![0x51, 0x20]),
        )];
        let precomputed = PrecomputedTransactionData::new(&tx, &spent_outputs);

        // Explicit SIGHASH_ALL = 0x01
        let hash_type = SIGHASH_ALL;
        let sighash = taproot_signature_hash(&tx, 0, hash_type, &precomputed, 0, None, None, None)
            .expect("valid hash_type");

        let msg = Message::from_digest_slice(sighash.as_bytes()).unwrap();
        let schnorr_sig = secp.sign_schnorr(&msg, &keypair);
        let mut sig_bytes = schnorr_sig.as_ref().to_vec();
        sig_bytes.push(hash_type as u8); // 65 bytes with explicit hash_type

        let checker = TransactionSignatureChecker::new(&tx, 0, amount, &precomputed);
        let exec_data = ScriptExecutionData::default();
        let mut error = ScriptError::Ok;
        assert!(checker.check_schnorr_signature(
            &sig_bytes,
            &pk_bytes,
            SigVersion::Taproot,
            &exec_data,
            &mut error,
        ));
        assert_eq!(error, ScriptError::Ok);
    }
}
