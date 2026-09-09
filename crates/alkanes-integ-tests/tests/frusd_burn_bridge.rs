//! Test frUSD deploy → mint → burn+bridge full lifecycle.
//!
//! Models the exact flow needed for cross-chain USDC ↔ frUSD bridge:
//!   1. Deploy frUSD auth token at reserved slot
//!   2. Deploy frUSD token at reserved slot (linked to auth token)
//!   3. Mint frUSD (requires auth token as input)
//!   4. Burn frUSD with BurnAndBridge (opcode 5, encodes EVM address)
//!   5. Query pending burns (opcode 6)

use alkanes_integ_tests::block_builder::{create_block_with_deploys, create_block_with_deploys_and_input, last_tx_outpoint, DeployPair};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use prost::Message;

const FRUSD_AUTH_SLOT: u128 = 8200;
const FRUSD_TOKEN_SLOT: u128 = 8201;

#[test]
fn frusd_deploy_mint_burn_bridge() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // ── Step 1: Deploy frUSD auth token ──
    // CREATERESERVED at slot 8200, init opcode 0 → mints 1 auth token to output 0
    let deploy_block = create_block_with_deploys(
        4,
        vec![
            DeployPair::new(
                fixtures::FRUSD_AUTH_TOKEN.to_vec(),
                Cellpack {
                    target: AlkaneId { block: 3, tx: FRUSD_AUTH_SLOT },
                    inputs: vec![0],
                },
            ),
        ],
    );
    runtime.index_block(&deploy_block, 4)?;
    let auth_outpoint = last_tx_outpoint(&deploy_block);
    println!("Auth token deployed at [4:{}], outpoint: {:?}", FRUSD_AUTH_SLOT, auth_outpoint);

    // Verify auth token exists via getbytecode
    let bc_req = build_bytecode_request(4, FRUSD_AUTH_SLOT);
    let bc_resp = runtime.alkanes_view("getbytecode", &bc_req, 4)?;
    println!("Auth token bytecode: {} bytes", bc_resp.len());
    assert!(bc_resp.len() > 100, "auth token bytecode should be stored");

    // ── Step 2: Deploy frUSD token ──
    // CREATERESERVED at slot 8201, init: opcode 0, auth_block=4, auth_tx=8200
    let deploy_block2 = create_block_with_deploys(
        5,
        vec![
            DeployPair::new(
                fixtures::FRUSD_TOKEN.to_vec(),
                Cellpack {
                    target: AlkaneId { block: 3, tx: FRUSD_TOKEN_SLOT },
                    inputs: vec![0, 4, FRUSD_AUTH_SLOT],
                },
            ),
        ],
    );
    runtime.index_block(&deploy_block2, 5)?;
    println!("frUSD token deployed at [4:{}]", FRUSD_TOKEN_SLOT);

    let bc_resp2 = runtime.alkanes_view("getbytecode", &build_bytecode_request(4, FRUSD_TOKEN_SLOT), 5)?;
    assert!(bc_resp2.len() > 100, "frUSD bytecode should be stored");

    // ── Step 3: Mint frUSD ──
    // Call opcode 1 on frUSD, passing 1 auth token as input.
    // The auth token was minted to output 0 of the deploy block at height 4.
    // We chain the mint tx to spend that outpoint so the auth token is in incoming_alkanes.
    let mint_block = create_block_with_deploys_and_input(
        6,
        vec![
            // Mint 1000 frUSD: [4, FRUSD_TOKEN_SLOT, 1, 0, 0, 1000]
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 4, tx: FRUSD_TOKEN_SLOT },
                inputs: vec![1, 0, 0, 1000],
            }),
        ],
        auth_outpoint, // spend the auth token UTXO
    );
    runtime.index_block(&mint_block, 6)?;

    // Check total supply via simulate (opcode 3)
    let sim_input = build_simulate_request(4, FRUSD_TOKEN_SLOT, &[3]);
    let sim_resp = runtime.alkanes_view("simulate", &sim_input, 6)?;
    let sim = alkanes_support::proto::alkanes::SimulateResponse::decode(sim_resp.as_slice())?;
    println!("Simulate supply: error='{}', gas={}", sim.error, sim.gas_used);
    if let Some(ref exec) = sim.execution {
        let supply_bytes = &exec.data;
        if supply_bytes.len() >= 16 {
            let supply = u128::from_le_bytes(supply_bytes[..16].try_into().unwrap());
            println!("frUSD total supply: {}", supply);
        }
    }

    // ── Step 4: Burn frUSD with BurnAndBridge ──
    // opcode 5 encodes EVM address as (hi_u128, lo_u128)
    // EVM address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
    let evm_addr = "f39fd6e51aad88f6f4ce6ab8827279cfffb92266";
    let hi = u128::from_str_radix(&evm_addr[..24], 16).unwrap();
    let lo = u128::from_str_radix(&evm_addr[24..40], 16).unwrap();

    // Spend the mint output which holds the frUSD tokens
    let mint_outpoint = last_tx_outpoint(&mint_block);
    let burn_block = create_block_with_deploys_and_input(
        7,
        vec![
            DeployPair::call_only(Cellpack {
                target: AlkaneId { block: 4, tx: FRUSD_TOKEN_SLOT },
                inputs: vec![5, hi, lo], // opcode 5 = BurnAndBridge
            }),
        ],
        mint_outpoint, // spend UTXO that holds frUSD
    );
    runtime.index_block(&burn_block, 7)?;
    println!("Burn+bridge executed at height 7");

    // ── Step 5: Query pending burns ──
    let burns_input = build_simulate_request(4, FRUSD_TOKEN_SLOT, &[6]);
    let burns_resp = runtime.alkanes_view("simulate", &burns_input, 7)?;
    let burns_sim = alkanes_support::proto::alkanes::SimulateResponse::decode(burns_resp.as_slice())?;
    println!("Pending burns: error='{}', gas={}", burns_sim.error, burns_sim.gas_used);
    if let Some(ref exec) = burns_sim.execution {
        println!("Burns data: {} bytes", exec.data.len());
        if exec.data.len() > 0 {
            println!("Burns hex: {}", hex::encode(&exec.data[..std::cmp::min(exec.data.len(), 100)]));
        }
    }

    Ok(())
}

/// Build a BytecodeRequest protobuf
fn build_bytecode_request(block: u128, tx: u128) -> Vec<u8> {
    let mut req = alkanes_support::proto::alkanes::BytecodeRequest::default();
    req.id = Some(alkanes_support::proto::alkanes::AlkaneId {
        block: Some(alkanes_support::proto::alkanes::Uint128 { lo: block as u64, hi: 0 }),
        tx: Some(alkanes_support::proto::alkanes::Uint128 { lo: tx as u64, hi: 0 }),
    });
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    buf
}

/// Build a simulate request: MessageContextParcel with cellpack calldata
fn build_simulate_request(block: u128, tx: u128, inputs: &[u128]) -> Vec<u8> {
    let cellpack = alkanes_support::cellpack::Cellpack {
        target: alkanes_support::id::AlkaneId { block, tx },
        inputs: inputs.to_vec(),
    };
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cellpack.encipher();
    let mut buf = Vec::new();
    parcel.encode(&mut buf).unwrap();
    buf
}
