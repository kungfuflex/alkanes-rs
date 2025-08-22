#![allow(unused_imports)]
use super::helpers::{
    self as alkane_helpers, create_block_with_coinbase_tx,
    create_multiple_cellpack_with_witness_and_txins_edicts, init_with_cellpack_pairs,
    BinaryAndCellpack,
};
use crate::message::AlkaneMessageContext;
use crate::precompiled::{alkanes_std_auth_token_build, fr_btc_build};
use crate::view;
use anyhow::Result;
use bitcoin::{
    consensus::{deserialize, encode},
    transaction::Version,
    Address, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use alkanes_support::{
    cellpack::Cellpack,
    constants::AUTH_TOKEN_FACTORY_ID,
    id::AlkaneId,
    parcel::AlkaneTransfer,
    response::{CallResponse, ExtendedCallResponse},
};
use protorune::Protorune;
use secp256k1::{rand, Secp256k1};

const FR_BTC_ID: AlkaneId = AlkaneId { block: 4, tx: 0 };
const AUTH_TOKEN_ID: AlkaneId = AlkaneId { block: 2, tx: 1 };

#[test]
fn test_fr_btc_unwrap_workflow() -> Result<()> {
    alkane_helpers::clear();

    // 1. Setup test environment
    let secp = Secp256k1::new();
    let (user_sk, user_pk) = secp.generate_keypair(&mut rand::thread_rng());
    let user_address = Address::p2wpkh(&user_pk.into(), bitcoin::Network::Regtest)?;

    let (signer_sk, signer_pk) = secp.generate_keypair(&mut rand::thread_rng());
    let signer_address = Address::p2wpkh(&signer_pk.into(), bitcoin::Network::Regtest)?;

    let mut block0 = create_block_with_coinbase_tx(0);
    let coinbase_txid = block0.txdata[0].txid();
    let user_utxo = OutPoint {
        txid: coinbase_txid,
        vout: 0,
    };
    let signer_utxo = OutPoint {
        txid: coinbase_txid,
        vout: 1,
    };
    block0.txdata[0].output[0].script_pubkey = user_address.script_pubkey();
    block0.txdata[0].output.push(TxOut {
        value: Amount::from_sat(1_000_000_000),
        script_pubkey: signer_address.script_pubkey(),
    });
    Protorune::index_block::<AlkaneMessageContext>(block0.clone(), 0)?;

    // 2. Initialize fr-btc and auth-token
    let init_block = init_with_cellpack_pairs(vec![
        BinaryAndCellpack::new(
            alkanes_std_auth_token_build::get_bytes(),
            Cellpack {
                target: AUTH_TOKEN_FACTORY_ID,
                inputs: vec![100],
            },
        ),
        BinaryAndCellpack::new(
            fr_btc_build::get_bytes(),
            Cellpack {
                target: FR_BTC_ID,
                inputs: vec![0],
            },
        ),
    ]);
    Protorune::index_block::<AlkaneMessageContext>(init_block.clone(), 1)?;

    // 3. Set the signer
    let set_signer_tx = create_multiple_cellpack_with_witness_and_txins_edicts(
        vec![Cellpack {
            target: FR_BTC_ID,
            inputs: vec![1, 0], // Opcode 1: SetSigner, vout: 0
        }],
        vec![TxIn {
            previous_output: user_utxo,
            ..Default::default()
        }],
        false,
        vec![],
    );
    let set_signer_block = create_block_with_coinbase_tx(2, vec![set_signer_tx]);
    Protorune::index_block::<AlkaneMessageContext>(set_signer_block.clone(), 2)?;

    // 4. Wrap BTC to get frBTC
    let wrap_amount = Amount::from_sat(100_000_000); // 1 BTC
    let wrap_tx = create_multiple_cellpack_with_witness_and_txins_edicts(
        vec![Cellpack {
            target: FR_BTC_ID,
            inputs: vec![77, 1], // Opcode 77: Wrap, pointer to vout 1
        }],
        vec![TxIn {
            previous_output: OutPoint {
                txid: set_signer_block.txdata[1].txid(),
                vout: 0,
            },
            ..Default::default()
        }],
        false,
        vec![],
    );
    let wrap_block = create_block_with_coinbase_tx(3, vec![wrap_tx.clone()]);
    Protorune::index_block::<AlkaneMessageContext>(wrap_block.clone(), 3)?;

    // 5. First unwrap
    let unwrap_amount = 50_000_000; // 0.5 BTC
    let unwrap_tx = create_multiple_cellpack_with_witness_and_txins_edicts(
        vec![Cellpack {
            target: FR_BTC_ID,
            inputs: vec![78, 0], // Opcode 78: Unwrap, pointer to vout 0
        }],
        vec![TxIn {
            previous_output: OutPoint {
                txid: wrap_tx.txid(),
                vout: 0,
            },
            ..Default::default()
        }],
        false,
        vec![(AUTH_TOKEN_ID, 1), (FR_BTC_ID, unwrap_amount)],
    );
    let unwrap_block = create_block_with_coinbase_tx(4, vec![unwrap_tx.clone()]);
    Protorune::index_block::<AlkaneMessageContext>(unwrap_block.clone(), 4)?;

    // 6. Get pending unwraps
    let pending_unwraps_res: CallResponse =
        view::call_view(&FR_BTC_ID, &vec![105], u64::MAX)?.try_into()?;
    let mut data = std::io::Cursor::new(pending_unwraps_res.data);
    let inputs: Vec<TxIn> = deserialize(data.get_ref())?;
    let outputs: Vec<TxOut> = deserialize(&data.get_ref()[data.position() as usize..])?;

    assert_eq!(
        inputs.len(),
        2,
        "Should have one accounting and one funding input"
    );
    assert_eq!(
        outputs.len(),
        2,
        "Should have one payout and one change output"
    );

    // 7. Simulate the signer payout transaction
    let payout_tx = Transaction {
        version: bitcoin::transaction::Version(2),
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: inputs,
        output: outputs,
    };
    let payout_block = create_block_with_coinbase_tx(5, vec![payout_tx]);
    Protorune::index_block::<AlkaneMessageContext>(payout_block.clone(), 5)?;

    // 8. Second unwrap
    let unwrap_tx_2 = create_multiple_cellpack_with_witness_and_txins_edicts(
        vec![Cellpack {
            target: FR_BTC_ID,
            inputs: vec![78, 0], // Opcode 78: Unwrap, pointer to vout 0
        }],
        vec![TxIn {
            previous_output: OutPoint {
                txid: unwrap_tx.txid(),
                vout: 0,
            },
            ..Default::default()
        }],
        false,
        vec![(FR_BTC_ID, 25_000_000)], // 0.25 BTC
    );
    let unwrap_block_2 = create_block_with_coinbase_tx(6, vec![unwrap_tx_2]);
    Protorune::index_block::<AlkaneMessageContext>(unwrap_block_2.clone(), 6)?;

    // 9. Get pending unwraps again
    let pending_unwraps_res_2: CallResponse =
        view::call_view(&FR_BTC_ID, &vec![105], u64::MAX)?.try_into()?;
    let mut data_2 = std::io::Cursor::new(pending_unwraps_res_2.data);
    let inputs_2: Vec<TxIn> = deserialize(data_2.get_ref())?;
    let outputs_2: Vec<TxOut> = deserialize(&data_2.get_ref()[data_2.position() as usize..])?;

    assert_eq!(
        inputs_2.len(),
        2,
        "Should have one new accounting input and one funding input"
    );
    assert_eq!(
        outputs_2.len(),
        2,
        "Should have one new payout output and one change output"
    );

    Ok(())
}