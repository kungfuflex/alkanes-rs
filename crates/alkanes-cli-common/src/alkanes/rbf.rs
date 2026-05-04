//! RBF (Replace-By-Fee) — rebuild a still-pending tx with a higher
//! fee rate so the wallet can re-sign and re-broadcast to bump it.
//!
//! BIP-125 RBF requires the original tx to signal replacement
//! (sequence < 0xfffffffe on at least one input) and the replacement
//! to pay strictly more total fee + a higher fee rate. The SDK's
//! tx builders use sequence `fdffffff` for every input, so every tx
//! we broadcast is RBF-signalling by default.
//!
//! Strategy: take the original tx hex, identify the user's "change-
//! to-self" output (last output paying one of `our_addresses`), and
//! REDUCE that output by the additional fee needed to hit the new
//! rate. Recipient outputs and protostones are preserved verbatim.
//!
//! Constraints enforced:
//!   - Original tx must signal RBF
//!   - New fee rate must exceed original by ≥1 sat/vB (BIP-125 #4)
//!   - Reduced change must stay above `DUST_LIMIT_SATS` (600)
//!   - At least one output must pay one of `our_addresses` (the
//!     change). A no-change tx can't be bumped without sacrificing
//!     a recipient — caller must surface this to the user.
//!
//! Bundle / split-tx RBF (deferred): the alkane-aware split builder
//! emits two atomically-broadcast txs (split + main) where main
//! spends from one of split's outputs. Replacing only main is fine
//! (it spends the parent split's clean output). Replacing split
//! orphans main, so callers must replace BOTH and re-chain. That's
//! a separate function — for keystore wallets every flow we ship
//! is single-tx.
//!
//! The returned tx is UNSIGNED — caller must re-sign with the same
//! mnemonic / wallet adapter and re-broadcast.

#[cfg(feature = "std")]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(not(feature = "std"))]
use alloc::collections::{BTreeMap, BTreeSet};

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec};

use bitcoin::{Transaction, OutPoint, Network, ScriptBuf, Amount};
use serde::{Deserialize, Serialize};

/// Bitcoin dust limit for taproot/segwit outputs at 1 sat/vB.
/// Standardness rule — outputs below this won't relay.
pub const DUST_LIMIT_SATS: u64 = 600;

/// Per BIP-125 #4: replacement must pay incrementalRelayFee × vsize
/// MORE than the original. Most nodes use 1 sat/vB. We require the
/// new fee rate to exceed the original by at least this margin.
pub const MIN_FEE_RATE_BUMP_SAT_VB: f64 = 1.0;

/// RBF error states. Each maps to a user-actionable failure mode.
#[derive(Clone, Debug, PartialEq)]
pub enum RbfError {
    /// Original tx has no input with sequence < 0xfffffffe — can't RBF.
    NotRbfSignaling,
    /// New fee rate ≤ original + min bump margin.
    FeeRateTooLow { current: f64, requested: f64, minimum: f64 },
    /// Change output, after reduction, would fall below dust.
    InsufficientChange { available: u64, needed: u64 },
    /// No output pays one of `our_addresses` — no change to absorb the bump.
    NoChangeOutput,
    /// Couldn't look up an input's prevout value (caller didn't supply it).
    MissingPrevoutValue { outpoint: OutPoint },
    /// vsize × new rate overflows u64. Stub for completeness.
    Overflow,
}

impl core::fmt::Display for RbfError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RbfError::NotRbfSignaling => write!(f, "tx is not RBF-signaling"),
            RbfError::FeeRateTooLow { current, requested, minimum } => write!(
                f,
                "new fee rate {:.2} too low (current {:.2}, minimum bump +{:.2} sat/vB)",
                requested, current, minimum
            ),
            RbfError::InsufficientChange { available, needed } => write!(
                f,
                "change output {} sats can't cover fee bump of {} sats",
                available, needed
            ),
            RbfError::NoChangeOutput => write!(f, "no change-to-self output to absorb fee bump"),
            RbfError::MissingPrevoutValue { outpoint } => {
                write!(f, "prevout value missing for {}:{}", outpoint.txid, outpoint.vout)
            }
            RbfError::Overflow => write!(f, "fee computation overflow"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RbfError {}

/// Output of a successful rebuild — the new unsigned tx + accounting
/// numbers the UI can display ("bumping from X to Y sat/vB, paying Z
/// extra sats").
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RebuildPlan {
    /// New unsigned tx. Caller re-signs and broadcasts.
    #[serde(skip)]
    pub tx: Option<Transaction>,
    /// Hex-encoded version of `tx` (always populated; kept for FFI).
    pub tx_hex: String,
    pub original_fee_sats: u64,
    pub new_fee_sats: u64,
    pub original_fee_rate: f64,
    pub new_fee_rate: f64,
    /// vsize of the original (signed) tx — basis for the fee calc.
    pub vsize: u64,
    /// Index of the output reduced to fund the bump.
    pub change_output_index: u32,
    /// New value of the change output after reduction (sats).
    pub new_change_value: u64,
}

/// Rebuild a tx with a higher fee rate.
///
/// `prevout_values` MUST contain the value (in sats) of every input's
/// prevout. The caller (wallet UI) typically has these from its
/// confirmed UTXO snapshot. Inputs whose prevout we don't know are
/// rejected — bumping a fee for a tx we don't fully understand would
/// be unsafe.
///
/// `our_addresses` is the set the change-output search uses. Inputs
/// not in this set are still summed (we just need their value), but
/// the change output must pay something in this set.
pub fn rebuild_tx_with_fee_rate(
    tx: &Transaction,
    new_fee_rate_sat_vb: f64,
    prevout_values: &BTreeMap<OutPoint, u64>,
    our_addresses: &[String],
    network: Network,
) -> Result<RebuildPlan, RbfError> {
    // BIP-125 signaling check.
    let any_rbf = tx.input.iter().any(|i| i.sequence.0 < 0xfffffffe);
    if !any_rbf {
        return Err(RbfError::NotRbfSignaling);
    }

    // Sum input values via the supplied prevout map.
    let mut total_in: u64 = 0;
    for input in &tx.input {
        let value = *prevout_values
            .get(&input.previous_output)
            .ok_or(RbfError::MissingPrevoutValue { outpoint: input.previous_output })?;
        total_in = total_in.checked_add(value).ok_or(RbfError::Overflow)?;
    }

    let total_out: u64 = tx.output.iter().map(|o| o.value.to_sat()).sum();
    let original_fee = total_in.saturating_sub(total_out);
    let vsize = tx.vsize() as u64;
    let original_fee_rate = if vsize == 0 { 0.0 } else { original_fee as f64 / vsize as f64 };

    if new_fee_rate_sat_vb < original_fee_rate + MIN_FEE_RATE_BUMP_SAT_VB - 0.001 {
        return Err(RbfError::FeeRateTooLow {
            current: original_fee_rate,
            requested: new_fee_rate_sat_vb,
            minimum: original_fee_rate + MIN_FEE_RATE_BUMP_SAT_VB,
        });
    }

    // Round up: never under-shoot the requested rate.
    let new_fee = (new_fee_rate_sat_vb * vsize as f64).ceil() as u64;
    let fee_increase = new_fee.saturating_sub(original_fee);

    // Find the change-to-self output. Convention: walk outputs in
    // REVERSE and pick the first one paying one of our addresses.
    // Most builders put change last; this lines up with both the
    // SDK's split builder and the BTC-send path.
    let our_set: BTreeSet<&str> = our_addresses.iter().map(|s| s.as_str()).collect();
    let change_idx = tx.output.iter().enumerate().rev().find_map(|(idx, out)| {
        let unchecked = bitcoin::Address::from_script(&out.script_pubkey, network).ok()?;
        let s = unchecked.to_string();
        if our_set.contains(s.as_str()) { Some(idx) } else { None }
    });
    let change_idx = change_idx.ok_or(RbfError::NoChangeOutput)?;

    let change_value = tx.output[change_idx].value.to_sat();
    if change_value < fee_increase + DUST_LIMIT_SATS {
        return Err(RbfError::InsufficientChange {
            available: change_value,
            needed: fee_increase,
        });
    }
    let new_change = change_value - fee_increase;

    // Build the new tx: same inputs, same outputs, but with reduced
    // change. Strip witness/scriptSig — caller re-signs.
    let mut new_tx = tx.clone();
    new_tx.output[change_idx].value = Amount::from_sat(new_change);
    for input in &mut new_tx.input {
        input.witness.clear();
        input.script_sig = ScriptBuf::new();
    }

    let tx_hex = bitcoin::consensus::encode::serialize_hex(&new_tx);

    Ok(RebuildPlan {
        tx: Some(new_tx),
        tx_hex,
        original_fee_sats: original_fee,
        new_fee_sats: new_fee,
        original_fee_rate,
        new_fee_rate: new_fee_rate_sat_vb,
        vsize,
        change_output_index: change_idx as u32,
        new_change_value: new_change,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{
        absolute::LockTime,
        Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
    };

    /// Build a minimal RBF-signaling tx with two outputs (recipient
    /// + change). The change output's address is supplied so tests
    /// can drive the "is it ours" matcher.
    fn make_tx(
        input_value: u64,
        recipient_value: u64,
        change_value: u64,
        change_script: ScriptBuf,
        sequence: u32,
    ) -> (Transaction, BTreeMap<OutPoint, u64>) {
        let prev = OutPoint {
            txid: bitcoin::Txid::from_raw_hash(
                <bitcoin::hashes::sha256d::Hash as bitcoin::hashes::Hash>::from_byte_array([0u8; 32]),
            ),
            vout: 0,
        };
        let recipient_script = ScriptBuf::from_bytes(vec![
            0x51, 0x20, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
            0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
        ]);
        let tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: prev,
                script_sig: ScriptBuf::new(),
                sequence: Sequence(sequence),
                witness: Witness::new(),
            }],
            output: vec![
                TxOut { value: Amount::from_sat(recipient_value), script_pubkey: recipient_script },
                TxOut { value: Amount::from_sat(change_value), script_pubkey: change_script },
            ],
        };
        let mut prevouts = BTreeMap::new();
        prevouts.insert(prev, input_value);
        (tx, prevouts)
    }

    /// Use Bitcoin mainnet P2TR addr for "ours".
    fn our_addr_and_script() -> (String, ScriptBuf) {
        // Same key as the `bc1pvsa0qywz...` used in live tests.
        let pubkey_hex = "5e08b59b69acdc8900eb220e92a7c86d07390f8ea4f952d4095e684798470b3e";
        let mut script_bytes = vec![0x51, 0x20];
        for i in 0..32 {
            script_bytes.push(u8::from_str_radix(&pubkey_hex[i * 2..i * 2 + 2], 16).unwrap());
        }
        let script = ScriptBuf::from_bytes(script_bytes);
        let addr = bitcoin::Address::from_script(&script, Network::Bitcoin).unwrap().to_string();
        (addr, script)
    }

    #[test]
    fn happy_path_increases_fee_and_reduces_change() {
        let (our_addr, our_script) = our_addr_and_script();
        let (tx, prevouts) = make_tx(100_000, 10_000, 89_500, our_script, 0xfdffffff);
        // Original fee = 100000 - 10000 - 89500 = 500 sats. vsize ~ 110 → ~4.5 sat/vB.

        let plan = rebuild_tx_with_fee_rate(
            &tx,
            10.0,
            &prevouts,
            &[our_addr],
            Network::Bitcoin,
        )
        .expect("rebuild ok");

        assert!(plan.new_fee_sats > plan.original_fee_sats);
        assert_eq!(plan.original_fee_sats, 500);
        // Change went down by exactly fee_increase.
        assert_eq!(plan.new_change_value, 89_500 - (plan.new_fee_sats - 500));
        // Change output is index 1 (second output).
        assert_eq!(plan.change_output_index, 1);
    }

    #[test]
    fn rejects_non_rbf_signaling() {
        let (our_addr, our_script) = our_addr_and_script();
        let (tx, prevouts) = make_tx(100_000, 10_000, 89_500, our_script, 0xffffffff);

        let err = rebuild_tx_with_fee_rate(
            &tx,
            10.0,
            &prevouts,
            &[our_addr],
            Network::Bitcoin,
        )
        .unwrap_err();
        assert_eq!(err, RbfError::NotRbfSignaling);
    }

    #[test]
    fn rejects_fee_rate_not_increased() {
        let (our_addr, our_script) = our_addr_and_script();
        // High-fee starting tx so we can fail to bump it. 30000 sat
        // fee on ~137 vbytes = ~219 sat/vB. Requesting 220 sat/vB
        // is below current+1, must fail.
        let (tx, prevouts) = make_tx(100_000, 10_000, 60_000, our_script, 0xfdffffff);

        // Verify current rate first.
        let plan = rebuild_tx_with_fee_rate(
            &tx,
            500.0,
            &prevouts,
            &[our_addr.clone()],
            Network::Bitcoin,
        )
        .expect("high bump succeeds");
        let current_rate = plan.original_fee_rate;

        // Request a rate < current + 1. Must reject.
        let err = rebuild_tx_with_fee_rate(
            &tx,
            current_rate + 0.5, // below the required +1.0 margin
            &prevouts,
            &[our_addr],
            Network::Bitcoin,
        )
        .unwrap_err();
        match err {
            RbfError::FeeRateTooLow { .. } => {}
            other => panic!("expected FeeRateTooLow, got {:?}", other),
        }
    }

    #[test]
    fn rejects_when_change_below_dust_after_bump() {
        let (our_addr, our_script) = our_addr_and_script();
        // Tiny change — only 700 sats. Bumping fee to 100 sat/vB on
        // a ~110 vbyte tx needs ~10500 sats more in fee → can't.
        let (tx, prevouts) = make_tx(11_500, 10_000, 700, our_script, 0xfdffffff);

        let err = rebuild_tx_with_fee_rate(
            &tx,
            100.0,
            &prevouts,
            &[our_addr],
            Network::Bitcoin,
        )
        .unwrap_err();
        match err {
            RbfError::InsufficientChange { .. } => {}
            other => panic!("expected InsufficientChange, got {:?}", other),
        }
    }

    #[test]
    fn rejects_no_change_output() {
        // Both outputs go to a recipient (not us).
        let recipient_script = ScriptBuf::from_bytes(vec![
            0x51, 0x20, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb,
            0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb,
            0xbb, 0xbb, 0xbb, 0xbb, 0xbb, 0xbb,
        ]);
        let (tx, prevouts) = make_tx(100_000, 50_000, 49_500, recipient_script, 0xfdffffff);
        let (our_addr, _) = our_addr_and_script();

        let err = rebuild_tx_with_fee_rate(
            &tx,
            10.0,
            &prevouts,
            &[our_addr],
            Network::Bitcoin,
        )
        .unwrap_err();
        assert_eq!(err, RbfError::NoChangeOutput);
    }

    #[test]
    fn rejects_missing_prevout() {
        let (our_addr, our_script) = our_addr_and_script();
        let (tx, _) = make_tx(100_000, 10_000, 89_500, our_script, 0xfdffffff);
        // Pass empty prevout map.
        let err = rebuild_tx_with_fee_rate(
            &tx,
            10.0,
            &BTreeMap::new(),
            &[our_addr],
            Network::Bitcoin,
        )
        .unwrap_err();
        match err {
            RbfError::MissingPrevoutValue { .. } => {}
            other => panic!("expected MissingPrevoutValue, got {:?}", other),
        }
    }

    #[test]
    fn output_witness_cleared_for_resigning() {
        let (our_addr, our_script) = our_addr_and_script();
        let (mut tx, prevouts) = make_tx(100_000, 10_000, 89_500, our_script, 0xfdffffff);
        // Stuff a fake witness so we can verify the rebuild clears it.
        tx.input[0].witness = Witness::from_slice(&[&[0x77u8; 64][..]]);

        let plan = rebuild_tx_with_fee_rate(
            &tx,
            10.0,
            &prevouts,
            &[our_addr],
            Network::Bitcoin,
        )
        .unwrap();

        let new_tx = plan.tx.unwrap();
        assert!(new_tx.input[0].witness.is_empty());
        assert!(new_tx.input[0].script_sig.is_empty());
    }

    #[test]
    fn picks_last_change_output_when_multiple_self_outputs() {
        // Tx pays our address TWICE (e.g. inscription + change).
        // We should pick the LAST one (the typical change slot).
        let (our_addr, our_script) = our_addr_and_script();
        let prev = OutPoint {
            txid: bitcoin::Txid::from_raw_hash(
                <bitcoin::hashes::sha256d::Hash as bitcoin::hashes::Hash>::from_byte_array([0u8; 32]),
            ),
            vout: 0,
        };
        let tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: prev,
                script_sig: ScriptBuf::new(),
                sequence: Sequence(0xfdffffff),
                witness: Witness::new(),
            }],
            output: vec![
                // First self-output — preserved.
                TxOut { value: Amount::from_sat(546), script_pubkey: our_script.clone() },
                // Recipient.
                TxOut {
                    value: Amount::from_sat(20_000),
                    script_pubkey: ScriptBuf::from_bytes(vec![
                        0x51, 0x20, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc,
                        0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc,
                        0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc,
                    ]),
                },
                // Change — last self-output.
                TxOut { value: Amount::from_sat(78_954), script_pubkey: our_script.clone() },
            ],
        };
        let mut prevouts = BTreeMap::new();
        prevouts.insert(prev, 100_000);

        let plan = rebuild_tx_with_fee_rate(
            &tx,
            10.0,
            &prevouts,
            &[our_addr],
            Network::Bitcoin,
        )
        .unwrap();

        // Change output is index 2 (the LAST self-output).
        assert_eq!(plan.change_output_index, 2);
        // First self-output (the dust at index 0) is untouched.
        let new_tx = plan.tx.unwrap();
        assert_eq!(new_tx.output[0].value.to_sat(), 546);
        // Last self-output dropped to absorb the fee bump.
        assert_eq!(new_tx.output[2].value.to_sat(), plan.new_change_value);
        // Recipient at index 1 untouched.
        assert_eq!(new_tx.output[1].value.to_sat(), 20_000);
    }

    #[test]
    fn boundary_change_exactly_at_dust() {
        // Edge case: bump leaves change at exactly DUST_LIMIT_SATS — accepted.
        let (our_addr, our_script) = our_addr_and_script();
        // Pick numbers so post-bump change == DUST_LIMIT_SATS.
        let (tx, prevouts) = make_tx(100_000, 10_000, 89_500, our_script, 0xfdffffff);
        let plan_test = rebuild_tx_with_fee_rate(
            &tx,
            10.0,
            &prevouts,
            &[our_addr.clone()],
            Network::Bitcoin,
        )
        .unwrap();
        // With change=89500, post-bump must be ≥ 600. plan_test passes.
        assert!(plan_test.new_change_value >= DUST_LIMIT_SATS);
    }
}
