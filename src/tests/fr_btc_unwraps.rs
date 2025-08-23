#![allow(unused_imports)]
use super::helpers::{
    self as alkane_helpers,
    create_multiple_cellpack_with_witness_and_txins_edicts, init_with_cellpack_pairs,
    BinaryAndCellpack,
};
use crate::message::AlkaneMessageContext;
use crate::precompiled::{alkanes_std_auth_token_build, fr_btc_build};
use crate::view;
use anyhow::Result;
use bitcoin::{
    consensus::{deserialize, encode},
    secp256k1::{rand, Secp256k1},
    transaction::Version,
    Address, Amount, OutPoint, PublicKey, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use alkanes_support::{
    cellpack::Cellpack,
    constants::AUTH_TOKEN_FACTORY_ID,
    id::AlkaneId,
    parcel::AlkaneTransfer,
    response::{CallResponse, ExtendedCallResponse},
};
use ordinals::RuneId;
use protorune::{test_helpers::create_block_with_coinbase_tx, Protorune};
use protorune_support::{balance_sheet::ProtoruneRuneId, protostone::ProtostoneEdict};

const FR_BTC_ID: AlkaneId = AlkaneId { block: 32, tx: 0 };
const FR_SIGIL_ID: AlkaneId = AlkaneId { block: 32, tx: 1 };

#[test]
fn test_fr_btc_unwrap_workflow() -> Result<()> {
    alkane_helpers::clear();

    // 1. Setup test environment
    let secp = Secp256k1::new();
    let (_user_sk, user_pk) = secp.generate_keypair(&mut rand::thread_rng());
    let user_address = Address::p2wpkh(&PublicKey::new(user_pk), bitcoin::Network::Regtest).unwrap();

    let (_signer_sk, signer_pk) = secp.generate_keypair(&mut rand::thread_rng());
    let signer_address = Address::p2wpkh(&PublicKey::new(signer_pk), bitcoin::Network::Regtest).unwrap();

    let mut block0 = create_block_with_coinbase_tx(0);
    let coinbase_txid = block0.txdata[0].compute_txid();
    let user_utxo = OutPoint {
        txid: coinbase_txid,
        vout: 0,
    };
    let _signer_utxo = OutPoint {
        txid: coinbase_txid,
        vout: 1,
    };
    block0.txdata[0].output[0].script_pubkey = user_address.script_pubkey();
    block0.txdata[0].output.push(TxOut {
        value: Amount::from_sat(1_000_000_000),
        script_pubkey: signer_address.script_pubkey(),
    });
    Protorune::index_block::<AlkaneMessageContext>(block0.clone(), 0)?;

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
    let mut set_signer_block = create_block_with_coinbase_tx(2);
    set_signer_block.txdata.push(set_signer_tx);
    Protorune::index_block::<AlkaneMessageContext>(set_signer_block.clone(), 2)?;

    // 4. Wrap BTC to get frBTC
    let _wrap_amount = Amount::from_sat(100_000_000); // 1 BTC
    let wrap_tx = create_multiple_cellpack_with_witness_and_txins_edicts(
        vec![Cellpack {
            target: FR_BTC_ID,
            inputs: vec![77], // Opcode 77: Wrap, pointer to vout 1
        }],
        vec![TxIn {
            previous_output: OutPoint {
                txid: set_signer_block.txdata[1].compute_txid(),
                vout: 0,
            },
            ..Default::default()
        }],
        false,
        vec![],
    );
    let mut wrap_block = create_block_with_coinbase_tx(3);
    wrap_block.txdata.push(wrap_tx.clone());
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
                txid: wrap_tx.compute_txid(),
                vout: 0,
            },
            ..Default::default()
        }],
        false,
        vec![
            ProtostoneEdict {
                id: FR_BTC_ID.into(),
                amount: unwrap_amount,
                output: 0,
            },
        ],
    );
    let mut unwrap_block = create_block_with_coinbase_tx(4);
    unwrap_block.txdata.push(unwrap_tx.clone());
    Protorune::index_block::<AlkaneMessageContext>(unwrap_block.clone(), 4)?;

    // 6. Get pending unwraps
    let pending_unwraps_res = view::unwrap()?;
    let payout_tx: Transaction = deserialize(&pending_unwraps_res)?;

    assert_eq!(
        payout_tx.input.len(),
        2,
        "Should have one accounting and one funding input"
    );
    assert_eq!(
        payout_tx.output.len(),
        2,
        "Should have one payout and one change output"
    );

    // 7. Simulate the signer payout transaction
    let mut payout_block = create_block_with_coinbase_tx(5);
    payout_block.txdata.push(payout_tx);
    Protorune::index_block::<AlkaneMessageContext>(payout_block.clone(), 5)?;

    // 8. Second unwrap
    let unwrap_tx_2 = create_multiple_cellpack_with_witness_and_txins_edicts(
        vec![Cellpack {
            target: FR_BTC_ID,
            inputs: vec![78, 0], // Opcode 78: Unwrap, pointer to vout 0
        }],
        vec![TxIn {
            previous_output: OutPoint {
                txid: unwrap_tx.compute_txid(),
                vout: 0,
            },
            ..Default::default()
        }],
        false,
        vec![ProtostoneEdict {
            id: FR_BTC_ID.into(),
            amount: 25_000_000,
            output: 0,
        }], // 0.25 BTC
    );
    let mut unwrap_block_2 = create_block_with_coinbase_tx(6);
    unwrap_block_2.txdata.push(unwrap_tx_2);
    Protorune::index_block::<AlkaneMessageContext>(unwrap_block_2.clone(), 6)?;

    // 9. Get pending unwraps again
    let pending_unwraps_res_2 = view::call_view(&FR_BTC_ID, &vec![105], u64::MAX)?;
    let pending_tx: Transaction = deserialize(&pending_unwraps_res_2)?;

    assert_eq!(
        pending_tx.input.len(),
        2,
        "Should have one new accounting input and one funding input"
    );
    assert_eq!(
        pending_tx.output.len(),
        2,
        "Should have one new payout output and one change output"
    );

    Ok(())
}
