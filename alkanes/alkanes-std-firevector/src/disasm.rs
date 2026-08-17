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

use crate::vm::{mnemonic, shape, Source, OP_IN_AMOUNT, OP_INPUT, OP_OUT_AMOUNT, OP_PUSH, OP_SHL, OP_SHR};

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
                OP_IN_AMOUNT | OP_OUT_AMOUNT => {
                    out.push_str(&format!(" {}:{}", program[i + 1], program[i + 2]));
                }
                OP_PUSH | OP_INPUT | OP_SHR | OP_SHL => {
                    out.push_str(&format!(" {}", program[i + 1]));
                }
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
pub fn describe(program: &[u128], source: Source) -> String {
    let mut out = disassemble(program);
    out.push_str("\n");
    match crate::vm::validate(program, source) {
        Ok(steps) => out.push_str(&format!("valid ({:?}, {} steps/item)\n", source, steps)),
        Err(e) => out.push_str(&format!("INVALID ({:?}): {}\n", source, e)),
    }
    out
}
