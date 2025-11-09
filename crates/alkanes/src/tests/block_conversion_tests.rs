//! Isolated tests for block conversion and compatibility
//! 
//! These tests verify that our BlockLike abstraction works correctly
//! with the alkanes indexing pipeline at each stage.

use crate::tests::helpers::{self as alkane_helpers};
use crate::vm::fuel::FuelTank;
use alkane_helpers::clear;
use alkanes_support::block_traits::BlockLike;
use anyhow::Result;
use bitcoin::{Block, Transaction};
use bitcoin::hashes::Hash as HashTrait;
use protorune::test_helpers::create_block_with_coinbase_tx;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_bitcoin_block_vfsize_calculation() -> Result<()> {
    clear();
    
    // Create a test block with known transactions
    let test_block = create_block_with_coinbase_tx(0);
    
    // Calculate vfsize manually
    let manual_vfsize: u64 = test_block.txdata.iter().map(|tx| {
        use bitcoin::consensus::Encodable;
        let mut buf = Vec::new();
        tx.consensus_encode(&mut buf).unwrap();
        buf.len() as u64
    }).sum();
    
    println!("Block has {} transactions", test_block.txdata.len());
    println!("Manual vfsize calculation: {}", manual_vfsize);
    
    assert!(manual_vfsize > 0, "vfsize should be non-zero");
    assert!(test_block.txdata.len() > 0, "Block should have transactions");
    
    Ok(())
}

#[wasm_bindgen_test]
fn test_block_conversion_preserves_vfsize() -> Result<()> {
    clear();
    
    // Create a test block
    let test_block = create_block_with_coinbase_tx(0);
    
    // Calculate original vfsize
    let original_vfsize: u64 = test_block.txdata.iter().map(|tx| {
        use bitcoin::consensus::Encodable;
        let mut buf = Vec::new();
        tx.consensus_encode(&mut buf).unwrap();
        buf.len() as u64
    }).sum();
    
    println!("Original vfsize: {}", original_vfsize);
    
    // Convert using BlockLike trait
    let converted_block = test_block.to_bitcoin_block();
    
    // Calculate converted vfsize
    let converted_vfsize: u64 = converted_block.txdata.iter().map(|tx| {
        use bitcoin::consensus::Encodable;
        let mut buf = Vec::new();
        tx.consensus_encode(&mut buf).unwrap();
        buf.len() as u64
    }).sum();
    
    println!("Converted vfsize: {}", converted_vfsize);
    
    assert_eq!(
        original_vfsize, converted_vfsize,
        "Conversion should preserve vfsize"
    );
    assert!(converted_vfsize > 0, "Converted vfsize should be non-zero");
    
    Ok(())
}

#[wasm_bindgen_test]
fn test_fuel_tank_with_original_block() -> Result<()> {
    clear();
    
    // Create a test block
    let test_block = create_block_with_coinbase_tx(0);
    
    // Initialize FuelTank with original block
    FuelTank::initialize(&test_block, 0);
    
    // Get the tank to verify it was initialized
    let tank = FuelTank::get_fuel_tank_copy();
    assert!(tank.is_some(), "FuelTank should be initialized");
    
    let tank = tank.unwrap();
    println!("FuelTank size: {}", tank.size);
    println!("FuelTank block_fuel: {}", tank.block_fuel);
    
    assert!(tank.size > 0, "FuelTank size should be non-zero");
    assert!(tank.block_fuel > 0, "FuelTank block_fuel should be non-zero");
    
    Ok(())
}

#[wasm_bindgen_test]
fn test_fuel_tank_with_converted_block() -> Result<()> {
    clear();
    
    // Create a test block
    let test_block = create_block_with_coinbase_tx(0);
    
    // Convert the block
    let converted_block = test_block.to_bitcoin_block();
    
    println!("Original block tx count: {}", test_block.txdata.len());
    println!("Converted block tx count: {}", converted_block.txdata.len());
    
    // Initialize FuelTank with converted block
    FuelTank::initialize(&converted_block, 0);
    
    // Get the tank to verify it was initialized
    let tank = FuelTank::get_fuel_tank_copy();
    assert!(tank.is_some(), "FuelTank should be initialized");
    
    let tank = tank.unwrap();
    println!("FuelTank size with converted block: {}", tank.size);
    println!("FuelTank block_fuel: {}", tank.block_fuel);
    
    assert!(tank.size > 0, "FuelTank size should be non-zero with converted block");
    assert!(tank.block_fuel > 0, "FuelTank block_fuel should be non-zero");
    
    Ok(())
}

#[wasm_bindgen_test]
fn test_fuel_tank_consistency() -> Result<()> {
    clear();
    
    // Create a test block
    let test_block = create_block_with_coinbase_tx(0);
    
    // Test with original block
    FuelTank::initialize(&test_block, 0);
    let tank_original = FuelTank::get_fuel_tank_copy().unwrap();
    
    clear();
    
    // Test with converted block
    let converted_block = test_block.to_bitcoin_block();
    FuelTank::initialize(&converted_block, 0);
    let tank_converted = FuelTank::get_fuel_tank_copy().unwrap();
    
    println!("Original tank size: {}", tank_original.size);
    println!("Converted tank size: {}", tank_converted.size);
    
    assert_eq!(
        tank_original.size, tank_converted.size,
        "FuelTank should have same size with original and converted blocks"
    );
    assert_eq!(
        tank_original.block_fuel, tank_converted.block_fuel,
        "FuelTank should have same block_fuel with original and converted blocks"
    );
    
    Ok(())
}

#[wasm_bindgen_test]
fn test_empty_block_detection() -> Result<()> {
    clear();
    
    // Create an empty block (just header, no transactions)
    use bitcoin::{
        block::Header, block::Version, BlockHash, CompactTarget, TxMerkleNode,
    };
    
    let empty_block = Block {
        header: Header {
            version: Version::from_consensus(1),
            prev_blockhash: BlockHash::all_zeros(),
            merkle_root: TxMerkleNode::all_zeros(),
            time: 0,
            bits: CompactTarget::from_consensus(0x1d00ffff),
            nonce: 0,
        },
        txdata: vec![],
    };
    
    // Try to initialize FuelTank with empty block
    FuelTank::initialize(&empty_block, 0);
    
    let tank = FuelTank::get_fuel_tank_copy().unwrap();
    println!("Empty block tank size: {}", tank.size);
    
    assert_eq!(tank.size, 0, "Empty block should have size 0");
    
    // This is the bug we fixed - division by zero would happen here
    // Now it should handle gracefully
    
    Ok(())
}

#[wasm_bindgen_test]
fn test_block_with_multiple_transactions() -> Result<()> {
    clear();
    
    use bitcoin::{
        absolute::LockTime, block::Header, block::Version, transaction::Version as TxVersion,
        Amount, BlockHash, CompactTarget, OutPoint, ScriptBuf, Sequence, Transaction, TxIn,
        TxMerkleNode, TxOut, Witness,
    };
    use bitcoin::hashes::Hash;
    
    // Create a block with multiple transactions
    let tx1 = Transaction {
        version: TxVersion(1),
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(5000000000),
            script_pubkey: ScriptBuf::new(),
        }],
    };
    
    let tx2 = Transaction {
        version: TxVersion(1),
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: tx1.compute_txid(),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(1000000),
            script_pubkey: ScriptBuf::new(),
        }],
    };
    
    let multi_tx_block = Block {
        header: Header {
            version: Version::from_consensus(1),
            prev_blockhash: BlockHash::all_zeros(),
            merkle_root: TxMerkleNode::all_zeros(),
            time: 0,
            bits: CompactTarget::from_consensus(0x1d00ffff),
            nonce: 0,
        },
        txdata: vec![tx1, tx2],
    };
    
    // Test original block
    let original_vfsize: u64 = multi_tx_block.txdata.iter().map(|tx| {
        use bitcoin::consensus::Encodable;
        let mut buf = Vec::new();
        tx.consensus_encode(&mut buf).unwrap();
        buf.len() as u64
    }).sum();
    
    println!("Original multi-tx block vfsize: {}", original_vfsize);
    
    // Test converted block
    let converted = multi_tx_block.to_bitcoin_block();
    let converted_vfsize: u64 = converted.txdata.iter().map(|tx| {
        use bitcoin::consensus::Encodable;
        let mut buf = Vec::new();
        tx.consensus_encode(&mut buf).unwrap();
        buf.len() as u64
    }).sum();
    
    println!("Converted multi-tx block vfsize: {}", converted_vfsize);
    
    assert_eq!(original_vfsize, converted_vfsize, "Multi-tx block vfsize should be preserved");
    assert_eq!(converted.txdata.len(), 2, "Should have 2 transactions");
    
    // Test FuelTank
    FuelTank::initialize(&converted, 0);
    let tank = FuelTank::get_fuel_tank_copy().unwrap();
    
    println!("Multi-tx block FuelTank size: {}", tank.size);
    assert_eq!(tank.size, converted_vfsize, "FuelTank size should match vfsize");
    
    Ok(())
}
