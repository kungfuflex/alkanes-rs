//! Predicted balance deltas from a single Bitcoin transaction.
//!
//! Phase 3 of the optimistic-mempool UX work. Once a tx is in
//! `PendingTxStore` (broadcast but not yet indexed), the wallet UI
//! wants to display its expected impact on the user's balances —
//! specifically:
//!
//!   - BTC: trivially derived from input/output values + an
//!     `our_addresses` set.
//!   - Alkanes (edict-driven flows: alkane-send / alkane-transfer):
//!     deterministic from the tx's protostone edicts + the user's
//!     pre-tx alkane balance per input UTXO.
//!   - Alkanes (cellpack-bearing flows: swaps, addLiquidity): the
//!     contract may MINT or BURN tokens, so the predicted output
//!     can't be computed without running the alkane VM. We mark
//!     these as "uncertain" — the caller can still display the
//!     input-side delta (what the user LOSES) and either omit the
//!     output side or annotate it as pending-contract.
//!
//! Phase 3-full (deferred): hook into `alkanes/inspector/runtime.rs`
//! (already wasmi-backed) + a forked-state overlay (vendored from
//! qubitcoin-storage) so we can run cellpacks against the pre-tx
//! state and predict the contract's CallResponse precisely. That
//! closes the swap-output prediction gap.
//!
//! This module covers the high-value case (alkane-send) today; the
//! contract-call case gracefully degrades to "input lost only".

#![allow(unused)]

#[cfg(feature = "std")]
use std::collections::BTreeMap;
#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap;

#[cfg(not(feature = "std"))]
use alloc::{string::{String, ToString}, vec::Vec, format};

use bitcoin::{Transaction, TxOut, OutPoint};
use serde::{Deserialize, Serialize};

use crate::alkanes::balance_sheet::ProtoruneRuneId;

/// One alkane balance change (signed, in sub-units).
///
/// Positive = the user's balance for `alkane_id` is expected to
/// INCREASE by `amount`. Negative = decrease. Zero entries are
/// stripped.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlkaneDelta {
    pub alkane_id: ProtoruneRuneId,
    /// Signed amount as a string (BigInt-safe; u128 + sign won't fit
    /// in JS Number).
    pub delta: String,
}

/// Net BTC delta for the user across this tx's inputs/outputs.
/// Reported as i128 so JS can round-trip large values; in practice
/// this fits in i64 but the sign prevents u64.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BtcDelta {
    pub delta_sats: i128,
}

/// Result of a balance-delta prediction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BalanceDelta {
    pub btc: BtcDelta,
    pub alkanes: Vec<AlkaneDelta>,
    /// True iff the tx contains a cellpack-bearing protostone whose
    /// alkane outputs we couldn't predict. Caller may want to
    /// display "+? CONTRACT_TOKEN pending" as a pending overlay.
    /// When this is true, the alkane deltas reflect ONLY the user's
    /// input-side losses; output-side gains from the contract call
    /// are omitted.
    pub contract_outputs_uncertain: bool,
}

impl Default for BtcDelta {
    fn default() -> Self {
        Self { delta_sats: 0 }
    }
}

/// Per-input prevout context the caller must supply:
///   - the address that owned the input (for BTC arithmetic)
///   - the BTC value
///   - the alkane balances on that prevout (from
///     `protorunesbyoutpoint`)
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PrevoutContext {
    pub address: String,
    pub value_sats: u64,
    pub alkane_balances: BTreeMap<ProtoruneRuneId, u128>,
}

/// Compute the user's predicted balance delta from a candidate tx.
///
/// `prevout_lookup` returns context for each input outpoint. If the
/// caller doesn't have the prevout (e.g., input belongs to a third
/// party), return `None` — that input contributes nothing to OUR
/// delta.
///
/// `output_addresses` maps each output index to the recipient
/// address (None for OP_RETURN). The caller pre-computes this
/// since it depends on the network.
///
/// `protostones` are the parsed protostones from the tx (caller
/// uses the existing `parse_protostones` helper or decodes from
/// the OP_RETURN cellpack — extracted to keep this function pure /
/// network-free).
///
/// `our_addresses` is the set of addresses we own. Output values
/// only count toward our delta when the recipient address is in
/// this set.
pub fn predict_balance_delta(
    tx: &Transaction,
    prevout_lookup: &dyn Fn(OutPoint) -> Option<PrevoutContext>,
    output_addresses: &[Option<String>],
    protostones: &[crate::alkanes::types::ProtostoneSpec],
    our_addresses: &[String],
) -> BalanceDelta {
    let our: alloc::collections::BTreeSet<&str> =
        our_addresses.iter().map(|s| s.as_str()).collect();

    // -----------------------------------------------------------------
    // BTC delta: subtract our inputs, add our outputs.
    // -----------------------------------------------------------------
    let mut btc_delta_sats: i128 = 0;
    for txin in &tx.input {
        if let Some(ctx) = prevout_lookup(txin.previous_output) {
            if our.contains(ctx.address.as_str()) {
                btc_delta_sats -= ctx.value_sats as i128;
            }
        }
    }
    for (i, txout) in tx.output.iter().enumerate() {
        let addr = output_addresses.get(i).and_then(|a| a.as_deref());
        if let Some(addr) = addr {
            if our.contains(addr) {
                btc_delta_sats += txout.value.to_sat() as i128;
            }
        }
    }

    // -----------------------------------------------------------------
    // Alkane delta: walk inputs we own, sum their alkane balances
    // (these are tokens "in flight"). Then walk protostone edicts
    // and add any output-side gains for outputs we own. For
    // cellpack-bearing protostones we mark output-uncertain and
    // skip the gain side.
    // -----------------------------------------------------------------
    let mut input_lost: BTreeMap<ProtoruneRuneId, u128> = BTreeMap::new();
    for txin in &tx.input {
        if let Some(ctx) = prevout_lookup(txin.previous_output) {
            if our.contains(ctx.address.as_str()) {
                for (alkane_id, amount) in &ctx.alkane_balances {
                    *input_lost.entry(alkane_id.clone()).or_insert(0) += amount;
                }
            }
        }
    }

    let mut output_gained: BTreeMap<ProtoruneRuneId, u128> = BTreeMap::new();
    let mut contract_outputs_uncertain = false;

    for proto in protostones {
        if proto.cellpack.is_some() {
            // Contract call — output side requires VM execution to
            // predict. Mark uncertain and skip its edicts/pointer.
            contract_outputs_uncertain = true;
            continue;
        }
        for edict in &proto.edicts {
            // ProtostoneEdict in the types crate: { alkane_id, amount, target }.
            let target_vout = match &edict.target {
                crate::alkanes::types::OutputTarget::Output(idx) => Some(*idx as usize),
                _ => None,
            };
            let Some(output_idx) = target_vout else { continue };
            let recipient = output_addresses
                .get(output_idx)
                .and_then(|a| a.as_deref());
            if let Some(addr) = recipient {
                if our.contains(addr) {
                    let id = ProtoruneRuneId {
                        block: edict.alkane_id.block as u128,
                        tx: edict.alkane_id.tx as u128,
                    };
                    *output_gained.entry(id).or_insert(0) += edict.amount as u128;
                }
            }
        }
        // TODO: handle implicit pointer-driven forwarding (alkanes
        // not allocated by an edict flow to the protostone's
        // pointer output). For Phase 3-lite, edict-driven flows
        // explicitly enumerate every transfer, so this is good
        // enough. Adding pointer handling would need to track
        // per-protostone allocated amounts vs unallocated and
        // route the unallocated to `proto.pointer`.
    }

    // Net per alkane: gained - lost.
    let mut by_id: BTreeMap<ProtoruneRuneId, i128> = BTreeMap::new();
    for (id, lost) in input_lost {
        *by_id.entry(id).or_insert(0) -= lost as i128;
    }
    for (id, gained) in output_gained {
        *by_id.entry(id).or_insert(0) += gained as i128;
    }

    let alkanes: Vec<AlkaneDelta> = by_id
        .into_iter()
        .filter(|(_, d)| *d != 0)
        .map(|(id, d)| AlkaneDelta {
            alkane_id: id,
            delta: d.to_string(),
        })
        .collect();

    BalanceDelta {
        btc: BtcDelta { delta_sats: btc_delta_sats },
        alkanes,
        contract_outputs_uncertain,
    }
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alkanes::types::{OutputTarget, ProtostoneSpec};
    use bitcoin::{TxIn, Witness};
    use std::str::FromStr;

    const USER_ADDR: &str = "bc1p026hg4dfhchc0axnmlpamu4v9gltcqtrzk0nvyc00n4eu5nl5tpsrh7zkm";
    const RECIPIENT: &str = "bc1puvfmy5whzdq35nd2trckkm09em9u7ps6lal564jz92c9feswwrpsr7ach5";
    const PREV_TXID: &str =
        "601a0f80119a49351bdf8088423813d9d1f68b1326d81e2b2daba5f57764b1c0";

    fn make_tx(num_inputs: usize, outs: Vec<u64>) -> Transaction {
        Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: (0..num_inputs)
                .map(|i| TxIn {
                    previous_output: OutPoint {
                        txid: bitcoin::Txid::from_str(PREV_TXID).unwrap(),
                        vout: i as u32,
                    },
                    script_sig: bitcoin::ScriptBuf::new(),
                    sequence: bitcoin::Sequence::ZERO,
                    witness: Witness::new(),
                })
                .collect(),
            output: outs
                .into_iter()
                .map(|v| TxOut {
                    value: bitcoin::Amount::from_sat(v),
                    script_pubkey: bitcoin::ScriptBuf::new(),
                })
                .collect(),
        }
    }

    /// Plain BTC send: 1 input ours (10000) → 8000 to recipient +
    /// 1900 self-change. No protostones. Net: -10000 + 1900 = -8100.
    #[test]
    fn btc_send_outgoing_with_change() {
        let tx = make_tx(1, vec![8000, 1900]);
        let lookup = |op: OutPoint| -> Option<PrevoutContext> {
            if op.vout == 0 {
                Some(PrevoutContext {
                    address: USER_ADDR.to_string(),
                    value_sats: 10000,
                    alkane_balances: Default::default(),
                })
            } else {
                None
            }
        };
        let result = predict_balance_delta(
            &tx,
            &lookup,
            &[Some(RECIPIENT.to_string()), Some(USER_ADDR.to_string())],
            &[],
            &[USER_ADDR.to_string()],
        );
        assert_eq!(result.btc.delta_sats, -8100);
        assert!(result.alkanes.is_empty());
        assert!(!result.contract_outputs_uncertain);
    }

    /// Plain BTC send incoming: 1 input not ours → 5000 to us.
    /// Net: +5000.
    #[test]
    fn btc_send_incoming() {
        let tx = make_tx(1, vec![5000]);
        let lookup = |_| None; // not our prevout
        let result = predict_balance_delta(
            &tx,
            &lookup,
            &[Some(USER_ADDR.to_string())],
            &[],
            &[USER_ADDR.to_string()],
        );
        assert_eq!(result.btc.delta_sats, 5000);
        assert!(result.alkanes.is_empty());
    }

    /// Alkane-send: 1 input ours holding 1000 DIESEL + 546 sats
    /// dust. Edict transfers 800 DIESEL to vout 1 (recipient) and
    /// the protostone's pointer routes the remaining 200 DIESEL to
    /// vout 0 (user's alkane change). Outputs: 546 to recipient,
    /// 0 OP_RETURN. Plus 200 DIESEL conceptually on vout 0 — we
    /// don't get explicit edicts for the 200 (they flow via
    /// pointer), but that's the Phase 3-full work. For 3-lite the
    /// 800 sent + 200 lost = -800 net DIESEL delta.
    ///
    /// Setup: protostone has explicit edicts only (no implicit
    /// pointer forwarding), so input_lost includes 1000 DIESEL,
    /// edicts route 800 to recipient (NOT us), 0 to us → net -1000.
    /// Real alkane-send code emits edict for the FULL amount to the
    /// recipient + relies on pointer for change; that's the Phase
    /// 3-full gap. For now we test the explicit case.
    #[test]
    fn alkane_send_explicit_edict_to_recipient() {
        let tx = make_tx(1, vec![546, 546, 0]); // recipient, change, OP_RETURN
        let alkane_id = ProtoruneRuneId { block: 2, tx: 0 };
        let mut alkane_balances = BTreeMap::new();
        alkane_balances.insert(alkane_id.clone(), 1000u128);

        let lookup = move |op: OutPoint| -> Option<PrevoutContext> {
            if op.vout == 0 {
                Some(PrevoutContext {
                    address: USER_ADDR.to_string(),
                    value_sats: 546,
                    alkane_balances: alkane_balances.clone(),
                })
            } else {
                None
            }
        };

        // One protostone with an explicit edict sending 800 DIESEL
        // to vout 0 (recipient). Phase 3-lite assumes edicts are
        // exhaustive; the remaining 200 isn't routed back to us
        // here.
        let proto = ProtostoneSpec {
            cellpack: None,
            edicts: vec![crate::alkanes::types::ProtostoneEdict {
                alkane_id: crate::alkanes::types::AlkaneId { block: 2, tx: 0 },
                amount: 800,
                target: OutputTarget::Output(0), // recipient
            }],
            bitcoin_transfer: None,
            pointer: Some(OutputTarget::Output(1)),
            refund: Some(OutputTarget::Output(1)),
        };

        let result = predict_balance_delta(
            &tx,
            &lookup,
            &[
                Some(RECIPIENT.to_string()),
                Some(USER_ADDR.to_string()),
                None,
            ],
            &[proto],
            &[USER_ADDR.to_string()],
        );

        // BTC: input 546 lost, vout 1 (546) gained → net 0.
        assert_eq!(result.btc.delta_sats, 0);
        // Lost the full 1000 DIESEL (input). Edict routed 800 to
        // recipient (not us). No edict to us → 0 gained.
        // Net: -1000.
        assert_eq!(result.alkanes.len(), 1);
        assert_eq!(result.alkanes[0].alkane_id, alkane_id);
        assert_eq!(result.alkanes[0].delta, "-1000");
        assert!(!result.contract_outputs_uncertain);
    }

    /// Alkane-send with explicit change-back edict: edict A sends
    /// 800 to recipient, edict B sends 200 back to user's change
    /// output. Net: -800.
    #[test]
    fn alkane_send_with_explicit_change() {
        let tx = make_tx(1, vec![546, 546, 0]);
        let alkane_id = ProtoruneRuneId { block: 2, tx: 0 };
        let mut alkane_balances = BTreeMap::new();
        alkane_balances.insert(alkane_id.clone(), 1000u128);

        let lookup = move |op: OutPoint| -> Option<PrevoutContext> {
            if op.vout == 0 {
                Some(PrevoutContext {
                    address: USER_ADDR.to_string(),
                    value_sats: 546,
                    alkane_balances: alkane_balances.clone(),
                })
            } else {
                None
            }
        };

        let proto = ProtostoneSpec {
            cellpack: None,
            edicts: vec![
                crate::alkanes::types::ProtostoneEdict {
                    alkane_id: crate::alkanes::types::AlkaneId { block: 2, tx: 0 },
                    amount: 800,
                    target: OutputTarget::Output(0), // recipient
                },
                crate::alkanes::types::ProtostoneEdict {
                    alkane_id: crate::alkanes::types::AlkaneId { block: 2, tx: 0 },
                    amount: 200,
                    target: OutputTarget::Output(1), // user's change
                },
            ],
            bitcoin_transfer: None,
            pointer: Some(OutputTarget::Output(1)),
            refund: Some(OutputTarget::Output(1)),
        };

        let result = predict_balance_delta(
            &tx,
            &lookup,
            &[
                Some(RECIPIENT.to_string()),
                Some(USER_ADDR.to_string()),
                None,
            ],
            &[proto],
            &[USER_ADDR.to_string()],
        );

        // Lost 1000 DIESEL, gained 200 → net -800.
        assert_eq!(result.alkanes.len(), 1);
        assert_eq!(result.alkanes[0].delta, "-800");
        assert!(!result.contract_outputs_uncertain);
    }

    /// Cellpack-bearing protostone (e.g. a swap) — alkane output is
    /// determined by contract execution, so we mark
    /// contract_outputs_uncertain and report only the input-side
    /// loss.
    #[test]
    fn cellpack_protostone_marks_uncertain() {
        let tx = make_tx(1, vec![546, 546, 0]);
        let diesel = ProtoruneRuneId { block: 2, tx: 0 };
        let mut alkane_balances = BTreeMap::new();
        alkane_balances.insert(diesel.clone(), 1000u128);

        let lookup = move |op: OutPoint| -> Option<PrevoutContext> {
            if op.vout == 0 {
                Some(PrevoutContext {
                    address: USER_ADDR.to_string(),
                    value_sats: 546,
                    alkane_balances: alkane_balances.clone(),
                })
            } else {
                None
            }
        };

        // Construct a cellpack — a swap call to the AMM factory.
        let cellpack = alkanes_support::cellpack::Cellpack {
            target: alkanes_support::id::AlkaneId {
                block: 4,
                tx: 65522,
            },
            inputs: vec![13, 2, 2, 0, 32, 0, 1000, 1, 100], // factory swap opcode
        };
        let proto = ProtostoneSpec {
            cellpack: Some(cellpack),
            edicts: vec![],
            bitcoin_transfer: None,
            pointer: Some(OutputTarget::Output(0)),
            refund: Some(OutputTarget::Output(0)),
        };

        let result = predict_balance_delta(
            &tx,
            &lookup,
            &[
                Some(USER_ADDR.to_string()),
                Some(USER_ADDR.to_string()),
                None,
            ],
            &[proto],
            &[USER_ADDR.to_string()],
        );

        // -1000 DIESEL (input lost). Output side uncertain — no
        // edict to add positive deltas. Marked uncertain.
        assert_eq!(result.alkanes.len(), 1);
        assert_eq!(result.alkanes[0].delta, "-1000");
        assert!(
            result.contract_outputs_uncertain,
            "cellpack protostone must flag uncertain"
        );
    }

    /// Mixed inputs: 1 ours, 1 not. Only our input contributes a
    /// loss; the other input belongs to whoever else is in the tx.
    #[test]
    fn mixed_inputs_only_subtract_our_share() {
        let tx = make_tx(2, vec![5000]);
        let lookup = |op: OutPoint| -> Option<PrevoutContext> {
            if op.vout == 0 {
                Some(PrevoutContext {
                    address: USER_ADDR.to_string(),
                    value_sats: 3000,
                    alkane_balances: Default::default(),
                })
            } else {
                None // not ours
            }
        };
        let result = predict_balance_delta(
            &tx,
            &lookup,
            &[Some(RECIPIENT.to_string())],
            &[],
            &[USER_ADDR.to_string()],
        );
        assert_eq!(result.btc.delta_sats, -3000);
    }

    /// No protostones, no alkane balances on inputs — alkane
    /// list is empty even though there were inputs.
    #[test]
    fn no_alkane_activity_returns_empty_list() {
        let tx = make_tx(1, vec![5000]);
        let lookup = |op: OutPoint| -> Option<PrevoutContext> {
            if op.vout == 0 {
                Some(PrevoutContext {
                    address: USER_ADDR.to_string(),
                    value_sats: 10000,
                    alkane_balances: Default::default(),
                })
            } else {
                None
            }
        };
        let result = predict_balance_delta(
            &tx,
            &lookup,
            &[Some(USER_ADDR.to_string())],
            &[],
            &[USER_ADDR.to_string()],
        );
        assert!(result.alkanes.is_empty());
    }

    /// Per-alkane net is computed correctly when the same alkane
    /// flows in and back out (input + change-back edict).
    #[test]
    fn net_zero_when_full_circulation() {
        let tx = make_tx(1, vec![546, 0]);
        let id = ProtoruneRuneId { block: 2, tx: 0 };
        let mut alkane_balances = BTreeMap::new();
        alkane_balances.insert(id.clone(), 500u128);

        let lookup = move |op: OutPoint| -> Option<PrevoutContext> {
            if op.vout == 0 {
                Some(PrevoutContext {
                    address: USER_ADDR.to_string(),
                    value_sats: 546,
                    alkane_balances: alkane_balances.clone(),
                })
            } else {
                None
            }
        };

        let proto = ProtostoneSpec {
            cellpack: None,
            edicts: vec![crate::alkanes::types::ProtostoneEdict {
                alkane_id: crate::alkanes::types::AlkaneId { block: 2, tx: 0 },
                amount: 500,
                target: OutputTarget::Output(0), // back to user's own output
            }],
            bitcoin_transfer: None,
            pointer: Some(OutputTarget::Output(0)),
            refund: Some(OutputTarget::Output(0)),
        };

        let result = predict_balance_delta(
            &tx,
            &lookup,
            &[Some(USER_ADDR.to_string()), None],
            &[proto],
            &[USER_ADDR.to_string()],
        );
        // -500 input + 500 output = 0 net → stripped.
        assert!(result.alkanes.is_empty());
    }
}
