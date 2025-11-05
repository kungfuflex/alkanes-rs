//! Zcash-specific test helpers
//!
//! This module provides utilities for testing alkanes-rs with Zcash features:
//! - ScriptSig-based inscriptions (not witness)
//! - P2PKH transparent addresses (not P2TR)
//! - Z-address fallback testing

use alkanes_support::cellpack::Cellpack;
use alkanes_support::envelope::RawEnvelope;
use alkanes_support::gz::compress;
use anyhow::Result;
use bitcoin::blockdata::transaction::Version;
use bitcoin::script::Builder;
use bitcoin::{opcodes, Address, Amount, Network, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness};
use bitcoin::{Block, Transaction};
use metashrew_support::utils::consensus_encode;
use ordinals::{Runestone, Etching, Rune};
use protorune::test_helpers::create_block_with_coinbase_tx;
use protorune_support::protostone::{Protostone, ProtostoneEdict};
use std::str::FromStr;

/// Create a Zcash P2PKH output (t-address)
pub fn create_zcash_p2pkh_output(value: u64) -> TxOut {
    // Create a mock P2PKH script for Zcash t1 address
    let script = Builder::new()
        .push_opcode(opcodes::all::OP_DUP)
        .push_opcode(opcodes::all::OP_HASH160)
        .push_slice(&[0x1c; 20]) // Mock pubkey hash with Zcash prefix byte
        .push_opcode(opcodes::all::OP_EQUALVERIFY)
        .push_opcode(opcodes::all::OP_CHECKSIG)
        .into_script();
    
    TxOut {
        value: Amount::from_sat(value),
        script_pubkey: script,
    }
}

/// Create a Zcash P2SH output (t3 address)
pub fn create_zcash_p2sh_output(value: u64) -> TxOut {
    let script = Builder::new()
        .push_opcode(opcodes::all::OP_HASH160)
        .push_slice(&[0x1d; 20]) // Mock script hash with Zcash prefix byte
        .push_opcode(opcodes::all::OP_EQUAL)
        .into_script();
    
    TxOut {
        value: Amount::from_sat(value),
        script_pubkey: script,
    }
}

/// Create a mock non-standard output (simulating z-address pointer)
pub fn create_zcash_nonstandard_output(value: u64) -> TxOut {
    // Non-standard script that would represent a z-address pointer
    let script = Builder::new()
        .push_opcode(opcodes::OP_TRUE)
        .push_slice(b"z-addr")
        .into_script();
    
    TxOut {
        value: Amount::from_sat(value),
        script_pubkey: script,
    }
}

/// Create OP_RETURN output
pub fn create_op_return(data: &[u8]) -> TxOut {
    let script = Builder::new()
        .push_opcode(opcodes::all::OP_RETURN)
        .push_slice(data)
        .into_script();
    
    TxOut {
        value: Amount::ZERO,
        script_pubkey: script,
    }
}

/// Create a scriptSig-based inscription for Zcash (ord-dogecoin pattern)
///
/// This creates the scriptSig envelope containing the alkane bytecode,
/// following the ord-dogecoin pattern instead of witness-based inscriptions.
pub fn create_zcash_scriptsig_envelope(bytecode: Vec<u8>) -> Result<ScriptBuf> {
    let compressed_bytecode = compress(bytecode)?;
    
    // Create ZAK envelope in scriptSig
    let envelope = Builder::new()
        .push_opcode(opcodes::OP_FALSE)
        .push_opcode(opcodes::all::OP_IF)
        .push_slice(b"ZAK") // Zcash Alkanes protocol identifier
        .push_slice(&compressed_bytecode)
        .push_opcode(opcodes::all::OP_ENDIF)
        .into_script();
    
    Ok(envelope)
}

/// Create a Zcash transaction with scriptSig inscription (deployment)
///
/// Unlike Bitcoin which uses witness/tapscript, Zcash inscriptions go in scriptSig
pub fn create_zcash_deployment_tx(
    bytecode: Vec<u8>,
    previous_output: OutPoint,
) -> Result<Transaction> {
    let scriptsig_envelope = create_zcash_scriptsig_envelope(bytecode)?;
    
    let txin = TxIn {
        previous_output,
        script_sig: scriptsig_envelope, // Inscription in scriptSig (not witness!)
        sequence: Sequence::MAX,
        witness: Witness::default(),
    };
    
    let mut txouts = vec![
        // Output 0: OP_RETURN with CREATE cellpack
        create_op_return(&[]), // Simplified for testing
        // Output 1: First t-address (receives deployed alkane)
        create_zcash_p2pkh_output(1000),
    ];
    
    Ok(Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin],
        output: txouts,
    })
}

/// Create a Zcash transaction with protostones and multiple output types
///
/// This tests the z-address fallback logic by including:
/// - OP_RETURN at output 0
/// - Non-standard (z-address-like) at output 1
/// - P2PKH (t-address) at output 2 (should be used as fallback)
pub fn create_zcash_tx_with_fallback(
    cellpacks: Vec<Cellpack>,
    previous_output: OutPoint,
    include_z_pointer: bool,
) -> Transaction {
    let protocol_id = 1;
    
    let protostones: Vec<Protostone> = cellpacks
        .into_iter()
        .map(|cellpack| Protostone {
            message: cellpack.encipher(),
            pointer: if include_z_pointer {
                Some(1) // Point to non-standard (z-address-like) output
            } else {
                Some(2) // Point directly to t-address
            },
            refund: None,
            edicts: vec![],
            from: None,
            burn: None,
            protocol_tag: protocol_id as u128,
        })
        .collect();
    
    let runestone = Runestone {
        etching: None,
        pointer: Some(2), // Default pointer to t-address
        edicts: Vec::new(),
        mint: None,
        protocol: protostones.encipher().ok(),
    };
    
    let txin = TxIn {
        previous_output,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::default(),
    };
    
    let mut outputs = vec![
        // Output 0: OP_RETURN with runestone
        TxOut {
            value: Amount::ZERO,
            script_pubkey: runestone.encipher(),
        },
    ];
    
    if include_z_pointer {
        // Output 1: Non-standard script (simulates z-address)
        outputs.push(create_zcash_nonstandard_output(500));
    }
    
    // Output 2 (or 1 if no z-pointer): P2PKH t-address
    outputs.push(create_zcash_p2pkh_output(1000));
    
    // Output 3 (or 2): Another t-address
    outputs.push(create_zcash_p2sh_output(2000));
    
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin],
        output: outputs,
    }
}

/// Initialize a test block with Zcash-specific characteristics
pub fn init_zcash_test_block(
    bytecode: Vec<u8>,
    cellpacks: Vec<Cellpack>,
    test_z_fallback: bool,
) -> Result<Block> {
    let mut block = create_block_with_coinbase_tx(0);
    
    // First transaction: Deploy alkane with scriptSig inscription
    let deployment_tx = create_zcash_deployment_tx(
        bytecode,
        OutPoint::null(),
    )?;
    
    block.txdata.push(deployment_tx);
    
    // Second transaction: Execute cellpacks with optional z-address fallback test
    if !cellpacks.is_empty() {
        let execution_tx = create_zcash_tx_with_fallback(
            cellpacks,
            OutPoint {
                txid: block.txdata[1].compute_txid(),
                vout: 1,
            },
            test_z_fallback,
        );
        block.txdata.push(execution_tx);
    }
    
    Ok(block)
}

/// Test helper: verify that a transaction has t-address outputs
pub fn has_t_address_output(tx: &Transaction) -> bool {
    tx.output.iter().any(|output| {
        output.script_pubkey.is_p2pkh() || output.script_pubkey.is_p2sh()
    })
}

/// Test helper: count t-address outputs
pub fn count_t_address_outputs(tx: &Transaction) -> usize {
    tx.output.iter().filter(|output| {
        output.script_pubkey.is_p2pkh() || output.script_pubkey.is_p2sh()
    }).count()
}

/// Test helper: find first t-address output index
pub fn find_first_t_address(tx: &Transaction) -> Option<usize> {
    tx.output.iter().position(|output| {
        !output.script_pubkey.is_op_return() &&
        (output.script_pubkey.is_p2pkh() || output.script_pubkey.is_p2sh())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_zcash_outputs() {
        let p2pkh = create_zcash_p2pkh_output(1000);
        assert!(p2pkh.script_pubkey.is_p2pkh());
        assert_eq!(p2pkh.value.to_sat(), 1000);
        
        let p2sh = create_zcash_p2sh_output(2000);
        assert!(p2sh.script_pubkey.is_p2sh());
        assert_eq!(p2sh.value.to_sat(), 2000);
        
        let nonstandard = create_zcash_nonstandard_output(500);
        assert!(!nonstandard.script_pubkey.is_p2pkh());
        assert!(!nonstandard.script_pubkey.is_p2sh());
    }
    
    #[test]
    fn test_has_t_address_output() {
        let tx = Transaction {
            version: Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                create_op_return(b"test"),
                create_zcash_nonstandard_output(500),
                create_zcash_p2pkh_output(1000),
            ],
        };
        
        assert!(has_t_address_output(&tx));
        assert_eq!(count_t_address_outputs(&tx), 1);
        assert_eq!(find_first_t_address(&tx), Some(2));
    }
    
    #[test]
    fn test_scriptsig_envelope() {
        let bytecode = vec![0x01, 0x02, 0x03];
        let envelope = create_zcash_scriptsig_envelope(bytecode).unwrap();
        
        // Should contain ZAK identifier
        let script_bytes = envelope.as_bytes();
        let script_str = String::from_utf8_lossy(script_bytes);
        assert!(script_str.contains("ZAK"), "Envelope should contain ZAK identifier");
    }
}
