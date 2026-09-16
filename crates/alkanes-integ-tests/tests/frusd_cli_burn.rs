//! Test frUSD burn via the same UTXO discovery path alkanes-cli uses.
//!
//! Models the regtest bug where burn+bridge fails because:
//! 1. frUSD is minted to an outpoint
//! 2. alkanes-cli queries protorunesbyaddress to find that outpoint
//! 3. If the query doesn't find the frUSD UTXO, the burn tx has no tokens to burn
//!
//! This test verifies:
//! - protorunesbyoutpoint correctly shows frUSD balance after mint
//! - The burn tx works when spending the correct outpoint
//! - Supply decreases after burn
//! - Pending burns record appears

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
fn frusd_mint_verify_balance_burn_verify_burns() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // ── Deploy auth token + frUSD token ──
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

    // ── Mint frUSD ──
    let mint_block = create_block_with_deploys_and_input(6, vec![
        DeployPair::call_only(Cellpack {
            target: AlkaneId { block: 4, tx: TOKEN_SLOT },
            inputs: vec![1, 0, 0, 5000], // mint 5000 frUSD
        }),
    ], auth_outpoint);
    runtime.index_block(&mint_block, 6)?;

    let mint_outpoint = last_tx_outpoint(&mint_block);
    println!("Mint outpoint: {:?}", mint_outpoint);

    // ── Verify: check supply via simulate ──
    let supply = get_supply(&runtime, 6)?;
    println!("Supply after mint: {}", supply);
    assert_eq!(supply, 5000, "supply should be 5000 after mint");

    // ── Verify: check balance at mint outpoint ──
    let bal = query::get_alkane_balance(&runtime, &mint_outpoint, 4, TOKEN_SLOT, 6)?;
    println!("frUSD balance at mint outpoint: {}", bal);
    assert!(bal > 0, "mint outpoint should have frUSD tokens");

    // ── Verify: also check auth token balance (should be returned after mint) ──
    let auth_bal = query::get_alkane_balance(&runtime, &mint_outpoint, 4, AUTH_SLOT, 6)?;
    println!("Auth token balance at mint outpoint: {}", auth_bal);

    // ── Verify: query protorunesbyaddress (same as alkanes-cli) ──
    // This is the critical path — if this doesn't find frUSD, alkanes-cli can't burn
    let address = protorune::test_helpers::ADDRESS1();
    let address_balances = query_protorunesbyaddress(&runtime, &address, 6)?;
    println!("protorunesbyaddress for {}: {} outpoints", address, address_balances.len());
    for (outpoint_str, alkanes) in &address_balances {
        for (block, tx, balance) in alkanes {
            println!("  {} → [{block}:{tx}] = {balance}", outpoint_str);
        }
    }
    let has_frusd = address_balances.iter().any(|(_, alkanes)| {
        alkanes.iter().any(|(b, t, bal)| *b == 4 && *t == TOKEN_SLOT && *bal > 0)
    });
    assert!(has_frusd, "protorunesbyaddress should find frUSD at the default address");

    // ── Burn 500 frUSD with BurnAndBridge ──
    let evm_addr = "f39fd6e51aad88f6f4ce6ab8827279cfffb92266";
    let hi = u128::from_str_radix(&evm_addr[..24], 16).unwrap();
    let lo = u128::from_str_radix(&evm_addr[24..40], 16).unwrap();

    let burn_block = create_block_with_deploys_and_input(7, vec![
        DeployPair::call_only(Cellpack {
            target: AlkaneId { block: 4, tx: TOKEN_SLOT },
            inputs: vec![5, hi, lo],
        }),
    ], mint_outpoint);
    runtime.index_block(&burn_block, 7)?;

    // ── Verify: supply should decrease ──
    let supply_after = get_supply(&runtime, 7)?;
    println!("Supply after burn: {}", supply_after);
    // The burn burns ALL incoming tokens (not just 500) because we sent all frUSD
    assert!(supply_after < supply, "supply should decrease after burn");

    // ── Verify: pending burns should have data ──
    let burns = get_pending_burns(&runtime, 7)?;
    println!("Pending burns: {} bytes", burns.len());
    assert!(burns.len() > 16, "should have burn records (not just zeros)");

    // Decode the burn record
    if burns.len() >= 32 {
        // First 16 bytes: count (u128 LE)
        let count = u128::from_le_bytes(burns[0..16].try_into().unwrap());
        println!("Burn record count: {}", count);
        assert!(count > 0, "should have at least 1 burn record");
    }

    println!("\n✓ Full lifecycle: deploy → mint → verify balance → burn → verify burns");
    Ok(())
}

fn get_supply(runtime: &TestRuntime, height: u32) -> Result<u128> {
    let cellpack = alkanes_support::cellpack::Cellpack {
        target: AlkaneId { block: 4, tx: TOKEN_SLOT },
        inputs: vec![3], // opcode 3 = get_total_supply
    };
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cellpack.encipher();
    let mut buf = Vec::new();
    parcel.encode(&mut buf).unwrap();

    let resp = runtime.alkanes_view("simulate", &buf, height)?;
    let sim = alkanes_support::proto::alkanes::SimulateResponse::decode(resp.as_slice())?;
    if !sim.error.is_empty() {
        return Err(anyhow::anyhow!("simulate error: {}", sim.error));
    }
    if let Some(exec) = &sim.execution {
        if exec.data.len() >= 16 {
            return Ok(u128::from_le_bytes(exec.data[..16].try_into().unwrap()));
        }
    }
    Ok(0)
}

fn get_pending_burns(runtime: &TestRuntime, height: u32) -> Result<Vec<u8>> {
    let cellpack = alkanes_support::cellpack::Cellpack {
        target: AlkaneId { block: 4, tx: TOKEN_SLOT },
        inputs: vec![6], // opcode 6 = get_pending_burns
    };
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cellpack.encipher();
    let mut buf = Vec::new();
    parcel.encode(&mut buf).unwrap();

    let resp = runtime.alkanes_view("simulate", &buf, height)?;
    let sim = alkanes_support::proto::alkanes::SimulateResponse::decode(resp.as_slice())?;
    if !sim.error.is_empty() {
        return Err(anyhow::anyhow!("simulate error: {}", sim.error));
    }
    Ok(sim.execution.map(|e| e.data).unwrap_or_default())
}

/// Query protorunesbyaddress — same view function alkanes-cli uses for UTXO discovery.
/// Returns Vec<(outpoint_string, Vec<(block, tx, balance)>)>
fn query_protorunesbyaddress(
    runtime: &TestRuntime,
    address: &str,
    height: u32,
) -> Result<Vec<(String, Vec<(u128, u128, u128)>)>> {
    use protorune_support::proto::protorune;

    // Build ProtorunesWalletRequest protobuf
    // wallet is bytes (the address string as bytes), protocol_tag is Uint128
    let mut req = protorune::ProtorunesWalletRequest::default();
    req.wallet = address.as_bytes().to_vec();
    req.protocol_tag = Some(protorune::Uint128 { lo: 1, hi: 0 });

    let request_bytes = req.encode_to_vec();
    let response_bytes = runtime
        .alkanes_view("protorunesbyaddress", &request_bytes, height)
        .context("protorunesbyaddress view call failed")?;

    // Decode WalletResponse
    let response = protorune::WalletResponse::decode(response_bytes.as_slice())
        .context("failed to decode WalletResponse")?;

    let mut result = Vec::new();
    for op_resp in &response.outpoints {
        let outpoint_str = if let Some(ref op) = op_resp.outpoint {
            format!("{}:{}", hex::encode(&op.txid), op.vout)
        } else {
            "unknown".to_string()
        };
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
