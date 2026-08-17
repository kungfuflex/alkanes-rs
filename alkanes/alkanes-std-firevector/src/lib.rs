//! FIREVECTOR — the DIESEL emission-weighting VM, as an addressable alkane.
//!
//! Deployed as a precompiled alkane at **12:0**, the same way DIESEL (2:0),
//! frBTC (32:0) and frSIGIL (32:1) are seeded: the indexer writes the compiled
//! module into `/alkanes/<id>` at an activation height and invokes it with a
//! synthetic cellpack.
//!
//! Being an alkane rather than native indexer code buys three things:
//!
//! - **`simulate`** — a wallet can show a user the weight a transaction will
//!   earn *before* broadcasting it.
//! - **`disassemble`** — anyone can turn the active weightmap back into readable
//!   pseudocode, so emission policy is auditable by voters rather than only by
//!   people who read Rust.
//! - **one content-addressed audit surface** — the VM is a single small module
//!   whose bytes every indexer runs identically, instead of a diff spread through
//!   indexer internals.
//!
//! # Two upgrade paths, never conflated
//!
//! This distinction is load-bearing and must not erode:
//!
//! - the **weightmap blob** is governance-writable. Its blast radius is emission
//!   direction and nothing else — it cannot mint, cannot alter the schedule,
//!   halving or supply cap, cannot touch the fee pot, cannot move anyone's funds.
//! - the **VM itself** is upgradeable *only* by height-gated consensus upgrade,
//!   exactly like the DIESEL genesis binary at 908888 and 917888. The VM executes
//!   in the indexer preamble with indexer privileges; if governance could swap it,
//!   the bounded-blast-radius property above would be false.

pub mod abi;
pub mod disasm;
pub mod programs;
pub mod vm;

#[cfg(feature = "alkane")]
pub mod alkane;

pub use disasm::{describe, disassemble};
pub use vm::{eval, isqrt, mul_div_floor, validate, Item, Source, ValidateError};

/// Flat `Vec<u128>` codec for [`Item`].
///
/// The whole system speaks `Vec<u128>`: cellpack calldata is `Vec<u128>`, the
/// program is `Vec<u128>`, and the item table crossing into the VM is
/// `Vec<u128>`. That symmetry is deliberate — it matches the cellpack encoding
/// natively and it matches the existing house style for packed structs (see
/// `fire-position-token`'s fixed-offset u128-LE `GetAllDetails`).
///
/// Layout, per item:
///
/// ```text
///   target_block, target_tx, opcode, status, txindex, pstone_index, height,
///   mint_rank,
///   n_inputs,   inputs...            (n_inputs words)
///   n_incoming, (block, tx, amount)* (3 * n_incoming words)
///   n_outgoing, (block, tx, amount)* (3 * n_outgoing words)
/// ```
pub mod codec {
    use super::vm::Item;

    /// Number of fixed-position words preceding the variable-length sections.
    const FIXED: usize = 8;

    pub fn encode_item(item: &Item) -> Vec<u128> {
        let mut out = Vec::with_capacity(FIXED + 3 + item.inputs.len() + 3 * (item.incoming.len() + item.outgoing.len()));
        out.push(item.target_block);
        out.push(item.target_tx);
        out.push(item.opcode);
        out.push(item.status);
        out.push(item.txindex);
        out.push(item.pstone_index);
        out.push(item.height);
        out.push(item.mint_rank);

        out.push(item.inputs.len() as u128);
        out.extend_from_slice(&item.inputs);

        out.push(item.incoming.len() as u128);
        for (b, t, a) in &item.incoming {
            out.push(*b);
            out.push(*t);
            out.push(*a);
        }

        out.push(item.outgoing.len() as u128);
        for (b, t, a) in &item.outgoing {
            out.push(*b);
            out.push(*t);
            out.push(*a);
        }
        out
    }

    pub fn encode_items(items: &[Item]) -> Vec<u128> {
        let mut out = vec![items.len() as u128];
        for it in items {
            out.extend(encode_item(it));
        }
        out
    }

    /// Decode one item, returning it and the number of words consumed.
    ///
    /// Total: returns `None` on any truncation or absurd length rather than
    /// panicking or over-allocating. A malformed item table must degrade to "no
    /// weights", never to a wedged indexer.
    // Fields are filled one at a time because each read can fail with `?`;
    // a struct initialiser cannot early-return per field.
    #[allow(clippy::field_reassign_with_default)]
    pub fn decode_item(w: &[u128]) -> Option<(Item, usize)> {
        let mut i = 0usize;
        let take = |i: &mut usize| -> Option<u128> {
            let v = w.get(*i).copied()?;
            *i += 1;
            Some(v)
        };
        let take_len = |i: &mut usize| -> Option<usize> {
            let v = w.get(*i).copied()?;
            *i += 1;
            // Bound against the remaining slice so a huge count can't drive a
            // giant allocation before it fails.
            if v > w.len() as u128 {
                return None;
            }
            Some(v as usize)
        };

        let mut item = Item::default();
        item.target_block = take(&mut i)?;
        item.target_tx = take(&mut i)?;
        item.opcode = take(&mut i)?;
        item.status = take(&mut i)?;
        item.txindex = take(&mut i)?;
        item.pstone_index = take(&mut i)?;
        item.height = take(&mut i)?;
        item.mint_rank = take(&mut i)?;

        let n_inputs = take_len(&mut i)?;
        item.inputs.reserve(n_inputs);
        for _ in 0..n_inputs {
            item.inputs.push(take(&mut i)?);
        }

        let n_incoming = take_len(&mut i)?;
        for _ in 0..n_incoming {
            let b = take(&mut i)?;
            let t = take(&mut i)?;
            let a = take(&mut i)?;
            item.incoming.push((b, t, a));
        }

        let n_outgoing = take_len(&mut i)?;
        for _ in 0..n_outgoing {
            let b = take(&mut i)?;
            let t = take(&mut i)?;
            let a = take(&mut i)?;
            item.outgoing.push((b, t, a));
        }

        Some((item, i))
    }

    pub fn decode_items(w: &[u128]) -> Option<Vec<Item>> {
        let count = *w.first()? ;
        if count > w.len() as u128 {
            return None;
        }
        let mut out = Vec::with_capacity(count as usize);
        let mut off = 1usize;
        for _ in 0..count {
            let (item, used) = decode_item(w.get(off..)?)?;
            off += used;
            out.push(item);
        }
        Some(out)
    }
}

/// Evaluate a program over a whole item table in one pass.
///
/// This is the shape the indexer must use: **one call per block, not one per
/// item.** Blocks have carried 4–5k DIESEL mints, and paying wasm instantiation
/// plus a host-boundary crossing per item would be thousands of times the cost of
/// looping inside the module.
///
/// The program is validated once here, not once per item. An invalid program
/// yields all-zero weights, which the caller must treat as "nothing qualifies"
/// and fall back to the identity vector — never as a reason to abort the block.
pub fn evaluate_table(program: &[u128], items: &[Item], source: Source) -> Vec<u128> {
    match validate(program, source) {
        Err(_) => vec![0; items.len()],
        Ok(steps) => {
            let budget = steps.max(1);
            items.iter().map(|it| eval(program, it, budget)).collect()
        }
    }
}
