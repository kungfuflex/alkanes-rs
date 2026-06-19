//! Test frUSD UTXO management — multiple mints, partial burns, address queries.
//!
//! Models the real alkanes-cli flow where:
//! - Multiple mint txs create frUSD at the same address
//! - protorunesbyaddress finds all UTXOs with balances
//! - Burns consume tokens from specific UTXOs
//! - Supply tracks correctly across multiple operations

use alkanes_integ_tests::block_builder::{
    create_block_with_deploys, create_block_with_deploys_and_input, last_tx_outpoint, DeployPair,
};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::{Context, Result};
use prost::Message;

const AUTH_SLOT: u128 = 8200;
const TOKEN_SLOT: u128 = 8201;

#[test]
fn multiple_mints_then_burn() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Deploy auth + frUSD
    let deploy1 = create_block_with_deploys(4, vec![
        DeployPair::new(
            fixtures::FRUSD_AUTH_TOKEN.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: AUTH_SLOT }, inputs: vec![0] },
        ),
    ]);
    runtime.index_block(&deploy1, 4)?;
    let auth_outpoint = last_tx_outpoint(&deploy1);

    let deploy2 = create_block_with_deploys(5, vec![
        DeployPair::new(
            fixtures::FRUSD_TOKEN.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: TOKEN_SLOT }, inputs: vec![0, 4, AUTH_SLOT] },
        ),
    ]);
    runtime.index_block(&deploy2, 5)?;

    // ── Mint #1: 3000 frUSD ──
    let mint1 = create_block_with_deploys_and_input(6, vec![
        DeployPair::call_only(Cellpack {
            target: AlkaneId { block: 4, tx: TOKEN_SLOT },
            inputs: vec![1, 0, 0, 3000],
        }),
    ], auth_outpoint);
    runtime.index_block(&mint1, 6)?;
    let mint1_outpoint = last_tx_outpoint(&mint1);

    assert_eq!(get_supply(&runtime, 6)?, 3000, "supply after mint #1");
    let bal1 = query::get_alkane_balance(&runtime, &mint1_outpoint, 4, TOKEN_SLOT, 6)?;
    println!("Mint #1 outpoint balance: {} frUSD", bal1);
    assert_eq!(bal1, 3000);

    // ── Mint #2: 2000 frUSD (chained from mint #1 output, which has auth token + frUSD) ──
    let mint2 = create_block_with_deploys_and_input(7, vec![
        DeployPair::call_only(Cellpack {
            target: AlkaneId { block: 4, tx: TOKEN_SLOT },
            inputs: vec![1, 0, 0, 2000],
        }),
    ], mint1_outpoint);
    runtime.index_block(&mint2, 7)?;
    let mint2_outpoint = last_tx_outpoint(&mint2);

    let supply_after_2 = get_supply(&runtime, 7)?;
    println!("Supply after mint #2: {}", supply_after_2);
    assert_eq!(supply_after_2, 5000, "supply should be 3000 + 2000");

    // Check balances at mint2 outpoint (should have both auth token + frUSD from both mints)
    let bal2 = query::get_alkane_balance(&runtime, &mint2_outpoint, 4, TOKEN_SLOT, 7)?;
    println!("Mint #2 outpoint frUSD balance: {}", bal2);
    assert!(bal2 > 0, "mint2 should have frUSD");

    // ── Query protorunesbyaddress ──
    let address = protorune::test_helpers::ADDRESS1();
    let addr_balances = query_protorunesbyaddress(&runtime, &address, 7)?;
    println!("Address has {} outpoints", addr_balances.len());
    let total_frusd: u128 = addr_balances.iter()
        .flat_map(|(_, alkanes)| alkanes.iter())
        .filter(|(b, t, _)| *b == 4 && *t == TOKEN_SLOT)
        .map(|(_, _, bal)| *bal)
        .sum();
    println!("Total frUSD across all outpoints: {}", total_frusd);
    assert_eq!(total_frusd, 5000, "total frUSD across address should be 5000");

    // ── Burn: spend mint2 output (burns ALL frUSD at that UTXO) ──
    let evm_addr = "f39fd6e51aad88f6f4ce6ab8827279cfffb92266";
    let hi = u128::from_str_radix(&evm_addr[..24], 16).unwrap();
    let lo = u128::from_str_radix(&evm_addr[24..40], 16).unwrap();

    let burn = create_block_with_deploys_and_input(8, vec![
        DeployPair::call_only(Cellpack {
            target: AlkaneId { block: 4, tx: TOKEN_SLOT },
            inputs: vec![5, hi, lo],
        }),
    ], mint2_outpoint);
    runtime.index_block(&burn, 8)?;

    let supply_after_burn = get_supply(&runtime, 8)?;
    println!("Supply after burn: {}", supply_after_burn);
    assert!(supply_after_burn < supply_after_2, "supply should decrease after burn");

    // ── Verify burn record ──
    let burns = get_pending_burns(&runtime, 8)?;
    println!("Pending burns: {} bytes", burns.len());
    assert!(burns.len() > 16, "should have burn records");
    if burns.len() >= 16 {
        let count = u128::from_le_bytes(burns[0..16].try_into().unwrap());
        println!("Burn count: {}", count);
        assert_eq!(count, 1, "should have exactly 1 burn record");
    }

    println!("\n✓ Multiple mints + burn: supply tracking correct");
    Ok(())
}

#[test]
fn protorunesbyaddress_tracks_all_outpoints() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Deploy auth + frUSD
    let deploy1 = create_block_with_deploys(4, vec![
        DeployPair::new(
            fixtures::FRUSD_AUTH_TOKEN.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: AUTH_SLOT }, inputs: vec![0] },
        ),
    ]);
    runtime.index_block(&deploy1, 4)?;
    let auth_outpoint = last_tx_outpoint(&deploy1);

    let deploy2 = create_block_with_deploys(5, vec![
        DeployPair::new(
            fixtures::FRUSD_TOKEN.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: TOKEN_SLOT }, inputs: vec![0, 4, AUTH_SLOT] },
        ),
    ]);
    runtime.index_block(&deploy2, 5)?;

    // Mint frUSD
    let mint = create_block_with_deploys_and_input(6, vec![
        DeployPair::call_only(Cellpack {
            target: AlkaneId { block: 4, tx: TOKEN_SLOT },
            inputs: vec![1, 0, 0, 10000],
        }),
    ], auth_outpoint);
    runtime.index_block(&mint, 6)?;

    // Query by address — should find the alkanes
    let address = protorune::test_helpers::ADDRESS1();
    let results = query_protorunesbyaddress(&runtime, &address, 6)?;

    let mut found_auth = false;
    let mut found_frusd = false;
    for (outpoint_str, alkanes) in &results {
        for (block, tx, balance) in alkanes {
            println!("  {} → [{block}:{tx}] = {balance}", outpoint_str);
            if *block == 4 && *tx == AUTH_SLOT && *balance > 0 { found_auth = true; }
            if *block == 4 && *tx == TOKEN_SLOT && *balance > 0 { found_frusd = true; }
        }
    }
    assert!(found_auth, "protorunesbyaddress should find auth token");
    assert!(found_frusd, "protorunesbyaddress should find frUSD token");

    println!("\n✓ protorunesbyaddress correctly returns all alkane balances");
    Ok(())
}

fn get_supply(runtime: &TestRuntime, height: u32) -> Result<u128> {
    let cellpack = alkanes_support::cellpack::Cellpack {
        target: AlkaneId { block: 4, tx: TOKEN_SLOT },
        inputs: vec![3],
    };
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cellpack.encipher();
    let resp = runtime.alkanes_view("simulate", &parcel.encode_to_vec(), height)?;
    let sim = alkanes_support::proto::alkanes::SimulateResponse::decode(resp.as_slice())?;
    if !sim.error.is_empty() { return Err(anyhow::anyhow!("simulate: {}", sim.error)); }
    Ok(sim.execution.map(|e| {
        if e.data.len() >= 16 { u128::from_le_bytes(e.data[..16].try_into().unwrap()) } else { 0 }
    }).unwrap_or(0))
}

fn get_pending_burns(runtime: &TestRuntime, height: u32) -> Result<Vec<u8>> {
    let cellpack = alkanes_support::cellpack::Cellpack {
        target: AlkaneId { block: 4, tx: TOKEN_SLOT },
        inputs: vec![6],
    };
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cellpack.encipher();
    let resp = runtime.alkanes_view("simulate", &parcel.encode_to_vec(), height)?;
    let sim = alkanes_support::proto::alkanes::SimulateResponse::decode(resp.as_slice())?;
    if !sim.error.is_empty() { return Err(anyhow::anyhow!("simulate: {}", sim.error)); }
    Ok(sim.execution.map(|e| e.data).unwrap_or_default())
}

fn query_protorunesbyaddress(
    runtime: &TestRuntime,
    address: &str,
    height: u32,
) -> Result<Vec<(String, Vec<(u128, u128, u128)>)>> {
    use protorune_support::proto::protorune;
    let mut req = protorune::ProtorunesWalletRequest::default();
    req.wallet = address.as_bytes().to_vec();
    req.protocol_tag = Some(protorune::Uint128 { lo: 1, hi: 0 });
    let resp = runtime.alkanes_view("protorunesbyaddress", &req.encode_to_vec(), height)
        .context("protorunesbyaddress failed")?;
    let response = protorune::WalletResponse::decode(resp.as_slice())
        .context("decode WalletResponse")?;
    let mut result = Vec::new();
    for op_resp in &response.outpoints {
        let outpoint_str = op_resp.outpoint.as_ref()
            .map(|op| format!("{}:{}", hex::encode(&op.txid), op.vout))
            .unwrap_or_else(|| "?".into());
        let mut alkanes = Vec::new();
        if let Some(sheet) = &op_resp.balances {
            for entry in &sheet.entries {
                if let Some(rune) = &entry.rune {
                    if let Some(id) = &rune.rune_id {
                        let block = id.height.as_ref().map(|v| v.lo as u128).unwrap_or(0);
                        let tx = id.txindex.as_ref().map(|v| v.lo as u128).unwrap_or(0);
                        let balance = entry.balance.as_ref().map(|v| v.lo as u128).unwrap_or(0);
                        alkanes.push((block, tx, balance));
                    }
                }
            }
        }
        result.push((outpoint_str, alkanes));
    }
    Ok(result)
}
