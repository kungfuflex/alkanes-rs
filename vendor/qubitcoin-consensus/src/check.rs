//! Context-free consensus validation functions.
//! Maps to: src/consensus/tx_check.cpp, src/pow.cpp, src/validation.cpp (GetBlockSubsidy)

use crate::params::ConsensusParams;
use crate::transaction::{OutPoint, Transaction};
use crate::validation_state::{TxValidationResult, TxValidationState, ValidationState};
use qubitcoin_primitives::arith_uint256::{uint256_to_arith, ArithUint256};
use qubitcoin_primitives::{money_range, Amount, Uint256, COIN, MAX_MONEY};
use qubitcoin_script::Script;
use std::collections::HashSet;

/// Maximum allowed weight for a block (BIP141).
pub const MAX_BLOCK_WEIGHT: u32 = 4_000_000;

/// Maximum allowed size for a serialized block (1 MB before segwit).
pub const MAX_BLOCK_SERIALIZED_SIZE: u32 = 4_000_000;

/// Maximum allowed sigops per block.
pub const MAX_BLOCK_SIGOPS_COST: u32 = 80_000;

/// Coinbase maturity: coinbase outputs cannot be spent for this many blocks.
pub const COINBASE_MATURITY: i32 = 100;

/// Witness scale factor (BIP141).
pub const WITNESS_SCALE_FACTOR: u32 = 4;

/// Context-free transaction validation.
///
/// Port of Bitcoin Core's CheckTransaction().
/// Checks that don't require UTXO context:
/// - Non-empty inputs and outputs
/// - Serialized size within limits
/// - Non-negative, non-overflow output values
/// - No duplicate inputs
/// - Coinbase script length
pub fn check_transaction(tx: &Transaction, state: &mut TxValidationState) -> bool {
    // Basic checks that don't depend on any context

    if tx.vin.is_empty() {
        return state.invalid(TxValidationResult::Consensus, "bad-txns-vin-empty", "");
    }

    if tx.vout.is_empty() {
        return state.invalid(TxValidationResult::Consensus, "bad-txns-vout-empty", "");
    }

    // Size limits: reject transactions whose non-witness serialized size
    // exceeds MAX_BLOCK_WEIGHT / WITNESS_SCALE_FACTOR. Matches Bitcoin Core's
    // "bad-txns-oversize" check.
    {
        use qubitcoin_serialize::Encodable;
        let mut no_witness_buf = Vec::new();
        // Serialize version
        tx.version.encode(&mut no_witness_buf).unwrap();
        // Serialize inputs (without witness)
        let _ = qubitcoin_serialize::write_compact_size(&mut no_witness_buf, tx.vin.len() as u64);
        for input in &tx.vin {
            input.prevout.encode(&mut no_witness_buf).unwrap();
            input.script_sig.encode(&mut no_witness_buf).unwrap();
            input.sequence.encode(&mut no_witness_buf).unwrap();
        }
        // Serialize outputs
        let _ = qubitcoin_serialize::write_compact_size(&mut no_witness_buf, tx.vout.len() as u64);
        for output in &tx.vout {
            output.value.to_sat().encode(&mut no_witness_buf).unwrap();
            output.script_pubkey.encode(&mut no_witness_buf).unwrap();
        }
        // locktime
        tx.lock_time.encode(&mut no_witness_buf).unwrap();

        let stripped_size = no_witness_buf.len() as u32;
        if stripped_size * WITNESS_SCALE_FACTOR > MAX_BLOCK_WEIGHT {
            return state.invalid(TxValidationResult::Consensus, "bad-txns-oversize", "");
        }
    }

    // Check for negative or overflow output values
    let mut total_out = Amount::ZERO;
    for txout in &tx.vout {
        if txout.value.to_sat() < 0 {
            return state.invalid(TxValidationResult::Consensus, "bad-txns-vout-negative", "");
        }
        if !money_range(txout.value.to_sat()) {
            return state.invalid(TxValidationResult::Consensus, "bad-txns-vout-toolarge", "");
        }
        total_out = total_out + txout.value;
        if !money_range(total_out.to_sat()) {
            return state.invalid(
                TxValidationResult::Consensus,
                "bad-txns-txouttotal-toolarge",
                "",
            );
        }
    }

    // Check for duplicate inputs
    let mut seen_outpoints = HashSet::new();
    for txin in &tx.vin {
        if !seen_outpoints.insert(txin.prevout.clone()) {
            return state.invalid(
                TxValidationResult::Consensus,
                "bad-txns-inputs-duplicate",
                "",
            );
        }
    }

    if tx.is_coinbase() {
        let script_len = tx.vin[0].script_sig.len();
        if script_len < 2 || script_len > 100 {
            return state.invalid(TxValidationResult::Consensus, "bad-cb-length", "");
        }
    } else {
        for txin in &tx.vin {
            if txin.prevout.is_null() {
                return state.invalid(TxValidationResult::Consensus, "bad-txns-prevout-null", "");
            }
        }
    }

    true
}

/// Check proof of work: verify that the block hash meets the target.
///
/// Port of Bitcoin Core's CheckProofOfWork().
pub fn check_proof_of_work(hash: &Uint256, bits: u32, params: &ConsensusParams) -> bool {
    let mut target = ArithUint256::zero();
    let (negative, overflow) = target.set_compact(bits);

    // Check range
    if negative || target == ArithUint256::zero() || overflow {
        return false;
    }

    let pow_limit = uint256_to_arith(&params.pow_limit);
    if target > pow_limit {
        return false;
    }

    // Check proof of work matches claimed amount
    let hash_arith = uint256_to_arith(hash);
    if hash_arith > target {
        return false;
    }

    true
}

/// Calculate block subsidy (mining reward) for a given height.
///
/// Port of Bitcoin Core's GetBlockSubsidy().
/// Starts at 50 BTC, halves every subsidy_halving_interval blocks.
pub fn get_block_subsidy(height: i32, params: &ConsensusParams) -> Amount {
    let halvings = height / params.subsidy_halving_interval;

    // Force block reward to zero when right shift is undefined.
    if halvings >= 64 {
        return Amount::ZERO;
    }

    let mut subsidy: i64 = 50 * COIN;
    // Subsidy is cut in half every halving interval
    subsidy >>= halvings;
    Amount::from_sat(subsidy)
}

/// Count the legacy signature operations in a transaction.
///
/// Port of Bitcoin Core's `GetLegacySigOpCount()`.
/// Counts OP_CHECKSIG, OP_CHECKMULTISIG etc. in scriptSig and scriptPubKey
/// without considering P2SH redeem scripts.
pub fn get_legacy_sigop_count(tx: &Transaction) -> u32 {
    let mut n_sig_ops: u32 = 0;
    for input in &tx.vin {
        n_sig_ops += input.script_sig.get_sig_op_count(false) as u32;
    }
    for output in &tx.vout {
        n_sig_ops += output.script_pubkey.get_sig_op_count(false) as u32;
    }
    n_sig_ops
}

/// Count the P2SH signature operations in a transaction.
///
/// Port of Bitcoin Core's `GetP2SHSigOpCount()`.
/// For each P2SH input, deserializes the redeem script from the scriptSig
/// and counts sigops with `accurate=true`.
pub fn get_p2sh_sigop_count<F>(tx: &Transaction, get_script_pubkey: F) -> u32
where
    F: Fn(&OutPoint) -> Option<Script>,
{
    if tx.is_coinbase() {
        return 0;
    }
    let mut n_sig_ops: u32 = 0;
    for input in &tx.vin {
        if let Some(prev_script) = get_script_pubkey(&input.prevout) {
            if prev_script.is_p2sh() {
                // Walk scriptSig to find the last push data (the serialized redeem script).
                let mut pos = 0;
                let mut last_data = Vec::new();
                while let Some((opcode, data, new_pos)) = input.script_sig.get_op(pos) {
                    if opcode > qubitcoin_script::Opcode::Op16 as u8 {
                        return 0; // non-push in scriptSig
                    }
                    if !data.is_empty() {
                        last_data = data;
                    }
                    pos = new_pos;
                }
                let subscript = Script::from_bytes(last_data);
                n_sig_ops += subscript.get_sig_op_count(true) as u32;
            }
        }
    }
    n_sig_ops
}

/// Count witness sigops for a single input.
///
/// Port of Bitcoin Core's `CountWitnessSigOps()`.
fn count_witness_sigops(
    script_sig: &Script,
    script_pubkey: &Script,
    witness: &crate::transaction::Witness,
    flags: u32,
) -> u32 {
    if (flags & 0x800) == 0 {
        // SCRIPT_VERIFY_WITNESS
        return 0;
    }

    // Check if scriptPubKey is a witness program directly.
    if let Some((version, program)) = script_pubkey.is_witness_program() {
        return witness_sigops(version, program, witness);
    }

    // Check if it's P2SH-wrapped witness (P2SH flag must be set).
    if (flags & 0x1) != 0 && script_pubkey.is_p2sh() {
        // Get the redeem script from scriptSig (last push).
        let mut pos = 0;
        let mut last_data = Vec::new();
        while let Some((_opcode, data, new_pos)) = script_sig.get_op(pos) {
            if !data.is_empty() {
                last_data = data;
            }
            pos = new_pos;
        }
        let redeem_script = Script::from_bytes(last_data);
        if let Some((version, program)) = redeem_script.is_witness_program() {
            return witness_sigops(version, program, witness);
        }
    }

    0
}

/// Count sigops for a specific witness version.
///
/// Port of Bitcoin Core's `WitnessSigOps()`.
fn witness_sigops(version: u8, program: &[u8], witness: &crate::transaction::Witness) -> u32 {
    if version == 0 {
        if program.len() == 20 {
            // P2WPKH: always 1 sigop
            return 1;
        }
        if program.len() == 32 && !witness.stack.is_empty() {
            // P2WSH: count sigops in the witness script (last stack element)
            let witness_script = Script::from_slice(witness.stack.last().unwrap());
            return witness_script.get_sig_op_count(true) as u32;
        }
    }
    // Taproot (version 1): counted per-opcode in tapscript, not here.
    0
}

/// Calculate the total sigop cost for a transaction.
///
/// Port of Bitcoin Core's `GetTransactionSigOpCost()`.
pub fn get_transaction_sigop_cost<F>(tx: &Transaction, flags: u32, get_prevout_script: F) -> i64
where
    F: Fn(&OutPoint) -> Option<Script>,
{
    let mut n_sig_ops = get_legacy_sigop_count(tx) as i64 * WITNESS_SCALE_FACTOR as i64;

    if tx.is_coinbase() {
        return n_sig_ops;
    }

    // P2SH sigops
    if (flags & 0x1) != 0 {
        // SCRIPT_VERIFY_P2SH
        n_sig_ops += get_p2sh_sigop_count(tx, |op| get_prevout_script(op)) as i64
            * WITNESS_SCALE_FACTOR as i64;
    }

    // Witness sigops
    for input in &tx.vin {
        if let Some(prev_spk) = get_prevout_script(&input.prevout) {
            n_sig_ops +=
                count_witness_sigops(&input.script_sig, &prev_spk, &input.witness, flags) as i64;
        }
    }

    n_sig_ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::*;
    use qubitcoin_primitives::Txid;
    use qubitcoin_script::Script;

    fn make_valid_tx() -> Transaction {
        Transaction::new(
            1,
            vec![TxIn::new(
                OutPoint::new(Txid::from_bytes([1u8; 32]), 0),
                Script::from_bytes(vec![0x00]),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(
                Amount::from_sat(50_000),
                Script::from_bytes(vec![0x76]),
            )],
            0,
        )
    }

    fn make_coinbase_tx() -> Transaction {
        Transaction::new(
            1,
            vec![TxIn::coinbase(Script::from_bytes(vec![
                0x04, 0xff, 0xff, 0x00, 0x1d,
            ]))],
            vec![TxOut::new(
                Amount::from_btc(50),
                Script::from_bytes(vec![0x76, 0xa9]),
            )],
            0,
        )
    }

    #[test]
    fn test_check_valid_transaction() {
        let tx = make_valid_tx();
        let mut state = TxValidationState::new();
        assert!(check_transaction(&tx, &mut state));
    }

    #[test]
    fn test_check_coinbase_transaction() {
        let tx = make_coinbase_tx();
        let mut state = TxValidationState::new();
        assert!(check_transaction(&tx, &mut state));
    }

    #[test]
    fn test_check_empty_vin() {
        let tx = Transaction::new(1, vec![], vec![TxOut::default()], 0);
        let mut state = TxValidationState::new();
        assert!(!check_transaction(&tx, &mut state));
        assert_eq!(state.get_reject_reason(), "bad-txns-vin-empty");
    }

    #[test]
    fn test_check_empty_vout() {
        let tx = Transaction::new(1, vec![TxIn::default()], vec![], 0);
        let mut state = TxValidationState::new();
        assert!(!check_transaction(&tx, &mut state));
        assert_eq!(state.get_reject_reason(), "bad-txns-vout-empty");
    }

    #[test]
    fn test_check_negative_output() {
        let tx = Transaction::new(
            1,
            vec![TxIn::default()],
            vec![TxOut::new(Amount::from_sat(-1), Script::new())],
            0,
        );
        let mut state = TxValidationState::new();
        assert!(!check_transaction(&tx, &mut state));
        assert_eq!(state.get_reject_reason(), "bad-txns-vout-negative");
    }

    #[test]
    fn test_check_overflow_output() {
        let tx = Transaction::new(
            1,
            vec![TxIn::default()],
            vec![TxOut::new(Amount::from_sat(MAX_MONEY + 1), Script::new())],
            0,
        );
        let mut state = TxValidationState::new();
        assert!(!check_transaction(&tx, &mut state));
        assert_eq!(state.get_reject_reason(), "bad-txns-vout-toolarge");
    }

    #[test]
    fn test_check_duplicate_inputs() {
        let outpoint = OutPoint::new(Txid::from_bytes([1u8; 32]), 0);
        let tx = Transaction::new(
            1,
            vec![
                TxIn::new(outpoint.clone(), Script::new(), SEQUENCE_FINAL),
                TxIn::new(outpoint, Script::new(), SEQUENCE_FINAL),
            ],
            vec![TxOut::new(Amount::from_sat(100), Script::new())],
            0,
        );
        let mut state = TxValidationState::new();
        assert!(!check_transaction(&tx, &mut state));
        assert_eq!(state.get_reject_reason(), "bad-txns-inputs-duplicate");
    }

    #[test]
    fn test_check_bad_coinbase_length() {
        // Coinbase script too short (< 2 bytes)
        let tx = Transaction::new(
            1,
            vec![TxIn::coinbase(Script::from_bytes(vec![0x01]))],
            vec![TxOut::new(Amount::from_btc(50), Script::new())],
            0,
        );
        let mut state = TxValidationState::new();
        assert!(!check_transaction(&tx, &mut state));
        assert_eq!(state.get_reject_reason(), "bad-cb-length");
    }

    #[test]
    fn test_block_subsidy() {
        let params = ConsensusParams::mainnet();
        assert_eq!(get_block_subsidy(0, &params).to_sat(), 50 * COIN);
        assert_eq!(get_block_subsidy(209_999, &params).to_sat(), 50 * COIN);
        assert_eq!(get_block_subsidy(210_000, &params).to_sat(), 25 * COIN);
        assert_eq!(
            get_block_subsidy(420_000, &params).to_sat(),
            125 * COIN / 10
        );
        assert_eq!(get_block_subsidy(13_440_000, &params).to_sat(), 0);
    }

    #[test]
    fn test_block_subsidy_regtest() {
        let params = ConsensusParams::regtest();
        assert_eq!(get_block_subsidy(0, &params).to_sat(), 50 * COIN);
        assert_eq!(get_block_subsidy(150, &params).to_sat(), 25 * COIN);
        assert_eq!(get_block_subsidy(300, &params).to_sat(), 125 * COIN / 10);
    }

    #[test]
    fn test_check_proof_of_work_genesis() {
        let params = ConsensusParams::mainnet();
        // Genesis block hash
        let hash =
            Uint256::from_hex("000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f")
                .unwrap();
        assert!(check_proof_of_work(&hash, 0x1d00ffff, &params));
    }

    #[test]
    fn test_check_proof_of_work_too_easy() {
        let params = ConsensusParams::mainnet();
        // A hash that's too large (doesn't meet target)
        let hash =
            Uint256::from_hex("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
                .unwrap();
        assert!(!check_proof_of_work(&hash, 0x1d00ffff, &params));
    }
}
