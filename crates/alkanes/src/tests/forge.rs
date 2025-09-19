use crate::index_block;
use crate::message::AlkaneMessageContext;
use crate::tests::helpers::{self as alkane_helpers};
use crate::tests::test_runtime::TestRuntime;
use anyhow::Result;
use bitcoin::address::NetworkChecked;
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, Block, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use metashrew_support::environment::RuntimeEnvironment;
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use ordinals::Runestone;
use protorune::test_helpers::get_address;
use protorune::{
    balance_sheet::load_sheet, message::MessageContext, tables::RuneTable, test_helpers as helpers,
};
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::protostone::{Protostone, ProtostoneEdict};
use protorune::protostone::{ProtostoneEncoder, Protostones};

pub fn create_protostone_encoded_transaction<E: RuntimeEnvironment>(
    previous_output: OutPoint,
    protostones: Vec<Protostone>,
) -> Result<Transaction> {
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
        protocol: Some(<Vec<Protostone> as ProtostoneEncoder<TestRuntime>>::encipher(&protostones)?),
    })
    .encipher();

    // op return is at output 1
    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };

    Ok(Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin],
        output: vec![txout, op_return],
    })
}

#[test]
fn test_cant_forge_edicts() -> Result<()> {
    let block_height = 0;
    let mut test_block: Block = helpers::create_block_with_coinbase_tx(block_height);
    let outpoint = OutPoint {
        txid: test_block.txdata[0].compute_txid(),
        vout: 0,
    };
    test_block.txdata.push(create_protostone_encoded_transaction::<TestRuntime>(
        outpoint,
        vec![Protostone {
            protocol_tag: 1,
            from: None,
            edicts: vec![ProtostoneEdict {
                id: ProtoruneRuneId { block: 2, tx: 100 },
                amount: 100000,
                output: 0,
            }],
            pointer: Some(0),
            refund: Some(0),
            message: vec![],
            burn: None,
        }],
    )?);
    index_block::<TestRuntime>(&test_block, block_height)?;
    let edict_outpoint = OutPoint {
        txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
        vout: 0,
    };
    let sheet = load_sheet(
        &RuneTable::<TestRuntime>::for_protocol(AlkaneMessageContext::<TestRuntime>::protocol_tag())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&edict_outpoint)?),
    );
    TestRuntime::log(format!("{:?}", sheet));
    Ok(())
}