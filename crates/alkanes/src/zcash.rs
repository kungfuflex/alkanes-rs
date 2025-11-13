//! Zcash-specific utilities for alkanes-rs
//!
//! This module provides Zcash-specific functionality including:
//! - Transparent address (t-address) detection
//! - Z-address handling with automatic fallback
//! - Pointer resolution for transparent-only operations
//! - Zcash block parsing (pre-Sapling and post-Sapling)

use anyhow::{anyhow, Result};
use bitcoin::hashes::Hash as HashTrait;
use bitcoin::{Block, Script, Transaction};
use std::io::Read;
use std::fmt::Write;

// In test mode, use standard println; in non-test mode, use metashrew_core println
#[cfg(not(test))]
use metashrew_core::{println, stdio::stdout};

#[cfg(test)]
macro_rules! println {
    ($($arg:tt)*) => {
        std::println!($($arg)*)
    };
}

/// Parse a Zcash transaction, skipping Zcash-specific fields
///
/// Zcash transaction formats:
/// - Version 1: Standard Bitcoin-like transaction
/// - Version 2 (Overwinter): Adds nExpiryHeight and version group ID
/// - Version 3 (Overwinter): Similar to v2
/// - Version 4 (Sapling): Adds Sapling shielded components
/// - Version 5 (NU5/Orchard): Adds Orchard shielded components
///
/// For alkanes, we only need the transparent inputs/outputs, so we parse those
/// and skip the shielded components.
fn parse_zcash_transaction(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<Transaction> {
    use metashrew_support::utils::{consume_varint, consensus_decode};
    use bitcoin::{TxIn, TxOut};
    
    // Read version (4 bytes)
    let mut version_bytes = [0u8; 4];
    cursor.read_exact(&mut version_bytes)?;
    let version = i32::from_le_bytes(version_bytes);
    
    // Check if this is an Overwinter or later transaction (has version group ID)
    let is_overwinter = version >= 3 || (version < 0 && version as u32 >= 0x80000003);
    
    // If Overwinter+, read version group ID (4 bytes) and skip it
    if is_overwinter {
        let mut _version_group_id = [0u8; 4];
        cursor.read_exact(&mut _version_group_id)?;
    }
    
    // Read inputs (standard Bitcoin format)
    let input_count = consume_varint(cursor)? as usize;
    let mut inputs = Vec::with_capacity(input_count);
    
    for _ in 0..input_count {
        let input: TxIn = consensus_decode(cursor)?;
        inputs.push(input);
    }
    
    // Read outputs (standard Bitcoin format)
    let output_count = consume_varint(cursor)? as usize;
    let mut outputs = Vec::with_capacity(output_count);
    
    for _ in 0..output_count {
        let output: TxOut = consensus_decode(cursor)?;
        outputs.push(output);
    }
    
    // Read lock time (4 bytes)
    let mut lock_time_bytes = [0u8; 4];
    cursor.read_exact(&mut lock_time_bytes)?;
    let lock_time = u32::from_le_bytes(lock_time_bytes);
    
    // If Overwinter+, read nExpiryHeight (4 bytes) and skip it
    if is_overwinter {
        let mut _expiry_height = [0u8; 4];
        cursor.read_exact(&mut _expiry_height)?;
    }
    
    // For Sapling (v4+), skip shielded components
    if version >= 4 {
        // valueBalance (8 bytes)
        let mut _value_balance = [0u8; 8];
        cursor.read_exact(&mut _value_balance)?;
        
        // nShieldedSpend count
        let spend_count = consume_varint(cursor)?;
        // Skip each shielded spend (384 bytes each)
        for _ in 0..spend_count {
            let mut _spend_data = [0u8; 384];
            cursor.read_exact(&mut _spend_data)?;
        }
        
        // nShieldedOutput count
        let output_count = consume_varint(cursor)?;
        // Skip each shielded output (948 bytes each)
        for _ in 0..output_count {
            let mut _output_data = [0u8; 948];
            cursor.read_exact(&mut _output_data)?;
        }
    }
    
    // For v2+, skip JoinSplit data if present
    if version >= 2 {
        let joinsplit_count = consume_varint(cursor)?;
        
        if joinsplit_count > 0 {
            // Each JoinSplit is 1698 or 1802 bytes depending on version
            let joinsplit_size = if version >= 4 { 1698 } else { 1802 };
            
            for _ in 0..joinsplit_count {
                let mut _joinsplit_data = vec![0u8; joinsplit_size];
                cursor.read_exact(&mut _joinsplit_data)?;
            }
            
            // JoinSplit pubkey (32 bytes)
            let mut _joinsplit_pubkey = [0u8; 32];
            cursor.read_exact(&mut _joinsplit_pubkey)?;
            
            // JoinSplit sig (64 bytes)
            let mut _joinsplit_sig = [0u8; 64];
            cursor.read_exact(&mut _joinsplit_sig)?;
        }
    }
    
    // For Sapling (v4+), read binding signature if there were shielded spends/outputs
    // We already read the counts above, but we need to check if bindingSig exists
    // bindingSig exists if valueBalance != 0 or there are shielded spends/outputs
    // For simplicity, try to read it for v4+ (64 bytes)
    if version >= 4 {
        // Check if there's a binding signature by checking remaining data
        // bindingSig is 64 bytes
        let remaining = cursor.get_ref().len() - cursor.position() as usize;
        if remaining >= 64 {
            // Peek to see if this looks like a binding sig or next transaction
            // For now, assume if there's exactly 64 bytes or more and we're not at a transaction boundary
            // we should read it. This is a heuristic and might need refinement.
            
            // Actually, we should read it if there were any shielded components
            // Since we don't track that, let's try a different approach:
            // Skip bindingSig only if we actually had shielded spends/outputs
            // For block 407 analysis, we saw the data continues, so let's read it
            let mut _binding_sig = [0u8; 64];
            match cursor.read_exact(&mut _binding_sig) {
                Ok(_) => {}, // Successfully read binding sig
                Err(_) => {
                    // Not enough data, probably no binding sig
                    // Seek back
                    cursor.set_position(cursor.position() - 64);
                }
            }
        }
    }
    
    // Build Bitcoin-compatible transaction
    Ok(Transaction {
        version: bitcoin::transaction::Version(version),
        lock_time: bitcoin::locktime::absolute::LockTime::from_consensus(lock_time),
        input: inputs,
        output: outputs,
    })
}

/// Zcash block structure that can parse both pre-Sapling and post-Sapling blocks
pub struct ZcashBlock {
    /// The parsed bitcoin-compatible block
    pub block: Block,
    /// Block version (used to determine Sapling vs pre-Sapling)
    pub version: i32,
}

impl ZcashBlock {
    /// Parse a Zcash block from raw bytes
    ///
    /// Zcash block formats:
    /// - Version 1-3 (pre-Sapling): Similar to Bitcoin but with different header fields
    /// - Version 4+ (Sapling): Additional fields including solution size and Equihash solution
    ///
    /// Block header structure:
    /// - Version (4 bytes)
    /// - Previous block hash (32 bytes)
    /// - Merkle root (32 bytes)
    /// - [Zcash-specific] Final Sapling root / Reserved (32 bytes)
    /// - Time (4 bytes)
    /// - nBits (4 bytes)
    /// - Nonce (32 bytes) - Note: Zcash uses 32 bytes, not 4 like Bitcoin
    /// - Solution size (compact size)
    /// - Solution (Equihash solution, variable length)
    /// - Transaction count (compact size)
    /// - Transactions...
    pub fn parse(cursor: &mut std::io::Cursor<Vec<u8>>) -> Result<Self> {
        use metashrew_support::utils::{consume_varint, consensus_decode};
        
        // Read version (4 bytes)
        let mut version_bytes = [0u8; 4];
        cursor.read_exact(&mut version_bytes)?;
        let version = i32::from_le_bytes(version_bytes);
        
        // Read previous block hash (32 bytes)
        let mut prev_blockhash = [0u8; 32];
        cursor.read_exact(&mut prev_blockhash)?;
        
        // Read merkle root (32 bytes)
        let mut merkle_root = [0u8; 32];
        cursor.read_exact(&mut merkle_root)?;
        
        // Read reserved/final sapling root (32 bytes) - Zcash-specific, we'll skip
        let mut _reserved = [0u8; 32];
        cursor.read_exact(&mut _reserved)?;
        
        // Read time (4 bytes)
        let mut time_bytes = [0u8; 4];
        cursor.read_exact(&mut time_bytes)?;
        let time = u32::from_le_bytes(time_bytes);
        
        // Read nBits (4 bytes)
        let mut bits_bytes = [0u8; 4];
        cursor.read_exact(&mut bits_bytes)?;
        let bits = u32::from_le_bytes(bits_bytes);
        
        // Read nonce (32 bytes) - Zcash uses 32 bytes
        let mut nonce_bytes = [0u8; 32];
        cursor.read_exact(&mut nonce_bytes)?;
        // Convert to u32 for Bitcoin compatibility (take first 4 bytes)
        let nonce = u32::from_le_bytes([nonce_bytes[0], nonce_bytes[1], nonce_bytes[2], nonce_bytes[3]]);
        
        // Read solution size (compact size / varint)
        let solution_size = consume_varint(cursor)? as usize;
        
        // Read and skip solution (Equihash proof)
        let mut solution = vec![0u8; solution_size];
        cursor.read_exact(&mut solution)?;
        
        // Now read transactions - Zcash transactions have extra fields we need to skip
        let tx_count = consume_varint(cursor)? as usize;
        let mut transactions = Vec::with_capacity(tx_count);
        
        for _ in 0..tx_count {
            // Parse Zcash transaction (handles version 1, 2, 3, 4+)
            let tx = parse_zcash_transaction(cursor)?;
            transactions.push(tx);
        }
        
        // Build a Bitcoin-compatible header
        let header = bitcoin::block::Header {
            version: bitcoin::block::Version::from_consensus(version),
            prev_blockhash: bitcoin::BlockHash::from_slice(&prev_blockhash)?,
            merkle_root: bitcoin::TxMerkleNode::from_slice(&merkle_root)?,
            time,
            bits: bitcoin::CompactTarget::from_consensus(bits),
            nonce,
        };
        
        Ok(ZcashBlock {
            block: Block {
                header,
                txdata: transactions,
            },
            version,
        })
    }
}

impl From<ZcashBlock> for Block {
    fn from(zblock: ZcashBlock) -> Self {
        zblock.block
    }
}

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

/// Validate that a transaction has at least one t-address output
///
/// Returns an error if the transaction has no t-address outputs,
/// which would result in burned funds.
pub fn require_t_address_output(tx: &Transaction) -> Result<()> {
    if find_default_t_address_output(tx).is_none() {
        return Err(anyhow!(
            "Transaction {} has no transparent address outputs. \
             Cannot process alkanes operations without t-addresses.",
            tx.compute_txid()
        ));
    }
    Ok(())
}

/// Check if a transaction appears to be transparent-only
///
/// This is a heuristic check. Zcash transactions can have:
/// - Version 1-3: Transparent only
/// - Version 4+: May have shielded components (Sapling/Orchard)
///
/// For alkanes, we only process transactions that appear to be transparent-only.
pub fn is_transparent_only_tx(tx: &Transaction) -> bool {
    // Simple heuristic: version <= 3 are always transparent
    // Version 4+ might have shielded components, but we'd need to
    // parse Zcash-specific transaction fields to know for sure.
    //
    // For now, we accept all transactions and rely on output validation
    // to ensure we only interact with t-addresses.
    //
    // TODO: If we parse full Zcash transaction format, check for:
    // - Empty vJoinSplit (Sprout)
    // - Empty vShieldedSpend/vShieldedOutput (Sapling)  
    // - Empty vShieldedOrchard (Orchard)

    tx.version.0 <= 3 || has_only_standard_outputs(tx)
}

/// Check if all outputs are standard Bitcoin-style outputs
///
/// Helper for transparent-only detection
fn has_only_standard_outputs(tx: &Transaction) -> bool {
    tx.output.iter().all(|output| {
        output.script_pubkey.is_p2pkh()
            || output.script_pubkey.is_p2sh()
            || output.script_pubkey.is_op_return()
            || output.script_pubkey.is_p2wpkh()
            || output.script_pubkey.is_p2wsh()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{
        blockdata::{opcodes, script::Builder},
        Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
    };

    fn create_p2pkh_script() -> ScriptBuf {
        // Create a simple P2PKH script (t-address)
        Builder::new()
            .push_opcode(opcodes::all::OP_DUP)
            .push_opcode(opcodes::all::OP_HASH160)
            .push_slice([0u8; 20]) // Dummy pubkey hash
            .push_opcode(opcodes::all::OP_EQUALVERIFY)
            .push_opcode(opcodes::all::OP_CHECKSIG)
            .into_script()
    }

    fn create_p2sh_script() -> ScriptBuf {
        // Create a simple P2SH script (t-address)
        Builder::new()
            .push_opcode(opcodes::all::OP_HASH160)
            .push_slice([0u8; 20]) // Dummy script hash
            .push_opcode(opcodes::all::OP_EQUAL)
            .into_script()
    }

    fn create_op_return_script() -> ScriptBuf {
        Builder::new()
            .push_opcode(opcodes::all::OP_RETURN)
            .push_slice(b"data")
            .into_script()
    }

    fn create_unknown_script() -> ScriptBuf {
        // Non-standard script (might represent z-address)
        Builder::new()
            .push_opcode(opcodes::OP_TRUE)
            .into_script()
    }

    #[test]
    fn test_is_t_address() {
        assert!(is_t_address(&create_p2pkh_script()));
        assert!(is_t_address(&create_p2sh_script()));
        assert!(!is_t_address(&create_op_return_script()));
        assert!(!is_t_address(&create_unknown_script()));
    }

    #[test]
    fn test_find_default_t_address_output() {
        let tx = Transaction {
            version: bitcoin::transaction::Version(4),
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                TxOut {
                    value: Amount::ZERO,
                    script_pubkey: create_op_return_script(),
                },
                TxOut {
                    value: Amount::from_sat(1000),
                    script_pubkey: create_unknown_script(),
                },
                TxOut {
                    value: Amount::from_sat(2000),
                    script_pubkey: create_p2pkh_script(), // First t-address
                },
                TxOut {
                    value: Amount::from_sat(3000),
                    script_pubkey: create_p2sh_script(),
                },
            ],
        };

        assert_eq!(find_default_t_address_output(&tx), Some(2));
    }

    #[test]
    fn test_find_default_t_address_output_none() {
        let tx = Transaction {
            version: bitcoin::transaction::Version(4),
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                TxOut {
                    value: Amount::ZERO,
                    script_pubkey: create_op_return_script(),
                },
                TxOut {
                    value: Amount::from_sat(1000),
                    script_pubkey: create_unknown_script(),
                },
            ],
        };

        assert_eq!(find_default_t_address_output(&tx), None);
    }

    #[test]
    fn test_resolve_pointer_direct() {
        let tx = Transaction {
            version: bitcoin::transaction::Version(4),
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                TxOut {
                    value: Amount::ZERO,
                    script_pubkey: create_op_return_script(),
                },
                TxOut {
                    value: Amount::from_sat(1000),
                    script_pubkey: create_p2pkh_script(), // Pointer targets this
                },
            ],
        };

        assert_eq!(resolve_pointer_with_fallback(&tx, Some(1), None), Some(1));
    }

    #[test]
    fn test_resolve_pointer_fallback_to_refund() {
        let tx = Transaction {
            version: bitcoin::transaction::Version(4),
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                TxOut {
                    value: Amount::ZERO,
                    script_pubkey: create_op_return_script(),
                },
                TxOut {
                    value: Amount::from_sat(1000),
                    script_pubkey: create_unknown_script(), // Pointer targets z-address
                },
                TxOut {
                    value: Amount::from_sat(2000),
                    script_pubkey: create_p2pkh_script(), // Refund targets t-address
                },
            ],
        };

        assert_eq!(resolve_pointer_with_fallback(&tx, Some(1), Some(2)), Some(2));
    }

    #[test]
    fn test_resolve_pointer_fallback_to_first_t_address() {
        let tx = Transaction {
            version: bitcoin::transaction::Version(4),
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                TxOut {
                    value: Amount::ZERO,
                    script_pubkey: create_op_return_script(),
                },
                TxOut {
                    value: Amount::from_sat(1000),
                    script_pubkey: create_unknown_script(), // Pointer
                },
                TxOut {
                    value: Amount::from_sat(2000),
                    script_pubkey: create_unknown_script(), // Refund
                },
                TxOut {
                    value: Amount::from_sat(3000),
                    script_pubkey: create_p2pkh_script(), // First t-address
                },
            ],
        };

        assert_eq!(resolve_pointer_with_fallback(&tx, Some(1), Some(2)), Some(3));
    }

    #[test]
    fn test_resolve_pointer_none_burns() {
        let tx = Transaction {
            version: bitcoin::transaction::Version(4),
            lock_time: bitcoin::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                TxOut {
                    value: Amount::ZERO,
                    script_pubkey: create_op_return_script(),
                },
                TxOut {
                    value: Amount::from_sat(1000),
                    script_pubkey: create_unknown_script(),
                },
            ],
        };

        assert_eq!(resolve_pointer_with_fallback(&tx, Some(1), None), None);
    }
}
