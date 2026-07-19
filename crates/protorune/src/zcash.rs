//! Zcash-specific utilities for protorune
//!
//! This module provides Zcash-specific functionality for handling:
//! - Transparent address (t-address) detection
//! - Z-address handling with automatic fallback
//! - Pointer resolution for transparent-only operations

use bitcoin::{Script, Transaction};
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};

/// Check if a script_pubkey represents a transparent address (P2PKH or P2SH)
///
/// Zcash transparent addresses:
/// - P2PKH: t1... addresses (0x1c prefix)
/// - P2SH: t3... addresses (0x1d prefix)
///
/// Shielded addresses (z-addresses) are NOT represented as standard script types
/// and cannot appear in transaction outputs in a way we can track.
pub fn is_t_address(script_pubkey: &Script) -> bool {
    script_pubkey.is_p2pkh() || script_pubkey.is_p2sh()
}

/// Check if a script_pubkey might represent a z-address or non-standard output
///
/// This is a heuristic: we can't positively identify z-addresses in outputs,
/// but we can identify outputs that are NOT t-addresses and NOT OP_RETURN.
pub fn is_z_address_or_unknown(script_pubkey: &Script) -> bool {
    !is_t_address(script_pubkey) && !script_pubkey.is_op_return()
}

/// Find the first t-address output in a transaction (default output logic)
///
/// This mimics the default_output() behavior from protorune but only considers
/// transparent addresses. Returns None if no t-address outputs exist.
pub fn find_default_t_address_output(tx: &Transaction) -> Option<u32> {
    for (i, output) in tx.output.iter().enumerate() {
        if !output.script_pubkey.is_op_return() && is_t_address(&output.script_pubkey) {
            return Some(i as u32);
        }
    }
    None
}

/// Resolve a pointer with automatic fallback to handle z-addresses
///
/// Fallback chain:
/// 1. Try primary pointer if it targets a t-address
/// 2. Try refund_pointer if it targets a t-address  
/// 3. Find first t-address output (default output logic)
/// 4. Return None (funds will be burned)
///
/// # Arguments
/// * `tx` - The transaction containing outputs
/// * `pointer` - Primary pointer from runestone/protostone
/// * `refund_pointer` - Refund pointer (if available)
///
/// # Returns
/// * `Some(output_index)` - Resolved t-address output index
/// * `None` - No t-address found, funds should be burned
pub fn resolve_pointer_with_fallback(
    tx: &Transaction,
    pointer: Option<u32>,
    refund_pointer: Option<u32>,
) -> Option<u32> {
    // 1. Try primary pointer
    if let Some(p) = pointer {
        if (p as usize) < tx.output.len() {
            let script = &tx.output[p as usize].script_pubkey;
            if is_t_address(script) {
                return Some(p);
            } else if is_z_address_or_unknown(script) {
                println!(
                    "[ZCASH] Warning: Pointer {} targets z-address or non-standard output, attempting fallback",
                    p
                );
            }
        }
    }

    // 2. Try refund_pointer
    if let Some(rp) = refund_pointer {
        if (rp as usize) < tx.output.len() {
            let script = &tx.output[rp as usize].script_pubkey;
            if is_t_address(script) {
                println!(
                    "[ZCASH] Using refund_pointer {} instead of primary pointer",
                    rp
                );
                return Some(rp);
            } else if is_z_address_or_unknown(script) {
                println!(
                    "[ZCASH] Warning: Refund pointer {} also targets z-address or non-standard output",
                    rp
                );
            }
        }
    }

    // 3. Find first t-address output (default output logic)
    if let Some(default_t) = find_default_t_address_output(tx) {
        println!(
            "[ZCASH] Both pointer and refund_pointer unusable, using first t-address output {}",
            default_t
        );
        return Some(default_t);
    }

    // 4. No t-address found - funds will be burned
    println!(
        "[ZCASH] ERROR: Transaction {} has no transparent address outputs. Funds will be BURNED.",
        tx.compute_txid()
    );
    None
}
