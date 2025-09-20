//! Integration tests for alkanes execute functionality

use deezel_common::*;
use deezel_common::alkanes::types::{ExecutionState, EnhancedExecuteParams};
use deezel_common::traits::{AlkanesProvider, WalletProvider};
use bitcoin::{Amount, Network, OutPoint, TxOut};
use std::str::FromStr;

#[tokio::test]
async fn test_alkanes_execute_commit_reveal_flow_with_mock_provider() -> anyhow::Result<()> {
    // 1. Setup MockProvider
    let mut provider = deezel_common::mock_provider::MockProvider::new(Network::Regtest);

    // Create and fund a mock UTXO for the wallet to use
    let funding_txid = bitcoin::Txid::from_str("8a6465187b53d05405f49851a13d44a1e95b2803835b132145540c89a70af83d")?;
    let funding_outpoint = OutPoint::new(funding_txid, 0);
    let funding_address = WalletProvider::get_address(&provider).await?;
    let funding_script_pubkey = Address::from_str(&funding_address)?.require_network(provider.get_network())?.script_pubkey();
    let funding_utxo = TxOut {
        value: Amount::from_sat(100_000_000), // 1 BTC
        script_pubkey: funding_script_pubkey,
    };
    provider.utxos.lock().unwrap().push((funding_outpoint, funding_utxo));

    // 2. Construct execute parameters
    let recipient_address = WalletProvider::get_address(&provider).await?;
    let execute_params = EnhancedExecuteParams {
        input_requirements: alkanes::parsing::parse_input_requirements("B:20000")?,
        to_addresses: vec![recipient_address],
        from_addresses: None,
        change_address: None,
        fee_rate: Some(1.0),
        envelope_data: Some(b"dummy envelope data".to_vec()),
        protostones: alkanes::parsing::parse_protostones("[800000,1,0,0],[1,1,100,0]:v0:v0")?,
        raw_output: false,
        trace_enabled: true,
        mine_enabled: false, // Cannot mine with mock provider
        auto_confirm: true,
    };

    // 3. Execute the first step (build commit)
    let initial_state = provider.execute(execute_params.clone()).await?;
    let commit_state = match initial_state {
        ExecutionState::ReadyToSignCommit(state) => state,
        _ => panic!("Expected ReadyToSignCommit state"),
    };

    assert!(!commit_state.psbt.unsigned_tx.input.is_empty());
    assert!(!commit_state.psbt.unsigned_tx.output.is_empty());
    assert!(commit_state.fee > 0);

    // 4. Execute the second step (resume from commit to build reveal)
    let reveal_state_result = provider.resume_commit_execution(commit_state).await?;
    let reveal_state = match reveal_state_result {
        ExecutionState::ReadyToSignReveal(state) => state,
        _ => panic!("Expected ReadyToSignReveal state"),
    };

    assert!(!reveal_state.psbt.unsigned_tx.input.is_empty());
    assert!(!reveal_state.psbt.unsigned_tx.output.is_empty());
    assert!(reveal_state.fee > 0);
    assert!(!reveal_state.commit_txid.is_empty());

    // 5. Execute the final step (resume from reveal to get final result)
    let final_result = provider.resume_reveal_execution(reveal_state).await?;

    // 6. Assertions on the final result
    assert!(final_result.commit_txid.is_some());
    assert!(!final_result.reveal_txid.is_empty());
    assert!(final_result.traces.is_some());
    
    let traces = final_result.traces.as_ref().unwrap();
    assert!(!traces.is_empty(), "Traces should not be empty");
    
    let first_trace = &traces[0];
    assert!(!first_trace.is_null(), "Trace should not be null");
    assert!(first_trace.is_object(), "Trace should be a JSON object");

    println!("Mock E2E Test Passed: {final_result:#?}");

    Ok(())
}