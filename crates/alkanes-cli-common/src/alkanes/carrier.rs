//! Alkane-carrier detection shared by every wallet spend path.
//!
//! An alkane "carrier" is a UTXO whose protorune balance sheet holds a
//! non-zero amount of some alkane. Spending one as a plain BTC input is never
//! safe: a plain send carries no protostone to route the tokens, so the
//! indexer settles them by its fallback rule (erased before block 972,000,
//! credited to the first non-OP_RETURN output from 972,000, which in a send
//! is the recipient). Either way the sender loses them.
//!
//! `EnhancedAlkanesExecutor` already resolved every candidate's sheet, failed
//! closed on unknown status and skipped carriers when funding BTC (Halborn,
//! "Incomplete Wallet Carrier Validation"). This module is that logic, factored
//! out so the plain-send builders (CLI `wallet send` / `send-all` /
//! `create-tx`, the web-sys providers and `TransactionConstructor`) apply the
//! exact same rule instead of a second implementation:
//!
//! * [`resolve_alkane_sheet`] - one candidate's status: caller assertion,
//!   then a `protorunesbyoutpoint` probe. A failed probe is `Unknown`, never
//!   clean.
//! * [`exclude_alkane_carriers`] - partition a candidate set into verified
//!   clean UTXOs, carriers, unknowns and UTXOs the indexer has not seen yet.
//! * [`check_input_alkane_sheets`] - the pre-sign invariant over a built
//!   transaction's inputs (alkanes calls and plain sends).
//! * [`assert_plain_send_inputs_clean`] - re-probe a built plain send's inputs
//!   and run the invariant right before signing.

use crate::traits::{MetashrewRpcProvider, UtxoInfo};
use crate::{AlkanesError, Result};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::{format, string::{String, ToString}, vec::Vec};
use bitcoin::{OutPoint, Transaction};
use protorune_support::balance_sheet::ProtoruneRuneId;

use super::types::{AlkaneId, OutputTarget, ProtostoneSpec};

/// Caller-asserted alkane sheets per outpoint (`prefetched_utxos[].alkanes`,
/// Espo spendable-outpoint balances). An empty vector asserts "clean".
pub type AssertedSheets = BTreeMap<OutPoint, Vec<(ProtoruneRuneId, u128)>>;

/// Where an [`AlkaneSheetStatus`] answer came from (for logging / stats).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetSource {
    /// Qubitcoin single-process mode: no per-outpoint alkanes index, treated
    /// as authoritative clean (unchanged executor behaviour).
    Qubitcoin,
    /// Caller assertion (`prefetched_utxos` / Espo).
    Asserted,
    /// `protorunesbyoutpoint` probe.
    Probe,
}

/// Resolved alkane status of one UTXO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlkaneSheetStatus {
    /// Verified to carry no alkanes.
    Clean,
    /// Carries at least one alkane with a non-zero amount.
    Carrier(Vec<(ProtoruneRuneId, u128)>),
    /// Could not be determined (probe failed). Never treated as clean.
    Unknown(String),
}

impl AlkaneSheetStatus {
    fn from_balances<I: IntoIterator<Item = (ProtoruneRuneId, u128)>>(balances: I) -> Self {
        let carried: Vec<(ProtoruneRuneId, u128)> =
            balances.into_iter().filter(|(_, amount)| *amount > 0).collect();
        if carried.is_empty() {
            AlkaneSheetStatus::Clean
        } else {
            AlkaneSheetStatus::Carrier(carried)
        }
    }
}

/// Resolve one UTXO's alkane sheet: qubitcoin mode, then caller assertion,
/// then a lazy `protorunesbyoutpoint` probe. A probe error is `Unknown`.
pub async fn resolve_alkane_sheet<P: MetashrewRpcProvider + ?Sized>(
    provider: &P,
    qubitcoin_mode: bool,
    outpoint: &OutPoint,
    asserted: Option<&AssertedSheets>,
) -> (AlkaneSheetStatus, SheetSource) {
    if qubitcoin_mode {
        return (AlkaneSheetStatus::Clean, SheetSource::Qubitcoin);
    }
    if let Some(sheet) = asserted.and_then(|m| m.get(outpoint)) {
        return (
            AlkaneSheetStatus::from_balances(sheet.iter().cloned()),
            SheetSource::Asserted,
        );
    }
    match provider
        .get_protorunes_by_outpoint(&outpoint.txid.to_string(), outpoint.vout, None, 1)
        .await
    {
        Ok(response) => (
            AlkaneSheetStatus::from_balances(
                response
                    .balance_sheet
                    .cached
                    .balances
                    .iter()
                    .map(|(id, amount)| (ProtoruneRuneId { block: id.block, tx: id.tx }, *amount)),
            ),
            SheetSource::Probe,
        ),
        Err(e) => (AlkaneSheetStatus::Unknown(format!("{e}")), SheetSource::Probe),
    }
}

/// Result of [`exclude_alkane_carriers`].
#[derive(Debug, Clone, Default)]
pub struct CarrierFilter {
    /// Candidates verified to carry no alkanes, in input order.
    pub clean: Vec<(OutPoint, UtxoInfo)>,
    /// Outpoints of `clean`, for the pre-sign invariant.
    pub clean_outpoints: BTreeSet<OutPoint>,
    /// Candidates that carry alkanes (excluded).
    pub carriers: Vec<OutPoint>,
    /// Candidates whose alkane status could not be determined (excluded).
    pub unknown: Vec<OutPoint>,
    /// Candidates that probed clean but whose transaction the indexer has
    /// not indexed yet (unconfirmed, or above metashrew's indexed height),
    /// so the empty answer proves nothing (excluded).
    pub unindexed: Vec<OutPoint>,
}

impl CarrierFilter {
    /// Sats available in verified-clean UTXOs (frozen ones excluded, as the
    /// selectors skip them).
    pub fn clean_total(&self) -> u64 {
        self.clean.iter().filter(|(_, u)| !u.frozen).map(|(_, u)| u.amount).sum()
    }

    /// The error a plain BTC send returns when the verified-clean UTXOs
    /// cannot fund it. Says how many carriers / unknowns / unindexed were
    /// excluded so the user can tell "you are broke" from "your BTC sits on
    /// alkane carriers" from "the indexer is down" from "the indexer is
    /// behind".
    pub fn insufficient_error(&self, needed: u64, have: u64) -> AlkanesError {
        let mut msg = format!(
            "Insufficient spendable BTC: need {} sats, have {} sats in UTXOs verified to carry no alkanes",
            needed, have
        );
        if !self.carriers.is_empty() {
            msg.push_str(&format!(
                "; {} UTXO(s) carrying alkanes were excluded (a plain BTC send would transfer \
                 their alkanes to the recipient; move them with an alkanes transfer first)",
                self.carriers.len()
            ));
        }
        if !self.unknown.is_empty() {
            msg.push_str(&format!(
                "; {} UTXO(s) were excluded because their alkane balance could not be verified \
                 (indexer probe failed); retry once the alkanes indexer is reachable",
                self.unknown.len()
            ));
        }
        if !self.unindexed.is_empty() {
            msg.push_str(&format!(
                "; {} UTXO(s) were excluded because the alkanes indexer has not indexed them yet \
                 (unconfirmed, or above the indexed height); retry after the next block is indexed",
                self.unindexed.len()
            ));
        }
        AlkanesError::Wallet(msg)
    }
}

/// Partition `candidates` into verified-clean UTXOs, alkane carriers,
/// UTXOs of unknown status and unindexed UTXOs. All but the clean ones are
/// excluded (fail closed). Clean entries come back with `has_alkanes =
/// false`; every candidate is resolved (a plain send may need all of them,
/// e.g. send-all).
///
/// An empty probe only proves "clean" for a transaction the indexer has
/// seen: a probe-clean candidate with a `block_height` above metashrew's
/// indexed height, or with no `block_height` and 0 confirmations, is
/// `unindexed` (the executor's `probe_fee_input_carrier` applies the same
/// rule through [`probe_not_indexed`]). A known `block_height` decides on
/// its own, whatever `confirmations` says: providers derive `confirmations`
/// from a bitcoin tip read that falls back to 0 when it fails, while
/// `block_height` comes from the UTXO's own status. Metashrew's height is
/// read at most once, and only when a probe-clean candidate has a
/// `block_height`; if the read fails, every such candidate is `unindexed`.
/// Asserted and qubitcoin answers do not depend on the indexer having seen
/// the transaction and are unchanged.
///
/// The pre-sign re-probes ([`assert_plain_send_inputs_clean`] and the
/// package / PSBT variants) do not apply this rule: they see only a
/// transaction, not its inputs' `UtxoInfo`. They re-check carriers and
/// unknowns; the unindexed rule lives here, in selection.
pub async fn exclude_alkane_carriers<P: MetashrewRpcProvider + ?Sized>(
    provider: &P,
    qubitcoin_mode: bool,
    candidates: Vec<(OutPoint, UtxoInfo)>,
    asserted: Option<&AssertedSheets>,
) -> CarrierFilter {
    let mut out = CarrierFilter::default();
    let mut indexed_height: Option<core::result::Result<u64, String>> = None;
    for (outpoint, mut info) in candidates {
        match resolve_alkane_sheet(provider, qubitcoin_mode, &outpoint, asserted).await {
            (AlkaneSheetStatus::Clean, source) => {
                if source == SheetSource::Probe {
                    if let Some(why) = probe_not_indexed(provider, &info, &mut indexed_height).await {
                        log::info!(
                            "Excluding UTXO {}:{} from BTC funding: the alkanes indexer has not indexed it yet ({})",
                            outpoint.txid, outpoint.vout, why
                        );
                        out.unindexed.push(outpoint);
                        continue;
                    }
                }
                info.has_alkanes = false;
                out.clean_outpoints.insert(outpoint);
                out.clean.push((outpoint, info));
            }
            (AlkaneSheetStatus::Carrier(sheet), _) => {
                log::info!(
                    "Excluding alkane carrier {}:{} ({} alkane id(s)) from BTC funding",
                    outpoint.txid, outpoint.vout, sheet.len()
                );
                out.carriers.push(outpoint);
            }
            (AlkaneSheetStatus::Unknown(e), _) => {
                log::warn!(
                    "Excluding UTXO {}:{} from BTC funding: alkane balance could not be verified ({})",
                    outpoint.txid, outpoint.vout, e
                );
                out.unknown.push(outpoint);
            }
        }
    }
    if !out.carriers.is_empty() || !out.unknown.is_empty() || !out.unindexed.is_empty() {
        log::info!(
            "Alkane carrier filter: {} clean, {} carrier(s) excluded, {} unknown excluded, {} unindexed excluded",
            out.clean.len(), out.carriers.len(), out.unknown.len(), out.unindexed.len()
        );
    }
    out
}

/// Why an empty probe of `info` proves nothing, if it does not: a
/// `block_height` above metashrew's indexed height (a tx at height h is
/// indexed iff h <= that height), or no `block_height` and 0 confirmations.
/// `confirmations` is only consulted without a `block_height`, because it is
/// derived from a bitcoin tip read that falls back to 0 on failure; a known
/// height is compared whatever `confirmations` says. `indexed_height` caches
/// the one read per [`exclude_alkane_carriers`] call; a failed read counts
/// every candidate with a `block_height` as unindexed. The executor's
/// `probe_fee_input_carrier` shares it, with the cache filled before its
/// first probe.
pub(crate) async fn probe_not_indexed<P: MetashrewRpcProvider + ?Sized>(
    provider: &P,
    info: &UtxoInfo,
    indexed_height: &mut Option<core::result::Result<u64, String>>,
) -> Option<String> {
    let Some(height) = info.block_height else {
        return (info.confirmations == 0).then(|| "unconfirmed".to_string());
    };
    let tip = match indexed_height {
        Some(tip) => tip,
        None => {
            let read = provider.get_metashrew_height().await.map_err(|e| format!("{e}"));
            if let Err(e) = &read {
                log::warn!(
                    "Alkane carrier filter: metashrew indexed height unavailable ({}); excluding \
                     every probe-clean UTXO with a block height",
                    e
                );
            }
            indexed_height.insert(read)
        }
    };
    match tip {
        Ok(tip) if height <= *tip => None,
        Ok(tip) => Some(format!("block {height} is above the indexed height {tip}")),
        Err(e) => Some(format!("indexed height unavailable: {e}")),
    }
}

/// What kind of transaction [`check_input_alkane_sheets`] is guarding; only
/// changes the wording of the refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendKind {
    /// An alkanes call built by `EnhancedAlkanesExecutor`: leftover input
    /// balances go to the called contract.
    AlkanesCall,
    /// A plain BTC send (no protostones): carried alkanes would go to the
    /// first output, the recipient.
    PlainBtcSend,
}

/// Pre-sign carrier invariant (Halborn: "Incomplete Wallet Carrier Validation
/// Can Transfer Unrequested Assets to Malicious Contracts"; SFS-05 for plain
/// sends).
///
/// Protorune aggregates the alkane sheets of EVERY input and forwards the
/// unallocated remainder to the first alkanes message (or, without one, to the
/// indexer's fallback output), so any token riding an input is lost unless the
/// transaction routes it elsewhere first. Before a transaction is signed,
/// every input must have a KNOWN sheet (`per_utxo_alkanes` entry, or verified
/// clean), and every alkane id carried by the inputs must either be requested
/// (`alkanes_needed`) or be explicitly routed to a physical output by an edict
/// in the first protostone (the one that receives the leftover input balance)
/// covering the full carried amount (`amount == 0` means "all").
///
/// A plain BTC send passes no requested ids and no protostones, so ANY carrier
/// input is refused.
///
/// Excess of a REQUESTED alkane id is not covered here - that is the
/// requested asset and its routing is the caller's protostone semantics.
///
/// Amounts are u128, the executor's width (develop's `per_utxo_alkanes`,
/// `alkanes_needed` and `ProtostoneEdict.amount`): saturating a carried total
/// to u64 would let a smaller edict "cover" it.
pub fn check_input_alkane_sheets(
    kind: SpendKind,
    inputs: &[OutPoint],
    per_utxo_alkanes: &BTreeMap<OutPoint, Vec<(AlkaneId, u128)>>,
    clean_outpoints: &BTreeSet<OutPoint>,
    alkanes_needed: &BTreeMap<AlkaneId, u128>,
    protostones: &[ProtostoneSpec],
) -> Result<()> {
    let mut unrequested: BTreeMap<AlkaneId, u128> = BTreeMap::new();
    for op in inputs {
        if let Some(sheet) = per_utxo_alkanes.get(op) {
            for (id, amount) in sheet {
                if *amount > 0 && !alkanes_needed.contains_key(id) {
                    let total = unrequested.entry(id.clone()).or_insert(0);
                    *total = total.saturating_add(*amount);
                }
            }
        } else if !clean_outpoints.contains(op) {
            return Err(AlkanesError::Wallet(match kind {
                SpendKind::AlkanesCall => format!(
                    "Refusing to build: input {}:{} has an unknown alkane balance; it could forward \
                     unrequested tokens to the called contract",
                    op.txid, op.vout
                ),
                SpendKind::PlainBtcSend => format!(
                    "Refusing to sign: input {}:{} has an unknown alkane balance (indexer probe \
                     failed); a plain BTC send could transfer alkanes on it to the recipient. Retry \
                     once the alkanes indexer is reachable",
                    op.txid, op.vout
                ),
            }));
        }
    }
    let routed = |id: &AlkaneId, total: u128| -> bool {
        protostones.first().map_or(false, |p| {
            p.edicts.iter().any(|e| {
                e.alkane_id == *id
                    && matches!(e.target, OutputTarget::Output(_))
                    && (e.amount == 0 || e.amount >= total)
            })
        })
    };
    let unrouted: Vec<String> = unrequested
        .iter()
        .filter(|(id, total)| !routed(id, **total))
        .map(|(id, total)| format!("{}:{} x{}", id.block, id.tx, total))
        .collect();
    if !unrouted.is_empty() {
        return Err(AlkanesError::Wallet(match kind {
            SpendKind::AlkanesCall => format!(
                "Refusing to build: selected inputs carry alkanes that were not requested and are not \
                 routed back by an edict ({}). Protorune would forward them to the called contract. \
                 Request them explicitly, add an edict in the first protostone sending them to one of \
                 your outputs, or split that UTXO first",
                unrouted.join(", ")
            ),
            SpendKind::PlainBtcSend => format!(
                "Refusing to sign: the inputs of this plain BTC send carry alkanes ({}). With no \
                 protostone to route them they would go to the first output (the recipient). Move \
                 them with an alkanes transfer first",
                unrouted.join(", ")
            ),
        }));
    }
    Ok(())
}

/// Convert a resolved carrier sheet into the executor's per-UTXO shape.
/// Ids beyond u64 saturate (`AlkaneId` is u64; a plain send requests no id,
/// so any non-zero amount is refused whatever the id reads as). Amounts are
/// kept as they are.
fn carrier_sheet(sheet: &[(ProtoruneRuneId, u128)]) -> Vec<(AlkaneId, u128)> {
    let sat = |v: u128| u64::try_from(v).unwrap_or(u64::MAX);
    sheet
        .iter()
        .map(|(id, amount)| (AlkaneId { block: sat(id.block), tx: sat(id.tx) }, *amount))
        .collect()
}

/// True when `tx` carries a runestone with a protocol field (protostones).
/// Such a transaction's alkane routing is the alkanes builder's business;
/// anything else (no OP_RETURN, a plain OP_RETURN, a cenotaph, a runestone
/// without protostones) is treated as a plain BTC send by the pre-sign check.
pub fn tx_has_protostones(tx: &Transaction) -> bool {
    use ordinals::{Artifact, Runestone};
    matches!(
        Runestone::decipher(tx),
        Some(Artifact::Runestone(ref r)) if r.protocol.is_some()
    )
}

/// Pre-sign check for a plain BTC send: independently re-resolve every input's
/// alkane sheet (it does not trust the builder's selection) and refuse if any
/// input carries alkanes or has an unknown status.
pub async fn assert_plain_send_inputs_clean<P: MetashrewRpcProvider + ?Sized>(
    provider: &P,
    qubitcoin_mode: bool,
    tx: &Transaction,
) -> Result<()> {
    let inputs: Vec<OutPoint> = tx.input.iter().map(|i| i.previous_output).collect();
    let mut per_utxo: BTreeMap<OutPoint, Vec<(AlkaneId, u128)>> = BTreeMap::new();
    let mut clean: BTreeSet<OutPoint> = BTreeSet::new();
    for op in &inputs {
        match resolve_alkane_sheet(provider, qubitcoin_mode, op, None).await.0 {
            AlkaneSheetStatus::Clean => {
                clean.insert(*op);
            }
            AlkaneSheetStatus::Carrier(sheet) => {
                per_utxo.insert(*op, carrier_sheet(&sheet));
            }
            AlkaneSheetStatus::Unknown(e) => {
                log::warn!("Pre-sign probe failed for {}:{}: {}", op.txid, op.vout, e);
            }
        }
    }
    check_input_alkane_sheets(
        SpendKind::PlainBtcSend,
        &inputs,
        &per_utxo,
        &clean,
        &BTreeMap::new(),
        &[],
    )
}

/// Inputs chosen for a plain BTC send by [`select_plain_send_inputs`].
#[derive(Debug, Clone)]
pub struct PlainSendSelection {
    pub inputs: Vec<(OutPoint, UtxoInfo)>,
    pub total_input: u64,
    /// Fee for the chosen shape (inputs, outputs incl. change if any).
    pub fee: u64,
    /// Value paid to the recipient (`amount`, or everything minus fee for
    /// send-all).
    pub send_value: u64,
    /// Change value; `0` means no change output (dust folded into fee).
    pub change: u64,
}

/// Dust threshold below which change is folded into the fee.
pub const PLAIN_SEND_DUST: u64 = 546;

/// Coin selection over a [`CarrierFilter`]'s verified-clean UTXOs only.
///
/// `fee_for(n_inputs, n_outputs)` gives the fee in sats for a candidate shape.
/// Send-all spends every clean UTXO to one output. Otherwise UTXOs are taken in
/// order until `amount + fee(n, 2)` is covered. Shortfalls return
/// [`CarrierFilter::insufficient_error`], which names the excluded carriers and
/// unknowns.
pub fn select_plain_send_inputs(
    filter: &CarrierFilter,
    amount: u64,
    send_all: bool,
    fee_for: impl Fn(usize, usize) -> u64,
) -> Result<PlainSendSelection> {
    if send_all {
        // Every clean UTXO the wallet has not frozen.
        let inputs: Vec<(OutPoint, UtxoInfo)> =
            filter.clean.iter().filter(|(_, u)| !u.frozen).cloned().collect();
        let total = filter.clean_total();
        let fee = fee_for(inputs.len(), 1);
        if inputs.is_empty() || total <= fee {
            return Err(filter.insufficient_error(fee.saturating_add(1), total));
        }
        return Ok(PlainSendSelection {
            inputs,
            total_input: total,
            fee,
            send_value: total - fee,
            change: 0,
        });
    }
    let mut inputs = Vec::new();
    let mut total = 0u64;
    for (op, info) in &filter.clean {
        if info.frozen {
            continue;
        }
        inputs.push((*op, info.clone()));
        total = total.saturating_add(info.amount);
        if total >= amount.saturating_add(fee_for(inputs.len(), 2)) {
            break;
        }
    }
    let fee_with_change = fee_for(inputs.len(), 2);
    let needed = amount.saturating_add(fee_with_change);
    if inputs.is_empty() || total < needed {
        // A one-output (no change) shape may still fit exactly.
        let fee_no_change = fee_for(inputs.len(), 1);
        if !inputs.is_empty() && total >= amount.saturating_add(fee_no_change) {
            return Ok(PlainSendSelection {
                fee: total - amount,
                inputs,
                total_input: total,
                send_value: amount,
                change: 0,
            });
        }
        return Err(filter.insufficient_error(needed, total));
    }
    let change = total - needed;
    if change > PLAIN_SEND_DUST {
        Ok(PlainSendSelection { inputs, total_input: total, fee: fee_with_change, send_value: amount, change })
    } else {
        Ok(PlainSendSelection { inputs, total_input: total, fee: total - amount, send_value: amount, change: 0 })
    }
}

/// Build the unsigned PSBT of a plain BTC send from the verified-clean UTXOs
/// of `from_addresses` (the browser builders, `WebProvider` and
/// `BrowserWalletProvider`, share this). Candidates go through
/// [`exclude_alkane_carriers`]; selection is [`select_plain_send_inputs`] with
/// `fee_for(n_inputs, n_outputs)`. Honours `params.send_all` (one output, no
/// change). Each input gets its `witness_utxo` from the UTXO's address. The
/// builder-side invariant re-checks that every input was verified clean.
pub async fn build_plain_send_psbt<P: crate::traits::DeezelProvider + ?Sized>(
    provider: &P,
    params: &crate::traits::SendParams,
    from_addresses: Vec<String>,
    change_address: &str,
    fee_for: impl Fn(usize, usize) -> u64,
) -> Result<bitcoin::psbt::Psbt> {
    use bitcoin::{Address, Amount, ScriptBuf, Sequence, TxIn, TxOut, Witness};
    use core::str::FromStr;

    let recipient = Address::from_str(&params.address)?.assume_checked();
    let utxos = provider.get_utxos(false, Some(from_addresses)).await?;
    if utxos.is_empty() {
        return Err(AlkanesError::Wallet("No UTXOs available".into()));
    }
    let filter =
        exclude_alkane_carriers(provider, provider.is_qubitcoin_mode(), utxos, None).await;
    let sel = select_plain_send_inputs(&filter, params.amount, params.send_all, fee_for)?;

    let mut outputs = alloc::vec![TxOut {
        value: Amount::from_sat(sel.send_value),
        script_pubkey: recipient.script_pubkey(),
    }];
    if sel.change > 0 {
        let change = Address::from_str(change_address)?.assume_checked();
        outputs.push(TxOut { value: Amount::from_sat(sel.change), script_pubkey: change.script_pubkey() });
    }
    let unsigned_tx = Transaction {
        version: bitcoin::transaction::Version(2),
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: sel
            .inputs
            .iter()
            .map(|(op, _)| TxIn {
                previous_output: *op,
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            })
            .collect(),
        output: outputs,
    };
    let inputs: Vec<OutPoint> = unsigned_tx.input.iter().map(|i| i.previous_output).collect();
    check_inputs_verified_clean(&inputs, &filter.clean_outpoints)?;

    let mut psbt = bitcoin::psbt::Psbt::from_unsigned_tx(unsigned_tx)?;
    for (i, (_, info)) in sel.inputs.iter().enumerate() {
        let script_pubkey = match &info.script_pubkey {
            Some(s) => s.clone(),
            None => Address::from_str(&info.address)?.assume_checked().script_pubkey(),
        };
        psbt.inputs[i].witness_utxo = Some(TxOut { value: Amount::from_sat(info.amount), script_pubkey });
    }
    Ok(psbt)
}

/// [`assert_plain_send_inputs_clean`] over a base64 PSBT (the browser
/// providers hand PSBTs to their signers).
pub async fn assert_plain_send_psbt_clean<P: MetashrewRpcProvider + ?Sized>(
    provider: &P,
    qubitcoin_mode: bool,
    psbt_base64: &str,
) -> Result<()> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = STANDARD
        .decode(psbt_base64)
        .map_err(|e| AlkanesError::Parse(format!("Invalid PSBT base64: {e}")))?;
    let psbt = bitcoin::psbt::Psbt::deserialize(&bytes)?;
    assert_plain_send_inputs_clean(provider, qubitcoin_mode, &psbt.unsigned_tx).await
}

/// Refuse a split transaction that carries protostones. The plain-send split
/// (`ConcreteProvider::prepare_send`) never routes alkanes
/// (`skip_diesel_mint`, no per-UTXO alkanes), so its runestone holds at most
/// rune edicts; with a protostone its outputs could receive alkanes and the
/// package rules below would no longer hold.
fn refuse_split_protostones(split: &Transaction) -> Result<()> {
    if tx_has_protostones(split) {
        return Err(AlkanesError::Wallet(format!(
            "Refusing to sign: the split transaction {} of this plain BTC send carries \
             protostones; a plain-send split must not route alkanes",
            split.compute_txid()
        )));
    }
    Ok(())
}

/// The inputs of `main` that do not spend `split`. A main input that spends
/// `split` is accepted only if it is one of `split_clean_outpoints` (the
/// split's clean funding outputs); one spending any other split output (the
/// inscription or rune output) is refused.
fn main_inputs_off_split(
    split: Option<&Transaction>,
    main: &Transaction,
    split_clean_outpoints: &BTreeSet<OutPoint>,
) -> Result<Vec<OutPoint>> {
    let split_txid = split.map(|t| t.compute_txid());
    let mut rest = Vec::new();
    for op in main.input.iter().map(|i| i.previous_output) {
        if Some(op.txid) == split_txid {
            if !split_clean_outpoints.contains(&op) {
                return Err(AlkanesError::Wallet(format!(
                    "Refusing to sign: input {}:{} spends an output of the split transaction \
                     that is not one of its clean outputs (it holds the protected inscription \
                     or runes)",
                    op.txid, op.vout
                )));
            }
            continue;
        }
        rest.push(op);
    }
    Ok(rest)
}

/// Builder-side invariant for a plain BTC send that may come with a split
/// transaction (inscribed or rune-bearing UTXOs lifted onto their own outputs
/// first; the main transaction may spend the split's clean outputs before
/// either is broadcast).
///
/// Every split input must be in `clean_outpoints` (verified by
/// [`exclude_alkane_carriers`]). Every main input must be in `clean_outpoints`
/// or in `split_clean_outpoints`: a split output is alkane-free by
/// construction once every split input is, as the split carries no
/// protostone (refused otherwise). Without a split this is the same check
/// [`build_plain_send_psbt`] runs on its own inputs.
pub fn check_plain_send_package(
    split: Option<&Transaction>,
    main: &Transaction,
    clean_outpoints: &BTreeSet<OutPoint>,
    split_clean_outpoints: &BTreeSet<OutPoint>,
) -> Result<()> {
    if let Some(split) = split {
        refuse_split_protostones(split)?;
        let inputs: Vec<OutPoint> = split.input.iter().map(|i| i.previous_output).collect();
        check_inputs_verified_clean(&inputs, clean_outpoints)?;
    }
    let rest = main_inputs_off_split(split, main, split_clean_outpoints)?;
    check_inputs_verified_clean(&rest, clean_outpoints)
}

/// Builder-side invariant: every input is one [`exclude_alkane_carriers`]
/// verified clean. An input outside `clean_outpoints` is a known carrier, an
/// unknown or unindexed UTXO, or a builder bug, so the refusal does not use
/// the pre-sign re-probe's "indexer probe failed" wording.
fn check_inputs_verified_clean(inputs: &[OutPoint], clean_outpoints: &BTreeSet<OutPoint>) -> Result<()> {
    match inputs.iter().find(|op| !clean_outpoints.contains(op)) {
        Some(op) => Err(AlkanesError::Wallet(format!(
            "Refusing to sign: input {}:{} of this plain BTC send was not verified to carry no alkanes",
            op.txid, op.vout
        ))),
        None => Ok(()),
    }
}

/// Pre-sign check for a plain BTC send with an optional split transaction:
/// [`assert_plain_send_inputs_clean`] over every split input and every main
/// input except those spending one of `split_clean_outpoints` (they do not
/// exist on-chain yet, so there is nothing to probe; they are alkane-free
/// once the split inputs pass). A main input spending any other split output,
/// or a split carrying protostones, is refused. Without a split this is
/// exactly `assert_plain_send_inputs_clean(main)`.
pub async fn assert_plain_send_package_clean<P: MetashrewRpcProvider + ?Sized>(
    provider: &P,
    qubitcoin_mode: bool,
    split: Option<&Transaction>,
    main: &Transaction,
    split_clean_outpoints: &BTreeSet<OutPoint>,
) -> Result<()> {
    if let Some(split) = split {
        refuse_split_protostones(split)?;
        assert_plain_send_inputs_clean(provider, qubitcoin_mode, split).await?;
    }
    let rest: BTreeSet<OutPoint> =
        main_inputs_off_split(split, main, split_clean_outpoints)?.into_iter().collect();
    let mut off_split = main.clone();
    off_split.input.retain(|i| rest.contains(&i.previous_output));
    assert_plain_send_inputs_clean(provider, qubitcoin_mode, &off_split).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_provider::MockProvider;
    use bitcoin::hashes::Hash;
    use bitcoin::{Amount, Network, ScriptBuf, TxIn, TxOut, Txid};

    fn txid_n(n: u8) -> Txid {
        Txid::from_byte_array([n; 32])
    }

    fn utxo(n: u8, sats: u64) -> (OutPoint, UtxoInfo) {
        let op = OutPoint::new(txid_n(n), 0);
        (
            op,
            UtxoInfo {
                txid: op.txid.to_string(),
                vout: 0,
                amount: sats,
                address: "bcrt1qtest".into(),
                script_pubkey: None,
                confirmations: 10,
                frozen: false,
                freeze_reason: None,
                block_height: Some(100),
                has_inscriptions: false,
                has_runes: false,
                has_alkanes: false,
                is_coinbase: false,
            },
        )
    }

    /// MockProvider in standard (non-qubitcoin) mode with `carrier` holding
    /// 2:0 x1000 and `failing` erroring on probe.
    fn mock(carriers: &[u8], failing: &[u8]) -> MockProvider {
        let mut p = MockProvider::new(Network::Regtest);
        p.qubitcoin_mode = false;
        for n in carriers {
            p.alkane_balances
                .lock()
                .unwrap()
                .insert(format!("{}:0", txid_n(*n)), vec![(2, 0, 1000)]);
        }
        for n in failing {
            p.failing_outpoints.lock().unwrap().insert(format!("{}:0", txid_n(*n)));
        }
        p
    }

    fn fee(n_in: usize, n_out: usize) -> u64 {
        (11 + 58 * n_in + 43 * n_out) as u64
    }

    /// The builder-side invariant's refusal.
    const NOT_VERIFIED: &str = "of this plain BTC send was not verified to carry no alkanes";

    /// The `insufficient_error` clause for unindexed UTXOs, without the count.
    const UNINDEXED: &str = "UTXO(s) were excluded because the alkanes indexer has not indexed them yet \
                             (unconfirmed, or above the indexed height); retry after the next block is indexed";

    /// `utxo(n, sats)` with the given confirmations and block height.
    fn utxo_at(n: u8, sats: u64, confirmations: u32, block_height: Option<u64>) -> (OutPoint, UtxoInfo) {
        let (op, mut info) = utxo(n, sats);
        info.confirmations = confirmations;
        info.block_height = block_height;
        (op, info)
    }

    fn plain_tx(inputs: &[u8]) -> Transaction {
        Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: inputs
                .iter()
                .map(|n| TxIn { previous_output: OutPoint::new(txid_n(*n), 0), ..Default::default() })
                .collect(),
            output: vec![TxOut { value: Amount::from_sat(1000), script_pubkey: ScriptBuf::new() }],
        }
    }

    #[tokio::test]
    async fn carrier_plus_clean_sends_from_clean_only() {
        // UTXO 1 is a 50k-sat carrier (alkanes ride non-dust outputs too),
        // 2 and 3 are clean.
        let p = mock(&[1], &[]);
        let f = exclude_alkane_carriers(
            &p, false, vec![utxo(1, 50_000), utxo(2, 20_000), utxo(3, 20_000)], None,
        ).await;
        assert_eq!(f.carriers, vec![OutPoint::new(txid_n(1), 0)]);
        assert!(f.unknown.is_empty());
        let sel = select_plain_send_inputs(&f, 30_000, false, fee).unwrap();
        let ops: Vec<_> = sel.inputs.iter().map(|(o, _)| *o).collect();
        assert_eq!(ops, vec![OutPoint::new(txid_n(2), 0), OutPoint::new(txid_n(3), 0)]);
        // send-all also stays on clean UTXOs.
        let all = select_plain_send_inputs(&f, 0, true, fee).unwrap();
        assert_eq!(all.inputs.len(), 2);
        assert_eq!(all.total_input, 40_000);
        assert_eq!(all.send_value, 40_000 - fee(2, 1));
    }

    #[tokio::test]
    async fn only_carriers_fails_with_clear_error() {
        let p = mock(&[1, 2], &[]);
        let f = exclude_alkane_carriers(&p, false, vec![utxo(1, 50_000), utxo(2, 546)], None).await;
        assert!(f.clean.is_empty());
        for send_all in [false, true] {
            let err = select_plain_send_inputs(&f, 10_000, send_all, fee).unwrap_err().to_string();
            assert!(err.contains("Insufficient spendable BTC"), "{err}");
            assert!(err.contains("2 UTXO(s) carrying alkanes were excluded"), "{err}");
        }
    }

    #[tokio::test]
    async fn unknown_status_fails_closed() {
        // UTXO 1's probe fails: it must not fund the send even though it
        // alone would cover it.
        let p = mock(&[], &[1]);
        let f = exclude_alkane_carriers(&p, false, vec![utxo(1, 100_000), utxo(2, 1_000)], None).await;
        assert_eq!(f.unknown, vec![OutPoint::new(txid_n(1), 0)]);
        let err = select_plain_send_inputs(&f, 10_000, false, fee).unwrap_err().to_string();
        assert!(err.contains("could not be verified"), "{err}");
        let err = select_plain_send_inputs(&f, 0, true, fee);
        // send-all over the remaining clean 1000 sats is still possible.
        assert_eq!(err.unwrap().inputs.len(), 1);
    }

    #[tokio::test]
    async fn asserted_sheets_short_circuit_probe() {
        // Asserted carrier wins over a probe that would say clean; asserted
        // clean wins over a probe that would fail.
        let p = mock(&[], &[2]);
        let mut asserted = AssertedSheets::new();
        asserted.insert(OutPoint::new(txid_n(1), 0), vec![(ProtoruneRuneId { block: 2, tx: 0 }, 5)]);
        asserted.insert(OutPoint::new(txid_n(2), 0), vec![]);
        let f = exclude_alkane_carriers(&p, false, vec![utxo(1, 9_000), utxo(2, 9_000)], Some(&asserted)).await;
        assert_eq!(f.carriers, vec![OutPoint::new(txid_n(1), 0)]);
        assert_eq!(f.clean.len(), 1);
    }

    #[tokio::test]
    async fn presign_refuses_carrier_and_unknown_inputs() {
        let p = mock(&[1], &[3]);
        assert_plain_send_inputs_clean(&p, false, &plain_tx(&[2])).await.expect("clean input signs");
        let err = assert_plain_send_inputs_clean(&p, false, &plain_tx(&[2, 1])).await.unwrap_err().to_string();
        assert!(err.contains("Refusing to sign") && err.contains("2:0 x1000"), "{err}");
        let err = assert_plain_send_inputs_clean(&p, false, &plain_tx(&[3])).await.unwrap_err().to_string();
        assert!(err.contains("Refusing to sign") && err.contains("unknown alkane balance"), "{err}");
    }

    #[test]
    fn plain_tx_has_no_protostones() {
        assert!(!tx_has_protostones(&plain_tx(&[1])));
    }

    #[test]
    fn selection_folds_dust_change_into_fee() {
        let f = CarrierFilter {
            clean: vec![utxo(1, 10_000 + fee(1, 2) + 100)],
            ..Default::default()
        };
        let sel = select_plain_send_inputs(&f, 10_000, false, fee).unwrap();
        assert_eq!(sel.change, 0);
        assert_eq!(sel.send_value, 10_000);
        assert_eq!(sel.fee, sel.total_input - 10_000);
    }

    // ── shared browser builder (WebProvider / BrowserWalletProvider) ──────

    fn send_params(to: &str, amount: u64, send_all: bool) -> crate::traits::SendParams {
        crate::traits::SendParams {
            address: to.into(),
            amount,
            fee_rate: Some(1.0),
            send_all,
            from: None,
            change_address: None,
            auto_confirm: true,
            use_rebar: false,
            rebar_tier: 1,
            lock_alkanes: false,
            ordinals_strategy: Default::default(),
            mempool_indexer: false,
        }
    }

    /// Mock wallet whose own address holds UTXOs `1..=n` with `sats`.
    async fn funded_mock(sats: &[u64], carriers: &[u8], failing: &[u8]) -> (MockProvider, String) {
        use crate::traits::WalletProvider;
        let p = mock(carriers, failing);
        let addr = WalletProvider::get_address(&p).await.unwrap();
        let spk = bitcoin::Address::from_str(&addr).unwrap().assume_checked().script_pubkey();
        for (i, v) in sats.iter().enumerate() {
            p.utxos.lock().unwrap().push((
                OutPoint::new(txid_n(i as u8 + 1), 0),
                TxOut { value: Amount::from_sat(*v), script_pubkey: spk.clone() },
            ));
        }
        (p, addr)
    }

    fn web_fee(n_in: usize, n_out: usize) -> u64 {
        (10 + 68 * n_in + 31 * n_out) as u64
    }

    use core::str::FromStr;

    #[tokio::test]
    async fn web_builder_spends_clean_utxos_only() {
        let (p, addr) = funded_mock(&[50_000, 20_000, 20_000], &[1], &[]).await;
        let psbt = build_plain_send_psbt(&p, &send_params(&addr, 30_000, false), vec![addr.clone()], &addr, web_fee)
            .await
            .unwrap();
        let ins: Vec<_> = psbt.unsigned_tx.input.iter().map(|i| i.previous_output).collect();
        assert_eq!(ins, vec![OutPoint::new(txid_n(2), 0), OutPoint::new(txid_n(3), 0)]);
        assert_eq!(psbt.unsigned_tx.output[0].value.to_sat(), 30_000);
        assert!(psbt.inputs.iter().all(|i| i.witness_utxo.is_some()));
        // send-all: every clean UTXO, one output, carrier untouched.
        let psbt = build_plain_send_psbt(&p, &send_params(&addr, 0, true), vec![addr.clone()], &addr, web_fee)
            .await
            .unwrap();
        assert_eq!(psbt.unsigned_tx.input.len(), 2);
        assert_eq!(psbt.unsigned_tx.output.len(), 1);
        assert_eq!(psbt.unsigned_tx.output[0].value.to_sat(), 40_000 - web_fee(2, 1));
        // the pre-sign check passes on what the builder produced.
        let b64 = {
            use base64::{engine::general_purpose::STANDARD, Engine as _};
            STANDARD.encode(psbt.serialize())
        };
        assert_plain_send_psbt_clean(&p, false, &b64).await.unwrap();
    }

    #[tokio::test]
    async fn web_builder_only_carriers_errors() {
        let (p, addr) = funded_mock(&[50_000, 546], &[1, 2], &[]).await;
        let err = build_plain_send_psbt(&p, &send_params(&addr, 10_000, false), vec![addr.clone()], &addr, web_fee)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("2 UTXO(s) carrying alkanes were excluded"), "{err}");
    }

    #[tokio::test]
    async fn web_builder_unknown_status_fails_closed() {
        let (p, addr) = funded_mock(&[100_000], &[], &[1]).await;
        let err = build_plain_send_psbt(&p, &send_params(&addr, 10_000, false), vec![addr.clone()], &addr, web_fee)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("could not be verified"), "{err}");
    }

    #[tokio::test]
    async fn presign_psbt_refuses_carrier_input() {
        let (p, addr) = funded_mock(&[50_000, 20_000], &[], &[]).await;
        let psbt = build_plain_send_psbt(&p, &send_params(&addr, 0, true), vec![addr.clone()], &addr, web_fee)
            .await
            .unwrap();
        // The UTXO becomes a carrier between build and sign (e.g. a stale
        // view at build time): the independent re-probe catches it.
        p.alkane_balances.lock().unwrap().insert(format!("{}:0", txid_n(1)), vec![(2, 0, 7)]);
        let b64 = {
            use base64::{engine::general_purpose::STANDARD, Engine as _};
            STANDARD.encode(psbt.serialize())
        };
        let err = assert_plain_send_psbt_clean(&p, false, &b64).await.unwrap_err().to_string();
        assert!(err.contains("Refusing to sign") && err.contains("2:0 x7"), "{err}");
    }

    #[test]
    fn selection_skips_frozen_clean_utxos() {
        // Frozen UTXOs are reported (web-sys flags them) and left to the
        // selectors: send-all and `clean_total` must skip them too.
        let mut frozen = utxo(1, 50_000);
        frozen.1.frozen = true;
        let f = CarrierFilter { clean: vec![frozen, utxo(2, 20_000)], ..Default::default() };
        assert_eq!(f.clean_total(), 20_000);
        let all = select_plain_send_inputs(&f, 0, true, fee).unwrap();
        assert_eq!(all.inputs.iter().map(|(o, _)| *o).collect::<Vec<_>>(), vec![OutPoint::new(txid_n(2), 0)]);
        assert_eq!(all.send_value, 20_000 - fee(1, 1));
        let err = select_plain_send_inputs(&f, 30_000, false, fee).unwrap_err().to_string();
        assert!(err.contains("have 20000 sats"), "{err}");
    }

    // ── split + main package (`ConcreteProvider::prepare_send` / `send`) ──

    fn op(n: u8) -> OutPoint {
        OutPoint::new(txid_n(n), 0)
    }

    fn ops(items: &[OutPoint]) -> BTreeSet<OutPoint> {
        items.iter().copied().collect()
    }

    /// A split-shaped tx spending UTXOs `inputs`: v0 is the clean funding
    /// output, v1 the lifted inscription / rune output, then `runestone`.
    fn split_tx(inputs: &[u8], runestone: Option<ordinals::Runestone>) -> Transaction {
        let mut tx = plain_tx(inputs);
        tx.output = vec![
            TxOut { value: Amount::from_sat(20_000), script_pubkey: ScriptBuf::new() },
            TxOut { value: Amount::from_sat(546), script_pubkey: ScriptBuf::new() },
        ];
        if let Some(rs) = runestone {
            tx.output.push(TxOut { value: Amount::ZERO, script_pubkey: rs.encipher() });
        }
        tx
    }

    fn main_tx(inputs: &[OutPoint]) -> Transaction {
        let mut tx = plain_tx(&[]);
        tx.input = inputs.iter().map(|o| TxIn { previous_output: *o, ..Default::default() }).collect();
        tx
    }

    fn rune_edicts() -> Vec<ordinals::Edict> {
        vec![ordinals::Edict { id: ordinals::RuneId { block: 840_000, tx: 1 }, amount: 500, output: 1 }]
    }

    #[tokio::test]
    async fn package_refuses_carrier_on_split_input() {
        let p = mock(&[1], &[]);
        let split = split_tx(&[1, 2], None);
        let s0 = OutPoint::new(split.compute_txid(), 0);
        let main = main_tx(&[s0, op(3)]);
        let err = assert_plain_send_package_clean(&p, false, Some(&split), &main, &ops(&[s0]))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("Refusing to sign") && err.contains("2:0 x1000"), "{err}");
        // Builder side: 1 never passed the filter, so it is not in the clean set.
        let err = check_plain_send_package(Some(&split), &main, &ops(&[op(2), op(3)]), &ops(&[s0]))
            .unwrap_err()
            .to_string();
        assert!(err.contains(&format!("{}:0", txid_n(1))) && err.contains(NOT_VERIFIED), "{err}");
    }

    #[tokio::test]
    async fn package_refuses_unknown_on_split_input() {
        let p = mock(&[], &[1]);
        let split = split_tx(&[1, 2], None);
        let s0 = OutPoint::new(split.compute_txid(), 0);
        let err = assert_plain_send_package_clean(&p, false, Some(&split), &main_tx(&[s0]), &ops(&[s0]))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains(&format!("{}:0", txid_n(1))) && err.contains("unknown alkane balance"), "{err}");
    }

    #[tokio::test]
    async fn package_accepts_split_clean_outputs_without_probe() {
        let p = mock(&[], &[]);
        let split = split_tx(&[2], None);
        let s0 = OutPoint::new(split.compute_txid(), 0);
        // A probe of the split's clean output would fail (it is not on-chain
        // yet), so it is only accepted if the helper does not probe it.
        p.failing_outpoints.lock().unwrap().insert(format!("{}:0", s0.txid));
        let main = main_tx(&[s0, op(3)]);
        assert_plain_send_package_clean(&p, false, Some(&split), &main, &ops(&[s0])).await.unwrap();
        check_plain_send_package(Some(&split), &main, &ops(&[op(2), op(3)]), &ops(&[s0])).unwrap();
        // Without the split context the same main tx is probed and refused.
        let err = assert_plain_send_inputs_clean(&p, false, &main).await.unwrap_err().to_string();
        assert!(err.contains("unknown alkane balance"), "{err}");
    }

    #[tokio::test]
    async fn package_refuses_main_input_on_split_non_clean_output() {
        // v1 holds the protected inscription / runes: not a funding output.
        let p = mock(&[], &[]);
        let split = split_tx(&[2], None);
        let s0 = OutPoint::new(split.compute_txid(), 0);
        let s1 = OutPoint::new(split.compute_txid(), 1);
        let main = main_tx(&[s0, s1]);
        let err = assert_plain_send_package_clean(&p, false, Some(&split), &main, &ops(&[s0]))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains(&format!("{}:1", s1.txid)) && err.contains("not one of its clean outputs"), "{err}");
        let err = check_plain_send_package(Some(&split), &main, &ops(&[op(2)]), &ops(&[s0]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("not one of its clean outputs"), "{err}");
    }

    #[tokio::test]
    async fn package_reprobes_main_inputs_outside_split() {
        let split = split_tx(&[2], None);
        let s0 = OutPoint::new(split.compute_txid(), 0);
        let main = main_tx(&[s0, op(3)]);
        // 3 turned carrier after the build: the re-probe catches it.
        let p = mock(&[3], &[]);
        let err = assert_plain_send_package_clean(&p, false, Some(&split), &main, &ops(&[s0]))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("Refusing to sign") && err.contains("2:0 x1000"), "{err}");
        let p = mock(&[], &[3]);
        let err = assert_plain_send_package_clean(&p, false, Some(&split), &main, &ops(&[s0]))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains(&format!("{}:0", txid_n(3))) && err.contains("unknown alkane balance"), "{err}");
        // No split: exactly the plain pre-sign check.
        let p = mock(&[1], &[]);
        assert_plain_send_package_clean(&p, false, None, &plain_tx(&[2]), &BTreeSet::new()).await.unwrap();
        let err = assert_plain_send_package_clean(&p, false, None, &plain_tx(&[2, 1]), &BTreeSet::new())
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("2:0 x1000"), "{err}");
    }

    #[test]
    fn package_builder_invariant_rejects_main_input_outside_clean_set() {
        let split = split_tx(&[2], None);
        let s0 = OutPoint::new(split.compute_txid(), 0);
        let clean = ops(&[op(2), op(3)]);
        check_plain_send_package(Some(&split), &main_tx(&[s0, op(3)]), &clean, &ops(&[s0])).unwrap();
        let err = check_plain_send_package(Some(&split), &main_tx(&[s0, op(9)]), &clean, &ops(&[s0]))
            .unwrap_err()
            .to_string();
        assert!(err.contains(&format!("{}:0", txid_n(9))) && err.contains(NOT_VERIFIED), "{err}");
        // Without a split, a "split clean output" is just an unverified input.
        check_plain_send_package(None, &main_tx(&[op(2)]), &clean, &BTreeSet::new()).unwrap();
        assert!(check_plain_send_package(None, &main_tx(&[s0]), &clean, &ops(&[s0])).is_err());
    }

    #[tokio::test]
    async fn split_runestone_with_rune_edicts_has_no_protostones() {
        // The split's shape when it routes runes: one runestone, rune edicts,
        // `protocol: None` (`ordinals::build_split_psbt` via
        // `assemble_runestone`). It must read as a plain send, so the generic
        // signers re-probe its inputs, and the package helpers accept it.
        let rs = crate::ordinals::assemble_runestone(None, Some((1, rune_edicts()))).unwrap();
        assert!(rs.protocol.is_none());
        let split = split_tx(&[2], Some(rs));
        assert!(
            matches!(ordinals::Runestone::decipher(&split), Some(ordinals::Artifact::Runestone(ref r)) if !r.edicts.is_empty()),
            "a runestone with edicts, not a cenotaph"
        );
        assert!(!tx_has_protostones(&split));
        let s0 = OutPoint::new(split.compute_txid(), 0);
        let main = main_tx(&[s0]);
        let p = mock(&[], &[]);
        assert_plain_send_package_clean(&p, false, Some(&split), &main, &ops(&[s0])).await.unwrap();
        check_plain_send_package(Some(&split), &main, &ops(&[op(2)]), &ops(&[s0])).unwrap();
    }

    #[tokio::test]
    async fn package_refuses_split_with_protostones() {
        // Same split, but its runestone also carries a protostone
        // (`protocol: Some`, a real encoded BTC-send protostone).
        let rs = crate::ordinals::assemble_runestone(Some((0, vec![1498966328321u128])), Some((1, rune_edicts())))
            .unwrap();
        let split = split_tx(&[2], Some(rs));
        assert!(tx_has_protostones(&split));
        let s0 = OutPoint::new(split.compute_txid(), 0);
        let main = main_tx(&[s0]);
        let p = mock(&[], &[]);
        let err = assert_plain_send_package_clean(&p, false, Some(&split), &main, &ops(&[s0]))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("carries protostones"), "{err}");
        let err = check_plain_send_package(Some(&split), &main, &ops(&[op(2)]), &ops(&[s0]))
            .unwrap_err()
            .to_string();
        assert!(err.contains("carries protostones"), "{err}");
    }

    // ── a probe-clean answer counts only once the indexer has the tx ────

    #[tokio::test]
    async fn filter_excludes_unindexed_probe_clean_coins() {
        // Metashrew has indexed up to block 100. 1 is unconfirmed, 2 is mined
        // at 101, 3 at exactly 100, 4 confirmed with no height reported. 5
        // (a carrier) and 6 (failing probe) are unconfirmed too: their own
        // bucket wins.
        let p = mock(&[5], &[6]);
        *p.metashrew_height.lock().unwrap() = Some(100);
        let f = exclude_alkane_carriers(
            &p,
            false,
            vec![
                utxo_at(1, 10_000, 0, None),
                utxo_at(2, 10_000, 3, Some(101)),
                utxo_at(3, 10_000, 3, Some(100)),
                utxo_at(4, 10_000, 3, None),
                utxo_at(5, 10_000, 0, None),
                utxo_at(6, 10_000, 0, None),
            ],
            None,
        )
        .await;
        let clean: Vec<OutPoint> = f.clean.iter().map(|(op, _)| *op).collect();
        assert_eq!(clean, vec![OutPoint::new(txid_n(3), 0), OutPoint::new(txid_n(4), 0)]);
        assert_eq!(f.unindexed, vec![OutPoint::new(txid_n(1), 0), OutPoint::new(txid_n(2), 0)]);
        assert_eq!(f.carriers, vec![OutPoint::new(txid_n(5), 0)]);
        assert_eq!(f.unknown, vec![OutPoint::new(txid_n(6), 0)]);
        assert!(!f.clean_outpoints.contains(&OutPoint::new(txid_n(1), 0)));
        let err = select_plain_send_inputs(&f, 25_000, false, fee).unwrap_err().to_string();
        assert!(err.contains(&format!("; 2 {UNINDEXED}")), "{err}");
    }

    #[tokio::test]
    async fn known_block_height_decides_whatever_confirmations_says() {
        // A failed bitcoin tip read reports 0 confirmations for every coin,
        // so a coin with a block height is judged by that height alone: 1 at
        // the indexed height 100 is clean, 2 at 101 is not. Only 3, with no
        // height, counts as unconfirmed.
        let p = mock(&[], &[]);
        *p.metashrew_height.lock().unwrap() = Some(100);
        let f = exclude_alkane_carriers(
            &p,
            false,
            vec![utxo_at(1, 30_000, 0, Some(100)), utxo_at(2, 10_000, 0, Some(101)), utxo_at(3, 10_000, 0, None)],
            None,
        )
        .await;
        assert_eq!(f.clean.iter().map(|(op, _)| *op).collect::<Vec<_>>(), vec![OutPoint::new(txid_n(1), 0)]);
        assert_eq!(f.unindexed, vec![OutPoint::new(txid_n(2), 0), OutPoint::new(txid_n(3), 0)]);
        let sel = select_plain_send_inputs(&f, 20_000, false, fee).unwrap();
        assert_eq!(sel.inputs.iter().map(|(op, _)| *op).collect::<Vec<_>>(), vec![OutPoint::new(txid_n(1), 0)]);
        let mut height = None;
        let why = |n: u8, c: u32, h: Option<u64>| utxo_at(n, 10_000, c, h).1;
        assert_eq!(probe_not_indexed(&p, &why(1, 0, Some(100)), &mut height).await, None);
        assert_eq!(
            probe_not_indexed(&p, &why(2, 0, Some(101)), &mut height).await.as_deref(),
            Some("block 101 is above the indexed height 100")
        );
        assert_eq!(probe_not_indexed(&p, &why(3, 0, None), &mut height).await.as_deref(), Some("unconfirmed"));
    }

    #[tokio::test]
    async fn filter_fails_closed_when_indexed_height_unknown() {
        let p = mock(&[], &[]);
        *p.metashrew_height.lock().unwrap() = None;
        let f = exclude_alkane_carriers(
            &p,
            false,
            vec![utxo(1, 10_000), utxo_at(2, 10_000, 3, None), utxo_at(3, 10_000, 0, None)],
            None,
        )
        .await;
        // 1 has a height that cannot be compared; 2 has none, so the read
        // does not decide it; 3 is unconfirmed either way.
        let clean: Vec<OutPoint> = f.clean.iter().map(|(op, _)| *op).collect();
        assert_eq!(clean, vec![OutPoint::new(txid_n(2), 0)]);
        assert_eq!(f.unindexed, vec![OutPoint::new(txid_n(1), 0), OutPoint::new(txid_n(3), 0)]);
        let err = f.insufficient_error(15_000, f.clean_total()).to_string();
        assert!(err.contains(&format!("; 2 {UNINDEXED}")), "{err}");
    }

    #[tokio::test]
    async fn asserted_and_qubitcoin_answers_ignore_indexing() {
        // An assertion does not come from metashrew's view of the tx, so an
        // asserted-clean unconfirmed coin stays spendable, and an asserted
        // carrier stays a carrier.
        let p = mock(&[], &[]);
        *p.metashrew_height.lock().unwrap() = None;
        let mut asserted = AssertedSheets::new();
        asserted.insert(OutPoint::new(txid_n(1), 0), vec![]);
        asserted.insert(OutPoint::new(txid_n(2), 0), vec![(ProtoruneRuneId { block: 2, tx: 0 }, 5)]);
        let f = exclude_alkane_carriers(
            &p,
            false,
            vec![utxo_at(1, 30_000, 0, None), utxo_at(2, 30_000, 0, None), utxo_at(3, 30_000, 3, Some(900_000))],
            Some(&asserted),
        )
        .await;
        assert_eq!(f.clean.iter().map(|(op, _)| *op).collect::<Vec<_>>(), vec![OutPoint::new(txid_n(1), 0)]);
        assert_eq!(f.carriers, vec![OutPoint::new(txid_n(2), 0)]);
        assert_eq!(f.unindexed, vec![OutPoint::new(txid_n(3), 0)]);
        let sel = select_plain_send_inputs(&f, 20_000, false, fee).unwrap();
        assert_eq!(sel.inputs[0].0, OutPoint::new(txid_n(1), 0));
        // Qubitcoin mode: no per-outpoint index, nothing to wait for.
        let f = exclude_alkane_carriers(
            &p,
            true,
            vec![utxo_at(1, 30_000, 0, None), utxo_at(3, 30_000, 3, Some(900_000))],
            None,
        )
        .await;
        assert_eq!(f.clean.len(), 2);
        assert!(f.unindexed.is_empty());
    }

    #[test]
    fn insufficient_error_names_every_excluded_bucket() {
        let f = CarrierFilter {
            carriers: vec![OutPoint::new(txid_n(1), 0)],
            unknown: vec![OutPoint::new(txid_n(2), 0), OutPoint::new(txid_n(3), 0)],
            unindexed: vec![OutPoint::new(txid_n(4), 0)],
            ..Default::default()
        };
        assert_eq!(
            f.insufficient_error(1_000, 0).to_string(),
            "Wallet error: Insufficient spendable BTC: need 1000 sats, have 0 sats in UTXOs verified to carry no alkanes\
             ; 1 UTXO(s) carrying alkanes were excluded (a plain BTC send would transfer \
             their alkanes to the recipient; move them with an alkanes transfer first)\
             ; 2 UTXO(s) were excluded because their alkane balance could not be verified \
             (indexer probe failed); retry once the alkanes indexer is reachable\
             ; 1 UTXO(s) were excluded because the alkanes indexer has not indexed them yet \
             (unconfirmed, or above the indexed height); retry after the next block is indexed"
        );
        // Without unindexed UTXOs the message is the stage 1 text.
        let f = CarrierFilter { unindexed: vec![], ..f };
        assert!(!f.insufficient_error(1_000, 0).to_string().contains("not indexed"));
    }
}
