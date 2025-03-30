use crate::{message::AlkaneMessageContext, tests::std::alkanes_std_auth_token_build};
use alkanes_support::id::AlkaneId;
use alkanes_support::{cellpack::Cellpack, constants::AUTH_TOKEN_FACTORY_ID};
use anyhow::{anyhow, Result};
use bitcoin::{Sequence, Witness, Transaction, Address, Amount, Block, TxOut, TxIn, ScriptBuf, OutPoint};
use ordinals::{Runestone};
use bitcoin::address::{NetworkChecked};
use protorune_support::balance_sheet::ProtoruneRuneId; 
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use protorune::{test_helpers as helpers, balance_sheet::load_sheet, message::MessageContext, tables::RuneTable};
use bitcoin::transaction::{Version};
use protorune::test_helpers::{get_address};
use protorune::protostone::Protostones; 
use protorune_support::protostone::ProtostoneEdict;    
use ordinals::{Artifact};

use crate::index_block;
use protorune_support::protostone::Protostone;
use crate::tests::helpers::{self as alkane_helpers, assert_binary_deployed_to_id};
use crate::tests::std::alkanes_std_owned_token_build;
use alkane_helpers::clear;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use wasm_bindgen_test::wasm_bindgen_test;

pub fn create_protostone_encoded_transaction(
    previous_output: OutPoint,
    protostones: Vec<Protostone>,
) -> Transaction {
    let input_script = ScriptBuf::new();

    // Create a transaction input
    let txin = TxIn {
        previous_output,
        script_sig: input_script,
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    let address: Address<NetworkChecked> = get_address(&helpers::ADDRESS1().as_str());

    let script_pubkey = address.script_pubkey();

    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey,
    };

    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: None, // points to the OP_RETURN, so therefore targets the protoburn
        edicts: vec![],
        mint: None,
        protocol: match protostones.encipher() {
            Ok(v) => Some(v),
            Err(_) => None,
        },
    })
    .encipher();

    // op return is at output 1
    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };

    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin],
        output: vec![txout, op_return],
    }
}

#[wasm_bindgen_test]
fn test_cant_forge_edicts() -> Result<()> {
    clear();
    let block_height = 840_000;
    let mut test_block: Block = helpers::create_block_with_coinbase_tx(block_height);
    let outpoint = OutPoint {
      txid: test_block.txdata[0].compute_txid(),
      vout: 0
    };
    test_block.txdata.push(create_protostone_encoded_transaction(outpoint, vec![Protostone {
      protocol_tag: 1,
      from: None,
      edicts: vec![ProtostoneEdict {
        id: ProtoruneRuneId {
          block: 2,
          tx: 100
        },
        amount: 100000,
        output: 0
      }],
      pointer: Some(0),
      refund: Some(0),
      message: vec![],
      burn: None
    }]));
    index_block(&test_block, block_height)?;
    let edict_outpoint = OutPoint {
      txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
      vout: 0
    };
    let sheet = load_sheet(
        &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&edict_outpoint)?),
    );
    println!("{:?}", sheet);
    Ok(())
}
