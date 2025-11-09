//! Test to verify witness data is preserved during block conversion

use alkanes_support::block_traits::BlockLike;
use alkanes_support::envelope::RawEnvelope;
use alkanes_support::witness::find_witness_payload;
use anyhow::Result;
use bitcoin::{Transaction, Witness};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn test_witness_preserved_in_conversion() -> Result<()> {
    use crate::tests::helpers::clear;
    use crate::tests::std::alkanes_std_test_build;
    use protorune::test_helpers::create_block_with_txs;
    
    clear();
    
    // Create a transaction with witness data
    let bytecode = alkanes_std_test_build::get_bytes();
    let witness = RawEnvelope::from(bytecode.clone()).to_witness(true);
    
    let tx = Transaction {
        version: bitcoin::transaction::Version(2),
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![bitcoin::TxIn {
            previous_output: bitcoin::OutPoint::null(),
            script_sig: bitcoin::ScriptBuf::new(),
            sequence: bitcoin::Sequence::MAX,
            witness,
        }],
        output: vec![bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(1000),
            script_pubkey: bitcoin::ScriptBuf::new(),
        }],
    };
    
    // Debug: Check witness structure
    println!("Transaction has {} inputs", tx.input.len());
    if tx.input.len() > 0 {
        println!("First input witness len: {}", tx.input[0].witness.len());
        for i in 0..tx.input[0].witness.len() {
            if let Some(element) = tx.input[0].witness.nth(i) {
                println!("  Witness element {}: {} bytes", i, element.len());
            }
        }
    }
    
    // Try to extract envelope
    let envelopes = RawEnvelope::from_transaction(&tx);
    println!("Found {} envelopes", envelopes.len());
    
    // Verify original transaction has witness data
    let original_payload = find_witness_payload(&tx, 0);
    assert!(original_payload.is_some(), "Original transaction should have witness payload");
    let original_data = original_payload.unwrap();
    println!("Original witness payload length: {}", original_data.len());
    assert!(original_data.len() > 0, "Original witness payload should not be empty");
    
    // Create a block with this transaction
    let block = create_block_with_txs(vec![tx.clone()]);
    
    // Convert the block
    let converted_block = block.to_bitcoin_block();
    
    // Verify converted transaction has witness data
    assert_eq!(converted_block.txdata.len(), 1, "Converted block should have 1 transaction");
    let converted_tx = &converted_block.txdata[0];
    
    let converted_payload = find_witness_payload(converted_tx, 0);
    assert!(converted_payload.is_some(), "Converted transaction should have witness payload");
    let converted_data = converted_payload.unwrap();
    println!("Converted witness payload length: {}", converted_data.len());
    
    // Verify witness data is identical
    assert_eq!(
        original_data.len(),
        converted_data.len(),
        "Witness payload length should be preserved"
    );
    assert_eq!(
        original_data,
        converted_data,
        "Witness payload data should be preserved"
    );
    
    println!("✅ Witness data preserved correctly!");
    Ok(())
}
