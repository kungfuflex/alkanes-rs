//! # alkanes-std-recycle — the `8:dead` recycle bin
//!
//! A precompiled "life WASM" living at **`AlkaneId(8, 0xdead)`** (the `8:*`
//! namespace is reserved for indexer-embedded precompiled alkanes; `1:*` is the
//! witness-payload deploy space). It is the **failsafe** for alkanes/protorunes
//! that get garbage-collected when a non-protostone (no-OP_RETURN) Bitcoin
//! transaction spends a UTXO that was carrying them — see FROST Batallion 6,
//! 2026-06-06, and `PROPOSAL-PROTORUNE-RECYCLE-BIN.md`.
//!
//! ## How the bin is filled (capture — done by the indexer, not this contract)
//!
//! When the indexer sees a transaction with **no protostone** spending inputs
//! that carry protocol-tag (alkane) balances, instead of leaving them stranded
//! at the now-spent input outpoint it sweeps them into this alkane:
//!   * credits **`8:dead`'s inventory** with the swept balances, and
//!   * appends them to a per-recipient ledger at `/recycle/<script_pubkey>`,
//!     keyed by the `script_pubkey` of `default_output(tx)` — i.e. the first
//!     non-OP_RETURN output, the address that *would* have received them.
//! Only **EOA** (key-path: p2tr / p2wpkh / p2pkh) recipients are recorded;
//! non-EOA outputs are left burned (anti-spam, per flex).
//!
//! ## How it is claimed (this contract, opcode 3)
//!
//! The rightful owner builds a transaction whose **first non-OP_RETURN output**
//! is their EOA address and whose protostone targets `8:dead` opcode `3`
//! (`Claim`) with its pointer set to that output. This contract reads
//! `/recycle/<that script_pubkey>`, emits the recorded balances **out of its own
//! inventory** to the response (routed to the protostone pointer), and zeroes
//! the ledger entry. Because the balances are emitted from inventory the indexer
//! actually credited — and clamped to the live inventory balance — the claim can
//! **never mint alkanes the bin was not given** (the core safety invariant).

use alkanes_runtime::runtime::AlkaneResponder;
use alkanes_runtime::{declare_alkane, message::MessageDispatch, storage::StoragePointer};
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::{
    context::Context, id::AlkaneId, parcel::AlkaneTransfer, response::CallResponse,
};
use anyhow::{anyhow, Result};
use bitcoin::{ScriptBuf, Transaction};
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

/// Storage key prefix for the per-recipient ledger. `/recycle/<spk>` holds the
/// serialized list of `(AlkaneId, value)` owed to that script_pubkey. The
/// indexer capture is the **only** writer; this contract only reads + clears.
const RECYCLE_LEDGER_PREFIX: &str = "/recycle/";

#[derive(Default)]
pub struct Recycle(());

#[derive(MessageDispatch)]
enum RecycleMessage {
    /// No-op initializer (8:dead is precompiled; kept for ABI symmetry).
    #[opcode(0)]
    Initialize,

    /// Claim the caller's stranded balances. The recipient is the first
    /// non-OP_RETURN output of the claiming transaction (EOA only); set your
    /// protostone pointer to that output. Releases `/recycle/<spk>` and clears.
    #[opcode(3)]
    Claim,

    /// View: balance the bin owes the first non-OP_RETURN output of the supplied
    /// (simulated) transaction. Returns the serialized `[(block,tx,value)...]`
    /// ledger in `response.data`. No state change.
    #[opcode(10)]
    #[returns(Vec<u8>)]
    GetRecycleBalance,

    #[opcode(99)]
    #[returns(String)]
    GetName,

    #[opcode(100)]
    #[returns(String)]
    GetSymbol,
}

impl Recycle {
    // ── storage ────────────────────────────────────────────────────────────
    fn ledger_pointer(&self, spk: &[u8]) -> StoragePointer {
        StoragePointer::from_keyword(RECYCLE_LEDGER_PREFIX).select(&spk.to_vec())
    }

    /// Decode the ledger blob: a flat sequence of (block:u128, tx:u128,
    /// value:u128) little-endian triples. Matches the capture's encoding.
    fn read_ledger(&self, spk: &[u8]) -> Vec<(AlkaneId, u128)> {
        let raw = self.ledger_pointer(spk).get();
        decode_ledger(raw.as_ref())
    }

    // ── helpers ──────────────────────────────────────────────────────────────
    /// The claim/view recipient = first non-OP_RETURN output of this tx, EOA only.
    fn recipient_script(&self) -> Result<ScriptBuf> {
        let tx: Transaction = bitcoin::consensus::deserialize(&self.transaction())
            .map_err(|e| anyhow!("could not parse transaction: {}", e))?;
        let vout = default_output(&tx)
            .ok_or_else(|| anyhow!("transaction has no non-OP_RETURN output"))?;
        let spk = tx.output[vout].script_pubkey.clone();
        if !is_eoa(&spk) {
            return Err(anyhow!("recycle recipient must be an EOA (p2tr/p2wpkh/p2pkh)"));
        }
        Ok(spk)
    }

    // ── handlers ─────────────────────────────────────────────────────────────
    fn initialize(&self) -> Result<CallResponse> {
        // Precompiled; nothing to set up. Just forward incoming.
        let context = self.context()?;
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    fn claim(&self) -> Result<CallResponse> {
        // SECURITY: the claim is handled NATIVELY in the indexer
        // (`alkanes::recycle::handle_claim`, dispatched from `handle_message`),
        // NOT here. A wasm claim cannot see the protostone `pointer` (the output
        // the response is routed to), so it cannot bind the payout to the ledger
        // key — that decoupling is exactly the theft vector ksyao found (a claim
        // would read `/recycle/<vout0_spk>` but pay out to an attacker-chosen
        // pointer). The native handler keys the ledger off the payout output's
        // spk. During real indexing this opcode never reaches the wasm (the
        // native intercept runs first); we hard-reject here so a simulate of
        // `8:dead:3` can never display the old, unsafe behavior.
        Err(anyhow!(
            "recycle claim (8:dead:3) is handled natively by the indexer, not the wasm"
        ))
    }

    fn get_recycle_balance(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let spk = self.recipient_script()?;
        let owed = self.read_ledger(spk.as_bytes());
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = encode_ledger(&owed);
        Ok(response)
    }

    fn get_name(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = b"RECYCLE".to_vec();
        Ok(response)
    }

    fn get_symbol(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = "\u{267B}".as_bytes().to_vec(); // ♻
        Ok(response)
    }
}

/// First non-OP_RETURN output index, or None if every output is OP_RETURN.
/// Mirrors `protorune::default_output` so capture and claim agree on the key.
fn default_output(tx: &Transaction) -> Option<usize> {
    tx.output
        .iter()
        .position(|o| !o.script_pubkey.is_op_return())
}

/// EOA = key-path spendable (p2tr / p2wpkh / p2pkh). Excludes scripts/contracts.
fn is_eoa(spk: &ScriptBuf) -> bool {
    spk.is_p2tr() || spk.is_p2wpkh() || spk.is_p2pkh()
}

/// Ledger codec: flat LE (block, tx, value) u128 triples. Shared with capture.
pub fn encode_ledger(entries: &[(AlkaneId, u128)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(entries.len() * 48);
    for (id, value) in entries {
        out.extend_from_slice(&id.block.to_le_bytes());
        out.extend_from_slice(&id.tx.to_le_bytes());
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

pub fn decode_ledger(raw: &[u8]) -> Vec<(AlkaneId, u128)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 48 <= raw.len() {
        let block = u128::from_le_bytes(raw[i..i + 16].try_into().unwrap());
        let tx = u128::from_le_bytes(raw[i + 16..i + 32].try_into().unwrap());
        let value = u128::from_le_bytes(raw[i + 32..i + 48].try_into().unwrap());
        out.push((AlkaneId { block, tx }, value));
        i += 48;
    }
    out
}

impl AlkaneResponder for Recycle {}

declare_alkane! {
    impl AlkaneResponder for Recycle {
        type Message = RecycleMessage;
    }
}
