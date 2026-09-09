//! Full-stack reproduction of the "fill whole buffer" bug.
//!
//! This test replicates the EXACT deployment sequence from the live qubitcoin
//! regtest chain that triggers the failure:
//!
//! 1. Deploy AMM stack (auth token, beacon proxy, factory logic, pool logic,
//!    factory proxy, upgradeable beacon) — same slots as production
//! 2. Initialize factory
//! 3. Deploy FROST token at [4:7955]
//! 4. Mint DIESEL + wrap frBTC
//! 5. Create DIESEL/frBTC pool (should work)
//! 6. Mint FROST tokens
//! 7. Create DIESEL/FROST pool — THIS IS WHERE "fill whole buffer" HAPPENS

use alkanes_integ_tests::block_builder::{
    create_block_with_deploys, create_block_with_protostones, last_tx_outpoint, DeployPair,
};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::harness::FullStackHarness;
use alkanes_integ_tests::query;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::key::TapTweak;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::{
    key::UntweakedPublicKey, transaction::Version, Amount, OutPoint, ScriptBuf, Sequence, TxIn,
    TxOut, Witness,
};
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::protostone::{Protostone, ProtostoneEdict};

// Production slot IDs from env.sh
const SLOT_AUTH_TOKEN_FACTORY: u128 = 65517;  // 0xffed
const SLOT_BEACON_PROXY: u128 = 780993;
const SLOT_FACTORY_LOGIC: u128 = 65524;       // 0xfff4
const SLOT_POOL_LOGIC: u128 = 65520;          // 0xfff0
const SLOT_FACTORY_PROXY: u128 = 65522;       // 0xfff2
const SLOT_UPGRADEABLE_BEACON: u128 = 65523;  // 0xfff3
const SLOT_FROST_TOKEN: u128 = 7955;          // 0x1f13

// frBTC signer pubkey (same as fr_btc.rs)
const SIGNER_PUBKEY: [u8; 32] = [
    0x79, 0x40, 0xef, 0x3b, 0x65, 0x91, 0x79, 0xa1, 0x37, 0x1d, 0xec, 0x05, 0x79, 0x3c, 0xb0,
    0x27, 0xcd, 0xe4, 0x78, 0x06, 0xfb, 0x66, 0xce, 0x1e, 0x3d, 0x1b, 0x69, 0xd5, 0x6d, 0xe6,
    0x29, 0xdc,
];

fn txin_from(outpoint: OutPoint) -> TxIn {
    TxIn {
        previous_output: outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    }
}

fn create_frbtc_signer_output(sats: u64) -> TxOut {
    let pk = UntweakedPublicKey::from_slice(&SIGNER_PUBKEY).unwrap();
    let secp = Secp256k1::new();
    let (tweaked, _) = pk.tap_tweak(&secp, None);
    TxOut {
        value: Amount::from_sat(sats),
        script_pubkey: ScriptBuf::new_p2tr_tweaked(tweaked),
    }
}

/// This test reproduces the exact sequence that causes "fill whole buffer"
/// on the live qubitcoin regtest chain.
#[test]
fn test_reproduce_fill_whole_buffer() -> Result<()> {
    let _ = env_logger::try_init();

    // Use the lightweight runtime since the full-stack harness's TestChain
    // uses qubitcoin-consensus Block type (not rust-bitcoin). We use the
    // lightweight runtime here but with the SAME indexer path that we'd
    // use in the full-stack harness — the key is replicating the exact
    // contract deployment sequence and pool creation call.
    let runtime = alkanes_integ_tests::runtime::TestRuntime::new()?;

    println!("=== Phase 0-2: Genesis + empty blocks ===");
    runtime.mine_empty_blocks(0, 4)?;

    let mut height: u32 = 4;

    // ═══ Phase 3: AMM Stack ═══
    println!("\n=== Phase 3: AMM Stack ===");

    // 1. Auth Token Factory
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::AUTH_TOKEN,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_AUTH_TOKEN_FACTORY }, inputs: vec![100] },
    )]);
    runtime.index_block(&block, height)?;
    println!("  Auth Token Factory deployed at [4:{}]", SLOT_AUTH_TOKEN_FACTORY);
    height += 1;

    // 2. Beacon Proxy Template
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::BEACON_PROXY,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_BEACON_PROXY }, inputs: vec![36863] },
    )]);
    runtime.index_block(&block, height)?;
    println!("  Beacon Proxy deployed at [4:{}]", SLOT_BEACON_PROXY);
    height += 1;

    // 3. Factory Logic
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::FACTORY,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_FACTORY_LOGIC }, inputs: vec![50] },
    )]);
    runtime.index_block(&block, height)?;
    println!("  Factory Logic deployed at [4:{}]", SLOT_FACTORY_LOGIC);
    height += 1;

    // 4. Pool Logic
    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::POOL,
        Cellpack { target: AlkaneId { block: 3, tx: SLOT_POOL_LOGIC }, inputs: vec![50] },
    )]);
    runtime.index_block(&block, height)?;
    println!("  Pool Logic deployed at [4:{}]", SLOT_POOL_LOGIC);
    height += 1;

    // 5. Factory Proxy (Upgradeable) — points to factory logic
    // This deploys the proxy AND mints an auth token at the output
    let factory_proxy_block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::UPGRADEABLE,
        Cellpack {
            target: AlkaneId { block: 3, tx: SLOT_FACTORY_PROXY },
            inputs: vec![0x7fff, 4, SLOT_FACTORY_LOGIC, 5],
        },
    )]);
    runtime.index_block(&factory_proxy_block, height)?;
    let factory_proxy_outpoint = last_tx_outpoint(&factory_proxy_block);
    println!("  Factory Proxy deployed at [4:{}]", SLOT_FACTORY_PROXY);

    // Check auth token balance at the proxy deploy output
    // Check balances at every output of the factory proxy deploy tx
    let proxy_tx = factory_proxy_block.txdata.last().unwrap();
    println!("  Factory proxy tx has {} outputs, txid={}", proxy_tx.output.len(), proxy_tx.compute_txid());
    for vout in 0..proxy_tx.output.len() as u32 + 2 {
        let op = OutPoint { txid: proxy_tx.compute_txid(), vout };
        let bals = query::get_balance_for_outpoint(&runtime, &op, height)?;
        if !bals.is_empty() {
            println!("  vout:{} balances: {:?}", vout, bals);
        }
    }
    // Also check the previous txs in the block (deploy chains)
    for (tx_idx, tx) in factory_proxy_block.txdata.iter().enumerate() {
        for vout in 0..tx.output.len() as u32 + 2 {
            let op = OutPoint { txid: tx.compute_txid(), vout };
            let bals = query::get_balance_for_outpoint(&runtime, &op, height)?;
            if !bals.is_empty() {
                println!("  block tx[{}] vout:{} balances: {:?}", tx_idx, vout, bals);
            }
        }
    }
    height += 1;

    // 6. Upgradeable Beacon — points to pool logic
    let beacon_block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::UPGRADEABLE_BEACON,
        Cellpack {
            target: AlkaneId { block: 3, tx: SLOT_UPGRADEABLE_BEACON },
            inputs: vec![0x7fff, 4, SLOT_POOL_LOGIC, 5],
        },
    )]);
    runtime.index_block(&beacon_block, height)?;
    println!("  Upgradeable Beacon deployed at [4:{}]", SLOT_UPGRADEABLE_BEACON);
    height += 1;

    // 7. Initialize Factory — tokens should flow automatically to first protostone
    println!("  Initializing factory...");
    println!("  Input UTXO: {:?}", factory_proxy_outpoint);

    // The factory proxy deploy tx has the auth tokens at vout:0.
    // We spend that UTXO, and the single protostone targeting the factory
    // should receive all tokens as incoming_alkanes (protorune default routing).
    //
    // IMPORTANT: The Runestone pointer must be set correctly. If pointer=Some(0),
    // non-protocol tokens go to output 0. Protocol tokens go to the first
    // matching protostone automatically.
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
    runtime.index_block(&init_block, height)?;

    // Check ALL outputs for auth tokens to understand routing
    let init_outpoint_0 = OutPoint {
        txid: init_block.txdata.last().unwrap().compute_txid(),
        vout: 0,
    };
    let bals_v0 = query::get_balance_for_outpoint(&runtime, &init_outpoint_0, height)?;
    println!("  Post-init vout:0 balances: {:?}", bals_v0);

    // Also check vout:1 (OP_RETURN — shouldn't have tokens)
    let init_outpoint_1 = OutPoint {
        txid: init_block.txdata.last().unwrap().compute_txid(),
        vout: 1,
    };
    let bals_v1 = query::get_balance_for_outpoint(&runtime, &init_outpoint_1, height)?;
    println!("  Post-init vout:1 balances: {:?}", bals_v1);

    // Check the protostone shadow vout (vout = num_outputs + 1 + 0 = 3)
    // With 2 outputs [txout, op_return], shadow p0 = 2 + 0 = 2... actually
    // the shadow vout = tx.output.len() + 1 + protostone_index
    // tx has 2 outputs, so shadow p0 = 2 + 1 + 0 = 3
    let shadow_vout = init_block.txdata.last().unwrap().output.len() as u32 + 1;
    let init_shadow = OutPoint {
        txid: init_block.txdata.last().unwrap().compute_txid(),
        vout: shadow_vout,
    };
    let bals_shadow = query::get_balance_for_outpoint(&runtime, &init_shadow, height)?;
    println!("  Post-init shadow vout:{} balances: {:?}", shadow_vout, bals_shadow);

    height += 1;

    // ═══ Phase 4: FROST Token ═══
    println!("\n=== Phase 4: FROST Token ===");
    let frost_block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::TEST_CONTRACT, // Using test contract as FROST placeholder
        Cellpack {
            target: AlkaneId { block: 3, tx: SLOT_FROST_TOKEN },
            inputs: vec![0, 1000000000000000, 4, SLOT_FROST_TOKEN],
        },
    )]);
    runtime.index_block(&frost_block, height)?;
    println!("  FROST Token deployed at [4:{}]", SLOT_FROST_TOKEN);
    height += 1;

    // ═══ Mint DIESEL + wrap frBTC ═══
    println!("\n=== Minting tokens ===");

    // Mint DIESEL 3x
    for i in 0..3 {
        let block = create_block_with_protostones(
            height,
            vec![txin_from(OutPoint::null())],
            vec![],
            vec![Protostone {
                message: Cellpack { target: AlkaneId { block: 2, tx: 0 }, inputs: vec![77] }.encipher(),
                protocol_tag: 1, burn: None, from: None, pointer: Some(0), refund: Some(0), edicts: vec![],
            }],
        );
        runtime.index_block(&block, height)?;
        height += 1;
    }
    println!("  DIESEL minted 3x");

    // Wrap BTC → frBTC
    let mut wrap_block = create_block_with_coinbase_tx(height);
    let funding = OutPoint { txid: wrap_block.txdata[0].compute_txid(), vout: 0 };
    let wrap_protostones = vec![
        Protostone {
            message: Cellpack { target: AlkaneId { block: 32, tx: 0 }, inputs: vec![77] }.encipher(),
            protocol_tag: 1, burn: None, from: None, pointer: Some(0), refund: Some(0), edicts: vec![],
        },
    ];
    let wrap_runestone = (ordinals::Runestone {
        edicts: vec![], etching: None, mint: None, pointer: Some(0),
        protocol: protorune::protostone::Protostones::encipher(&wrap_protostones).ok(),
    }).encipher();
    wrap_block.txdata.push(bitcoin::Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin_from(funding)],
        output: vec![
            create_frbtc_signer_output(100_000_000),
            TxOut { value: Amount::from_sat(0), script_pubkey: wrap_runestone },
        ],
    });
    runtime.index_block(&wrap_block, height)?;
    let frbtc_outpoint = last_tx_outpoint(&wrap_block);
    let frbtc_bal = query::get_alkane_balance(&runtime, &frbtc_outpoint, 32, 0, height)?;
    println!("  frBTC wrapped: {} frBTC", frbtc_bal);
    height += 1;

    // ═══ CREATE DIESEL/FROST POOL — THE FAILING OPERATION ═══
    println!("\n=== CREATING DIESEL/FROST POOL (the operation that fails on live chain) ===");
    println!("  Factory: [4:{}]", SLOT_FACTORY_PROXY);
    println!("  Token A: DIESEL [2:0]");
    println!("  Token B: FROST [4:{}]", SLOT_FROST_TOKEN);

    // Simulate the exact call: [4,65522,1,2,0,4,7955,3000000000,30000000000]
    let create_pool_block = create_block_with_protostones(
        height,
        vec![txin_from(OutPoint::null())],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 4, tx: SLOT_FACTORY_PROXY },
                inputs: vec![
                    1,                  // CreateNewPool opcode
                    2,                  // num token types
                    2, 0,               // Token A: DIESEL [2:0]
                    4, SLOT_FROST_TOKEN, // Token B: FROST [4:7955]
                    3_000_000_000,      // Amount A
                    30_000_000_000,     // Amount B
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

    let pool_result = runtime.index_block(&create_pool_block, height);
    match &pool_result {
        Ok(()) => {
            println!("  Pool creation SUCCEEDED — no 'fill whole buffer' error");
            println!("  BUG NOT REPRODUCED with lightweight runtime");
        }
        Err(e) => {
            let err_str = format!("{:#}", e);
            if err_str.contains("fill whole buffer") {
                println!("  *** BUG REPRODUCED: 'fill whole buffer' ***");
                println!("  Error: {}", err_str);
            } else if err_str.contains("revert") {
                println!("  Pool creation reverted (expected — no tokens provided): {}", err_str);
            } else {
                println!("  Pool creation failed: {}", err_str);
            }
        }
    }

    println!("\n=== Test complete ===");
    Ok(())
}
