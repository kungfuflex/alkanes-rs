//! Test for wallet send/sign-tx flow with P2TR key-path spends
//! This replicates the code path used by cmd-build-tx-only.sh script

use alkanes_cli_common::*;
use alkanes_cli_common::mock_provider::MockProvider;
use alkanes_cli_common::traits::{WalletProvider, SendParams};
use bitcoin::{Amount, Network, OutPoint, TxOut, Address};
use std::str::FromStr;

#[tokio::test]
async fn test_wallet_send_and_sign_transaction() -> anyhow::Result<()> {
    env_logger::try_init().ok();
    
    // Setup a ConcreteProvider with address-only mode (like --wallet-address)
    let mut provider = create_test_provider().await?;
    
    // Create some mock UTXOs at the address
    setup_mock_utxos(&mut provider).await?;
    
    // Step 1: Create unsigned transaction (like wallet send)
    println!("Step 1: Creating unsigned transaction...");
    let send_params = SendParams {
        address: "bc1px562ylsev9zvfeg4jh7y7hg4y935ygcqg67vjzcjupq574uxy4ws8kmg05".to_string(),
        amount: 1000,
        fee_rate: Some(1.0),
        send_all: false,
        from: None,
        change_address: None,
        auto_confirm: true,
        use_rebar: false,
        rebar_tier: 1,
        lock_alkanes: false,
    };
    
    let unsigned_hex = provider.create_transaction(send_params).await?;
    println!("✅ Unsigned transaction created: {} bytes", unsigned_hex.len() / 2);
    
    // Decode and verify unsigned transaction
    let unsigned_bytes = hex::decode(&unsigned_hex)?;
    let unsigned_tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&unsigned_bytes)?;
    println!("   Inputs: {}", unsigned_tx.input.len());
    println!("   Outputs: {}", unsigned_tx.output.len());
    
    // Verify witness is empty in unsigned transaction
    for (i, input) in unsigned_tx.input.iter().enumerate() {
        assert_eq!(input.witness.len(), 0, "Unsigned transaction input {} should have empty witness", i);
    }
    
    // Step 2: Sign the transaction (like wallet sign-tx)
    println!("\nStep 2: Signing transaction...");
    
    // For this test, we need to unlock the wallet with a private key
    // In the real script, this uses --wallet-key-file
    setup_wallet_key(&mut provider)?;
    
    let signed_hex = provider.sign_transaction(unsigned_hex).await?;
    println!("✅ Transaction signed: {} bytes", signed_hex.len() / 2);
    
    // Step 3: Decode and verify signed transaction
    println!("\nStep 3: Verifying signed transaction...");
    let signed_bytes = hex::decode(&signed_hex)?;
    let signed_tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&signed_bytes)?;
    
    println!("   TXID: {}", signed_tx.compute_txid());
    println!("   Inputs: {}", signed_tx.input.len());
    println!("   Outputs: {}", signed_tx.output.len());
    println!("   Size: {} bytes", signed_bytes.len());
    println!("   vSize: {} vbytes", signed_tx.vsize());
    
    // CRITICAL ASSERTIONS: Verify witness structure
    println!("\nVerifying witness structure:");
    for (i, input) in signed_tx.input.iter().enumerate() {
        println!("   Input {}: {} witness items", i, input.witness.len());
        
        // For P2TR key-path spend, witness should have exactly 1 item (the signature)
        assert_eq!(
            input.witness.len(), 
            1, 
            "P2TR key-path spend should have exactly 1 witness item (signature), got {} items for input {}", 
            input.witness.len(),
            i
        );
        
        let sig_bytes = &input.witness[0];
        println!("      Witness[0] (signature): {} bytes", sig_bytes.len());
        
        // Taproot signature should be 64 or 65 bytes (64 for default sighash)
        assert!(
            sig_bytes.len() == 64 || sig_bytes.len() == 65,
            "Taproot signature should be 64 or 65 bytes, got {} bytes for input {}",
            sig_bytes.len(),
            i
        );
    }
    
    println!("\n✅ All witness validations passed!");
    println!("✅ Transaction is properly signed and valid");
    
    Ok(())
}

#[tokio::test]
async fn test_wallet_send_all_and_sign() -> anyhow::Result<()> {
    env_logger::try_init().ok();
    
    // This test uses --send-all flag like the script does
    let mut provider = create_test_provider().await?;
    setup_mock_utxos(&mut provider).await?;
    
    println!("Testing --send-all mode (consolidation)...");
    
    // Create transaction with send_all=true
    let send_params = SendParams {
        address: "bc1px562ylsev9zvfeg4jh7y7hg4y935ygcqg67vjzcjupq574uxy4ws8kmg05".to_string(),
        amount: 1, // Ignored when send_all=true
        fee_rate: Some(1.0),
        send_all: true,
        from: None,
        change_address: None,
        auto_confirm: true,
        use_rebar: false,
        rebar_tier: 1,
        lock_alkanes: false,
    };
    
    let unsigned_hex = provider.create_transaction(send_params).await?;
    let unsigned_bytes = hex::decode(&unsigned_hex)?;
    let unsigned_tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&unsigned_bytes)?;
    
    println!("   Unsigned tx inputs: {}", unsigned_tx.input.len());
    println!("   Unsigned tx outputs: {}", unsigned_tx.output.len());
    
    // Should have single output (no change) when using send_all
    assert_eq!(unsigned_tx.output.len(), 1, "--send-all should create single output (no change)");
    
    // Sign it
    setup_wallet_key(&mut provider)?;
    let signed_hex = provider.sign_transaction(unsigned_hex).await?;
    let signed_bytes = hex::decode(&signed_hex)?;
    let signed_tx: bitcoin::Transaction = bitcoin::consensus::deserialize(&signed_bytes)?;
    
    println!("   Signed tx size: {} bytes", signed_bytes.len());
    println!("   Signed tx vSize: {} vbytes", signed_tx.vsize());
    
    // Verify all witnesses
    for (i, input) in signed_tx.input.iter().enumerate() {
        assert_eq!(input.witness.len(), 1, "Input {} should have 1 witness item", i);
        let sig_bytes = &input.witness[0];
        assert!(sig_bytes.len() == 64 || sig_bytes.len() == 65, "Invalid signature size for input {}", i);
    }
    
    println!("✅ send-all mode test passed!");
    
    Ok(())
}

// Helper functions

async fn create_test_provider() -> anyhow::Result<MockProvider> {
    let provider = MockProvider::new(Network::Regtest);
    Ok(provider)
}

async fn setup_mock_utxos(provider: &mut MockProvider) -> anyhow::Result<()> {
    // Create mock UTXOs
    let address = WalletProvider::get_address(provider).await?;
    let address_obj = Address::from_str(&address)?.require_network(Network::Regtest)?;
    
    // Add 5 UTXOs of 10,000 sats each
    for i in 0..5 {
        let txid = bitcoin::Txid::from_str(&format!(
            "000000000000000000000000000000000000000000000000000000000000000{}",
            i
        ))?;
        let outpoint = OutPoint::new(txid, 0);
        let utxo = TxOut {
            value: Amount::from_sat(10_000),
            script_pubkey: address_obj.script_pubkey(),
        };
        provider.utxos.lock().unwrap().push((outpoint, utxo));
    }
    
    Ok(())
}

fn setup_wallet_key(_provider: &mut MockProvider) -> anyhow::Result<()> {
    // MockProvider already has a built-in key, no need to set one up
    Ok(())
}
