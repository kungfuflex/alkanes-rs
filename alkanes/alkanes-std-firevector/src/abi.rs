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
use crate::vm::{eval, validate, Item, Mode, Qualifier};

/// Discriminant for [`Mode`] on the wire.
pub const MODE_SPLIT: u128 = 0;
pub const MODE_RATE: u128 = 1;

/// Leading word of a packed weightmap: `"FIREVECT"` as ASCII.
///
/// Present so that a blob written by an older build — a bare program with no
/// header — is rejected rather than reinterpreted, its first opcode silently
/// becoming a mode. The indexer treats a missing magic as "no weightmap
/// configured", which falls back to today's emission.
pub const WEIGHTMAP_MAGIC: u128 = 0x4649_5245_5645_4354;

/// Current weightmap encoding version.
pub const WEIGHTMAP_VERSION: u128 = 1;

/// Fixed header words preceding the program.
const HEADER_WORDS: usize = 9;

/// A complete emission policy: how to weigh, what to weigh, and how weights
/// become payouts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Weightmap {
    pub mode: Mode,
    /// The prior action a mint must follow to earn anything. `None` means every
    /// DIESEL mint qualifies, which is what the identity vector needs.
    pub qualifier: Option<Qualifier>,
    /// [`Mode::Rate`] only: the virtual denominator. A claimant weighing `w`
    /// receives roughly `emission * w / rate_floor`, and cumulative weight in a
    /// block is clamped here so the block can never pay out more than once over.
    /// Ignored in [`Mode::Split`].
    pub rate_floor: u128,
    pub program: Vec<u128>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AbiError {
    UnknownMode(u128),
    MalformedItemTable,
    MalformedItem,
    MalformedWeightmap,
}

impl core::fmt::Display for AbiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AbiError::UnknownMode(v) => write!(
                f,
                "unknown mode {} (expected {} = split or {} = rate)",
                v, MODE_SPLIT, MODE_RATE
            ),
            AbiError::MalformedItemTable => write!(f, "malformed item table"),
            AbiError::MalformedItem => write!(f, "malformed item"),
            AbiError::MalformedWeightmap => write!(f, "malformed weightmap blob"),
        }
    }
}

/// Map the wire discriminant to a [`Mode`].
pub fn mode_from_word(v: u128) -> Result<Mode, AbiError> {
    match v {
        MODE_SPLIT => Ok(Mode::Split),
        MODE_RATE => Ok(Mode::Rate),
        other => Err(AbiError::UnknownMode(other)),
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

// ---------------------------------------------------------------------------
// Weightmap blob
// ---------------------------------------------------------------------------

/// Serialise a whole weightmap to storage bytes.
///
/// One blob rather than the mode and the program in separate keys. The mode
/// decides whether a program is *legal at all* — `OP_INCOMING_AMOUNT` is valid
/// under [`Mode::Rate`] and a validation error under [`Mode::Split`] — so two
/// keys admit a torn write in which the stored program does not validate against
/// the stored mode. A single blob makes that state unrepresentable.
///
/// Layout, in little-endian `u128` words:
///
/// ```text
///   0  MAGIC
///   1  version
///   2  mode
///   3  has_qualifier (0 or 1)
///   4  qualifier.target_block
///   5  qualifier.target_tx
///   6  qualifier.opcode
///   7  rate_floor
///   8  program length in words
///   9.. program
/// ```
pub fn pack_weightmap(w: &Weightmap) -> Vec<u8> {
    let q = w.qualifier.unwrap_or(Qualifier {
        target_block: 0,
        target_tx: 0,
        opcode: 0,
    });
    let words: Vec<u128> = vec![
        WEIGHTMAP_MAGIC,
        WEIGHTMAP_VERSION,
        w.mode as u128,
        w.qualifier.is_some() as u128,
        q.target_block,
        q.target_tx,
        q.opcode,
        w.rate_floor,
        w.program.len() as u128,
    ];
    let mut out = Vec::with_capacity((words.len() + w.program.len()) * 16);
    for x in words.iter().chain(w.program.iter()) {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// Inverse of [`pack_weightmap`]. Total: returns `None` on anything it cannot
/// read rather than partially decoding.
pub fn unpack_weightmap(bytes: &[u8]) -> Option<Weightmap> {
    if bytes.is_empty() || bytes.len() % 16 != 0 {
        return None;
    }
    let words: Vec<u128> = bytes
        .chunks_exact(16)
        .map(|c| u128::from_le_bytes(c.try_into().expect("chunks_exact(16)")))
        .collect();
    if words.len() < HEADER_WORDS {
        return None;
    }
    if words[0] != WEIGHTMAP_MAGIC || words[1] != WEIGHTMAP_VERSION {
        return None;
    }
    let mode = mode_from_word(words[2]).ok()?;
    let qualifier = match words[3] {
        0 => None,
        1 => Some(Qualifier {
            target_block: words[4],
            target_tx: words[5],
            opcode: words[6],
        }),
        _ => return None,
    };
    let rate_floor = words[7];
    let len = words[8];
    // Bound against what is actually present before indexing, so a huge declared
    // length cannot drive an allocation or an out-of-range slice.
    if len != (words.len() - HEADER_WORDS) as u128 {
        return None;
    }
    Some(Weightmap {
        mode,
        qualifier,
        rate_floor,
        program: words[HEADER_WORDS..].to_vec(),
    })
}

// ---------------------------------------------------------------------------
// Call surface
// ---------------------------------------------------------------------------

/// Opcode 1 — evaluate a program across a whole item table.
///
/// One call per block. An invalid program produces `items.len()` zeros; it is
/// not an error.
pub fn evaluate_call(mode: u128, program: &[u128], item_table: &[u128]) -> Result<Vec<u8>, AbiError> {
    let mode = mode_from_word(mode)?;
    let items = decode_items(item_table).ok_or(AbiError::MalformedItemTable)?;

    let weights = match validate(program, mode) {
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
pub fn simulate_call(mode: u128, program: &[u128], item: &[u128]) -> Result<Vec<u8>, AbiError> {
    let mode = mode_from_word(mode)?;
    let (item, _) = decode_item(item).ok_or(AbiError::MalformedItem)?;

    let weight = match validate(program, mode) {
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
pub fn validate_call(mode: u128, program: &[u128]) -> String {
    let mode = match mode_from_word(mode) {
        Ok(m) => m,
        Err(e) => return format!("INVALID: {e}"),
    };
    match validate(program, mode) {
        Ok(steps) => format!("valid ({mode:?}, {steps} steps/item)"),
        Err(e) => format!("INVALID ({mode:?}): {e}"),
    }
}

// ---------------------------------------------------------------------------
// Adoption gate
// ---------------------------------------------------------------------------

/// The gate on `set_weightmap`.
///
/// Rejects a weightmap that cannot run, and — deliberately — one that is
/// well-formed but weighs nothing at all. Adopting a vector that qualifies no
/// transaction would silently halt emission to everybody; that is a governance
/// accident worth blocking at the setter rather than discovering a block later.
///
/// "Weighs nothing" is judged against a small standard probe set rather than
/// against a real block, because the setter has no block to look at. It catches
/// the blunt mistakes — an always-zero program, a curve that can never pay — not
/// every subtle one.
pub fn check_adoptable(w: &Weightmap) -> Result<(), String> {
    let steps = validate(&w.program, w.mode).map_err(|e| e.to_string())?;
    let budget = steps.max(1);

    if w.mode == Mode::Rate && w.rate_floor == 0 {
        return Err(
            "rate mode needs a non-zero rate_floor; zero would divide every payout to nothing"
                .into(),
        );
    }

    let probes = adoption_probes();
    if probes.iter().all(|it| eval(&w.program, it, budget) == 0) {
        return Err(
            "program weighs nothing on any probe transaction; adopting it would halt emission"
                .into(),
        );
    }

    // A SPLIT program whose weights are not already 0/1 is not rejected — the
    // indexer clamps it, so the result is still exact and still safe — but the
    // author almost certainly meant something the clamp will not deliver.
    if w.mode == Mode::Split {
        if let Some(v) = probes
            .iter()
            .map(|it| eval(&w.program, it, budget))
            .find(|v| *v > 1)
        {
            return Err(format!(
                "split mode settles membership, not magnitude, and clamps every non-zero \
                 weight to 1 -- this program returns {v} on a probe. Use rate mode for a \
                 weighted payout, or make the program a 0/1 predicate."
            ));
        }
    }
    Ok(())
}

/// Representative items a candidate weightmap is tested against.
///
/// Deliberately small and boring: plain DIESEL mints at several ranks, one mint
/// qualified by a prior pool call declaring a size, and one carrying realized
/// incoming alkanes. Enough to catch "this can never pay anyone".
///
/// The last two matter: without a probe carrying `prior_inputs` every
/// qualifier-driven weightmap would be rejected as weighing nothing, and without
/// one carrying `incoming` the same would be true of every RATE weightmap.
fn adoption_probes() -> Vec<Item> {
    let mint = |rank: u128, txindex: u128| Item {
        target_block: 2,
        target_tx: 0,
        opcode: 77,
        txindex,
        mint_rank: rank,
        ..Default::default()
    };
    vec![
        mint(0, 1),
        mint(1, 2),
        mint(7, 9),
        Item {
            prior_inputs: vec![1_000_000, 7],
            ..mint(0, 3)
        },
        Item {
            incoming: vec![(32, 0, 1_000_000)],
            ..mint(0, 4)
        },
    ]
}

/// Whether a candidate weightmap would pay anything on the supplied probes.
///
/// The caller-supplied-probes form of [`check_adoptable`], for a governance UI
/// that wants to ask "what would this do to *these* transactions" rather than to
/// the built-in set.
///
/// Applies exactly the same rules as [`check_adoptable`], differing only in which
/// probes it judges against. Keeping them in step matters: a UI that showed green
/// here for a weightmap the setter then rejected would be worse than no check at
/// all.
pub fn is_adoptable(w: &Weightmap, probes: &[Item]) -> bool {
    let Ok(steps) = validate(&w.program, w.mode) else {
        return false;
    };
    if w.mode == Mode::Rate && w.rate_floor == 0 {
        return false;
    }
    let budget = steps.max(1);
    let values: Vec<u128> = probes.iter().map(|it| eval(&w.program, it, budget)).collect();
    if w.mode == Mode::Split && values.iter().any(|v| *v > 1) {
        return false;
    }
    values.iter().any(|v| *v > 0)
}
