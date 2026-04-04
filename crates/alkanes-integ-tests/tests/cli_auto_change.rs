//! CLI Integration: Auto-change protostone through the full pipeline.
//!
//! This test uses the REAL alkanes-cli-common EnhancedAlkanesExecutor
//! to build a transaction with auto-change protostones, then feeds
//! the resulting raw transaction through the wasmtime indexer.
//!
//! This closes the loop: CLI code → raw TX → wasmtime indexer → state verification.

use alkanes_integ_tests::block_builder::{create_block_with_deploys, last_tx_outpoint, DeployPair};
use alkanes_integ_tests::cli_bridge::CliBridge;
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_cli_common::alkanes::types::{
    EnhancedExecuteParams, InputRequirement, OrdinalsStrategy, ProtostoneSpec,
};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::OutPoint;
use std::str::FromStr;

#[test]
fn test_cli_auto_change_protostone_through_indexer() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    // Mine 4 empty blocks for genesis setup
    runtime.mine_empty_blocks(0, 4)?;

    // Block 4: deploy test contract and mint 1000 tokens of alkane 2:1
    let deploy_block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![30, 2, 1, 1000], // opcode 30 = arb_mint, 1000 tokens
            },
        )],
    );
    runtime.index_block(&deploy_block, 4)?;

    // Verify tokens were minted at vout:0 of last tx
    let minted_outpoint = last_tx_outpoint(&deploy_block);
    let minted_balance =
        query::get_alkane_balance(&runtime, &minted_outpoint, 2, 1, 4)?;
    println!("Minted balance at {:?}: {}", minted_outpoint, minted_balance);
    assert_eq!(minted_balance, 1000, "should have minted 1000 tokens");

    // Now set up the CLI bridge
    let mut bridge = CliBridge::new();

    // The MockProvider has its own keypair. We need to add UTXOs with the
    // MockProvider's own script_pubkey so it can sign them.
    let cli_address = bridge.address();
    println!("MockProvider address: {}", cli_address);

    // Add the token-bearing UTXO with MockProvider's own address as script_pubkey.
    // The indexer tracks tokens by outpoint, so the script_pubkey doesn't matter
    // for protorune balance — but MockProvider needs it to match its own address
    // for UTXO selection and signing.
    let mock_script = bitcoin::Address::from_str(&cli_address)
        .unwrap()
        .require_network(bitcoin::Network::Regtest)
        .unwrap()
        .script_pubkey();
    let token_txout = bitcoin::TxOut {
        value: deploy_block.txdata.last().unwrap().output[0].value,
        script_pubkey: mock_script.clone(),
    };
    bridge.add_utxo(minted_outpoint, token_txout);
    // Tell MockProvider this UTXO has 1000 tokens of alkane 2:1
    bridge.set_alkane_balance(&minted_outpoint, 2, 1, 1000);

    // Add BTC for fees (a large UTXO from "somewhere")
    let fee_outpoint = OutPoint::new(
        bitcoin::Txid::from_slice(&[0xaa; 32]).unwrap(),
        0,
    );
    let fee_txout = bitcoin::TxOut {
        value: bitcoin::Amount::from_sat(10_000_000),
        script_pubkey: bitcoin::Address::from_str(&cli_address)
            .unwrap()
            .require_network(bitcoin::Network::Regtest)
            .unwrap()
            .script_pubkey(),
    };
    bridge.add_utxo(fee_outpoint, fee_txout);

    // Build params that request only 300 tokens (UTXO has 1000)
    // This should trigger auto-change protostone generation
    let params = EnhancedExecuteParams {
        fee_rate: Some(1.0),
        to_addresses: vec![cli_address.clone()],
        from_addresses: Some(vec![cli_address.clone()]),
        change_address: Some(cli_address.clone()),
        alkanes_change_address: Some(cli_address.clone()),
        input_requirements: vec![
            InputRequirement::Alkanes { block: 2, tx: 1, amount: 300 },
        ],
        protostones: vec![ProtostoneSpec {
            cellpack: Some(Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![5], // opcode 5 = forward tokens
            }),
            edicts: vec![],
            bitcoin_transfer: None,
            pointer: None,
            refund: None,
        }],
        envelope_data: None,
        raw_output: true,
        trace_enabled: false,
        mine_enabled: false,
        auto_confirm: true,
        ordinals_strategy: OrdinalsStrategy::default(),
        mempool_indexer: false,
    };

    // Execute through the real CLI pipeline
    println!("Executing CLI auto-change pipeline...");
    let result = bridge.execute_and_extract_tx(params);

    match &result {
        Ok(tx) => {
            println!(
                "CLI produced transaction with {} inputs, {} outputs",
                tx.input.len(),
                tx.output.len()
            );

            // Wrap in block and index
            let mut block5 =
                protorune::test_helpers::create_block_with_coinbase_tx(5);
            block5.txdata.push(tx.clone());
            runtime.index_block(&block5, 5)?;

            // Verify the indexer processed it correctly
            let result_outpoint = last_tx_outpoint(&block5);
            let result_balance =
                query::get_alkane_balance(&runtime, &result_outpoint, 2, 1, 5)?;
            println!("Result balance at {:?}: {}", result_outpoint, result_balance);

            // The contract (opcode 5) forwards incoming tokens to pointer output.
            // With auto-change, we expect:
            // - 300 tokens routed to contract via auto-change edict
            // - 700 excess tokens routed to change output via auto-change edict
            // Total at some output(s) should be 1000
            assert!(
                result_balance > 0,
                "tokens should arrive at output after CLI auto-change routing"
            );
            println!("CLI auto-change test PASSED — {} tokens at output", result_balance);
        }
        Err(e) => {
            // Print the full error chain for debugging
            println!("CLI execute_full failed: {:#}", e);
            // Fail the test — this is the code path we need to make work
            return Err(anyhow::anyhow!("CLI auto-change pipeline failed: {:#}", e));
        }
    }

    Ok(())
}
