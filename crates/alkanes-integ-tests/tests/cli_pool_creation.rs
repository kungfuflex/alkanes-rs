//! CLI Integration: AMM pool creation through the full pipeline.
//!
//! 1. Deploy AMM infrastructure via block_builder
//! 2. Mint tokens
//! 3. Create pool via EnhancedAlkanesExecutor
//! 4. Index and verify

use alkanes_integ_tests::block_builder::{create_block_with_deploys, last_tx_outpoint, DeployPair};
use alkanes_integ_tests::cli_bridge::CliBridge;
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_cli_common::alkanes::types::{
    EnhancedExecuteParams, InputRequirement, OrdinalsStrategy, OutputTarget, ProtostoneSpec,
};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::OutPoint;
use std::str::FromStr;

#[test]
fn test_cli_mint_and_query() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Block 4: deploy test contract and mint tokens
    let deploy_block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![30, 2, 1, 5000],
            },
        )],
    );
    runtime.index_block(&deploy_block, 4)?;

    // Verify minted tokens via view function
    let outpoint = last_tx_outpoint(&deploy_block);
    let bal = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 4)?;
    assert_eq!(bal, 5000, "should have 5000 tokens");

    // Now use CLI bridge to execute a call against the minted tokens
    let mut bridge = CliBridge::new();
    let cli_address = bridge.address();
    let mock_script = bitcoin::Address::from_str(&cli_address)
        .unwrap()
        .require_network(bitcoin::Network::Regtest)
        .unwrap()
        .script_pubkey();

    // Add the token-bearing UTXO with mock provider's address
    bridge.add_utxo(
        outpoint,
        bitcoin::TxOut {
            value: deploy_block.txdata.last().unwrap().output[0].value,
            script_pubkey: mock_script.clone(),
        },
    );
    bridge.set_alkane_balance(&outpoint, 2, 1, 5000);

    // Add BTC for fees
    bridge.add_utxo(
        OutPoint::new(bitcoin::Txid::from_slice(&[0xcc; 32]).unwrap(), 0),
        bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(10_000_000),
            script_pubkey: mock_script,
        },
    );

    // Execute: forward all 5000 tokens via opcode 5
    let params = EnhancedExecuteParams {
        fee_rate: Some(1.0),
        to_addresses: vec![cli_address.clone()],
        from_addresses: Some(vec![cli_address.clone()]),
        change_address: Some(cli_address.clone()),
        alkanes_change_address: Some(cli_address.clone()),
        input_requirements: vec![
            InputRequirement::Alkanes { block: 2, tx: 1, amount: 5000 },
        ],
        protostones: vec![ProtostoneSpec {
            cellpack: Some(Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![5], // forward
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

    let result = bridge.execute_and_index(params, &runtime, 5);
    match result {
        Ok(block5) => {
            let out = last_tx_outpoint(&block5);
            let bal = query::get_alkane_balance(&runtime, &out, 2, 1, 5)?;
            println!("CLI mint+forward: 2:1 balance at output = {}", bal);
            assert!(bal > 0, "tokens should arrive at output via CLI pipeline");
            println!("CLI mint_and_query test PASSED — {} tokens forwarded", bal);
        }
        Err(e) => {
            println!("CLI execute failed: {:#}", e);
            return Err(e);
        }
    }

    Ok(())
}
