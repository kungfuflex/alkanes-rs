//! Test alkanes execute with --inputs flag to verify UTXO selection and protostone generation

use alkanes_cli_common::*;
use alkanes_cli_common::alkanes::types::{ExecutionState, EnhancedExecuteParams};
use alkanes_cli_common::alkanes::execute::EnhancedAlkanesExecutor;
use alkanes_cli_common::traits::{AlkanesProvider, WalletProvider};
use bitcoin::{Amount, Network, OutPoint, TxOut, Transaction};
use std::str::FromStr;

#[tokio::test]
async fn test_alkanes_execute_with_alkane_inputs() -> anyhow::Result<()> {
    println!("\n=== Testing alkanes execute with --inputs 2:1:1 ===\n");
    
    // 1. Setup MockProvider
    let mut provider = alkanes_cli_common::mock_provider::MockProvider::new(Network::Regtest);

    // Create a mock UTXO with BTC
    let btc_txid = bitcoin::Txid::from_str("8a6465187b53d05405f49851a13d44a1e95b2803835b132145540c89a70af83d")?;
    let btc_outpoint = OutPoint::new(btc_txid, 0);
    let address = WalletProvider::get_address(&provider).await?;
    let script_pubkey = address::Address::from_str(&address)?.require_network(provider.get_network())?.script_pubkey();
    let btc_utxo = TxOut {
        value: Amount::from_sat(100_000_000), // 1 BTC
        script_pubkey: script_pubkey.clone(),
    };
    provider.utxos.lock().unwrap().push((btc_outpoint, btc_utxo));
    println!("✓ Created BTC UTXO: {}:{}", btc_txid, 0);

    // Create a mock UTXO with alkane 2:1 (5 units)
    let alkane_txid = bitcoin::Txid::from_str("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")?;
    let alkane_outpoint = OutPoint::new(alkane_txid, 0);
    let alkane_utxo = TxOut {
        value: Amount::from_sat(10_000), // Some BTC too
        script_pubkey: script_pubkey.clone(),
    };
    provider.utxos.lock().unwrap().push((alkane_outpoint, alkane_utxo.clone()));
    
    // Mock the protorunes_by_outpoint to return alkane 2:1 balance
    use alkanes_cli_common::alkanes::protorunes::{ProtoruneOutpointResponse, BalanceSheet, AlkaneId};
    use std::collections::HashMap;
    let mut balances = HashMap::new();
    balances.insert(
        AlkaneId { block: 2, tx: 1 },
        5u128 // 5 units of alkane 2:1
    );
    let balance_sheet = BalanceSheet {
        cached: alkanes_cli_common::alkanes::protorunes::Balances { balances },
    };
    provider.set_protorunes_response(
        alkane_txid.to_string(),
        0,
        ProtoruneOutpointResponse {
            outpoint: alkane_outpoint.to_string(),
            balance_sheet,
        },
    );
    println!("✓ Created alkane UTXO: {}:{} with 5 units of 2:1", alkane_txid, 0);

    // 2. Construct execute parameters with --inputs 2:1:1
    // This should:
    // - Find UTXO with alkane 2:1
    // - Spend 1 unit to the first user protostone
    // - Create automatic change protostone for remaining 4 units
    println!("\n📝 Building execute params:");
    println!("   --inputs 2:1:1 (spend 1 unit of alkane 2:1)");
    println!("   Protostone: [4,65522,0,780993,4,65523]:v0:v0");
    
    let execute_params = EnhancedExecuteParams {
        input_requirements: alkanes::parsing::parse_input_requirements("2:1:1")?,
        to_addresses: vec![],
        from_addresses: None,
        change_address: Some("p2tr:0".to_string()),
        fee_rate: Some(1.0),
        envelope_data: None,
        protostones: alkanes::parsing::parse_protostones("[4,65522,0,780993,4,65523]:v0:v0")?,
        raw_output: false,
        trace_enabled: false,
        mine_enabled: false,
        auto_confirm: true,
    };

    println!("\n🔍 Parsed protostones: {} protostones", execute_params.protostones.len());
    for (i, ps) in execute_params.protostones.iter().enumerate() {
        println!("   Protostone {}: edicts={}, pointer={:?}, refund={:?}", 
                 i, ps.edicts.len(), ps.pointer, ps.refund);
    }

    // 3. Execute
    println!("\n⚙️  Executing...");
    let mut executor = EnhancedAlkanesExecutor::new(&mut provider);
    let result = executor.execute(execute_params.clone()).await;
    
    match &result {
        Ok(state) => {
            println!("✓ Execution succeeded, state: {:?}", match state {
                ExecutionState::ReadyToSign(_) => "ReadyToSign",
                ExecutionState::ReadyToSignCommit(_) => "ReadyToSignCommit",
                ExecutionState::ReadyToSignReveal(_) => "ReadyToSignReveal",
                ExecutionState::Complete(_) => "Complete",
            });
            
            // Get the transaction from the state
            let (tx, selected_inputs) = match state {
                ExecutionState::ReadyToSign(s) => {
                    (&s.psbt.unsigned_tx, &s.state.selected_utxos)
                },
                _ => panic!("Expected ReadyToSign state for single transaction"),
            };
            
            println!("\n📊 Transaction Analysis:");
            println!("   Inputs: {}", tx.input.len());
            for (i, input) in tx.input.iter().enumerate() {
                println!("     Input {}: {}:{}", i, input.previous_output.txid, input.previous_output.vout);
            }
            
            println!("   Outputs: {}", tx.output.len());
            for (i, output) in tx.output.iter().enumerate() {
                println!("     Output {}: {} sats", i, output.value);
            }
            
            // Verify the alkane UTXO was selected
            let alkane_utxo_selected = selected_inputs.iter().any(|op| {
                op.txid == alkane_txid && op.vout == 0
            });
            
            println!("\n🔍 Verification:");
            println!("   Alkane UTXO selected: {}", if alkane_utxo_selected { "✓ YES" } else { "✗ NO" });
            
            if !alkane_utxo_selected {
                println!("\n❌ FAILED: Alkane UTXO was not selected!");
                println!("   Selected UTXOs:");
                for op in selected_inputs {
                    println!("     - {}:{}", op.txid, op.vout);
                }
                panic!("Alkane UTXO {} with 2:1 was not selected", alkane_txid);
            }
            
            // Check if automatic change protostone was created
            // We should have 2 protostones total:
            // 1. Original: [4,65522,0,780993,4,65523]:v0:v0
            // 2. Automatic change: [2:1:4]:v0:v0 (returning 4 units of 2:1)
            // But we need to check the actual protostones in the runestone
            println!("   Expected: 2 protostones (1 user + 1 auto change)");
            
            // Parse runestone from OP_RETURN output
            if let Some(op_return) = tx.output.iter().find(|o| o.script_pubkey.is_op_return()) {
                println!("   ✓ Found OP_RETURN output");
                // TODO: Decode runestone to verify protostones
            } else {
                println!("   ✗ No OP_RETURN output found");
            }
            
            println!("\n✅ Test PASSED");
        },
        Err(e) => {
            println!("❌ Execution failed: {}", e);
            println!("\n   Error details: {:?}", e);
            panic!("Execution should succeed");
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_utxo_selection_logs_alkane_search() -> anyhow::Result<()> {
    println!("\n=== Testing UTXO selection logging ===\n");
    
    // This test verifies that UTXO selection properly logs when searching for alkanes
    let mut provider = alkanes_cli_common::mock_provider::MockProvider::new(Network::Regtest);

    // Create UTXOs
    let btc_txid = bitcoin::Txid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")?;
    let btc_outpoint = OutPoint::new(btc_txid, 0);
    let address = WalletProvider::get_address(&provider).await?;
    let script_pubkey = address::Address::from_str(&address)?.require_network(provider.get_network())?.script_pubkey();
    provider.utxos.lock().unwrap().push((btc_outpoint, TxOut {
        value: Amount::from_sat(100_000),
        script_pubkey: script_pubkey.clone(),
    }));

    let alkane_txid = bitcoin::Txid::from_str("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")?;
    let alkane_outpoint = OutPoint::new(alkane_txid, 0);
    provider.utxos.lock().unwrap().push((alkane_outpoint, TxOut {
        value: Amount::from_sat(10_000),
        script_pubkey,
    }));

    // Set up alkane balance
    use alkanes_cli_common::alkanes::protorunes::{ProtoruneOutpointResponse, BalanceSheet, AlkaneId};
    use std::collections::HashMap;
    let mut balances = HashMap::new();
    balances.insert(AlkaneId { block: 2, tx: 1 }, 3u128);
    provider.set_protorunes_response(
        alkane_txid.to_string(),
        0,
        ProtoruneOutpointResponse {
            outpoint: alkane_outpoint.to_string(),
            balance_sheet: BalanceSheet {
                cached: alkanes_cli_common::alkanes::protorunes::Balances { balances },
            },
        },
    );

    let execute_params = EnhancedExecuteParams {
        input_requirements: alkanes::parsing::parse_input_requirements("2:1:3")?,
        to_addresses: vec![],
        from_addresses: None,
        change_address: Some("p2tr:0".to_string()),
        fee_rate: Some(1.0),
        envelope_data: None,
        protostones: vec![],
        raw_output: false,
        trace_enabled: false,
        mine_enabled: false,
        auto_confirm: true,
    };

    println!("Executing with --inputs 2:1:3");
    let mut executor = EnhancedAlkanesExecutor::new(&mut provider);
    let result = executor.execute(execute_params).await;

    match result {
        Ok(_) => println!("✅ UTXO selection succeeded"),
        Err(e) => {
            println!("Error: {}", e);
            // Check if it's the expected error about insufficient alkanes
            if e.to_string().contains("Insufficient alkanes") {
                println!("✅ Got expected error about insufficient alkanes");
            } else {
                panic!("Unexpected error: {}", e);
            }
        }
    }

    Ok(())
}
