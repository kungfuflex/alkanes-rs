use crate::message::AlkaneMessageContext;
use crate::precompiled::{alkanes_std_auth_token_build, fr_btc_build};
use crate::view::{self, simulate_parcel, unwrap};
use alkanes_support::constants::AUTH_TOKEN_FACTORY_ID;
use alkanes_support::gz::compress;
use alkanes_support::id::AlkaneId;
use alkanes_support::response::ExtendedCallResponse;
use alkanes_support::trace::Trace;
use anyhow::Result;
use bitcoin::address::NetworkChecked;
use bitcoin::blockdata::transaction::OutPoint;
use bitcoin::key::TapTweak;
use bitcoin::transaction::Version;
use bitcoin::{
    secp256k1::{self, Secp256k1, XOnlyPublicKey},
    Address, Amount, Block, Script, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
#[allow(unused_imports)]
use hex;
use metashrew_core::index_pointer::AtomicPointer;
use metashrew_support::index_pointer::KeyValuePointer;
#[allow(unused_imports)]
use metashrew_support::utils::format_key;
use protorune::message::MessageContextParcel;
use protorune::protostone::Protostones;
use protorune::test_helpers::{create_block_with_coinbase_tx, ADDRESS1};
use protorune::{
    balance_sheet::load_sheet, message::MessageContext, tables::RuneTable,
    test_helpers::get_address,
};
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::protostone::Protostone;

use crate::indexer::index_block;
use crate::network::set_view_mode;
use crate::tests::helpers::{
    self as alkane_helpers, assert_return_context, assert_revert_context, clear,
    get_last_outpoint_sheet,
};
use crate::unwrap::{deserialize_payments, Payment};
use alkanes_support::cellpack::Cellpack;
#[allow(unused_imports)]
use metashrew_core::{get_cache, index_pointer::IndexPointer, println, stdio::stdout};
use ordinals::{Artifact, Runestone};
use protorune_support::utils::consensus_encode;
use std::fmt::Write;
use wasm_bindgen_test::wasm_bindgen_test;

pub fn simulate_cellpack(height: u64, cellpack: Cellpack) -> Result<(ExtendedCallResponse, u64)> {
    let parcel = MessageContextParcel {
        atomic: AtomicPointer::default(),
        runes: vec![],
        transaction: Transaction {
            version: bitcoin::blockdata::transaction::Version::ONE,
            input: vec![],
            output: vec![],
            lock_time: bitcoin::absolute::LockTime::ZERO,
        },
        block: create_block_with_coinbase_tx(height as u32),
        height,
        pointer: 0,
        refund_pointer: 0,
        calldata: cellpack.encipher(),
        sheets: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
        txindex: 0,
        vout: 0,
        runtime_balances: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
    };
    simulate_parcel(&parcel, u64::MAX)
}

pub fn create_frbtc_signer_output() -> TxOut {
    // Get the signer pubkey from the contract
    let signer_pubkey_bytes = [
        0x79, 0x40, 0xef, 0x3b, 0x65, 0x91, 0x79, 0xa1, 0x37, 0x1d, 0xec, 0x05, 0x79, 0x3c, 0xb0,
        0x27, 0xcd, 0xe4, 0x78, 0x06, 0xfb, 0x66, 0xce, 0x1e, 0x3d, 0x1b, 0x69, 0xd5, 0x6d, 0xe6,
        0x29, 0xdc,
    ]
    .to_vec();
    let signer_pubkey = XOnlyPublicKey::from_slice(&signer_pubkey_bytes).unwrap();
    let secp = Secp256k1::new();
    let (tweaked_signer_pubkey, _) = signer_pubkey.tap_tweak(&secp, None);
    let signer_script = ScriptBuf::new_p2tr_tweaked(tweaked_signer_pubkey);

    return TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey: signer_script,
    };
}

pub fn create_alkane_tx_frbtc_signer_script(
    cellpacks: Vec<Cellpack>,
    previous_output: OutPoint,
) -> Transaction {
    let txins = vec![TxIn {
        previous_output,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::default(),
    }];
    let protocol_id = 1;
    let mut protostones: Vec<Protostone> = [cellpacks
        .into_iter()
        .map(|cellpack| Protostone {
            message: cellpack.encipher(),
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
            from: None,
            burn: None,
            protocol_tag: protocol_id as u128,
        })
        .collect::<Vec<Protostone>>()]
    .concat();
    protostones.push(Protostone {
        // mint diesel test
        message: Cellpack {
            target: AlkaneId { block: 2, tx: 0 },
            inputs: vec![77],
        }
        .encipher(),
        pointer: Some(0),
        refund: Some(0),
        edicts: vec![],
        from: None,
        burn: None,
        protocol_tag: protocol_id as u128,
    });
    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: Some(0),
        edicts: Vec::new(),
        mint: None,
        protocol: protostones.encipher().ok(),
    })
    .encipher();

    //     // op return is at output 1
    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };

    let txout = create_frbtc_signer_output();
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: txins,
        output: vec![txout, op_return],
    }
}

fn wrap_btc() -> Result<(OutPoint, u64)> {
    let fr_btc_id = AlkaneId { block: 32, tx: 0 };
    let mut block = create_block_with_coinbase_tx(880_001);
    let funding_outpoint = OutPoint {
        txid: block.txdata[0].compute_txid(),
        vout: 0,
    };
    let wrap_tx = create_alkane_tx_frbtc_signer_script(
        vec![Cellpack {
            target: fr_btc_id.clone(),
            inputs: vec![77],
        }],
        funding_outpoint,
    );

    // Create a block and index it
    block.txdata.push(wrap_tx.clone());
    index_block(&block, 880_001)?;

    let sheet = get_last_outpoint_sheet(&block)?;
    let balance = sheet.get(&fr_btc_id.clone().into());

    let expected_frbtc_amt = 99500000;

    assert_eq!(balance, expected_frbtc_amt);
    assert_eq!(sheet.get(&AlkaneId { block: 2, tx: 0 }.into()), 312500000);

    let wrap_outpoint = OutPoint {
        txid: wrap_tx.compute_txid(),
        vout: 0,
    };

    Ok((wrap_outpoint, expected_frbtc_amt as u64))
}

fn unwrap_btc_tx(
    fr_btc_input_outpoint: OutPoint,
    amount_frbtc: u64,
    desired_vout: u128,
) -> Transaction {
    let fr_btc_id = AlkaneId { block: 32, tx: 0 };
    let txins = vec![TxIn {
        previous_output: fr_btc_input_outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::default(),
    }];
    let protocol_id = 1;
    let protostone: Vec<Protostone> = vec![Protostone {
        message: Cellpack {
            target: fr_btc_id.clone(),
            inputs: vec![78, desired_vout, amount_frbtc as u128],
        }
        .encipher(),
        pointer: Some(0),
        refund: Some(0),
        edicts: vec![],
        from: None,
        burn: None,
        protocol_tag: protocol_id as u128,
    }];
    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: Some(0),
        edicts: Vec::new(),
        mint: None,
        protocol: protostone.encipher().ok(),
    })
    .encipher();

    //     // op return is at output 1
    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };

    let signer_txout = create_frbtc_signer_output();

    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());

    let script_pubkey = address.script_pubkey();
    let my_txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey,
    };
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: txins,
        output: vec![my_txout, signer_txout, op_return],
    }
}

fn unwrap_btc(
    fr_btc_input_outpoint: OutPoint,
    amount_original_frbtc: u64,
    amount_frbtc_to_burn: u64,
    vout: u128,
    height: u32,
) -> Result<()> {
    let fr_btc_id = AlkaneId { block: 32, tx: 0 };
    let mut block = create_block_with_coinbase_tx(height);
    let unwrap_tx = unwrap_btc_tx(fr_btc_input_outpoint, amount_frbtc_to_burn, vout);
    let amt_actual_burn = std::cmp::min(amount_original_frbtc, amount_frbtc_to_burn);

    // Create a block and index it
    block.txdata.push(unwrap_tx.clone());
    index_block(&block, height)?;

    let sheet = get_last_outpoint_sheet(&block)?;
    let balance = sheet.get(&fr_btc_id.clone().into());

    assert_eq!(balance as u64, amount_original_frbtc - amt_actual_burn);

    let (response, _) = simulate_cellpack(
        height as u64,
        Cellpack {
            target: AlkaneId { block: 4, tx: 0 },
            inputs: vec![101],
        },
    )?;

    let payments = deserialize_payments(&response.data)?;

    assert_eq!(
        payments[0],
        Payment {
            output: TxOut {
                script_pubkey: unwrap_tx.output[0].script_pubkey.clone(),
                value: Amount::from_sat(amt_actual_burn),
            },
            spendable: OutPoint {
                txid: unwrap_tx.compute_txid(),
                vout: vout.try_into()?,
            },
            fulfilled: false,
        }
    );
    assert_eq!(sheet.get(&AlkaneId { block: 2, tx: 0 }.into()), 312500000);

    Ok(())
}

fn set_signer(input_outpoint: OutPoint, signer_vout: u128) -> Result<Transaction> {
    let fr_btc_id = AlkaneId { block: 32, tx: 0 };
    let height = 880_000;
    let mut block = create_block_with_coinbase_tx(height);
    let set_signer = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::default(),
        vec![Cellpack {
            target: fr_btc_id.clone(),
            inputs: vec![1, signer_vout],
        }],
        input_outpoint,
        false,
    );

    // Create a block and index it
    block.txdata.push(set_signer.clone());
    index_block(&block, height)?;

    Ok(set_signer)
}

#[wasm_bindgen_test]
fn test_fr_btc_wrap_correct_signer() -> Result<()> {
    clear();
    wrap_btc()?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_fr_btc_unwrap() -> Result<()> {
    clear();
    let (wrap_out, amt) = wrap_btc()?;
    unwrap_btc(wrap_out, amt, amt, 1, 880_002)
}

#[wasm_bindgen_test]
fn test_fr_btc_unwrap_partial() -> Result<()> {
    clear();
    let (wrap_out, amt) = wrap_btc()?;
    unwrap_btc(wrap_out, amt, amt / 2, 1, 880_002)
}

#[wasm_bindgen_test]
fn test_fr_btc_unwrap_more() -> Result<()> {
    clear();
    let (wrap_out, amt) = wrap_btc()?;
    unwrap_btc(wrap_out, amt, amt * 2, 1, 880_002)
}

#[wasm_bindgen_test]
fn test_set_signer_no_auth() -> Result<()> {
    clear();
    let set_signer_tx = set_signer(OutPoint::default(), 0)?;
    let outpoint = OutPoint {
        txid: set_signer_tx.compute_txid(),
        vout: 3,
    };
    assert_revert_context(&outpoint, "Auth token is not in incoming alkanes")?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_fr_btc_wrap_incorrect_signer() -> Result<()> {
    clear();
    let fr_btc_id = AlkaneId { block: 32, tx: 0 };
    let mut block = create_block_with_coinbase_tx(880_001);
    let funding_outpoint = OutPoint {
        txid: block.txdata[0].compute_txid(),
        vout: 0,
    };
    let wrap_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::default(),
        vec![Cellpack {
            target: fr_btc_id.clone(),
            inputs: vec![77],
        }],
        funding_outpoint,
        false,
    );

    // Create a block and index it
    block.txdata.push(wrap_tx.clone());
    index_block(&block, 880_001)?;

    let sheet = get_last_outpoint_sheet(&block)?;
    let balance = sheet.get(&fr_btc_id.clone().into());

    // No BTC sent to correct signer, so no frBTC should be minted.
    assert_eq!(balance, 0);

    Ok(())
}
