use super::helpers::*;
use crate::{
    message::AlkaneMessageContext,
    vm::{
        run_message,
        tests::{
            setup_test_vm,
            tests::std::{alkanes_std_genesis_alkane_build, alkanes_std_test_build},
        },
    },
};
use alkanes_support::{
    cellpack::Cellpack,
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
    storage::StorageMap,
};
use anyhow::{anyhow, Result};
use bitcoin::{OutPoint, ScriptBuf, Transaction, TxOut};
use metashrew_core::ToHex;
use metashrew_support::utils::consensus_encode;
use ordinals::{Artifact, Edict, RuneId, Runestone};
use protorune_support::{
    protostone::Protostone,
    utils::{decode_varint_list, encode_varint_list},
};
use std::io::Cursor;

#[test]
fn test_precompiled_diesel_mints() -> Result<()> {
    let mut vm = setup_test_vm();
    let mut test_block = build_test_block_with_precompiled_call();
    let response = run_message(&mut vm, &test_block, 0, 0)?;
    let call_response: CallResponse = response.into();
    let mint_count = u128::from_le_bytes(call_response.data.try_into().unwrap());
    assert_eq!(mint_count, 1);
    Ok(())
}

fn build_test_block_with_precompiled_call() -> Transaction {
    let mut builder = script::Builder::new()
        .push_opcode(opcodes::all::OP_RETURN)
        .push_opcode(Runestone::MAGIC_NUMBER);
    let cellpack = Cellpack {
        target: AlkaneId::new(800000000, 2),
        inputs: vec![],
    };
    let protostone = Protostone {
        protocol_tag: 1,
        message: encode_varint_list(&cellpack.to_vec()),
        ..Default::default()
    };
    let runestone = Runestone {
        protocol: Some(protostone.to_integers().unwrap()),
        ..Default::default()
    };
    let script_pubkey = runestone.encipher();
    let mut instructions = script_pubkey.instructions();
    // skip OP_RETURN and MAGIC_NUMBER
    instructions.next();
    instructions.next();
    for instruction in instructions {
        if let Ok(instruction) = instruction {
            builder = builder.push_bytes(instruction.push_bytes().unwrap());
        }
    }

    Transaction {
        version: 2,
        lock_time: LockTime::ZERO,
        input: vec![],
        output: vec![TxOut {
            value: 0,
            script_pubkey: builder.into_script(),
        }],
    }
}