//! E2E test: deploy frUSD, send auth token to signer address, verify discovery.
//!
//! Models the subzero signal engine flow:
//! 1. Deploy auth token + frUSD
//! 2. Transfer auth token to the signer's derived address
//! 3. Verify protorunesbyaddress finds it
//! 4. Mint frUSD using the auth token at the signer address

use alkanes_integ_tests::block_builder::{
    create_block_with_deploys, create_block_with_deploys_and_input,
    create_block_with_deploys_to_address, last_tx_outpoint, DeployPair,
};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use prost::Message;

const AUTH_SLOT: u128 = 9000;
const TOKEN_SLOT: u128 = 9001;

/// The signer's derived address (what the WASIP2 signal program scans).
/// In production this is derived from the FROST group key.
/// For testing we use a known regtest P2TR address.
const SIGNER_ADDRESS: &str = "bcrt1pxm6sv08u3flgg8enr3schygg7nh3elk097d2542tqv02z24f0m0s7x4jxn";

#[test]
fn deploy_frusd_and_transfer_auth_to_signer() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // ── Step 1: Deploy auth token ──
    println!("=== Step 1: Deploy auth token at 4:{AUTH_SLOT} ===");
    let deploy_auth = create_block_with_deploys(4, vec![
        DeployPair::new(
            fixtures::FRUSD_AUTH_TOKEN.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: AUTH_SLOT }, inputs: vec![0] },
        ),
    ]);
    runtime.index_block(&deploy_auth, 4)?;
    let auth_outpoint = last_tx_outpoint(&deploy_auth);
    println!("Auth token deployed, outpoint: {:?}", auth_outpoint);

    // ── Step 2: Deploy frUSD token (init with auth slot) ──
    println!("\n=== Step 2: Deploy frUSD at 4:{TOKEN_SLOT} ===");
    let deploy_frusd = create_block_with_deploys(5, vec![
        DeployPair::new(
            fixtures::FRUSD_TOKEN.to_vec(),
            Cellpack {
                target: AlkaneId { block: 3, tx: TOKEN_SLOT },
                inputs: vec![0, 4, AUTH_SLOT],
            },
        ),
    ]);
    runtime.index_block(&deploy_frusd, 5)?;
    println!("frUSD deployed");

    // ── Step 3: Check auth token is at deployer address ──
    println!("\n=== Step 3: Verify auth token at deployer ===");
    let deployer_addr = protorune::test_helpers::ADDRESS1();
    let deployer_balances = query_protorunesbyaddress(&runtime, &deployer_addr, 5)?;
    let mut auth_found_at_deployer = false;
    for (outpoint, alkanes) in &deployer_balances {
        for (b, t, bal) in alkanes {
            if *b == 4 && *t == AUTH_SLOT {
                println!("Auth token at deployer: {} balance={}", outpoint, bal);
                auth_found_at_deployer = true;
            }
        }
    }
    assert!(auth_found_at_deployer, "Auth token should be at deployer after deploy");

    // ── Step 4: Transfer auth token to signer address ──
    // This is the key operation: the auth token UTXO at the deployer
    // needs to move to the signer's address.
    //
    // We do this by creating a block with a TX that:
    // - Spends the auth token outpoint as input
    // - Has the signer address as the output
    // - Uses a protostone that points to the signer output
    println!("\n=== Step 4: Transfer auth token to signer ===");

    // Create a mint TX that sends auth token output to signer
    // The mint call (opcode 1) on frUSD requires the auth token.
    // After the call, the auth token goes to the pointer output.
    // We set the pointer output to be the signer's address.
    let mint_to_signer = create_block_with_deploys_to_address(6, vec![
        DeployPair::call_only(Cellpack {
            target: AlkaneId { block: 4, tx: TOKEN_SLOT },
            inputs: vec![1, 0, 0, 1000], // mint 1000 frUSD
        }),
    ], auth_outpoint, SIGNER_ADDRESS);
    runtime.index_block(&mint_to_signer, 6)?;
    let signer_outpoint = last_tx_outpoint(&mint_to_signer);
    println!("Mint TX sent to signer, outpoint: {:?}", signer_outpoint);

    // ── Step 5: Verify auth token is now at signer address ──
    println!("\n=== Step 5: Verify auth token at signer ===");
    let signer_balances = query_protorunesbyaddress(&runtime, SIGNER_ADDRESS, 6)?;
    let mut auth_found_at_signer = false;
    let mut frusd_found_at_signer = false;
    for (outpoint, alkanes) in &signer_balances {
        for (b, t, bal) in alkanes {
            println!("  Signer has: [{}:{}] = {} at {}", b, t, bal, outpoint);
            if *b == 4 && *t == AUTH_SLOT && *bal > 0 { auth_found_at_signer = true; }
            if *b == 4 && *t == TOKEN_SLOT && *bal > 0 { frusd_found_at_signer = true; }
        }
    }
    assert!(auth_found_at_signer, "Auth token should be at signer address after transfer");
    assert!(frusd_found_at_signer, "frUSD should be at signer address after mint");

    // ── Step 6: Verify supply ──
    let supply = get_supply(&runtime, 6)?;
    println!("\nfrUSD supply: {}", supply);
    assert_eq!(supply, 1000, "frUSD supply should be 1000");

    println!("\n✓ E2E: deploy → transfer auth → mint at signer → verified");
    Ok(())
}

fn get_supply(runtime: &TestRuntime, height: u32) -> Result<u128> {
    let cellpack = Cellpack {
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

fn query_protorunesbyaddress(
    runtime: &TestRuntime,
    address: &str,
    height: u32,
) -> Result<Vec<(String, Vec<(u128, u128, u128)>)>> {
    use protorune_support::proto::protorune;
    let mut req = protorune::ProtorunesWalletRequest::default();
    req.wallet = address.as_bytes().to_vec();
    req.protocol_tag = Some(protorune::Uint128 { lo: 1, hi: 0 });
    let resp = runtime.alkanes_view("protorunesbyaddress", &req.encode_to_vec(), height)?;
    let response = protorune::WalletResponse::decode(resp.as_slice())?;
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
