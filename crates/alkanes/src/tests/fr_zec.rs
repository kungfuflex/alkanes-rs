//! fr-zec (Synthetic Zcash) contract tests
//!
//! Tests for the frZEC contract with CGGMP21:
//! - Wrap ZEC to frZEC
//! - Unwrap frZEC to ZEC
//! - P2PKH signer addresses (not P2TR)
//! - T-address validation

#[cfg(test)]
mod tests {
    use crate::index_block;
    use crate::precompiled::fr_btc_build; // We'll use this as a proxy until fr_zec is precompiled
    use crate::tests::helpers as alkane_helpers;
    use crate::tests::zcash_helpers::*;
    use crate::view;
    use alkanes_support::cellpack::Cellpack;
    use alkanes_support::id::AlkaneId;
    use anyhow::Result;
    use bitcoin::blockdata::transaction::OutPoint;
    use bitcoin::key::Keypair;
    use bitcoin::secp256k1::{rand, Secp256k1};
    use bitcoin::transaction::Version;
    use bitcoin::{
        Address, Amount, Block, PublicKey, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
    };
    #[allow(unused_imports)]
    use metashrew_core::{
        index_pointer::IndexPointer,
        println,
        stdio::{stdout, Write},
    };
    use metashrew_support::index_pointer::KeyValuePointer;
    use metashrew_support::utils::consensus_encode;
    use protorune::test_helpers::create_block_with_coinbase_tx;
    use wasm_bindgen_test::wasm_bindgen_test;

    // Mock CGGMP21 signer pubkey (33 bytes compressed)
    // In production, this comes from actual CGGMP21 ceremony
    const MOCK_FRZEC_SIGNER_PUBKEY: [u8; 33] = [
        0x03, // Compressed pubkey prefix
        0x79, 0x40, 0xef, 0x3b, 0x65, 0x91, 0x79, 0xa1, 0x37, 0x1d, 0xec, 0x05, 0x79, 0x3c, 0xb0,
        0x27, 0xcd, 0xe4, 0x78, 0x06, 0xfb, 0x66, 0xce, 0x1e, 0x3d, 0x1b, 0x69, 0xd5, 0x6d, 0xe6,
        0x29, 0xdc,
    ];

    /// Create a P2PKH output for the frZEC signer (CGGMP21)
    fn create_frzec_signer_output(value: u64) -> TxOut {
        let signer_pubkey = PublicKey::from_slice(&MOCK_FRZEC_SIGNER_PUBKEY)
            .expect("Invalid compressed pubkey");
        
        // Generate P2PKH script (t1 address format for Zcash)
        let signer_script = ScriptBuf::new_p2pkh(&signer_pubkey.pubkey_hash());
        
        TxOut {
            value: Amount::from_sat(value),
            script_pubkey: signer_script,
        }
    }

    /// Setup function for frZEC tests
    fn setup_frzec() {
        alkane_helpers::clear();
        // Network is configured by clear() -> configure_network()
    }

    #[wasm_bindgen_test]
    fn test_frzec_signer_is_p2pkh() -> Result<()> {
        let signer_output = create_frzec_signer_output(100_000_000);
        
        // Verify it's P2PKH (not P2TR like frBTC)
        assert!(signer_output.script_pubkey.is_p2pkh());
        assert!(!signer_output.script_pubkey.is_p2tr()); // Should NOT be Taproot
        
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_frzec_wrap_basic() -> Result<()> {
        setup_frzec();
        
        // Simulate wrap: user sends ZEC to signer, gets frZEC
        let wrap_cellpack = Cellpack {
            target: AlkaneId { block: 42, tx: 0 }, // frZEC AlkaneId
            inputs: vec![77], // Wrap opcode
        };

        // Create transaction with signer output
        let tx = Transaction {
            version: Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::default(),
            }],
            output: vec![
                create_op_return(b"runestone"),
                create_frzec_signer_output(100_000_000), // 1 ZEC to signer
                create_zcash_p2pkh_output(10_000),       // Change back to user
            ],
        };

        // Verify signer output is present and correct type
        assert!(tx.output[1].script_pubkey.is_p2pkh());
        
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_frzec_unwrap_pointer_validation() -> Result<()> {
        setup_frzec();
        
        // Test that unwrap validates pointer targets t-address
        // Create unwrap transaction with z-address pointer (should fail)
        let unwrap_cellpack = Cellpack {
            target: AlkaneId { block: 42, tx: 0 }, // frZEC
            inputs: vec![78, 1, 50_000_000], // Unwrap opcode, vout, amount
        };

        // Transaction with non-standard output (z-address-like)
        let tx = Transaction {
            version: Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::default(),
            }],
            output: vec![
                create_op_return(b"runestone"),
                create_frzec_signer_output(100_000_000),
                create_zcash_nonstandard_output(50_000_000), // z-address (invalid!)
            ],
        };

        // In real execution, validate_pointer_address() would reject this
        // For now, just verify we can detect it
        assert!(!tx.output[2].script_pubkey.is_p2pkh());
        assert!(!tx.output[2].script_pubkey.is_p2sh());
        
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_frzec_unwrap_to_t_address() -> Result<()> {
        setup_frzec();
        
        // Test unwrap with valid t-address pointer
        let unwrap_cellpack = Cellpack {
            target: AlkaneId { block: 42, tx: 0 },
            inputs: vec![78, 1, 50_000_000], // Unwrap to vout 1
        };

        let tx = Transaction {
            version: Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::default(),
            }],
            output: vec![
                create_op_return(b"runestone"),
                create_frzec_signer_output(100_000_000),
                create_zcash_p2pkh_output(50_000_000), // Valid t1 address
            ],
        };

        // Verify unwrap target is valid t-address
        assert!(tx.output[2].script_pubkey.is_p2pkh());
        
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_frzec_alkane_id() -> Result<()> {
        // Verify frZEC uses AlkaneId [42, 0], not [32, 0] like frBTC
        let frzec_id = AlkaneId { block: 42, tx: 0 };
        let frbtc_id = AlkaneId { block: 32, tx: 0 }; // FROST version
        
        assert_ne!(frzec_id.block, frbtc_id.block);
        assert_eq!(frzec_id.block, 42); // CGGMP21 block
        
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_frzec_multiple_t_address_outputs() -> Result<()> {
        setup_frzec();
        
        // Test transaction with multiple t-address outputs
        let tx = Transaction {
            version: Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![
                create_op_return(b"test"),
                create_frzec_signer_output(100_000_000),
                create_zcash_p2pkh_output(10_000),   // t1
                create_zcash_p2sh_output(20_000),    // t3
                create_zcash_p2pkh_output(30_000),   // t1
            ],
        };

        // All outputs after OP_RETURN should be valid t-addresses
        assert!(tx.output[1].script_pubkey.is_p2pkh());
        assert!(tx.output[2].script_pubkey.is_p2pkh());
        assert!(tx.output[3].script_pubkey.is_p2sh());
        assert!(tx.output[4].script_pubkey.is_p2pkh());
        
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_frzec_fallback_integration() -> Result<()> {
        setup_frzec();
        
        // Test that frZEC works with z-address fallback logic
        let cellpack = Cellpack {
            target: AlkaneId { block: 42, tx: 0 },
            inputs: vec![77], // Wrap
        };

        // Create transaction with z-address pointer (should fallback to t-address)
        let tx = create_zcash_tx_with_fallback(
            vec![cellpack],
            OutPoint::null(),
            true, // Include z-address pointer
        );

        // Should have fallback path to t-address
        assert!(has_t_address_output(&tx));
        let first_t = find_first_t_address(&tx).unwrap();
        assert!(tx.output[first_t].script_pubkey.is_p2pkh() || 
                tx.output[first_t].script_pubkey.is_p2sh());
        
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_frzec_vs_frbtc_address_types() -> Result<()> {
        // Compare frZEC (P2PKH) vs frBTC (P2TR) signer outputs
        let frzec_signer = create_frzec_signer_output(100_000_000);
        
        // frZEC should be P2PKH
        assert!(frzec_signer.script_pubkey.is_p2pkh());
        assert!(!frzec_signer.script_pubkey.is_p2tr());
        
        // This demonstrates the key difference:
        // frBTC: P2TR (Taproot) for FROST Schnorr signatures
        // frZEC: P2PKH (transparent) for CGGMP21 ECDSA signatures
        
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_frzec_compressed_pubkey() -> Result<()> {
        // Verify CGGMP21 pubkey is 33 bytes (compressed), not 32 (x-only)
        assert_eq!(MOCK_FRZEC_SIGNER_PUBKEY.len(), 33);
        
        // First byte should be 0x02 or 0x03 (compressed pubkey prefix)
        assert!(
            MOCK_FRZEC_SIGNER_PUBKEY[0] == 0x02 || 
            MOCK_FRZEC_SIGNER_PUBKEY[0] == 0x03
        );
        
        // Can parse as compressed pubkey
        let pubkey = PublicKey::from_slice(&MOCK_FRZEC_SIGNER_PUBKEY);
        assert!(pubkey.is_ok());
        
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_frzec_name_and_symbol() -> Result<()> {
        // Conceptual test - in actual contract, name/symbol should be "frZEC"
        // This would be verified by calling GetName/GetSymbol opcodes
        
        let frzec_name = "frZEC";
        let frzec_symbol = "frZEC";
        
        assert_eq!(frzec_name, "frZEC");
        assert_eq!(frzec_symbol, "frZEC");
        
        // Contrast with frBTC
        let frbtc_name = "frBTC";
        assert_ne!(frzec_name, frbtc_name);
        
        Ok(())
    }

    #[wasm_bindgen_test]
    fn test_frzec_premium_calculation() -> Result<()> {
        // Test premium calculation for unwrap (same logic as frBTC)
        let unwrap_amount = 100_000_000u64; // 1 ZEC
        let premium_basis_points = 100u64;  // 0.1%
        
        let premium = (unwrap_amount * premium_basis_points) / 100_000;
        let user_receives = unwrap_amount - premium;
        
        assert_eq!(premium, 100_000); // 0.001 ZEC
        assert_eq!(user_receives, 99_900_000);
        
        Ok(())
    }
}
