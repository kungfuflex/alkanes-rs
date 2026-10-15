//! testnet4 (`--features testnet`): every consensus rule is live from block 0.
//!
//! Locks in the testnet genesis module: no height-gated activation is left
//! pending, `2:0` resolves to the testnet DIESEL v3 build (V3_ACTIVATION_HEIGHT
//! = 0) at every height, and the genesis block initializes v3 directly (its
//! premine lands on the genesis outpoint, exactly as on regtest).
#![cfg(feature = "testnet")]

use crate::index_block;
use crate::network::{
    diesel_v3_bytes, genesis, genesis_alkane_wasm_for_height, precompiled_alkane_wasm_for_height,
};
use crate::precompiled::{alkanes_std_diesel_v3_regtest_build, alkanes_std_diesel_v3_testnet_build};
use crate::tests::fr_btc::simulate_cellpack;
use crate::tests::helpers::{assert_return_context, clear};
use alkanes::message::AlkaneMessageContext;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::transaction::Version;
use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Txid, Witness};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use ordinals::{Artifact, Runestone};
use protorune::balance_sheet::{load_sheet, PersistentRecord};
use protorune::message::MessageContext;
use protorune::protostone::Protostones;
use protorune::tables::RuneTable;
use protorune::test_helpers::create_block_with_coinbase_tx;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::protostone::Protostone;
use protorune_support::utils::consensus_encode;
use std::str::FromStr;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn testnet_has_no_pending_activation() {
    assert_eq!(genesis::GENESIS_BLOCK, 0);
    assert_eq!(genesis::GENESIS_OUTPOINT_BLOCK_HEIGHT, 0);
    assert_eq!(genesis::GENESIS_UPGRADE_BLOCK_HEIGHT, 0);
    assert_eq!(genesis::GENESIS_UPGRADE_EOA_BLOCK_HEIGHT, 0);
    assert_eq!(genesis::DIESEL_V3_BLOCK_HEIGHT, 0);
    assert_eq!(genesis::V220_FORK_HEIGHT, 0);
    assert_eq!(genesis::FRBTC_V130_FORK_HEIGHT, 0);
    assert_eq!(genesis::FRBTC_V131_FORK_HEIGHT, 0);
    assert_eq!(protorune::POST_AUDIT_FORK_HEIGHT, 0);
    assert_eq!(protorune::V217_FIX_HEIGHT, 0);
    assert!(protorune::post_audit_fork_active(0));
}

#[wasm_bindgen_test]
fn testnet_2_0_is_diesel_v3_testnet_build_from_genesis() {
    let v3 = alkanes_std_diesel_v3_testnet_build::get_bytes();
    assert_ne!(v3, alkanes_std_diesel_v3_regtest_build::get_bytes());
    assert_eq!(diesel_v3_bytes(), v3);
    for h in [0u32, 1, 50_000, 156_000, 2_000_000] {
        assert_eq!(genesis_alkane_wasm_for_height(h), v3, "height {}", h);
        assert_eq!(
            precompiled_alkane_wasm_for_height(&AlkaneId { block: 2, tx: 0 }, h),
            Some(v3.clone())
        );
    }
}

#[wasm_bindgen_test]
fn testnet_genesis_block_initializes_v3() -> Result<()> {
    clear();
    let block = create_block_with_coinbase_tx(0);
    index_block(&block, 0)?;
    let outpoint = OutPoint {
        txid: Txid::from_byte_array(
            hex::decode(genesis::GENESIS_OUTPOINT)?
                .into_iter()
                .rev()
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap(),
        ),
        vout: 0,
    };
    let sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint)?),
    );
    assert_eq!(sheet.get(&ProtoruneRuneId { block: 2, tx: 0 }), 50_000_000u128);
    // a second block indexes cleanly on top
    index_block(&create_block_with_coinbase_tx(1), 1)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The testnet4 genesis outpoint: a REAL hotpurse output whose creating tx is
// indexed long after block 0 credits it.
// ---------------------------------------------------------------------------

/// testnet4 tx 37ecd12e…87fd (height 156_445), byte for byte: a reverted
/// 4:9010 deploy reveal. vout 0 = hotpurse, 546 sat, no alkanes of its own.
const T4_GENESIS_TX: &[u8] = include_bytes!("static/testnet4_37ecd12e_tx.bin");
const T4_GENESIS_TX_HEIGHT: u32 = 156_445;
/// P2TR scripts: hotpurse (holder of 32:1) and the t4-frbtc 6-of-9 group.
const HOTPURSE_SCRIPT: &str = "5120da6ffee8fd5deff663319127b995240adb04dc91b200530606c100b89e96ea07";
const T4_FRBTC_GROUP_SCRIPT: &str =
    "512030685674457c3eaa6d2904a1340ab80296625e8c7313f54c6ae7c51318f58607";

const FRBTC: AlkaneId = AlkaneId { block: 32, tx: 0 };
const FRSIGIL: ProtoruneRuneId = ProtoruneRuneId { block: 32, tx: 1 };
const DIESEL: ProtoruneRuneId = ProtoruneRuneId { block: 2, tx: 0 };

fn genesis_outpoint() -> OutPoint {
    OutPoint {
        txid: Txid::from_str(genesis::GENESIS_OUTPOINT).unwrap(),
        vout: 0,
    }
}

fn tag1_sheet(outpoint: &OutPoint) -> Result<BalanceSheet<IndexPointer>> {
    Ok(load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(outpoint)?),
    ))
}

/// frBTC opcode 103 (`get_signer`), the same call `alkanes_simulate 32:0:103` makes.
fn frbtc_signer(height: u64) -> Result<Vec<u8>> {
    let (response, _) = simulate_cellpack(
        height,
        Cellpack {
            target: FRBTC,
            inputs: vec![103],
        },
    )?;
    Ok(response.data)
}

fn p2tr_out(script_hex: &str) -> TxOut {
    TxOut {
        value: Amount::from_sat(546),
        script_pubkey: ScriptBuf::from_bytes(hex::decode(script_hex).unwrap()),
    }
}

#[wasm_bindgen_test]
fn testnet_genesis_outpoint_is_the_real_hotpurse_output() -> Result<()> {
    let tx: Transaction = bitcoin::consensus::deserialize(T4_GENESIS_TX)?;
    assert_eq!(tx.compute_txid(), genesis_outpoint().txid);
    assert_eq!(
        tx.output[0].script_pubkey.as_bytes(),
        hex::decode(HOTPURSE_SCRIPT)?.as_slice(),
        "vout 0 must be the hotpurse holder"
    );
    // still the GENESIS_OUTPOINT_BLOCK_HEIGHT = 0 convention (see network.rs)
    assert_eq!(genesis::GENESIS_OUTPOINT_BLOCK_HEIGHT, 0);
    Ok(())
}

/// End to end on the testnet genesis module:
/// 1. block 0 credits frSIGIL 32:1 (and the DIESEL premine) to the outpoint;
/// 2. the REAL creating tx, indexed at its real height, does not clobber it;
/// 3. hotpurse spends it with a protostone calling 32:0 `set_signer` with the
///    32:1 in its inputs, and opcode 103 then returns the new group key.
#[wasm_bindgen_test]
fn testnet_frsigil_on_real_outpoint_survives_creation_and_sets_signer() -> Result<()> {
    clear();
    index_block(&create_block_with_coinbase_tx(0), 0)?;
    let op = genesis_outpoint();

    // (1) credited at genesis
    let sheet = tag1_sheet(&op)?;
    assert_eq!(sheet.get(&FRSIGIL), 1, "frSIGIL 32:1 on the genesis outpoint");
    assert_eq!(sheet.get(&DIESEL), 50_000_000u128);
    let default_signer = frbtc_signer(1)?;
    assert_eq!(
        hex::encode(&default_signer),
        "7940ef3b659179a1371dec05793cb027cde47806fb66ce1e3d1b69d56de629dc",
        "before set_signer: DEFAULT_SIGNER_PUBKEY"
    );

    // (2) the creating tx arrives at its real height
    let creating: Transaction = bitcoin::consensus::deserialize(T4_GENESIS_TX)?;
    assert!(
        matches!(Runestone::decipher(&creating), Some(Artifact::Runestone(_))),
        "the creating tx carries a runestone + protostone"
    );
    let mut block = create_block_with_coinbase_tx(T4_GENESIS_TX_HEIGHT);
    block.txdata.push(creating.clone());
    index_block(&block, T4_GENESIS_TX_HEIGHT)?;
    let sheet = tag1_sheet(&op)?;
    assert_eq!(sheet.get(&FRSIGIL), 1, "creating tx clobbered 32:1");
    assert_eq!(sheet.get(&DIESEL), 50_000_000u128, "creating tx clobbered the premine");
    assert_eq!(
        sheet.balances().values().filter(|v| **v != 0).count(),
        2,
        "creating tx added alkanes to vout 0: {:?}",
        sheet.balances()
    );

    // (3) spend it: [32,0,1,<vout 1>] with pointer/refund v0 (back to hotpurse),
    //     the new signer is the script of v1 (t4-frbtc group).
    let height = T4_GENESIS_TX_HEIGHT + 1;
    let protostones = vec![Protostone {
        message: Cellpack {
            target: FRBTC,
            inputs: vec![1, 1],
        }
        .encipher(),
        pointer: Some(0),
        refund: Some(0),
        edicts: vec![],
        from: None,
        burn: None,
        protocol_tag: AlkaneMessageContext::protocol_tag(),
    }];
    let runestone = Runestone {
        etching: None,
        pointer: Some(0),
        edicts: vec![],
        mint: None,
        protocol: protostones.encipher().ok(),
    }
    .encipher();
    let spend = Transaction {
        version: Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: op,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::default(),
        }],
        output: vec![
            p2tr_out(HOTPURSE_SCRIPT),
            p2tr_out(T4_FRBTC_GROUP_SCRIPT),
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: runestone,
            },
        ],
    };
    let mut block = create_block_with_coinbase_tx(height);
    block.txdata.push(spend.clone());
    index_block(&block, height)?;

    // the protomessage (virtual vout = outputs + 1) returned, it did not revert
    let msg_vout = OutPoint {
        txid: spend.compute_txid(),
        vout: spend.output.len() as u32 + 1,
    };
    assert_return_context(&msg_vout, |_| Ok(()))?;

    let new_signer = frbtc_signer(height as u64 + 1)?;
    assert_eq!(
        hex::encode(&new_signer),
        T4_FRBTC_GROUP_SCRIPT,
        "opcode 103 must return the t4-frbtc group script after set_signer"
    );
    assert_ne!(new_signer, default_signer);

    // 32:1 and the premine moved to the spend's v0; the genesis outpoint is empty.
    let back = tag1_sheet(&OutPoint {
        txid: spend.compute_txid(),
        vout: 0,
    })?;
    assert_eq!(back.get(&FRSIGIL), 1);
    assert_eq!(back.get(&DIESEL), 50_000_000u128);
    let spent = tag1_sheet(&op)?;
    assert_eq!(spent.get(&FRSIGIL), 0);
    assert_eq!(spent.get(&DIESEL), 0);
    Ok(())
}

/// Same no-clobber property for a PLAIN creating tx (no OP_RETURN, no
/// runestone): credit a tx's future vout 0 the way setup_frsigil does, then
/// index the tx. It goes through `transfer_runes_without_runestone` +
/// `settle_protocol_inputs_fallback`, which leave a pre-existing tag-1 sheet
/// alone when the tx's inputs carry no alkanes.
#[wasm_bindgen_test]
fn testnet_plain_creating_tx_does_not_clobber_precredited_output() -> Result<()> {
    clear();
    index_block(&create_block_with_coinbase_tx(0), 0)?;
    let plain = Transaction {
        version: Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: Txid::from_byte_array([7u8; 32]),
                vout: 1,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::default(),
        }],
        output: vec![p2tr_out(HOTPURSE_SCRIPT), p2tr_out(T4_FRBTC_GROUP_SCRIPT)],
    };
    let out0 = OutPoint {
        txid: plain.compute_txid(),
        vout: 0,
    };
    let mut atomic = AtomicPointer::default();
    let mut credit = BalanceSheet::<AtomicPointer>::default();
    credit.set(&FRSIGIL, 1);
    credit.save(
        &mut atomic.derive(
            &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&out0)?),
        ),
        false,
    );
    atomic.commit();
    assert_eq!(tag1_sheet(&out0)?.get(&FRSIGIL), 1);

    let mut block = create_block_with_coinbase_tx(10);
    block.txdata.push(plain);
    index_block(&block, 10)?;
    assert_eq!(tag1_sheet(&out0)?.get(&FRSIGIL), 1, "plain creating tx clobbered the credit");
    Ok(())
}
