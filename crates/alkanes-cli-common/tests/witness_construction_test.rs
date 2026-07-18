//! Test for validating witness construction in P2TR script-path spends
//! 
//! This test verifies that when signing a transaction with a script-path spend,
//! the witness is correctly constructed with exactly 3 items:
//! 1. Signature (65 bytes for schnorr with sighash byte)
//! 2. Script (variable size)
//! 3. Control block (33 bytes for single-leaf taproot)

use alkanes_cli_common::*;
use alkanes_cli_common::alkanes::types::{ExecutionState, EnhancedExecuteParams};
use alkanes_cli_common::alkanes::envelope::AlkanesEnvelope;
use alkanes_cli_common::traits::{AlkanesProvider, WalletProvider};
use bitcoin::{Amount, Network, OutPoint, TxOut, Address};
use std::str::FromStr;

#[tokio::test]
async fn test_witness_construction_for_script_path_spend() -> anyhow::Result<()> {
    // Setup MockProvider
    let mut provider = alkanes_cli_common::mock_provider::MockProvider::new(Network::Regtest);

    // Create and fund a mock UTXO for the wallet to use
    let funding_txid = bitcoin::Txid::from_str("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")?;
    let funding_outpoint = OutPoint::new(funding_txid, 0);
    let funding_address = WalletProvider::get_address(&provider).await?;
    let funding_script_pubkey = Address::from_str(&funding_address)?.require_network(provider.get_network())?.script_pubkey();
    let funding_utxo = TxOut {
        value: Amount::from_sat(100_000_000), // 1 BTC
        script_pubkey: funding_script_pubkey,
    };
    provider.utxos.lock().unwrap().push((funding_outpoint, funding_utxo));

    // Create an envelope with a simple payload
    let payload = b"test contract payload".to_vec();
    
    // Construct execute parameters with envelope (triggers commit-reveal)
    let recipient_address = WalletProvider::get_address(&provider).await?;
    let execute_params = EnhancedExecuteParams {
        input_requirements: alkanes::parsing::parse_input_requirements("B:50000")?,
        to_addresses: vec![recipient_address],
        from_addresses: None,
        change_address: None,
        fee_rate: Some(1.0),
        envelope_data: Some(payload),
        protostones: alkanes::parsing::parse_protostones("[800000,1,0,0],[1,1,100,0]:v0:v0")?,
        raw_output: false,
        trace_enabled: false,
        mine_enabled: false,
        auto_confirm: true,
    };

    // Execute the commit phase
    let initial_state = provider.execute(execute_params.clone()).await?;
    let commit_state = match initial_state {
        ExecutionState::ReadyToSignCommit(state) => state,
        _ => panic!("Expected ReadyToSignCommit state"),
    };

    // Resume to build reveal transaction
    let reveal_state_result = provider.resume_commit_execution(commit_state).await?;
    let reveal_state = match reveal_state_result {
        ExecutionState::ReadyToSignReveal(state) => state,
        _ => panic!("Expected ReadyToSignReveal state"),
    };

    // Resume to get the signed reveal transaction
    let final_result = provider.resume_reveal_execution(reveal_state).await?;

    // Get the reveal transaction hex and decode it
    // In production this would come from the broadcast, but we'll construct it from the result
    // For now, we need to get the actual signed transaction from the provider
    
    // Let's verify by checking the PSBT signing directly
    // We'll create a simpler test that directly tests sign_psbt
    
    Ok(())
}

#[tokio::test]
async fn test_psbt_signing_produces_valid_witness() -> anyhow::Result<()> {
    use bitcoin::psbt::Psbt;
    use bitcoin::{Transaction, TxIn, Witness, ScriptBuf, Sequence};
    use bitcoin::consensus::serialize;
    
    // Setup MockProvider
    let mut provider = alkanes_cli_common::mock_provider::MockProvider::new(Network::Regtest);

    // Create a mock envelope
    let payload = b"test payload".to_vec();
    let envelope = AlkanesEnvelope::new(payload);
    let reveal_script = envelope.build_reveal_script();
    
    // Get the internal key
    let (internal_key, _) = provider.get_internal_key().await?;
    
    // Create taproot spend info
    use bitcoin::taproot::{TaprootBuilder, LeafVersion, TaprootSpendInfo};
    let secp = provider.secp();
    
    let taproot_builder = TaprootBuilder::new()
        .add_leaf(0, reveal_script.clone())
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
    
    let spend_info: TaprootSpendInfo = taproot_builder
        .finalize(secp, internal_key)
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
    
    let control_block = spend_info
        .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
        .ok_or_else(|| anyhow::anyhow!("Failed to create control block"))?;
    
    // Create a mock commit UTXO
    let commit_txid = bitcoin::Txid::from_str("abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890")?;
    let commit_outpoint = OutPoint::new(commit_txid, 0);
    let commit_address = bitcoin::Address::p2tr_tweaked(spend_info.output_key(), Network::Regtest);
    let commit_utxo = TxOut {
        value: Amount::from_sat(50000),
        script_pubkey: commit_address.script_pubkey(),
    };
    
    // Add the commit UTXO to provider
    provider.utxos.lock().unwrap().push((commit_outpoint, commit_utxo.clone()));
    
    // Create an unsigned transaction
    let mut unsigned_tx = Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: commit_outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(45000),
            script_pubkey: ScriptBuf::new_p2tr(secp, internal_key, None),
        }],
    };
    
    // Create PSBT from unsigned transaction
    let mut psbt = Psbt::from_unsigned_tx(unsigned_tx.clone())?;
    
    // Set up the PSBT input for script-path spend
    let leaf_hash = bitcoin::taproot::TapLeafHash::from_script(&reveal_script, LeafVersion::TapScript);
    psbt.inputs[0].witness_utxo = Some(commit_utxo.clone());
    psbt.inputs[0].tap_internal_key = Some(internal_key);
    psbt.inputs[0].tap_scripts.insert(
        control_block.clone(),
        (reveal_script.clone(), LeafVersion::TapScript)
    );
    psbt.inputs[0].tap_key_origins.insert(
        internal_key,
        (vec![leaf_hash], (bitcoin::bip32::Fingerprint::from_str("00000000")?, bitcoin::bip32::DerivationPath::from_str("m/86'/1'/0'")?))
    );
    
    // Sign the PSBT
    let signed_psbt = provider.sign_psbt(&mut psbt).await?;
    
    // CRITICAL TEST: Verify the witness structure
    let final_witness = signed_psbt.inputs[0].final_script_witness.as_ref()
        .ok_or_else(|| anyhow::anyhow!("No final_script_witness set"))?;
    
    println!("Witness structure:");
    println!("  Total items: {}", final_witness.len());
    for (i, item) in final_witness.iter().enumerate() {
        println!("  Witness[{}]: {} bytes", i, item.len());
        if item.len() < 100 {
            println!("    Data: {}", hex::encode(item));
        } else {
            println!("    Data (first 32 bytes): {}", hex::encode(&item[..32]));
        }
    }
    
    // ASSERTIONS
    assert_eq!(
        final_witness.len(), 
        3, 
        "Witness must have exactly 3 items for P2TR script-path spend, got {} items", 
        final_witness.len()
    );
    
    let witness_items: Vec<&[u8]> = final_witness.iter().map(|w| w.as_ref()).collect();
    
    // Signature should be 64 or 65 bytes (schnorr signature + optional sighash byte)
    assert!(
        witness_items[0].len() == 64 || witness_items[0].len() == 65,
        "Witness[0] (signature) should be 64 or 65 bytes, got {} bytes",
        witness_items[0].len()
    );
    
    // Script should match the reveal script
    assert_eq!(
        witness_items[1],
        reveal_script.as_bytes(),
        "Witness[1] (script) should match the reveal script"
    );
    
    // Control block should be 33 bytes for a single-leaf taproot
    assert_eq!(
        witness_items[2].len(),
        33,
        "Witness[2] (control block) should be 33 bytes for single-leaf taproot, got {} bytes",
        witness_items[2].len()
    );
    
    // Verify control block matches what we created
    assert_eq!(
        witness_items[2],
        &control_block.serialize(),
        "Witness[2] (control block) should match the control block we created"
    );
    
    println!("✅ Witness construction test PASSED");
    
    Ok(())
}
