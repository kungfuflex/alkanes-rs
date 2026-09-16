//! AMM Pool Creation Test — exercises the full factory → pool extcall chain.
//!
//! This is the operation that fails on the live chain with "fill whole buffer".
//! The factory contract does an extcall to instantiate a pool, which means
//! the inner wasmi interpreter loads the pool WASM. If the stored bytecode
//! is corrupted or the wrong bytes are loaded, wasmi produces "fill whole buffer".

use alkanes_integ_tests::block_builder::{
    create_block_with_deploys, create_block_with_protostones, last_tx_outpoint, DeployPair,
};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
use protorune_support::protostone::{Protostone, ProtostoneEdict};
use protorune_support::balance_sheet::ProtoruneRuneId;

const AUTH_TOKEN_FACTORY_ID: u128 = 0xffed;
const BEACON_ID: u128 = 0xbeac0;

fn txin_from(outpoint: OutPoint) -> TxIn {
    TxIn {
        previous_output: outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    }
}

/// Deploy full AMM infrastructure and create a pool.
#[test]
fn test_amm_deploy_and_create_pool() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    // Blocks 0-3: empty (genesis maturity)
    runtime.mine_empty_blocks(0, 4)?;

    // Block 4: deploy auth token factory
    let auth_block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::AUTH_TOKEN,
            Cellpack {
                target: AlkaneId { block: 3, tx: AUTH_TOKEN_FACTORY_ID },
                inputs: vec![100],
            },
        )],
    );
    runtime.index_block(&auth_block, 4)?;
    println!("Auth token factory deployed");

    // Block 5: deploy pool template
    let pool_block = create_block_with_deploys(
        5,
        vec![DeployPair::new(
            fixtures::POOL,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![0],
            },
        )],
    );
    runtime.index_block(&pool_block, 5)?;
    let pool_template_id = AlkaneId { block: 7, tx: 1 };
    println!("Pool template deployed at {:?}", pool_template_id);

    // Block 6: deploy upgradeable beacon pointing to pool template
    let beacon_block = create_block_with_deploys(
        6,
        vec![DeployPair::new(
            fixtures::UPGRADEABLE_BEACON,
            Cellpack {
                target: AlkaneId { block: 3, tx: BEACON_ID },
                inputs: vec![0x7fff, pool_template_id.block, pool_template_id.tx, 1],
            },
        )],
    );
    runtime.index_block(&beacon_block, 6)?;
    println!("Upgradeable beacon deployed");

    // Block 7: deploy factory logic
    let factory_block = create_block_with_deploys(
        7,
        vec![DeployPair::new(
            fixtures::FACTORY,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![0],
            },
        )],
    );
    runtime.index_block(&factory_block, 7)?;
    let factory_logic_id = AlkaneId { block: 9, tx: 1 };
    println!("Factory logic deployed at {:?}", factory_logic_id);

    // Block 8: deploy factory proxy (upgradeable) + initialize
    let beacon_proxy_id = AlkaneId { block: 8, tx: BEACON_ID };
    let factory_proxy_block = create_block_with_deploys(
        8,
        vec![
            DeployPair::new(
                fixtures::UPGRADEABLE,
                Cellpack {
                    target: AlkaneId { block: 1, tx: 0 },
                    inputs: vec![0x7fff, factory_logic_id.block, factory_logic_id.tx, 1],
                },
            ),
            // Initialize factory: set beacon for pool creation
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 10, tx: 1 },
                inputs: vec![
                    0, // InitFactory opcode
                    beacon_proxy_id.block,
                    beacon_proxy_id.tx,
                    8, // beacon block
                    BEACON_ID, // beacon tx
                ],
            }),
        ],
    );
    runtime.index_block(&factory_proxy_block, 8)?;
    let factory_id = AlkaneId { block: 10, tx: 1 };
    println!("Factory proxy deployed at {:?}", factory_id);

    // Block 9: deploy test contract and mint tokens for pool
    let mint_block = create_block_with_deploys(
        9,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![30, 2, 1, 1_000_000], // mint 1M tokens
            },
        )],
    );
    runtime.index_block(&mint_block, 9)?;
    let mint_outpoint = last_tx_outpoint(&mint_block);
    let token_bal = query::get_alkane_balance(&runtime, &mint_outpoint, 2, 1, 9)?;
    println!("Minted {} tokens of 2:1", token_bal);

    // Also mint diesel for the second token in the pair
    let diesel_block = create_block_with_protostones(
        10,
        vec![txin_from(OutPoint::null())],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 2, tx: 0 },
                inputs: vec![77],
            }.encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        }],
    );
    runtime.index_block(&diesel_block, 10)?;
    let diesel_outpoint = last_tx_outpoint(&diesel_block);
    let diesel_bal = query::get_alkane_balance(&runtime, &diesel_outpoint, 2, 0, 10)?;
    println!("Minted {} DIESEL (2:0)", diesel_bal);

    // Block 11: CREATE POOL via factory
    // This is the operation that fails on the live chain.
    // The factory does an extcall to instantiate a pool contract via the beacon.
    // If the inner wasmi can't parse the pool WASM, it errors with "fill whole buffer".
    println!("\n=== CREATING POOL (the critical operation) ===");
    let token_a = AlkaneId { block: 2, tx: 0 }; // DIESEL
    let token_b = AlkaneId { block: 2, tx: 1 }; // test token
    let amount_a: u128 = diesel_bal / 2;
    let amount_b: u128 = (token_bal / 2) as u128;

    // Two-protostone: auto-change routes tokens to factory call
    let create_pool_block = create_block_with_protostones(
        11,
        vec![
            txin_from(diesel_outpoint),
            txin_from(mint_outpoint),
        ],
        vec![],
        vec![
            // p0: route tokens to p1 via edicts
            Protostone {
                message: vec![],
                protocol_tag: 1,
                burn: None,
                from: None,
                pointer: Some(3), // p1 (2 outputs + 1 + 0 for shadow)
                refund: Some(0),
                edicts: vec![
                    ProtostoneEdict {
                        id: ProtoruneRuneId { block: token_a.block, tx: token_a.tx },
                        amount: amount_a,
                        output: 3, // to p1
                    },
                    ProtostoneEdict {
                        id: ProtoruneRuneId { block: token_b.block, tx: token_b.tx },
                        amount: amount_b,
                        output: 3, // to p1
                    },
                ],
            },
            // p1: factory CreateNewPool call
            Protostone {
                message: Cellpack {
                    target: factory_id.clone(),
                    inputs: vec![
                        1, // CreateNewPool opcode
                        2, // num_tokens
                        token_a.block, token_a.tx,
                        token_b.block, token_b.tx,
                        amount_a,
                        amount_b,
                    ],
                }.encipher(),
                protocol_tag: 1,
                burn: None,
                from: None,
                pointer: Some(0),
                refund: Some(0),
                edicts: vec![],
            },
        ],
    );

    let pool_result = runtime.index_block(&create_pool_block, 11);
    match &pool_result {
        Ok(()) => {
            println!("Pool creation SUCCEEDED — block indexed");
            let outpoint = last_tx_outpoint(&create_pool_block);
            let balances = query::get_balance_for_outpoint(&runtime, &outpoint, 11)?;
            println!("Balances at pool creation output: {:?}", balances);
        }
        Err(e) => {
            let err_str = format!("{:#}", e);
            if err_str.contains("fill whole buffer") {
                println!("BUG REPRODUCED: 'fill whole buffer' error during pool creation!");
                println!("Error: {}", err_str);
                println!("This confirms the inner wasmi can't parse the pool WASM bytecode.");
            } else {
                println!("Pool creation FAILED with different error: {}", err_str);
            }
        }
    }

    // Don't fail the test — we want to observe the result either way
    Ok(())
}
