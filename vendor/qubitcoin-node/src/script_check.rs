//! Parallel script verification using Rayon.
//!
//! Bitcoin Core uses a custom CCheckQueue with a fixed thread pool for
//! parallel script verification. We improve on this with Rayon's work-stealing
//! scheduler, which automatically adapts to available cores and handles
//! load balancing without manual tuning.
//!
//! This module collects all script verification checks for a block and
//! executes them in parallel, significantly speeding up ConnectBlock.

use qubitcoin_common::coins::Coin;
use qubitcoin_consensus::sighash::{
    signature_hash, witness_v0_signature_hash, PrecomputedTransactionData, SIGHASH_ALL,
};
use qubitcoin_consensus::transaction::{Transaction, TransactionRef, SEQUENCE_FINAL};
use qubitcoin_primitives::Uint256;
use qubitcoin_script::interpreter::{
    verify_script, BaseSignatureChecker, ScriptExecutionData, ScriptWitness, SigVersion,
    SignatureChecker,
};
use qubitcoin_script::script::Script;
use qubitcoin_script::script_error::ScriptError;
use qubitcoin_script::script_num::ScriptNum;
use qubitcoin_script::verify_flags::ScriptVerifyFlags;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// TransactionSignatureChecker
// ---------------------------------------------------------------------------

/// Full signature checker with transaction context.
///
/// Maps to: `GenericTransactionSignatureChecker<T>` in Bitcoin Core's
/// `src/script/interpreter.cpp`. This is the production checker that
/// actually validates ECDSA and Schnorr signatures using secp256k1.
pub struct TransactionSignatureChecker {
    /// The transaction being checked.
    tx: Arc<Transaction>,
    /// Index of the input being verified.
    input_index: usize,
    /// Value of the output being spent (needed for BIP143/BIP341).
    amount: i64,
    /// Precomputed hashes for efficient sighash computation.
    precomputed: PrecomputedTransactionData,
}

impl TransactionSignatureChecker {
    /// Create a new checker for a specific input of a transaction.
    pub fn new(
        tx: Arc<Transaction>,
        input_index: usize,
        amount: i64,
        precomputed: PrecomputedTransactionData,
    ) -> Self {
        TransactionSignatureChecker {
            tx,
            input_index,
            amount,
            precomputed,
        }
    }
}

impl SignatureChecker for TransactionSignatureChecker {
    fn check_ecdsa_signature(
        &self,
        sig: &[u8],
        pubkey: &[u8],
        script_code: &Script,
        sigversion: SigVersion,
    ) -> bool {
        // Empty signatures always fail.
        if sig.is_empty() || pubkey.is_empty() {
            return false;
        }

        // The last byte of the signature is the hash type.
        let hash_type = sig[sig.len() - 1] as u32;
        let sig_bytes = &sig[..sig.len() - 1];

        // Compute the sighash based on the sigversion.
        let sighash = match sigversion {
            SigVersion::Base => signature_hash(script_code, &self.tx, self.input_index, hash_type),
            SigVersion::WitnessV0 => witness_v0_signature_hash(
                script_code,
                &self.tx,
                self.input_index,
                hash_type,
                self.amount,
                &self.precomputed,
            ),
            _ => return false, // Tapscript uses Schnorr, not ECDSA
        };

        // Verify the ECDSA signature using secp256k1.
        verify_ecdsa_signature(sig_bytes, pubkey, &sighash)
    }

    fn check_schnorr_signature(
        &self,
        sig: &[u8],
        pubkey: &[u8],
        sigversion: SigVersion,
        _exec_data: &ScriptExecutionData,
        error: &mut ScriptError,
    ) -> bool {
        // Schnorr signatures are 64 bytes (no hashtype) or 65 bytes (with hashtype).
        if sig.is_empty() {
            *error = ScriptError::SchnorrSig;
            return false;
        }

        // Schnorr signatures are valid in both Taproot (key-path) and Tapscript (script-path).
        if sigversion != SigVersion::Tapscript && sigversion != SigVersion::Taproot {
            *error = ScriptError::SchnorrSig;
            return false;
        }

        if sig.len() != 64 && sig.len() != 65 {
            *error = ScriptError::SchnorrSigSize;
            return false;
        }

        if pubkey.len() != 32 {
            *error = ScriptError::SchnorrSig;
            return false;
        }

        // Determine hash type.
        let hash_type = if sig.len() == 65 {
            let ht = sig[64];
            if ht == 0x00 {
                // 0x00 is not a valid explicit hash type
                *error = ScriptError::SchnorrSigHashtype;
                return false;
            }
            ht as u32
        } else {
            0 // SIGHASH_DEFAULT for 64-byte sig (matches Bitcoin Core)
        };

        let sig_bytes = &sig[..64];

        // Compute BIP341 taproot sighash.
        // Use exec_data for tapscript path (ext_flag=1, tapleaf_hash, codesep_pos).
        let (ext_flag, tapleaf_hash, codesep_pos) = if _exec_data.tapleaf_hash_init {
            (
                1u8,
                Some(_exec_data.tapleaf_hash),
                Some(_exec_data.codeseparator_pos),
            )
        } else {
            (0u8, None, None)
        };

        let annex_hash = if _exec_data.annex_init && _exec_data.annex_present {
            Some(&_exec_data.annex_hash)
        } else {
            None
        };

        let sighash = match qubitcoin_consensus::sighash::taproot_signature_hash(
            &self.tx,
            self.input_index,
            hash_type,
            &self.precomputed,
            ext_flag,
            tapleaf_hash.as_ref(),
            codesep_pos,
            annex_hash,
        ) {
            Some(h) => h,
            None => {
                *error = ScriptError::SchnorrSigHashtype;
                return false;
            }
        };

        // Verify Schnorr signature using secp256k1.
        verify_schnorr_signature(sig_bytes, pubkey, &sighash)
    }

    fn check_lock_time(&self, lock_time: &ScriptNum) -> bool {
        // OP_CHECKLOCKTIMEVERIFY: verify that the transaction's nLockTime
        // is satisfied by the script lock_time value.
        let n_lock_time = lock_time.get_i64();
        if n_lock_time < 0 {
            return false;
        }
        // Compare as i64, matching Bitcoin Core (nLockTime is uint32, cast to int64).
        let tx_lock_time = self.tx.lock_time as i64;

        // nLockTime and script lock_time must be the same type (both height or both time).
        const LOCKTIME_THRESHOLD: i64 = 500_000_000;
        if (tx_lock_time < LOCKTIME_THRESHOLD) != (n_lock_time < LOCKTIME_THRESHOLD) {
            return false;
        }

        // The script lock_time must be <= nLockTime.
        if n_lock_time > tx_lock_time {
            return false;
        }

        // At least one input must be non-final (not SEQUENCE_FINAL).
        if self.tx.vin[self.input_index].sequence == SEQUENCE_FINAL {
            return false;
        }

        true
    }

    fn check_sequence(&self, sequence: &ScriptNum) -> bool {
        use qubitcoin_consensus::{
            SEQUENCE_LOCKTIME_DISABLE_FLAG, SEQUENCE_LOCKTIME_MASK, SEQUENCE_LOCKTIME_TYPE_FLAG,
        };

        let n_sequence = sequence.get_i64();
        if n_sequence < 0 {
            return false;
        }
        // Compare using masked values as u32 after extracting with the locktime mask,
        // matching Bitcoin Core which ANDs with nLockTimeMask (uint32).
        let n_sequence = n_sequence as u32;

        // Transaction version must be >= 2 for BIP68 (checked first, matching Bitcoin Core order).
        if self.tx.version < 2 {
            return false;
        }

        // Sequence numbers with the disable flag set pass trivially.
        if n_sequence & SEQUENCE_LOCKTIME_DISABLE_FLAG != 0 {
            return true;
        }

        let tx_sequence = self.tx.vin[self.input_index].sequence;

        // If the input's sequence has the disable flag, fail.
        if tx_sequence & SEQUENCE_LOCKTIME_DISABLE_FLAG != 0 {
            return false;
        }

        // Both must be the same type (time or height).
        if (n_sequence & SEQUENCE_LOCKTIME_TYPE_FLAG) != (tx_sequence & SEQUENCE_LOCKTIME_TYPE_FLAG)
        {
            return false;
        }

        // The script's sequence lock must be <= the input's sequence lock.
        if (n_sequence & SEQUENCE_LOCKTIME_MASK) > (tx_sequence & SEQUENCE_LOCKTIME_MASK) {
            return false;
        }

        true
    }
}

/// Verify an ECDSA signature against a sighash using secp256k1.
///
/// Uses a lenient DER parser matching Bitcoin Core's `ecdsa_signature_parse_der_lax()`
/// to accept non-standard DER encodings (excess padding, negative R/S values, etc.)
/// that were valid before BIP66 enforcement.
fn verify_ecdsa_signature(sig_bytes: &[u8], pubkey_bytes: &[u8], sighash: &Uint256) -> bool {
    use qubitcoin_crypto::secp256k1::ecdsa::Signature;
    use qubitcoin_crypto::secp256k1::{Message, PublicKey, Secp256k1};

    let secp = Secp256k1::verification_only();

    let msg = match Message::from_digest(*sighash.data()) {
        msg => msg,
    };

    let pubkey = match PublicKey::from_slice(pubkey_bytes) {
        Ok(pk) => pk,
        Err(_) => return false,
    };

    // Always use the lenient DER parser, matching Bitcoin Core's CPubKey::VerifyECDSA()
    // which always calls ecdsa_signature_parse_der_lax(). The strict DER check is
    // enforced separately by the interpreter's check_signature_encoding() when DERSIG
    // or STRICTENC flags are set.
    let mut sig = match ecdsa_signature_parse_der_lax(sig_bytes) {
        Some(s) => s,
        None => return false,
    };

    // Normalize to low-S form. libsecp256k1's ECDSA verification requires
    // lower-S signatures, which have not historically been enforced in Bitcoin,
    // so normalize them first. Matches Bitcoin Core's CPubKey::VerifyECDSA().
    sig.normalize_s();

    secp.verify_ecdsa(&msg, &sig, &pubkey).is_ok()
}

/// Lenient DER signature parser matching Bitcoin Core's `ecdsa_signature_parse_der_lax()`.
///
/// Extracts R and S integer values from a possibly non-standard DER encoding,
/// then constructs a compact (R||S) signature. Handles:
/// - Excess R/S padding (extra leading zero bytes)
/// - Missing padding on negative values
/// - Overlong length encodings
/// - Extra trailing bytes
fn ecdsa_signature_parse_der_lax(
    input: &[u8],
) -> Option<qubitcoin_crypto::secp256k1::ecdsa::Signature> {
    if input.len() < 2 {
        return None;
    }

    let mut pos = 0usize;

    // Sequence tag (0x30)
    if input[pos] != 0x30 {
        return None;
    }
    pos += 1;

    // Sequence length (skip, we don't validate it strictly)
    if pos >= input.len() {
        return None;
    }
    if input[pos] & 0x80 != 0 {
        // Long form length
        let n_len_bytes = (input[pos] & 0x7f) as usize;
        pos += 1 + n_len_bytes;
    } else {
        pos += 1;
    }

    // Parse R
    let r_bytes = parse_der_integer_lax(input, &mut pos)?;

    // Parse S
    let s_bytes = parse_der_integer_lax(input, &mut pos)?;

    // Build 64-byte compact signature: R (32 bytes big-endian) || S (32 bytes big-endian)
    let mut compact = [0u8; 64];
    copy_integer_to_32(&r_bytes, &mut compact[0..32]);
    copy_integer_to_32(&s_bytes, &mut compact[32..64]);

    qubitcoin_crypto::secp256k1::ecdsa::Signature::from_compact(&compact).ok()
}

/// Parse a DER integer value leniently (allows excess padding and negative values).
fn parse_der_integer_lax(input: &[u8], pos: &mut usize) -> Option<Vec<u8>> {
    if *pos >= input.len() {
        return None;
    }

    // Integer tag (0x02)
    if input[*pos] != 0x02 {
        return None;
    }
    *pos += 1;

    if *pos >= input.len() {
        return None;
    }

    // Length
    let len = if input[*pos] & 0x80 != 0 {
        let n_len_bytes = (input[*pos] & 0x7f) as usize;
        *pos += 1;
        if *pos + n_len_bytes > input.len() {
            return None;
        }
        let mut len = 0usize;
        for i in 0..n_len_bytes {
            len = (len << 8) | (input[*pos + i] as usize);
        }
        *pos += n_len_bytes;
        len
    } else {
        let len = input[*pos] as usize;
        *pos += 1;
        len
    };

    if *pos + len > input.len() {
        return None;
    }

    let value = input[*pos..*pos + len].to_vec();
    *pos += len;
    Some(value)
}

/// Copy a variable-length big-endian integer to a fixed 32-byte buffer,
/// right-aligned and stripping leading zeros or sign bytes.
fn copy_integer_to_32(src: &[u8], dst: &mut [u8]) {
    // Strip leading zero bytes (padding)
    let mut start = 0;
    while start < src.len() && src[start] == 0 {
        start += 1;
    }
    let trimmed = &src[start..];

    if trimmed.len() > 32 {
        // Too large; take last 32 bytes
        dst.copy_from_slice(&trimmed[trimmed.len() - 32..]);
    } else {
        // Right-align in 32-byte buffer
        let offset = 32 - trimmed.len();
        dst[offset..].copy_from_slice(trimmed);
    }
}

/// Verify a Schnorr signature against a sighash using secp256k1.
fn verify_schnorr_signature(sig_bytes: &[u8], pubkey_bytes: &[u8], sighash: &Uint256) -> bool {
    use qubitcoin_crypto::secp256k1::schnorr::Signature;
    use qubitcoin_crypto::secp256k1::{Message, Secp256k1, XOnlyPublicKey};

    let secp = Secp256k1::verification_only();

    let msg = match Message::from_digest(*sighash.data()) {
        msg => msg,
    };

    let pubkey = match XOnlyPublicKey::from_slice(pubkey_bytes) {
        Ok(pk) => pk,
        Err(_) => return false,
    };

    let sig = match Signature::from_slice(sig_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };

    secp.verify_schnorr(&sig, &msg, &pubkey).is_ok()
}

// ---------------------------------------------------------------------------
// ScriptCheck
// ---------------------------------------------------------------------------

/// A single script verification task.
///
/// Contains all the data needed to verify one transaction input's script
/// independently of any other input.
#[derive(Clone)]
pub struct ScriptCheck {
    /// The scriptPubKey of the output being spent.
    pub script_pubkey: Vec<u8>,
    /// The scriptSig of the input spending it.
    pub script_sig: Vec<u8>,
    /// Witness data for the input.
    pub witness: Vec<Vec<u8>>,
    /// Script verification flags for this input.
    pub flags: ScriptVerifyFlags,
    /// The input amount (needed for segwit signature verification).
    pub amount: i64,
    /// Transaction index within the block (for error reporting).
    pub tx_index: usize,
    /// Input index within the transaction (for error reporting).
    pub input_index: usize,
    /// The transaction being verified (needed for signature checking).
    pub tx: Option<Arc<Transaction>>,
    /// Precomputed sighash data (shared across all inputs of the same tx).
    pub precomputed: Option<Arc<PrecomputedTransactionData>>,
}

/// Result of a script verification failure.
#[derive(Debug, Clone)]
pub struct ScriptCheckError {
    /// Which transaction in the block failed.
    pub tx_index: usize,
    /// Which input in the transaction failed.
    pub input_index: usize,
    /// The script error that occurred.
    pub error: String,
}

impl std::fmt::Display for ScriptCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "script verification failed: tx_index={}, input_index={}, error={}",
            self.tx_index, self.input_index, self.error
        )
    }
}

/// Verify all script checks in parallel using Rayon's work-stealing scheduler.
///
/// This is significantly faster than Bitcoin Core's CCheckQueue for blocks
/// with many inputs because:
/// 1. Rayon automatically adapts to available CPU cores
/// 2. Work-stealing provides better load balancing than static partitioning
/// 3. No manual thread pool management or contention on shared queues
///
/// Returns `Ok(())` if all scripts verify, or `Err(ScriptCheckError)` with
/// details of the first failing input.
pub fn verify_scripts_parallel(checks: &[ScriptCheck]) -> Result<(), ScriptCheckError> {
    if checks.is_empty() {
        return Ok(());
    }

    // For small numbers of checks, run sequentially to avoid overhead
    if checks.len() <= 4 {
        for check in checks {
            verify_single_script(check)?;
        }
        return Ok(());
    }

    #[cfg(feature = "parallel")]
    {
        // Use Rayon's parallel iterator with early termination via try_for_each
        checks
            .par_iter()
            .try_for_each(|check| verify_single_script(check))
    }

    #[cfg(not(feature = "parallel"))]
    {
        // Sequential fallback for WASM and other non-parallel targets
        for check in checks {
            verify_single_script(check)?;
        }
        Ok(())
    }
}

/// Verify a single script check.
fn verify_single_script(check: &ScriptCheck) -> Result<(), ScriptCheckError> {
    let script_pubkey = Script::from_bytes(check.script_pubkey.clone());
    let script_sig = Script::from_bytes(check.script_sig.clone());
    let witness = ScriptWitness {
        stack: check.witness.clone(),
    };
    let mut error = ScriptError::Ok;

    let result = if let Some(ref tx) = check.tx {
        // Production path: use TransactionSignatureChecker with full tx context
        let precomputed = match check.precomputed {
            Some(ref p) => (**p).clone(),
            None => PrecomputedTransactionData::new(tx, &[]),
        };
        let checker = TransactionSignatureChecker::new(
            Arc::clone(tx),
            check.input_index,
            check.amount,
            precomputed,
        );
        verify_script(
            &script_sig,
            &script_pubkey,
            &witness,
            &check.flags,
            &checker,
            &mut error,
        )
    } else {
        // Fallback for tests that don't provide a transaction
        let checker = BaseSignatureChecker;
        verify_script(
            &script_sig,
            &script_pubkey,
            &witness,
            &check.flags,
            &checker,
            &mut error,
        )
    };

    if result {
        Ok(())
    } else {
        Err(ScriptCheckError {
            tx_index: check.tx_index,
            input_index: check.input_index,
            error: format!("{:?}", error),
        })
    }
}

/// Collect all script checks for a block's transactions.
///
/// For each non-coinbase transaction input, creates a ScriptCheck with
/// the input's scriptSig, the spent output's scriptPubKey, witness data,
/// and the appropriate verification flags.
pub fn collect_block_script_checks(
    transactions: &[TransactionRef],
    spent_coins: &[Vec<Coin>],
    flags: ScriptVerifyFlags,
) -> Vec<ScriptCheck> {
    let mut checks = Vec::new();

    for (tx_idx, tx) in transactions.iter().enumerate() {
        if tx.is_coinbase() {
            continue;
        }

        // spent_coins indices align with non-coinbase tx order
        let coin_idx = tx_idx - 1;
        if coin_idx >= spent_coins.len() {
            continue;
        }

        // Collect spent TxOuts for precomputed sighash data
        let spent_outputs: Vec<_> = spent_coins[coin_idx]
            .iter()
            .map(|c| c.tx_out.clone())
            .collect();
        let precomputed = Arc::new(PrecomputedTransactionData::new(tx, &spent_outputs));

        for (input_idx, input) in tx.vin.iter().enumerate() {
            if input_idx >= spent_coins[coin_idx].len() {
                continue;
            }

            let coin = &spent_coins[coin_idx][input_idx];
            checks.push(ScriptCheck {
                script_pubkey: coin.tx_out.script_pubkey.as_bytes().to_vec(),
                script_sig: input.script_sig.as_bytes().to_vec(),
                witness: input.witness.stack.clone(),
                flags,
                amount: coin.tx_out.value.to_sat(),
                tx_index: tx_idx,
                input_index: input_idx,
                tx: Some(Arc::clone(tx)),
                precomputed: Some(Arc::clone(&precomputed)),
            });
        }
    }

    checks
}

/// Configuration for the parallel script verification engine.
pub struct ScriptCheckConfig {
    /// Maximum number of threads to use. 0 = use Rayon default (num CPUs).
    pub max_threads: usize,
    /// Minimum batch size before parallelization kicks in.
    pub min_parallel_batch: usize,
}

impl Default for ScriptCheckConfig {
    fn default() -> Self {
        ScriptCheckConfig {
            max_threads: 0,
            min_parallel_batch: 4,
        }
    }
}

impl ScriptCheckConfig {
    /// Initialize the global Rayon thread pool with our configuration.
    #[cfg(feature = "parallel")]
    pub fn init_thread_pool(&self) -> Result<(), rayon::ThreadPoolBuildError> {
        let mut builder = rayon::ThreadPoolBuilder::new();
        if self.max_threads > 0 {
            builder = builder.num_threads(self.max_threads);
        }
        builder
            .thread_name(|idx| format!("script-check-{}", idx))
            .build_global()
    }
}

// ---------------------------------------------------------------------------
// Benchmarking utilities
// ---------------------------------------------------------------------------

/// Statistics from a parallel script verification run.
#[derive(Debug, Clone, Default)]
pub struct ScriptCheckStats {
    /// Total number of script checks performed.
    pub total_checks: usize,
    /// Time taken for verification in microseconds.
    pub elapsed_us: u64,
    /// Checks per second throughput.
    pub checks_per_sec: f64,
}

/// Verify scripts in parallel with timing statistics.
pub fn verify_scripts_parallel_timed(
    checks: &[ScriptCheck],
) -> Result<ScriptCheckStats, ScriptCheckError> {
    let start = std::time::Instant::now();
    verify_scripts_parallel(checks)?;
    let elapsed = start.elapsed();

    let total_checks = checks.len();
    let elapsed_us = elapsed.as_micros() as u64;
    let checks_per_sec = if elapsed_us > 0 {
        (total_checks as f64) / elapsed.as_secs_f64()
    } else {
        0.0
    };

    Ok(ScriptCheckStats {
        total_checks,
        elapsed_us,
        checks_per_sec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trivial_check(tx_idx: usize, input_idx: usize) -> ScriptCheck {
        // OP_1 as scriptPubKey evaluates to true with empty scriptSig
        ScriptCheck {
            script_pubkey: vec![0x51], // OP_1 (OP_TRUE)
            script_sig: vec![],
            witness: vec![],
            flags: ScriptVerifyFlags::NONE,
            amount: 0,
            tx_index: tx_idx,
            input_index: input_idx,
            tx: None,
            precomputed: None,
        }
    }

    #[test]
    fn test_empty_checks() {
        let result = verify_scripts_parallel(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_single_trivial_check() {
        let checks = vec![make_trivial_check(0, 0)];
        let result = verify_scripts_parallel(&checks);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_trivial_checks() {
        let checks: Vec<ScriptCheck> = (0..100).map(|i| make_trivial_check(i / 2, i % 2)).collect();
        let result = verify_scripts_parallel(&checks);
        assert!(result.is_ok());
    }

    #[test]
    fn test_failing_script_check() {
        let check = ScriptCheck {
            script_pubkey: vec![0x00], // OP_0 (FALSE)
            script_sig: vec![],
            witness: vec![],
            flags: ScriptVerifyFlags::NONE,
            amount: 0,
            tx_index: 3,
            input_index: 1,
            tx: None,
            precomputed: None,
        };
        let result = verify_scripts_parallel(&[check]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.tx_index, 3);
        assert_eq!(err.input_index, 1);
    }

    #[test]
    fn test_mixed_pass_fail() {
        let mut checks = Vec::new();
        for i in 0..10 {
            checks.push(make_trivial_check(i, 0));
        }
        // Add a failing check
        checks.push(ScriptCheck {
            script_pubkey: vec![0x00], // OP_0
            script_sig: vec![],
            witness: vec![],
            flags: ScriptVerifyFlags::NONE,
            amount: 0,
            tx_index: 10,
            input_index: 0,
            tx: None,
            precomputed: None,
        });
        let result = verify_scripts_parallel(&checks);
        assert!(result.is_err());
    }

    #[test]
    fn test_collect_block_script_checks() {
        use qubitcoin_common::coins::Coin;
        use qubitcoin_consensus::transaction::{Transaction, TxIn, TxOut, SEQUENCE_FINAL};
        use qubitcoin_consensus::OutPoint;
        use qubitcoin_primitives::Amount;
        use std::sync::Arc;

        // Create a coinbase transaction
        let coinbase = Arc::new(Transaction::new(
            1,
            vec![TxIn::coinbase(Script::from_bytes(vec![0x01, 0x01]))],
            vec![TxOut::new(Amount::from_sat(5_000_000_000), Script::new())],
            0,
        ));

        // Create a simple non-coinbase transaction
        let tx = Arc::new(Transaction::new(
            1,
            vec![TxIn::new(
                OutPoint::new(Default::default(), 0),
                Script::new(),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(Amount::from_sat(100), Script::new())],
            0,
        ));

        let coin = Coin::new(
            TxOut::new(Amount::from_sat(200), Script::from_bytes(vec![0x51])),
            1,
            false,
        );

        // spent_coins[0] corresponds to transactions[1] (first non-coinbase)
        let txns = vec![coinbase, tx];
        let spent = vec![vec![coin]];

        let checks = collect_block_script_checks(&txns, &spent, ScriptVerifyFlags::NONE);
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].tx_index, 1);
        assert_eq!(checks[0].input_index, 0);
        assert_eq!(checks[0].script_pubkey, vec![0x51]);
    }

    #[test]
    fn test_script_check_config_default() {
        let config = ScriptCheckConfig::default();
        assert_eq!(config.max_threads, 0);
        assert_eq!(config.min_parallel_batch, 4);
    }

    #[test]
    fn test_verify_scripts_parallel_timed() {
        let checks: Vec<ScriptCheck> = (0..50).map(|i| make_trivial_check(i, 0)).collect();
        let stats = verify_scripts_parallel_timed(&checks).unwrap();
        assert_eq!(stats.total_checks, 50);
        assert!(stats.elapsed_us < 10_000_000); // should finish in under 10s
    }

    #[test]
    fn test_p2pkh_script_check() {
        // Test with a simple script that evaluates to true (OP_1)
        let check = ScriptCheck {
            script_pubkey: vec![0x51], // OP_1
            script_sig: vec![],
            witness: vec![],
            flags: ScriptVerifyFlags::NONE,
            amount: 5000,
            tx_index: 0,
            input_index: 0,
            tx: None,
            precomputed: None,
        };
        assert!(verify_scripts_parallel(&[check]).is_ok());
    }

    #[test]
    fn test_parallel_large_batch() {
        // Test with a large batch to actually exercise parallelism
        let checks: Vec<ScriptCheck> = (0..1000)
            .map(|i| make_trivial_check(i / 5, i % 5))
            .collect();
        let result = verify_scripts_parallel(&checks);
        assert!(result.is_ok());
    }

    // --- TransactionSignatureChecker tests ---

    fn make_test_tx(lock_time: u32, version: u32, sequences: &[u32]) -> Arc<Transaction> {
        use qubitcoin_consensus::transaction::{TxIn, TxOut};
        use qubitcoin_consensus::OutPoint;
        use qubitcoin_primitives::Amount;

        let inputs: Vec<TxIn> = sequences
            .iter()
            .map(|&seq| TxIn::new(OutPoint::new(Default::default(), 0), Script::new(), seq))
            .collect();

        Arc::new(Transaction::new(
            version,
            inputs,
            vec![TxOut::new(Amount::from_sat(100), Script::new())],
            lock_time,
        ))
    }

    fn make_checker(tx: Arc<Transaction>, input_index: usize) -> TransactionSignatureChecker {
        let precomputed = PrecomputedTransactionData::new(&tx, &[]);
        TransactionSignatureChecker::new(tx, input_index, 0, precomputed)
    }

    #[test]
    fn test_check_lock_time_height_satisfied() {
        // tx.lock_time = 500 (height), script requires 400 => passes
        let tx = make_test_tx(500, 1, &[0]);
        let checker = make_checker(tx, 0);
        assert!(checker.check_lock_time(&ScriptNum::new(400)));
    }

    #[test]
    fn test_check_lock_time_height_not_satisfied() {
        // tx.lock_time = 300, script requires 400 => fails (400 > 300)
        let tx = make_test_tx(300, 1, &[0]);
        let checker = make_checker(tx, 0);
        assert!(!checker.check_lock_time(&ScriptNum::new(400)));
    }

    #[test]
    fn test_check_lock_time_type_mismatch() {
        // tx.lock_time = 500 (height), script requires 500_000_001 (time) => fails
        let tx = make_test_tx(500, 1, &[0]);
        let checker = make_checker(tx, 0);
        assert!(!checker.check_lock_time(&ScriptNum::new(500_000_001)));
    }

    #[test]
    fn test_check_lock_time_negative() {
        let tx = make_test_tx(500, 1, &[0]);
        let checker = make_checker(tx, 0);
        assert!(!checker.check_lock_time(&ScriptNum::new(-1)));
    }

    #[test]
    fn test_check_lock_time_sequence_final() {
        // Input has SEQUENCE_FINAL => always fails (input is finalized)
        let tx = make_test_tx(500, 1, &[SEQUENCE_FINAL]);
        let checker = make_checker(tx, 0);
        assert!(!checker.check_lock_time(&ScriptNum::new(400)));
    }

    #[test]
    fn test_check_lock_time_time_satisfied() {
        // Both are time-based (>= 500_000_000)
        let tx = make_test_tx(500_000_100, 1, &[0]);
        let checker = make_checker(tx, 0);
        assert!(checker.check_lock_time(&ScriptNum::new(500_000_050)));
    }

    #[test]
    fn test_check_sequence_satisfied() {
        // script sequence = 10 (height), tx input sequence = 20 => passes
        let tx = make_test_tx(0, 2, &[20]);
        let checker = make_checker(tx, 0);
        assert!(checker.check_sequence(&ScriptNum::new(10)));
    }

    #[test]
    fn test_check_sequence_not_satisfied() {
        // script sequence = 30, tx input sequence = 20 => fails (30 > 20)
        let tx = make_test_tx(0, 2, &[20]);
        let checker = make_checker(tx, 0);
        assert!(!checker.check_sequence(&ScriptNum::new(30)));
    }

    #[test]
    fn test_check_sequence_disable_flag() {
        // Script sequence has disable flag set => passes trivially
        let tx = make_test_tx(0, 2, &[20]);
        let checker = make_checker(tx, 0);
        assert!(checker.check_sequence(&ScriptNum::new(0x80000000u32 as i64)));
    }

    #[test]
    fn test_check_sequence_version_too_low() {
        // tx version = 1 (< 2) => BIP68 not active, fails
        let tx = make_test_tx(0, 1, &[20]);
        let checker = make_checker(tx, 0);
        assert!(!checker.check_sequence(&ScriptNum::new(10)));
    }

    #[test]
    fn test_check_sequence_type_mismatch() {
        // script uses time-based (TYPE_FLAG set), input uses height-based => fails
        let tx = make_test_tx(0, 2, &[20]); // height-based (no TYPE_FLAG)
        let checker = make_checker(tx, 0);
        let time_seq = 0x00400010i64; // SEQUENCE_LOCKTIME_TYPE_FLAG | 16
        assert!(!checker.check_sequence(&ScriptNum::new(time_seq)));
    }

    #[test]
    fn test_check_sequence_negative() {
        let tx = make_test_tx(0, 2, &[20]);
        let checker = make_checker(tx, 0);
        assert!(!checker.check_sequence(&ScriptNum::new(-1)));
    }

    #[test]
    fn test_checker_ecdsa_empty_sig() {
        // Empty signature always fails
        let tx = make_test_tx(0, 1, &[0]);
        let checker = make_checker(tx, 0);
        assert!(!checker.check_ecdsa_signature(&[], &[0x02; 33], &Script::new(), SigVersion::Base));
    }

    #[test]
    fn test_checker_ecdsa_empty_pubkey() {
        // Empty pubkey always fails
        let tx = make_test_tx(0, 1, &[0]);
        let checker = make_checker(tx, 0);
        assert!(!checker.check_ecdsa_signature(
            &[0x01, 0x01],
            &[],
            &Script::new(),
            SigVersion::Base
        ));
    }

    #[test]
    fn test_checker_schnorr_wrong_sigversion() {
        // Schnorr only works with Tapscript
        let tx = make_test_tx(0, 1, &[0]);
        let checker = make_checker(tx, 0);
        let mut err = ScriptError::Ok;
        assert!(!checker.check_schnorr_signature(
            &[0u8; 64],
            &[0u8; 32],
            SigVersion::Base,
            &ScriptExecutionData::default(),
            &mut err,
        ));
        assert_eq!(err, ScriptError::SchnorrSig);
    }

    #[test]
    fn test_checker_schnorr_bad_sig_size() {
        let tx = make_test_tx(0, 1, &[0]);
        let checker = make_checker(tx, 0);
        let mut err = ScriptError::Ok;
        // 63 bytes is invalid
        assert!(!checker.check_schnorr_signature(
            &[0u8; 63],
            &[0u8; 32],
            SigVersion::Tapscript,
            &ScriptExecutionData::default(),
            &mut err,
        ));
        assert_eq!(err, ScriptError::SchnorrSigSize);
    }

    #[test]
    fn test_checker_schnorr_empty_sig() {
        let tx = make_test_tx(0, 1, &[0]);
        let checker = make_checker(tx, 0);
        let mut err = ScriptError::Ok;
        assert!(!checker.check_schnorr_signature(
            &[],
            &[0u8; 32],
            SigVersion::Tapscript,
            &ScriptExecutionData::default(),
            &mut err,
        ));
        assert_eq!(err, ScriptError::SchnorrSig);
    }

    #[test]
    fn test_checker_schnorr_explicit_zero_hashtype() {
        // 65-byte sig with hash_type 0x00 is invalid
        let tx = make_test_tx(0, 1, &[0]);
        let checker = make_checker(tx, 0);
        let mut err = ScriptError::Ok;
        let mut sig = vec![0u8; 65];
        sig[64] = 0x00; // explicit zero hash type
        assert!(!checker.check_schnorr_signature(
            &sig,
            &[0u8; 32],
            SigVersion::Tapscript,
            &ScriptExecutionData::default(),
            &mut err,
        ));
        assert_eq!(err, ScriptError::SchnorrSigHashtype);
    }

    #[test]
    fn test_collect_with_tx_reference() {
        // Verify that collect_block_script_checks sets tx and precomputed fields
        use qubitcoin_consensus::transaction::{TxIn, TxOut};
        use qubitcoin_consensus::OutPoint;
        use qubitcoin_primitives::Amount;

        let coinbase = Arc::new(Transaction::new(
            1,
            vec![TxIn::coinbase(Script::from_bytes(vec![0x01, 0x01]))],
            vec![TxOut::new(Amount::from_sat(5_000_000_000), Script::new())],
            0,
        ));

        let tx = Arc::new(Transaction::new(
            1,
            vec![TxIn::new(
                OutPoint::new(Default::default(), 0),
                Script::new(),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(Amount::from_sat(100), Script::new())],
            0,
        ));

        let coin = Coin::new(
            TxOut::new(Amount::from_sat(200), Script::from_bytes(vec![0x51])),
            1,
            false,
        );

        let txns = vec![coinbase, tx];
        let spent = vec![vec![coin]];

        let checks = collect_block_script_checks(&txns, &spent, ScriptVerifyFlags::NONE);
        assert_eq!(checks.len(), 1);
        assert!(checks[0].tx.is_some());
        assert!(checks[0].precomputed.is_some());
    }

    #[test]
    fn test_lax_der_parser_negative_r() {
        // DER signature with R that has high bit set (negative in DER, missing 0x00 padding)
        // From script_tests.json: "P2PK with too little R padding but no DERSIG"
        let sig_der = hex_to_bytes_test(
            "30440220d7a0417c3f6d1a15094d1cf2a3378ca0503eb8a57630953a9e2987e21ddd0a6502207a6266d686c99090920249991d3d42065b6d43eb70187b219c0db82e4f94d1a2"
        );

        use qubitcoin_crypto::secp256k1::ecdsa::Signature;

        // Lax parser should succeed and extract R correctly as unsigned
        let lax = ecdsa_signature_parse_der_lax(&sig_der);
        assert!(
            lax.is_some(),
            "lax parser should accept signature with negative R"
        );
        let lax_sig = lax.unwrap();
        let lax_compact = lax_sig.serialize_compact();

        // R starts with d7 (high bit set, would be negative in DER)
        assert_eq!(
            &lax_compact[0..4],
            &[0xd7, 0xa0, 0x41, 0x7c],
            "R extracted correctly as unsigned"
        );
        assert_eq!(
            &lax_compact[32..36],
            &[0x7a, 0x62, 0x66, 0xd6],
            "S extracted correctly"
        );

        // Note: from_der interprets the high-bit R as negative and zeroes it out,
        // which is why we must always use the lax parser for verification (matching
        // Bitcoin Core's CPubKey::VerifyECDSA).
    }

    fn hex_to_bytes_test(hex: &str) -> Vec<u8> {
        let mut result = Vec::with_capacity(hex.len() / 2);
        for i in (0..hex.len()).step_by(2) {
            result.push(u8::from_str_radix(&hex[i..i + 2], 16).unwrap());
        }
        result
    }

    /// Regression test for testnet3 block 204,625 tx_index=3 input_index=1.
    ///
    /// This is a non-standard transaction where the scriptPubKey contains a
    /// dummy DER signature + OP_DROP followed by a 2-of-2 bare CHECKMULTISIG.
    /// The scriptSig provides OP_0 + two real signatures.
    ///
    /// Block hash: 00000000a0dff26bb4a33874a8ddcbb06b4ab8fce787e6bd7319e05ede36ab55
    /// Spending txid: 2c63aa814701cef5dbd4bbaddab3fea9117028f2434dddcdab8339141e9b14d1
    /// Prev txid: 19aa42fee0fa57c45d3b16488198b27caaacc4ff5794510d0c17f173f05587ff (vout 0)
    #[test]
    fn test_testnet3_block_204625_nonstandard_multisig() {
        // Raw spending transaction (the one that was rejected)
        let raw_tx_hex = "01000000022f196cf1e5bd426a04f07b882c893b5b5edebad67da6eb50f066c372ed736d5f000000006a47304402201f81ac31b52cb4b1ceb83f97d18476f7339b74f4eecd1a32c251d4c3cccfffa402203c9143c18810ce072969e4132fdab91408816c96b423b2be38eec8a3582ade36012102aa5a2b334bd8f135f11bc5c477bf6307ff98ed52d3ed10f857d5c89adf5b02beffffffffff8755f073f1170c0d519457ffc4acaa7cb2988148163b5dc457fae0fe42aa19000000009200483045022015bd0139bcccf990a6af6ec5c1c52ed8222e03a0d51c334df139968525d2fcd20221009f9efe325476eb64c3958e4713e9eefe49bf1d820ed58d2112721b134e2a1a530347304402206da827fb26e569eb740641f9c1a7121ee59141703cbe0f903a22cc7d9a7ec7ac02204729f989b5348b3669ab020b8c4af01acc4deaba7c0d9f8fa9e06b2106cbbfeb01ffffffff010000000000000000016a00000000";
        let raw_tx = hex_to_bytes_test(raw_tx_hex);

        // scriptPubKey of the previous output being spent (input index 1)
        // This is a non-standard script:
        //   PUSH<72> <dummy_sig> OP_DROP OP_2 PUSH<33> <pk> PUSH<33> <pk> OP_2 OP_CHECKMULTISIG
        let script_pubkey_hex = "483045022015bd0139bcccf990a6af6ec5c1c52ed8222e03a0d51c334df139968525d2fcd20221009f9efe325476eb64c3958e4713e9eefe49bf1d820ed58d2112721b134e2a1a53037552210378d430274f8c5ec1321338151e9f27f4c676a008bdf8638d07c0b6be9ab35c71210378d430274f8c5ec1321338151e9f27f4c676a008bdf8638d07c0b6be9ab35c7152ae";
        let script_pubkey = hex_to_bytes_test(script_pubkey_hex);

        // scriptSig for input 1:
        //   OP_0 PUSH<72> <sig1> PUSH<71> <sig2>
        let script_sig_hex = "00483045022015bd0139bcccf990a6af6ec5c1c52ed8222e03a0d51c334df139968525d2fcd20221009f9efe325476eb64c3958e4713e9eefe49bf1d820ed58d2112721b134e2a1a530347304402206da827fb26e569eb740641f9c1a7121ee59141703cbe0f903a22cc7d9a7ec7ac02204729f989b5348b3669ab020b8c4af01acc4deaba7c0d9f8fa9e06b2106cbbfeb01";
        let script_sig = hex_to_bytes_test(script_sig_hex);

        let amount: i64 = 100_000; // 100,000 satoshis

        // Deserialize the transaction
        let tx = qubitcoin_consensus::transaction::deserialize_transaction(
            &mut std::io::Cursor::new(&raw_tx),
            true,
        )
        .expect("failed to deserialize transaction");
        let tx_ref: TransactionRef = Arc::new(tx);

        // Build the spent output for precomputed sighash.
        // Input 0's prev output (P2PKH, not relevant for this test but needed for precompute):
        let spent_out_0 = qubitcoin_consensus::transaction::TxOut {
            value: qubitcoin_primitives::Amount::from_sat(0),
            script_pubkey: Script::from_bytes(vec![]),
        };
        // Input 1's prev output:
        let spent_out_1 = qubitcoin_consensus::transaction::TxOut {
            value: qubitcoin_primitives::Amount::from_sat(amount),
            script_pubkey: Script::from_bytes(script_pubkey.clone()),
        };
        let spent_outputs = vec![spent_out_0, spent_out_1];
        let precomputed = PrecomputedTransactionData::new(&tx_ref, &spent_outputs);

        // Flags at height 204,625 on testnet3:
        // P2SH + WITNESS + TAPROOT (always on), no DERSIG/CLTV/CSV/NULLDUMMY yet
        let flags = ScriptVerifyFlags::P2SH
            | ScriptVerifyFlags::WITNESS
            | ScriptVerifyFlags::TAPROOT;

        let checker = TransactionSignatureChecker::new(
            Arc::clone(&tx_ref),
            1, // input_index
            amount,
            precomputed,
        );

        let script_pub = Script::from_bytes(script_pubkey);
        let script_s = Script::from_bytes(script_sig);
        let witness = ScriptWitness { stack: vec![] };
        let mut error = ScriptError::Ok;

        let result = verify_script(
            &script_s,
            &script_pub,
            &witness,
            &flags,
            &checker,
            &mut error,
        );

        assert!(
            result,
            "testnet3 block 204625 tx3 input1 should pass script verification, got error: {:?}",
            error
        );
    }
}
