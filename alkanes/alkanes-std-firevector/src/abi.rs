//! The 12:0 call surface, as pure functions.
//!
//! Everything the alkane does lives here so it is testable natively, without a
//! wasm toolchain or a runtime. `alkane.rs` is a thin shell that only moves
//! `CallResponse` data in and out.
//!
//! # Two different failure policies, deliberately
//!
//! An invalid **program** yields all-zero weights rather than an error. Programs
//! are governance data and therefore attacker-influenced; "nothing qualifies" is
//! a safe, well-defined outcome and the caller falls back to the identity vector.
//!
//! A malformed **item table** *is* an error, because the indexer builds it
//! itself — silently returning zeros there would hide an indexer bug behind
//! plausible-looking output.
//!
//! In neither case may the caller treat the result as a reason to abort a block.
//! That is the h=953281 failure mode: one malformed input aborting a
//! block-wide computation took an indexer offline for the duration.

use crate::codec::{decode_item, decode_items};
use crate::vm::{eval, validate, Item, Source};

/// Discriminant for [`Source`] on the wire.
pub const SOURCE_PREV_BLOCK: u128 = 0;
pub const SOURCE_SAME_BLOCK: u128 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbiError {
    UnknownSource(u128),
    MalformedItemTable,
    MalformedItem,
}

impl core::fmt::Display for AbiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AbiError::UnknownSource(v) => write!(
                f,
                "unknown source {} (expected {} = prev-block or {} = same-block)",
                v, SOURCE_PREV_BLOCK, SOURCE_SAME_BLOCK
            ),
            AbiError::MalformedItemTable => write!(f, "malformed item table"),
            AbiError::MalformedItem => write!(f, "malformed item"),
        }
    }
}

/// Map the wire discriminant to a [`Source`].
pub fn source_from_word(v: u128) -> Result<Source, AbiError> {
    match v {
        SOURCE_PREV_BLOCK => Ok(Source::PrevBlock),
        SOURCE_SAME_BLOCK => Ok(Source::SameBlock),
        other => Err(AbiError::UnknownSource(other)),
    }
}

/// Pack weights as little-endian u128s — the same convention as the packed
/// structs elsewhere in the system, so a caller reads them at fixed 16-byte
/// offsets with no length prefix to get wrong.
pub fn pack_weights(weights: &[u128]) -> Vec<u8> {
    let mut out = Vec::with_capacity(weights.len() * 16);
    for w in weights {
        out.extend_from_slice(&w.to_le_bytes());
    }
    out
}

/// Inverse of [`pack_weights`]. Returns `None` on a length that is not a
/// multiple of 16 rather than silently dropping a partial trailing weight.
pub fn unpack_weights(bytes: &[u8]) -> Option<Vec<u128>> {
    if bytes.len() % 16 != 0 {
        return None;
    }
    Some(
        bytes
            .chunks_exact(16)
            .map(|c| u128::from_le_bytes(c.try_into().expect("chunks_exact(16) yields 16 bytes")))
            .collect(),
    )
}

/// Opcode 1 — evaluate a program across a whole item table.
///
/// One call per block. An invalid program produces `items.len()` zeros; it is
/// not an error.
pub fn evaluate_call(
    source: u128,
    program: &[u128],
    item_table: &[u128],
) -> Result<Vec<u8>, AbiError> {
    let source = source_from_word(source)?;
    let items = decode_items(item_table).ok_or(AbiError::MalformedItemTable)?;

    let weights = match validate(program, source) {
        Err(_) => vec![0u128; items.len()],
        Ok(steps) => {
            let budget = steps.max(1);
            items.iter().map(|it| eval(program, it, budget)).collect()
        }
    };
    Ok(pack_weights(&weights))
}

/// Opcode 2 — weight for one item.
///
/// This is the user-facing view: a wallet packs the transaction it is about to
/// broadcast and shows the weight it would earn. Takes a bare item, not a table.
pub fn simulate_call(source: u128, program: &[u128], item: &[u128]) -> Result<Vec<u8>, AbiError> {
    let source = source_from_word(source)?;
    let (item, _) = decode_item(item).ok_or(AbiError::MalformedItem)?;

    let weight = match validate(program, source) {
        Err(_) => 0u128,
        Ok(steps) => eval(program, &item, steps.max(1)),
    };
    Ok(pack_weights(&[weight]))
}

/// Opcode 3 — disassembly. Total: never fails, so a malformed program can still
/// be inspected by whoever has to work out why it is malformed.
pub fn disassemble_call(program: &[u128]) -> String {
    crate::disasm::disassemble(program)
}

/// Opcode 4 — static verdict, as text a governance UI can show verbatim.
pub fn validate_call(source: u128, program: &[u128]) -> String {
    let source = match source_from_word(source) {
        Ok(s) => s,
        Err(e) => return format!("INVALID: {e}"),
    };
    match validate(program, source) {
        Ok(steps) => format!("valid ({source:?}, {steps} steps/item)"),
        Err(e) => format!("INVALID ({source:?}): {e}"),
    }
}

/// Verify that a candidate weightmap is safe to adopt as a governance write.
///
/// Distinct from [`validate_call`] in intent: this is the check that should gate
/// a `set_weightmap`, and it deliberately refuses a program that is well-formed
/// but useless — one that weights nothing at all — because adopting it would
/// silently halt emission to everyone.
pub fn is_adoptable(program: &[u128], source: u128, probes: &[Item]) -> bool {
    let Ok(src) = source_from_word(source) else {
        return false;
    };
    let Ok(steps) = validate(program, src) else {
        return false;
    };
    let budget = steps.max(1);
    probes.iter().any(|it| eval(program, it, budget) > 0)
}
