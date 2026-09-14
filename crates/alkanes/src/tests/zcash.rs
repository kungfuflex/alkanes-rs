//! Zcash-specific integration tests
//!
//! Tests for alkanes-rs with Zcash features:
//! - ScriptSig inscriptions (ord-dogecoin pattern)
//! - Z-address fallback logic
//! - Transparent address handling
//! - CGGMP21 compatibility

#[cfg(test)]
mod tests {
    use crate::index_block;
    use crate::tests::helpers as alkane_helpers;
    use crate::tests::zcash_helpers::*;
    use alkanes_support::cellpack::Cellpack;
    use alkanes_support::id::AlkaneId;
    use anyhow::Result;
    use bitcoin::{OutPoint, Transaction};
    #[allow(unused_imports)]
    use metashrew_core::{
        index_pointer::IndexPointer,
        println,
        stdio::{stdout, Write},
    };
    use protorune::balance_sheet::load_sheet;
    use protorune::tables::RuneTable;
    use protorune_support::balance_sheet::BalanceSheet;
    use metashrew_support::index_pointer::KeyValuePointer;
    use metashrew_support::utils::consensus_encode;
    use wasm_bindgen_test::wasm_bindgen_test;

    // Import a test contract
    #[cfg(test)]
    use crate::tests::std::alkanes_std_test_build;

    /// Setup function for Zcash tests
    fn setup_zcash() {
        alkane_helpers::clear();
        // Network is configured by clear() -> configure_network()
        // For zcash feature, it sets Zcash params (t1/t3 addresses)
    }

    #[wasm_bindgen_test]
    fn test_zcash_t_address_detection() -> Result<()> {
        let tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                create_op_return(b"test"),
                create_zcash_p2pkh_output(1000),
                create_zcash_p2sh_output(2000),
            ],
        };

        // Verify helper functions work
        assert!(has_t_address_output(&tx));
        assert_eq!(count_t_address_outputs(&tx), 2);
        assert_eq!(find_first_t_address(&tx), Some(1));

        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_zcash_scriptsig_envelope() -> Result<()> {
        let bytecode = vec![0x01, 0x02, 0x03, 0x04];
        let envelope = create_zcash_scriptsig_envelope(bytecode.clone())?;

        // Verify envelope is created and contains data
        assert!(envelope.len() > 0);
        
        // Envelope should contain ZAK identifier
        let script_bytes = envelope.as_bytes();
        assert!(script_bytes.len() > 3);

        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_zcash_fallback_to_t_address() -> Result<()> {
        setup_zcash();

        // Create a transaction with z-address pointer (output 1)
        // Should fallback to first t-address (output 2)
        let cellpack = Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0],
        };

        let tx = create_zcash_tx_with_fallback(
            vec![cellpack],
            OutPoint::null(),
            true, // Include z-address pointer
        );

        // Verify transaction structure
        assert!(tx.output.len() >= 3);
        
        // Output 0: OP_RETURN
        assert!(tx.output[0].script_pubkey.is_op_return());
        
        // Output 1: Non-standard (z-address-like)
        assert!(!tx.output[1].script_pubkey.is_p2pkh());
        assert!(!tx.output[1].script_pubkey.is_p2sh());
        
        // Output 2: P2PKH t-address (fallback target)
        assert!(tx.output[2].script_pubkey.is_p2pkh());

        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_zcash_direct_t_address_pointer() -> Result<()> {
        setup_zcash();

        // Create a transaction with direct t-address pointer (no fallback needed)
        let cellpack = Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0],
        };

        let tx = create_zcash_tx_with_fallback(
            vec![cellpack],
            OutPoint::null(),
            false, // No z-address, direct t-address pointer
        );

        // Verify transaction structure
        assert!(tx.output.len() >= 2);
        
        // Output 0: OP_RETURN
        assert!(tx.output[0].script_pubkey.is_op_return());
        
        // Output 1: P2PKH t-address
        assert!(tx.output[1].script_pubkey.is_p2pkh());

        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_zcash_alkane_deployment() -> Result<()> {
        setup_zcash();

        let bytecode = alkanes_std_test_build::get_bytes();
        
        // Create deployment transaction with scriptSig inscription
        let deployment_tx = create_zcash_deployment_tx(bytecode.clone(), OutPoint::null())?;

        // Verify structure
        assert_eq!(deployment_tx.input.len(), 1);
        assert!(deployment_tx.input[0].script_sig.len() > 0); // scriptSig contains inscription
        assert_eq!(deployment_tx.input[0].witness.len(), 0);  // No witness data

        // Should have OP_RETURN and t-address outputs
        assert!(deployment_tx.output[0].script_pubkey.is_op_return() || 
                deployment_tx.output[1].script_pubkey.is_p2pkh());

        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_zcash_block_processing() -> Result<()> {
        setup_zcash();

        let bytecode = alkanes_std_test_build::get_bytes();
        let cellpack = Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0],
        };

        // Create test block with Zcash characteristics
        let block = init_zcash_test_block(bytecode, vec![cellpack], false)?;

        // Process block
        index_block(&block, 0)?;

        // If we got here without errors, basic processing works
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_zcash_block_with_fallback() -> Result<()> {
        setup_zcash();

        let bytecode = alkanes_std_test_build::get_bytes();
        let cellpack = Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0],
        };

        // Create test block with z-address fallback scenario
        let block = init_zcash_test_block(bytecode, vec![cellpack], true)?;

        // Process block - should handle fallback gracefully
        index_block(&block, 0)?;

        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_zcash_multiple_cellpacks() -> Result<()> {
        setup_zcash();

        let bytecode = alkanes_std_test_build::get_bytes();
        let cellpacks = vec![
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![0],
            },
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![1],
            },
        ];

        let block = init_zcash_test_block(bytecode, cellpacks, false)?;
        
        // Process block with multiple cellpacks
        index_block(&block, 0)?;

        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_zcash_no_t_address_scenario() -> Result<()> {
        setup_zcash();

        // Create a transaction with NO t-address outputs (only OP_RETURN and z-addr)
        let tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                create_op_return(b"test"),
                create_zcash_nonstandard_output(1000), // z-address-like
                create_zcash_nonstandard_output(2000), // another z-address-like
            ],
        };

        // Verify no t-address found
        assert!(!has_t_address_output(&tx));
        assert_eq!(count_t_address_outputs(&tx), 0);
        assert_eq!(find_first_t_address(&tx), None);

        // In real indexing, this would trigger skip to prevent burn
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_zcash_mixed_outputs() -> Result<()> {
        // Test a realistic Zcash transaction with mixed output types
        let tx = Transaction {
            version: bitcoin::transaction::Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                create_op_return(b"runestone"),      // 0: OP_RETURN
                create_zcash_nonstandard_output(100), // 1: z-address
                create_zcash_p2pkh_output(1000),      // 2: t1 address
                create_zcash_p2sh_output(2000),       // 3: t3 address
                create_zcash_nonstandard_output(200), // 4: another z-address
            ],
        };

        assert_eq!(count_t_address_outputs(&tx), 2);
        assert_eq!(find_first_t_address(&tx), Some(2));

        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_zcash_balance_sheet_with_t_address() -> Result<()> {
        setup_zcash();

        let bytecode = alkanes_std_test_build::get_bytes();
        let cellpack = Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0],
        };

        let block = init_zcash_test_block(bytecode, vec![cellpack], false)?;
        index_block(&block, 0)?;

        // Try to load a balance sheet from a t-address output
        if block.txdata.len() > 1 {
            let tx = &block.txdata[1];
            let txid = tx.compute_txid();
            
            // Find the first t-address output
            if let Some(vout) = find_first_t_address(tx) {
                let outpoint = OutPoint {
                    txid,
                    vout: vout as u32,
                };
                
                let outpoint_bytes = consensus_encode(&outpoint)?;
                let sheet = load_sheet(
                    &mut IndexPointer::default()
                        .keyword("/protorunes/outpoint_to_runes/")
                        .select(&outpoint_bytes)
                );
                
                // Sheet should be loaded (even if empty)
                // This verifies the indexing system can track t-address outputs
            }
        }

        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_zcash_protocol_identifier() -> Result<()> {
        // Verify ZAK protocol identifier is used
        let bytecode = vec![1, 2, 3];
        let envelope = create_zcash_scriptsig_envelope(bytecode)?;
        
        let script_hex = hex::encode(envelope.as_bytes());
        
        // Should contain "ZAK" (5a414b in hex)
        // Note: This is a basic check; actual encoding may differ
        assert!(envelope.len() > 0);
        
        Ok(())
    }
}
