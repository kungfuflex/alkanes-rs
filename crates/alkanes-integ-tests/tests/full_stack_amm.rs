//! Full-stack AMM reproduction using qubitcoin's real WasmIndexerRuntime.
//!
//! This runs the factory deploy + init + pool creation through the SAME
//! wasmtime config, host functions, and RocksDB storage as production.

use alkanes_integ_tests::block_builder::{
    create_block_with_deploys, create_block_with_protostones, last_tx_outpoint, DeployPair,
};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::harness::FullStackHarness;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
use protorune_support::protostone::Protostone;

const SLOT_AUTH_TOKEN_FACTORY: u128 = 65517;
const SLOT_BEACON_PROXY: u128 = 780993;
const SLOT_FACTORY_LOGIC: u128 = 65524;
const SLOT_POOL_LOGIC: u128 = 65520;
const SLOT_FACTORY_PROXY: u128 = 65522;
const SLOT_UPGRADEABLE_BEACON: u128 = 65523;

fn txin_null() -> TxIn {
    TxIn {
        previous_output: OutPoint::null(),
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    }
}

fn txin_from(outpoint: OutPoint) -> TxIn {
    TxIn {
        previous_output: outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    }
}

/// Full-stack AMM deploy + factory init using real qubitcoin runtime.
#[test]
fn test_full_stack_factory_deploy_and_init() -> Result<()> {
    let _ = env_logger::try_init();

    let mut harness = FullStackHarness::new()?;

    // Mine genesis blocks via TestChain (uses qubitcoin-consensus)
    harness.mine_empty_blocks(4)?;
    println!("Mined {} blocks via TestChain", harness.height());

    // Now use bitcoin::Block (from block_builder) indexed through qubitcoin runtime
    let mut height = harness.height() as u32 + 1;

    // Deploy auth token factory
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::AUTH_TOKEN,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_AUTH_TOKEN_FACTORY }, inputs: vec![100] },
    )]);
    harness.index_bitcoin_block(&block, height)?;
    println!("Auth Token Factory deployed (height {})", height);
    height += 1;

    // Deploy beacon proxy
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::BEACON_PROXY,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_BEACON_PROXY }, inputs: vec![36863] },
    )]);
    harness.index_bitcoin_block(&block, height)?;
    println!("Beacon Proxy deployed (height {})", height);
    height += 1;

    // Deploy factory logic
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::FACTORY,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_FACTORY_LOGIC }, inputs: vec![50] },
    )]);
    harness.index_bitcoin_block(&block, height)?;
    println!("Factory Logic deployed (height {})", height);
    height += 1;

    // Deploy pool logic
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::POOL,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_POOL_LOGIC }, inputs: vec![50] },
    )]);
    harness.index_bitcoin_block(&block, height)?;
    println!("Pool Logic deployed (height {})", height);
    height += 1;

    // Deploy factory proxy (upgradeable)
    let factory_proxy_block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::UPGRADEABLE,
        Cellpack {
            target: AlkaneId { block: 3, tx: SLOT_FACTORY_PROXY },
            inputs: vec![0x7fff, 4, SLOT_FACTORY_LOGIC, 5],
        },
    )]);
    harness.index_bitcoin_block(&factory_proxy_block, height)?;
    let factory_proxy_outpoint = last_tx_outpoint(&factory_proxy_block);
    println!("Factory Proxy deployed (height {}), outpoint: {:?}", height, factory_proxy_outpoint);
    height += 1;

    // Deploy upgradeable beacon
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::UPGRADEABLE_BEACON,
        Cellpack {
            target: AlkaneId { block: 3, tx: SLOT_UPGRADEABLE_BEACON },
            inputs: vec![0x7fff, 4, SLOT_POOL_LOGIC, 5],
        },
    )]);
    harness.index_bitcoin_block(&block, height)?;
    println!("Upgradeable Beacon deployed (height {})", height);
    height += 1;

    // Initialize factory — spend the factory proxy outpoint (has auth tokens)
    let init_block = create_block_with_protostones(
        height,
        vec![txin_from(factory_proxy_outpoint)],
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
    harness.index_bitcoin_block(&init_block, height)?;
    println!("Factory init block indexed (height {})", height);

    // Query auth token balance at init output to see if tokens were consumed
    let init_outpoint = last_tx_outpoint(&init_block);

    // Use alkanes view to check balance
    let request = {
        use prost::Message;
        use protorune_support::proto::protorune;
        let req = protorune::OutpointWithProtocol {
            txid: bitcoin::consensus::serialize(&init_outpoint.txid),
            vout: init_outpoint.vout,
            protocol: Some(protorune::Uint128 { lo: 1, hi: 0 }),
        };
        req.encode_to_vec()
    };
    let response = harness.alkanes_view("protorunesbyoutpoint", &request)?;
    println!("Post-init balance query: {} bytes response", response.len());

    // Decode response
    {
        use prost::Message;
        use protorune_support::proto::protorune;
        let resp = protorune::OutpointResponse::decode(response.as_slice())?;
        if let Some(sheet) = &resp.balances {
            for entry in &sheet.entries {
                if let Some(rune) = &entry.rune {
                    if let Some(id) = &rune.rune_id {
                        let bal = entry.balance.as_ref().map(|u| u.lo as u128 | ((u.hi as u128) << 64)).unwrap_or(0);
                        println!("  [{}:{}] = {}", id.height.as_ref().map(|u| u.lo).unwrap_or(0), id.txindex.as_ref().map(|u| u.lo).unwrap_or(0), bal);
                    }
                }
            }
        }
    }

    height += 1;

    // ═══ Mint DIESEL ═══
    println!("\nMinting DIESEL...");
    let diesel_block = create_block_with_protostones(
        height,
        vec![txin_null()],
        vec![],
        vec![Protostone {
            message: Cellpack { target: AlkaneId { block: 2, tx: 0 }, inputs: vec![77] }.encipher(),
            protocol_tag: 1, burn: None, from: None, pointer: Some(0), refund: Some(0), edicts: vec![],
        }],
    );
    harness.index_bitcoin_block(&diesel_block, height)?;
    let diesel_outpoint = last_tx_outpoint(&diesel_block);

    // Check diesel balance
    let dreq = {
        use prost::Message;
        use protorune_support::proto::protorune;
        protorune::OutpointWithProtocol {
            txid: bitcoin::consensus::serialize(&diesel_outpoint.txid),
            vout: diesel_outpoint.vout,
            protocol: Some(protorune::Uint128 { lo: 1, hi: 0 }),
        }.encode_to_vec()
    };
    let dresp = harness.alkanes_view("protorunesbyoutpoint", &dreq)?;
    {
        use prost::Message;
        use protorune_support::proto::protorune;
        let resp = protorune::OutpointResponse::decode(dresp.as_slice())?;
        if let Some(sheet) = &resp.balances {
            for entry in &sheet.entries {
                if let Some(rune) = &entry.rune {
                    if let Some(id) = &rune.rune_id {
                        let bal = entry.balance.as_ref().map(|u| u.lo as u128 | ((u.hi as u128) << 64)).unwrap_or(0);
                        println!("DIESEL balance: [{}:{}] = {}",
                            id.height.as_ref().map(|u| u.lo).unwrap_or(0),
                            id.txindex.as_ref().map(|u| u.lo).unwrap_or(0), bal);
                    }
                }
            }
        }
    }
    height += 1;

    // ═══ CREATE POOL ═══
    println!("\n=== CREATING DIESEL/TEST POOL via full-stack qubitcoin runtime ===");
    let pool_block = create_block_with_protostones(
        height,
        vec![txin_from(diesel_outpoint)],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 4, tx: SLOT_FACTORY_PROXY },
                inputs: vec![
                    1,     // CreateNewPool
                    2,     // num tokens
                    2, 0,  // DIESEL
                    2, 1,  // auth token (as second token — just to test extcall chain)
                    1000,  // amount A
                    1,     // amount B
                ],
            }.encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
        }],
    );

    let pool_result = harness.index_bitcoin_block(&pool_block, height);
    match &pool_result {
        Ok(()) => println!("Pool creation block indexed SUCCESSFULLY via full-stack runtime"),
        Err(e) => {
            let err_str = format!("{:#}", e);
            if err_str.contains("fill whole buffer") {
                println!("*** BUG REPRODUCED ON FULL-STACK: 'fill whole buffer' ***");
                println!("Error: {}", err_str);
            } else {
                println!("Pool creation failed: {}", err_str);
            }
        }
    }

    println!("\nFull-stack factory test complete");
    Ok(())
}
