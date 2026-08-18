//! Disassembler.
//!
//! This is not a debugging convenience — it is the reason the weightmap is
//! auditable. Emission policy today is a rule welded into the indexer, legible
//! only by reading Rust. A weightmap is a `Vec<u128>` that any wallet or voter
//! can turn back into a page of pseudocode before voting on it. A self-dealing
//! vector should be visibly self-dealing.
//!
//! Disassembly is total: it never fails and never panics. Malformed programs
//! disassemble to explicit `<...>` markers rather than erroring, because someone
//! auditing a bad vector needs to see where it goes wrong.

use crate::vm::{mnemonic, shape, Mode, OP_INCOMING_AMOUNT};

/// Render a program as one instruction per line.
pub fn disassemble(program: &[u128]) -> String {
    let mut out = String::new();
    let mut i: usize = 0;

    while i < program.len() {
        let op = program[i];
        let name = mnemonic(op);
        let imm = shape(op).map(|(n, _, _)| n).unwrap_or(0);

        out.push_str(&format!("{:>5}  ", i));

        match name {
            None => {
                out.push_str(&format!("<unknown opcode {}>\n", op));
                i += 1;
                continue;
            }
            Some(n) => out.push_str(n),
        }

        // Immediates, rendered in the shape that reads best for each opcode.
        if imm > 0 {
            if i + imm >= program.len() {
                out.push_str(" <truncated immediates>\n");
                return out;
            }
            match op {
                // Alkane ids read as block:tx rather than as two bare numbers.
                OP_INCOMING_AMOUNT => {
                    out.push_str(&format!(" {}:{}", program[i + 1], program[i + 2]));
                }
                // Everything else — PUSH, INPUT, PRIOR_INPUT, SHR, SHL — is
                // plain operands.
                _ => {
                    for k in 1..=imm {
                        out.push_str(&format!(" {}", program[i + k]));
                    }
                }
            }
        }

        out.push('\n');
        i += 1 + imm;
    }

    out
}

/// Disassembly plus a validation verdict — what a governance UI should show
/// before a vote.
pub fn describe(program: &[u128], mode: Mode) -> String {
    let mut out = disassemble(program);
    out.push('\n');
    match crate::vm::validate(program, mode) {
        Ok(steps) => out.push_str(&format!("valid ({:?}, {} steps/item)\n", mode, steps)),
        Err(e) => out.push_str(&format!("INVALID ({:?}): {}\n", mode, e)),
    }
    out
}

/// Disassembly plus the header fields, which is what a voter actually needs.
///
/// The header carries the two things the program body cannot show: which action
/// is being subsidised, and how weights become payouts. A disassembly on its own
/// is misleading — the same body means "a share of the block" under
/// [`Mode::Split`] and "a rate against `rate_floor`" under [`Mode::Rate`].
pub fn describe_weightmap(
    program: &[u128],
    mode: Mode,
    qualifier: Option<crate::vm::Qualifier>,
    rate_floor: u128,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("mode: {:?}\n", mode));
    match qualifier {
        Some(q) => out.push_str(&format!(
            "qualifies: a prior protostone calling {}:{} opcode {}\n",
            q.target_block, q.target_tx, q.opcode
        )),
        None => out.push_str("qualifies: every DIESEL mint (no prior action required)\n"),
    }
    if mode == Mode::Rate {
        out.push_str(&format!("rate_floor: {}\n", rate_floor));
    }
    out.push('\n');
    out.push_str(&describe(program, mode));
    out
}
