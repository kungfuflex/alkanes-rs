//! Diagnose the CLI's factory init auth token routing.
//!
//! This test:
//! 1. Deploys the AMM stack via block_builder (known working)
//! 2. Uses the CLI pipeline (EnhancedAlkanesExecutor) to build the factory init tx
//! 3. Inspects the raw transaction for protostone structure
//! 4. Indexes it through the full-stack qubitcoin runtime
//! 5. Verifies whether auth tokens reached the factory

use alkanes_integ_tests::block_builder::{
    create_block_with_deploys, create_block_with_protostones, last_tx_outpoint, DeployPair,
};
use alkanes_integ_tests::cli_bridge::CliBridge;
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::harness::FullStackHarness;
use alkanes_cli_common::alkanes::types::{
    EnhancedExecuteParams, InputRequirement, OrdinalsStrategy, ProtostoneSpec,
};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
use protorune_support::protostone::Protostone;
use std::str::FromStr;

const SLOT_AUTH_TOKEN_FACTORY: u128 = 65517;
const SLOT_BEACON_PROXY: u128 = 780993;
const SLOT_FACTORY_LOGIC: u128 = 65524;
const SLOT_POOL_LOGIC: u128 = 65520;
const SLOT_FACTORY_PROXY: u128 = 65522;
const SLOT_UPGRADEABLE_BEACON: u128 = 65523;

fn txin_from(outpoint: OutPoint) -> TxIn {
    TxIn {
        previous_output: outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    }
}

#[test]
fn test_cli_factory_init_auth_token_routing() -> Result<()> {
    let _ = env_logger::try_init();

    let mut harness = FullStackHarness::new()?;
    harness.mine_empty_blocks(4)?;
    let mut height = harness.height() as u32 + 1;

    // ═══ Deploy AMM stack (known working path) ═══
    println!("=== Deploying AMM stack ===");

    // Auth token factory
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::AUTH_TOKEN,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_AUTH_TOKEN_FACTORY }, inputs: vec![100] },
    )]);
    harness.index_bitcoin_block(&block, height)?;
    height += 1;

    // Beacon proxy
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::BEACON_PROXY,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_BEACON_PROXY }, inputs: vec![36863] },
    )]);
    harness.index_bitcoin_block(&block, height)?;
    height += 1;

    // Factory logic
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::FACTORY,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_FACTORY_LOGIC }, inputs: vec![50] },
    )]);
    harness.index_bitcoin_block(&block, height)?;
    height += 1;

    // Pool logic
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::POOL,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_POOL_LOGIC }, inputs: vec![50] },
    )]);
    harness.index_bitcoin_block(&block, height)?;
    height += 1;

    // Factory proxy (creates auth tokens at output)
    let proxy_block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::UPGRADEABLE,
        Cellpack {
            target: AlkaneId { block: 3, tx: SLOT_FACTORY_PROXY },
            inputs: vec![0x7fff, 4, SLOT_FACTORY_LOGIC, 5],
        },
    )]);
    harness.index_bitcoin_block(&proxy_block, height)?;
    let proxy_outpoint = last_tx_outpoint(&proxy_block);
    println!("Factory proxy deployed, auth tokens at {:?}", proxy_outpoint);
    height += 1;

    // Upgradeable beacon
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::UPGRADEABLE_BEACON,
        Cellpack {
            target: AlkaneId { block: 3, tx: SLOT_UPGRADEABLE_BEACON },
            inputs: vec![0x7fff, 4, SLOT_POOL_LOGIC, 5],
        },
    )]);
    harness.index_bitcoin_block(&block, height)?;
    height += 1;

    // ═══ APPROACH A: Direct protostone (known working) ═══
    println!("\n=== Approach A: Direct protostone factory init ===");
    let direct_init = create_block_with_protostones(
        height,
        vec![txin_from(proxy_outpoint)],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 4, tx: SLOT_FACTORY_PROXY },
                inputs: vec![0, SLOT_BEACON_PROXY, 4, SLOT_UPGRADEABLE_BEACON],
            }.encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        }],
    );

    // Inspect the direct init transaction
    let direct_tx = direct_init.txdata.last().unwrap();
    println!("Direct init tx: {} inputs, {} outputs", direct_tx.input.len(), direct_tx.output.len());
    for (i, out) in direct_tx.output.iter().enumerate() {
        println!("  output[{}]: {} sats, script_len={}, is_op_return={}",
            i, out.value, out.script_pubkey.len(), out.script_pubkey.is_op_return());
    }

    // Serialize the OP_RETURN to see protostones
    let op_return_hex = hex::encode(&direct_tx.output.last().unwrap().script_pubkey.as_bytes());
    println!("  OP_RETURN hex (full): {}", op_return_hex);

    harness.index_bitcoin_block(&direct_init, height)?;
    println!("Direct init indexed at height {}", height);
    height += 1;

    // ═══ APPROACH B: CLI pipeline ═══
    println!("\n=== Approach B: CLI pipeline factory init ===");
    let mut bridge = CliBridge::new();
    let cli_address = bridge.address();

    // Add the proxy output as a UTXO (with mock address for signing)
    let mock_script = bitcoin::Address::from_str(&cli_address)
        .unwrap()
        .require_network(bitcoin::Network::Regtest)
        .unwrap()
        .script_pubkey();

    // The direct init consumed the proxy_outpoint. We need a new one.
    // For this test, we redeploy the factory proxy to get fresh auth tokens.
    let proxy_block2 = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::UPGRADEABLE,
        Cellpack {
            target: AlkaneId { block: 3, tx: SLOT_FACTORY_PROXY },
            inputs: vec![0x7fff, 4, SLOT_FACTORY_LOGIC, 5],
        },
    )]);
    harness.index_bitcoin_block(&proxy_block2, height)?;
    let proxy_outpoint2 = last_tx_outpoint(&proxy_block2);
    println!("Fresh factory proxy deployed at {:?}", proxy_outpoint2);
    height += 1;

    // Fund the CLI bridge
    bridge.add_utxo(
        proxy_outpoint2,
        bitcoin::TxOut {
            value: proxy_block2.txdata.last().unwrap().output[0].value,
            script_pubkey: mock_script.clone(),
        },
    );
    bridge.set_alkane_balance(&proxy_outpoint2, 2, 1, 5);

    // Add BTC for fees
    bridge.add_utxo(
        OutPoint::new(bitcoin::Txid::from_slice(&[0xdd; 32]).unwrap(), 0),
        bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(10_000_000),
            script_pubkey: mock_script,
        },
    );

    // Build factory init via CLI
    let params = EnhancedExecuteParams {
        fee_rate: Some(1.0),
        to_addresses: vec![cli_address.clone()],
        from_addresses: Some(vec![cli_address.clone()]),
        change_address: Some(cli_address.clone()),
        alkanes_change_address: Some(cli_address.clone()),
        input_requirements: vec![
            InputRequirement::Alkanes { block: 2, tx: 1, amount: 1 },
        ],
        protostones: vec![ProtostoneSpec {
            cellpack: Some(Cellpack {
                target: AlkaneId { block: 4, tx: SLOT_FACTORY_PROXY },
                inputs: vec![0, SLOT_BEACON_PROXY, 4, SLOT_UPGRADEABLE_BEACON],
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

    let cli_result = bridge.execute_and_extract_tx(params);
    match cli_result {
        Ok(cli_tx) => {
            println!("CLI init tx: {} inputs, {} outputs", cli_tx.input.len(), cli_tx.output.len());
            for (i, out) in cli_tx.output.iter().enumerate() {
                println!("  output[{}]: {} sats, script_len={}, is_op_return={}",
                    i, out.value, out.script_pubkey.len(), out.script_pubkey.is_op_return());
            }

            // Compare OP_RETURN content
            if let Some(op_return) = cli_tx.output.iter().find(|o| o.script_pubkey.is_op_return()) {
                let cli_op_hex = hex::encode(op_return.script_pubkey.as_bytes());
                println!("  CLI OP_RETURN hex (full): {}", cli_op_hex);
                println!("  CLI OP_RETURN length: {} bytes", op_return.script_pubkey.len());
            }

            // Check if CLI tx has 2 protostones (auto-change) or 1
            let num_outputs = cli_tx.output.len();
            println!("\n  CLI tx has {} outputs (2=simple, 3+=auto-change likely)", num_outputs);
            println!("  Direct tx has {} outputs", direct_tx.output.len());

            // Index the CLI tx through full-stack runtime
            let mut cli_block = protorune::test_helpers::create_block_with_coinbase_tx(height);
            cli_block.txdata.push(cli_tx);
            let idx_result = harness.index_bitcoin_block(&cli_block, height);
            match idx_result {
                Ok(()) => println!("  CLI init tx indexed successfully"),
                Err(e) => println!("  CLI init tx indexing failed: {}", e),
            }
        }
        Err(e) => {
            println!("CLI execute_full failed: {:#}", e);
        }
    }

    println!("\n=== Comparison complete ===");
    Ok(())
}
