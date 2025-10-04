use crate::indexer::{index_block};
use crate::tests::helpers::{
    self as alkane_helpers,
    assert_revert_context,
    get_last_outpoint_sheet,
};
use crate::tests::test_runtime::TestRuntime;
use crate::unwrap as unwrap_view;
use crate::unwrap::{deserialize_payments, Payment};
use crate::view::{simulate_parcel};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::response::ExtendedCallResponse;
use anyhow::Result;
use bitcoin::address::NetworkChecked;
use bitcoin::blockdata::transaction::OutPoint;
use bitcoin::key::TapTweak;
use bitcoin::transaction::Version;
use bitcoin::{
    secp256k1::{Secp256k1, XOnlyPublicKey},
    Address, Amount, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use metashrew_support::environment::RuntimeEnvironment;
use metashrew_support::index_pointer::AtomicPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use ordinals::Runestone;
use protorune::message::MessageContextParcel;
use crate::tests::helpers::test_helpers::{create_block_with_coinbase_tx, ADDRESS1};
use crate::tests::helpers::test_helpers::get_address;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations};
use protorune_support::protostone::Protostone;
use protorune::protostone::ProtostoneEncoder;

pub fn simulate_cellpack<E: RuntimeEnvironment + Clone + Default + 'static>(
    env: &mut E,
    height: u64,
    cellpack: Cellpack,
) -> Result<(ExtendedCallResponse, u64)> {
    let parcel = MessageContextParcel {
        atomic: AtomicPointer::<E>::default(),
        runes: vec![],
        transaction: Transaction {
            version: bitcoin::blockdata::transaction::Version::ONE,
            input: vec![],
            output: vec![],
            lock_time: bitcoin::absolute::LockTime::ZERO,
        },
        block: create_block_with_coinbase_tx(height),
        height,
        pointer: 0,
        refund_pointer: 0,
        calldata: cellpack.encipher(),
        sheets: Box::<BalanceSheet<E, AtomicPointer<E>>>::new(BalanceSheet::default()),
        txindex: 0,
        vout: 0,
        runtime_balances: Box::<BalanceSheet<E, AtomicPointer<E>>>::new(BalanceSheet::default()),
        _phantom: std::marker::PhantomData,
    };
    simulate_parcel::<E>(env, &parcel, u64::MAX)
}

pub fn create_frbtc_signer_output() -> TxOut {
    // Get the signer pubkey from the contract
    let signer_pubkey_bytes = vec![
        0x79, 0x40, 0xef, 0x3b, 0x65, 0x91, 0x79, 0xa1, 0x37, 0x1d, 0xec, 0x05, 0x79, 0x3c, 0xb0,
        0x27, 0xcd, 0xe4, 0x78, 0x06, 0xfb, 0x66, 0xce, 0x1e, 0x3d, 0x1b, 0x69, 0xd5, 0x6d, 0xe6,
        0x29, 0xdc,
    ];
    let signer_pubkey = XOnlyPublicKey::from_slice(&signer_pubkey_bytes).unwrap();
    let secp = Secp256k1::new();
    let (tweaked_signer_pubkey, _) = signer_pubkey.tap_tweak(&secp, None);
    let signer_script = ScriptBuf::new_p2tr_tweaked(tweaked_signer_pubkey);

    return TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey: signer_script,
    };
}

pub fn create_alkane_tx_frbtc_signer_script<E: RuntimeEnvironment>(
    cellpacks: Vec<Cellpack>,
    previous_output: OutPoint,
) -> Result<Transaction> {
    let txins = vec![TxIn {
        previous_output,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::default(),
    }];
    let protocol_id = 1;
    let mut protostones: Vec<Protostone> = cellpacks
        .into_iter()
        .map(|cellpack| {
            Ok(Protostone {
                message: cellpack.encipher(),
                pointer: Some(0),
                refund: Some(0),
                edicts: vec![],
                from: None,
                burn: None,
                protocol_tag: protocol_id as u128,
            })
        })
        .collect::<Result<Vec<Protostone>>>()?;
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
        protocol: Some(<Vec<Protostone> as ProtostoneEncoder<TestRuntime>>::encipher(&protostones)?),
    })
    .encipher();

    //     // op return is at output 1
    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };

    let txout = create_frbtc_signer_output();
    Ok(Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: txins,
        output: vec![txout, op_return],
    })
}

fn wrap_btc<E: RuntimeEnvironment + Clone + Default + 'static>() -> Result<(OutPoint, u64)> {
    let fr_btc_id = AlkaneId { block: 32, tx: 0 };
    let mut block = create_block_with_coinbase_tx(1 as u64);
    let funding_outpoint = OutPoint {
        txid: block.txdata[0].compute_txid(),
        vout: 0,
    };
    let wrap_tx = create_alkane_tx_frbtc_signer_script::<E>(
        vec![Cellpack {
            target: fr_btc_id.clone(),
            inputs: vec![77],
        }],
        funding_outpoint,
    )?;

    // Create a block and index it
    block.txdata.push(wrap_tx.clone());
    let mut env = E::default();
    index_block::<E>(&mut env, &block, 1 as u32)?;

    let sheet = get_last_outpoint_sheet::<E>(&mut env, &block)?;
    let balance = sheet.get(&fr_btc_id.clone().into(), &mut env);

    let expected_frbtc_amt = 99500000;

    assert_eq!(balance, expected_frbtc_amt);
    assert_eq!(
        sheet.get(&AlkaneId { block: 2, tx: 0 }.into(), &mut env),
        5000000000
    );

    let wrap_outpoint = OutPoint {
        txid: wrap_tx.compute_txid(),
        vout: 0,
    };

    Ok((wrap_outpoint, expected_frbtc_amt as u64))
}

fn unwrap_btc_tx<E: RuntimeEnvironment>(
    fr_btc_input_outpoint: OutPoint,
    amount_frbtc: u64,
    desired_vout: u128,
) -> Result<Transaction> {
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
        protocol: Some(<Vec<Protostone> as ProtostoneEncoder<TestRuntime>>::encipher(&protostone)?),
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
    Ok(Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: txins,
        output: vec![my_txout, signer_txout, op_return],
    })
}

fn unwrap_btc<E: RuntimeEnvironment + Clone + Default + 'static>(
    fr_btc_input_outpoint: OutPoint,
    amount_original_frbtc: u64,
    amount_frbtc_to_burn: u64,
    vout: u128,
    height: u32,
) -> Result<()> {
    let fr_btc_id = AlkaneId { block: 32, tx: 0 };
    let mut block = create_block_with_coinbase_tx(height as u64);
    let unwrap_tx = unwrap_btc_tx::<E>(fr_btc_input_outpoint, amount_frbtc_to_burn, vout)?;
    let amt_actual_burn = std::cmp::min(amount_original_frbtc, amount_frbtc_to_burn);

    // Create a block and index it
    block.txdata.push(unwrap_tx.clone());
    let mut env = E::default();
    index_block::<E>(&mut env, &block, height as u32)?;

    let sheet = get_last_outpoint_sheet::<E>(&mut env, &block)?;
    let balance = sheet.get(&fr_btc_id.clone().into(), &mut env);

    assert_eq!(balance as u64, amount_original_frbtc - amt_actual_burn);

    let (response, _) =
        simulate_cellpack::<E>(
            &mut env,
            height as u64,
            Cellpack {
                target: AlkaneId { block: 32, tx: 0 },
                inputs: vec![101],
            },
        )?;

    let payments = deserialize_payments(&response.data)?;
    let expected_payment = Payment {
        output: TxOut {
            script_pubkey: unwrap_tx.output[0].script_pubkey.clone(),
            value: Amount::from_sat(amt_actual_burn),
        },
        spendable: OutPoint {
            txid: unwrap_tx.compute_txid(),
            vout: vout.try_into()?,
        },
        fulfilled: false,
    };

    assert_eq!(payments[0], expected_payment);
    assert_eq!(
        sheet.get(&AlkaneId { block: 2, tx: 0 }.into(), &mut env),
        5000000000
    );

    let response = unwrap_view::view::<E>(&mut env, height as u128).unwrap();
    assert_eq!(response.payments[0], expected_payment.into());

    Ok(())
}

fn set_signer<E: RuntimeEnvironment + Clone + Default + 'static>(
    env: &mut E,
    input_outpoint: OutPoint,
    signer_vout: u128,
) -> Result<Transaction> {
    let fr_btc_id = AlkaneId { block: 32, tx: 0 };
    let height = 3;
    let mut block = create_block_with_coinbase_tx(height as u64);
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
    index_block::<E>(env, &block, height as u32)?;

    Ok(set_signer)
}

#[test]
fn test_fr_btc_wrap_correct_signer() -> Result<()> {
    wrap_btc::<TestRuntime>()?;
    Ok(())
}

#[test]
fn test_fr_btc_unwrap() -> Result<()> {
    let (wrap_out, amt) = wrap_btc::<TestRuntime>()?;
    unwrap_btc::<TestRuntime>(wrap_out, amt, amt, 1, 2)
}

#[test]
fn test_fr_btc_unwrap_partial() -> Result<()> {
    let (wrap_out, amt) = wrap_btc::<TestRuntime>()?;
    unwrap_btc::<TestRuntime>(wrap_out, amt, amt / 2, 1, 2)
}

#[test]
fn test_fr_btc_unwrap_more() -> Result<()> {
    let (wrap_out, amt) = wrap_btc::<TestRuntime>()?;
    unwrap_btc::<TestRuntime>(wrap_out, amt, amt * 2, 1, 2)
}

#[test]
fn test_set_signer_no_auth() -> Result<()> {
    let mut env = TestRuntime::default();
    let set_signer_tx = set_signer::<TestRuntime>(&mut env, OutPoint::default(), 0)?;
    let outpoint = OutPoint {
        txid: set_signer_tx.compute_txid(),
        vout: 3,
    };
    assert_revert_context(&mut env, &outpoint, "Auth token is not in incoming alkanes")?;
    Ok(())
}

#[test]
fn test_fr_btc_wrap_incorrect_signer() -> Result<()> {
    let fr_btc_id = AlkaneId { block: 32, tx: 0 };
    let mut block = create_block_with_coinbase_tx(880_001 as u64);
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
    let mut env = TestRuntime::default();
    index_block::<TestRuntime>(&mut env, &block, 880_001 as u32)?;

    let sheet = get_last_outpoint_sheet::<TestRuntime>(&mut env, &block)?;
    let balance = sheet.get(&fr_btc_id.clone().into(), &mut env);

    // No BTC sent to correct signer, so no frBTC should be minted.
    assert_eq!(balance, 0);

    Ok(())
}

#[test]
fn test_last_block_updated_after_unwrap_fulfillment() -> Result<()> {
    let (wrap_outpoint, fr_btc_amount) = wrap_btc::<TestRuntime>()?; // height 1

    // Unwrap at height 2
    let height2 = 2;
    let vout_for_spendable = 1;
    let unwrap_tx =
        unwrap_btc_tx::<TestRuntime>(wrap_outpoint, fr_btc_amount, vout_for_spendable as u128)?;

    let mut env = TestRuntime::default();
    let mut block2 = create_block_with_coinbase_tx(height2 as u64);
    block2.txdata.push(unwrap_tx.clone());
    index_block::<TestRuntime>(&mut env, &block2, height2 as u32)?;

    // Before fulfillment, last_block should not have advanced past the block with unfulfilled payment
    let last_block_before: u128 = unwrap_view::fr_btc_storage_pointer::<TestRuntime>()
        .keyword("/last_block")
        .get_value(&mut env);

    // wrap_btc is at height 1, which has no payments. So last_block becomes 1.
    // unwrap_btc is at height 2, which has an unfulfilled payment. So last_block stays 1.
    assert_eq!(last_block_before, 1);

    // Check view has one payment
    let unwrap_view_response_before = unwrap_view::view::<TestRuntime>(&mut env, height2 as u128)?;
    assert_eq!(unwrap_view_response_before.payments.len(), 1);

    let height3 = 3;
    let spendable_outpoint = OutPoint {
        txid: unwrap_tx.compute_txid(),
        vout: vout_for_spendable as u32,
    };

    // let spendable_bytes = protorune_support::utils::consensus_encode(&spendable_outpoint)?;
    // protorune::tables::OUTPOINT_SPENDABLE_BY.select(&spendable_bytes).set(Arc::new(vec![]));
    crate::unwrap::update_last_block::<TestRuntime>(&mut env, height3 as u128)?;

    // After fulfillment, last_block should be updated to the latest block
    let last_block_after: u128 = unwrap_view::fr_btc_storage_pointer::<TestRuntime>()
        .keyword("/last_block")
        .get_value(&mut env);
    assert_eq!(last_block_after, height3 as u128);

    // Check view has no payments because the processed blocks are skipped
    let unwrap_view_response_after = unwrap_view::view::<TestRuntime>(&mut env, height3 as u128)?;
    assert_eq!(unwrap_view_response_after.payments.len(), 0);

    Ok(())
}