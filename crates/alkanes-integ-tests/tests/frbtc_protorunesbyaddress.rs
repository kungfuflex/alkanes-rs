//! Reproduce: frBTC wrap via CLI code path, then query protorunesbyaddress.
//!
//! Uses the same CliBridge / EnhancedAlkanesExecutor pipeline as the real
//! alkanes-cli, then checks whether protorunesbyaddress can find the minted
//! frBTC at the correct address.

use alkanes_cli_common::alkanes::types::{
    EnhancedExecuteParams, InputRequirement, OrdinalsStrategy, OutputTarget, ProtostoneSpec,
};
use alkanes_integ_tests::block_builder::last_tx_outpoint;
use alkanes_integ_tests::cli_bridge::CliBridge;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::{Context, Result};
use bitcoin::hashes::Hash;
use bitcoin::key::TapTweak;
use bitcoin::OutPoint;
use prost::Message;
use std::str::FromStr;

/// Query protorunesbyaddress and return (outpoint_hex, Vec<(block, tx, balance)>).
fn query_protorunesbyaddress(
    runtime: &TestRuntime,
    address: &str,
    height: u32,
) -> Result<Vec<(String, Vec<(u128, u128, u128)>)>> {
    use protorune_support::proto::protorune;
    let mut req = protorune::ProtorunesWalletRequest::default();
    req.wallet = address.as_bytes().to_vec();
    req.protocol_tag = Some(protorune::Uint128 { lo: 1, hi: 0 });
    let resp = runtime
        .alkanes_view("protorunesbyaddress", &req.encode_to_vec(), height)
        .context("protorunesbyaddress failed")?;
    let response = protorune::WalletResponse::decode(resp.as_slice())
        .context("decode WalletResponse")?;
    let mut result = Vec::new();
    for op_resp in &response.outpoints {
        let outpoint_str = op_resp
            .outpoint
            .as_ref()
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

/// Reproduces the proper frBTC wrap flow through the CLI code path.
///
/// The frBTC contract requires vout:0 to pay to the **signer** P2TR address.
/// The subfrost-app uses `toAddresses = [signerAddress, userTaprootAddress]`.
/// The protostone pointer=v1 sends minted frBTC to the user's output.
///
/// This test verifies:
///  1. CLI builds the tx correctly with signer at vout:0
///  2. frBTC is minted at the user's output (vout:1, via pointer)
///  3. protorunesbyaddress finds the frBTC at the user's address
#[test]
fn frbtc_cli_wrap_then_protorunesbyaddress() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    // Mine 1 empty block so genesis frBTC gets set up
    runtime.mine_empty_blocks(0, 1)?;

    // Set up CLI bridge with MockProvider
    let mut bridge = CliBridge::new();
    let cli_address = bridge.address();
    println!("MockProvider (user) address: {}", cli_address);

    // The hardcoded frBTC signer P2TR address (matches SIGNER_PUBKEY in fr_btc.rs)
    let signer_pubkey = bitcoin::key::UntweakedPublicKey::from_slice(&[
        0x79, 0x40, 0xef, 0x3b, 0x65, 0x91, 0x79, 0xa1, 0x37, 0x1d, 0xec, 0x05, 0x79, 0x3c,
        0xb0, 0x27, 0xcd, 0xe4, 0x78, 0x06, 0xfb, 0x66, 0xce, 0x1e, 0x3d, 0x1b, 0x69, 0xd5,
        0x6d, 0xe6, 0x29, 0xdc,
    ])
    .unwrap();
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let (tweaked, _) = signer_pubkey.tap_tweak(&secp, None);
    let signer_address = bitcoin::Address::from_script(
        &bitcoin::ScriptBuf::new_p2tr_tweaked(tweaked),
        bitcoin::Network::Regtest,
    )
    .unwrap()
    .to_string();
    println!("frBTC signer address: {}", signer_address);

    // Give the mock provider a BTC UTXO for funding
    let funding_outpoint = OutPoint::new(
        bitcoin::Txid::from_slice(&[0xbb; 32]).unwrap(),
        0,
    );
    let mock_script = bitcoin::Address::from_str(&cli_address)
        .unwrap()
        .require_network(bitcoin::Network::Regtest)
        .unwrap()
        .script_pubkey();
    bridge.add_utxo(
        funding_outpoint,
        bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(200_000_000), // 2 BTC
            script_pubkey: mock_script.clone(),
        },
    );

    // Build params matching the subfrost-app wrap flow:
    //   toAddresses = [signerAddress, userAddress]
    //   protostone [32,0,77] with pointer=v1 (frBTC goes to user at vout:1)
    //   B:100000000:v0 (send 1 BTC to signer at vout:0)
    let params = EnhancedExecuteParams {
        fee_rate: Some(1.0),
        to_addresses: vec![signer_address.clone(), cli_address.clone()],
        from_addresses: Some(vec![cli_address.clone()]),
        change_address: Some(cli_address.clone()),
        alkanes_change_address: Some(cli_address.clone()),
        input_requirements: vec![
            InputRequirement::BitcoinOutput {
                amount: 100_000_000,
                target: OutputTarget::Output(0),
            },
        ],
        protostones: vec![ProtostoneSpec {
            cellpack: Some(Cellpack {
                target: AlkaneId { block: 32, tx: 0 },
                inputs: vec![77],
            }),
            edicts: vec![],
            bitcoin_transfer: None,
            pointer: Some(OutputTarget::Output(1)),  // frBTC → user (vout:1)
            refund: Some(OutputTarget::Output(1)),
        }],
        envelope_data: None,
        raw_output: true,
        trace_enabled: false,
        mine_enabled: false,
        auto_confirm: true,
        ordinals_strategy: OrdinalsStrategy::default(),
        mempool_indexer: false,
    };

    println!("Executing CLI wrap pipeline...");
    let result = bridge.execute_and_extract_tx(params);
    match &result {
        Ok(tx) => {
            println!(
                "CLI produced tx: {} inputs, {} outputs",
                tx.input.len(),
                tx.output.len()
            );
            for (i, out) in tx.output.iter().enumerate() {
                println!(
                    "  vout:{} value={} script={}",
                    i,
                    out.value,
                    if out.script_pubkey.is_op_return() {
                        "OP_RETURN".to_string()
                    } else {
                        format!("{}...", hex::encode(&out.script_pubkey.as_bytes()[..20.min(out.script_pubkey.len())]))
                    }
                );
            }

            // Wrap in block and index at height 1
            let mut block = protorune::test_helpers::create_block_with_coinbase_tx(1);
            block.txdata.push(tx.clone());
            runtime.index_block(&block, 1)?;

            // Check frBTC balance at the output
            let outpoint = last_tx_outpoint(&block);
            let frbtc_bal = query::get_alkane_balance(&runtime, &outpoint, 32, 0, 1)?;
            println!("\nfrBTC balance at last outpoint: {}", frbtc_bal);

            // Check all outputs for frBTC
            let txid = tx.compute_txid();
            for vout in 0..tx.output.len() as u32 {
                let op = OutPoint::new(txid, vout);
                let bal = query::get_alkane_balance(&runtime, &op, 32, 0, 1)?;
                if bal > 0 {
                    println!("  frBTC at {}:{} = {}", txid, vout, bal);
                }
            }

            // Query protorunesbyaddress for the CLI address
            let results = query_protorunesbyaddress(&runtime, &cli_address, 1)?;
            println!(
                "\nprotorunesbyaddress for {}: {} outpoints",
                cli_address,
                results.len()
            );
            for (outpoint_str, alkanes) in &results {
                for (block, tx, balance) in alkanes {
                    println!("  {} → [{block}:{tx}] = {balance}", outpoint_str);
                }
            }

            let has_frbtc = results
                .iter()
                .any(|(_, alkanes)| alkanes.iter().any(|(b, t, bal)| *b == 32 && *t == 0 && *bal > 0));
            assert!(
                has_frbtc,
                "protorunesbyaddress should find frBTC at the CLI address"
            );
        }
        Err(e) => {
            println!("CLI execute_full failed: {:#}", e);
            return Err(anyhow::anyhow!("CLI wrap pipeline failed: {:#}", e));
        }
    }

    println!("\n✓ frBTC CLI wrap → protorunesbyaddress test complete");
    Ok(())
}
