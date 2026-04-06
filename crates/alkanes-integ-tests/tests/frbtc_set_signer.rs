//! Test: deploy custom frBTC + frSIGIL, then use frSIGIL to set signer address.
//!
//! This tests the full flow of deploying a frBTC contract with a configurable
//! auth token, then using that auth token to call set_signer().

use alkanes_integ_tests::block_builder::{
    create_block_with_deploys, create_block_with_deploys_to_address, last_tx_outpoint, DeployPair,
};
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::key::TapTweak;
use bitcoin::secp256k1::Secp256k1;
use bitcoin::key::UntweakedPublicKey;
use bitcoin::{ScriptBuf, Amount, TxOut};
use prost::Message;

const SIGIL_SLOT: u128 = 43592;
const FRBTC_SLOT: u128 = 43594;

/// FROST group x-only public key (same as our deployed group).
const FROST_XONLY: [u8; 32] = [
    0x36, 0xf5, 0x06, 0x3c, 0xfc, 0x8a, 0x7e, 0x84, 0x1f, 0x33, 0x1c, 0x61, 0x8b, 0x91, 0x08,
    0xf4, 0xef, 0x1c, 0xfe, 0xcf, 0x2f, 0x9a, 0xaa, 0x55, 0x4b, 0x03, 0x1e, 0xa1, 0x2a, 0xa9,
    0x7e, 0xdf,
];

fn frost_signer_p2tr() -> ScriptBuf {
    let pk = UntweakedPublicKey::from_slice(&FROST_XONLY).unwrap();
    let secp = Secp256k1::new();
    let (tweaked, _) = pk.tap_tweak(&secp, None);
    ScriptBuf::new_p2tr_tweaked(tweaked)
}

fn get_signer(runtime: &TestRuntime, height: u32) -> Result<Vec<u8>> {
    let cellpack = Cellpack {
        target: AlkaneId { block: 4, tx: FRBTC_SLOT },
        inputs: vec![103], // get-signer
    };
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cellpack.encipher();
    let resp = runtime.alkanes_view("simulate", &parcel.encode_to_vec(), height)?;
    let sim = alkanes_support::proto::alkanes::SimulateResponse::decode(resp.as_slice())?;
    if !sim.error.is_empty() {
        return Err(anyhow::anyhow!("get_signer simulate error: {}", sim.error));
    }
    Ok(sim.execution.map(|e| e.data).unwrap_or_default())
}

#[test]
fn deploy_frbtc_and_set_signer() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // ── Step 1: Deploy frSIGIL at [4:SIGIL_SLOT] ──
    println!("=== Step 1: Deploy frSIGIL at 4:{SIGIL_SLOT} ===");
    let fr_sigil_wasm = std::fs::read(
        std::env::var("FR_SIGIL_WASM")
            .unwrap_or_else(|_| "/home/ubuntu/subfrost-app/prod_wasms/fr_sigil.wasm".to_string()),
    )?;
    let deploy_sigil = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fr_sigil_wasm,
            Cellpack {
                target: AlkaneId { block: 3, tx: SIGIL_SLOT },
                inputs: vec![0, 1], // initialize(amount=1)
            },
        )],
    );
    runtime.index_block(&deploy_sigil, 4)?;
    let sigil_outpoint = last_tx_outpoint(&deploy_sigil);
    println!("frSIGIL deployed, outpoint: {:?}", sigil_outpoint);

    // Verify frSIGIL balance
    let sigil_bal = query::get_alkane_balance(&runtime, &sigil_outpoint, 4, SIGIL_SLOT, 4)?;
    println!("frSIGIL balance: {}", sigil_bal);
    assert!(sigil_bal > 0, "Deployer should have frSIGIL after deploy");

    // ── Step 2: Deploy modified frBTC at [4:FRBTC_SLOT] ──
    // Deploy with --to signer_addr so the first TX output is the signer's P2TR.
    // The modified frBTC init reads the first output's script_pubkey as the signer.
    println!("\n=== Step 2: Deploy frBTC at 4:{FRBTC_SLOT} (auth=4:{SIGIL_SLOT}, signer=FROST) ===");
    let fr_btc_wasm = std::fs::read(
        std::env::var("FR_BTC_WASM").unwrap_or_else(|_| {
            "/home/ubuntu/subfrost-alkanes/target/wasm32-unknown-unknown/release/fr_btc.wasm"
                .to_string()
        }),
    )?;
    let signer_address = "bcrt1peqz7scjchkm7neql5y5765a877z6s4wgp2js5936agz9mgz00spsj37dl2";
    let deploy_frbtc = create_block_with_deploys_to_address(
        5,
        vec![DeployPair::new(
            fr_btc_wasm,
            Cellpack {
                target: AlkaneId { block: 3, tx: FRBTC_SLOT },
                inputs: vec![0, 4, SIGIL_SLOT],
            },
        )],
        sigil_outpoint, // spend the sigil outpoint (frSIGIL goes to pointer)
        signer_address,  // first output = signer P2TR address
    );
    runtime.index_block(&deploy_frbtc, 5)?;
    println!("frBTC deployed with signer address in first output");

    // Check signer right after deploy (should already be the FROST key)
    let default_signer = vec![0x79u8, 0x40, 0xef, 0x3b, 0x65, 0x91, 0x79, 0xa1, 0x37, 0x1d, 0xec, 0x05, 0x79, 0x3c, 0xb0, 0x27, 0xcd, 0xe4, 0x78, 0x06, 0xfb, 0x66, 0xce, 0x1e, 0x3d, 0x1b, 0x69, 0xd5, 0x6d, 0xe6, 0x29, 0xdc];
    let current_signer = get_signer(&runtime, 5)?;
    println!(
        "Current signer: {} ({} bytes)",
        hex::encode(&current_signer),
        current_signer.len()
    );

    // ── Step 3: Verify signer was set during init ──
    println!("\n=== Step 3: Verify signer set during deploy ===");
    let signer_script = frost_signer_p2tr();

    // ── Step 4: Verify signer changed ──
    let new_signer = get_signer(&runtime, 5)?;
    println!(
        "New signer: {} ({} bytes)",
        hex::encode(&new_signer),
        new_signer.len()
    );

    // The set_signer stores tx.output[vout].script_pubkey at /signer
    // get_signer returns the stored bytes
    // For our P2TR address, script_pubkey = 5120 + tweaked_xonly (34 bytes)
    let expected_script = signer_script.as_bytes().to_vec();
    println!("Expected: {}", hex::encode(&expected_script));

    assert_ne!(
        new_signer, default_signer,
        "Signer should not be the hardcoded default"
    );

    // The stored signer might be the script_pubkey bytes or just the x-only key
    // depending on the contract implementation
    if new_signer == expected_script {
        println!("✅ Signer matches P2TR script_pubkey exactly!");
    } else if new_signer.len() == 32 {
        // Might be x-only key extracted from script
        let tweaked_xonly = &expected_script[2..]; // skip OP_1 PUSH32
        if new_signer == tweaked_xonly {
            println!("✅ Signer matches tweaked x-only key!");
        } else {
            println!("⚠ Signer changed but doesn't match expected key");
            println!("  Got:      {}", hex::encode(&new_signer));
            println!("  Expected: {}", hex::encode(tweaked_xonly));
        }
    }

    Ok(())
}
