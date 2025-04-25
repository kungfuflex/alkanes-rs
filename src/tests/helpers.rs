use crate::message::AlkaneMessageContext;
use crate::view;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::envelope::RawEnvelope;
use alkanes_support::gz::compress;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use anyhow::Result;
use bitcoin::blockdata::transaction::Version;
use bitcoin::{
    address::NetworkChecked, Address, Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness,
};
use bitcoin::{Block, Transaction};
use metashrew_core::index_pointer::IndexPointer;
#[allow(unused_imports)]
use metashrew_core::{
    clear as clear_base, println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_encode;
use ordinals::{Etching, Rune, Runestone};
use protorune::balance_sheet::load_sheet;
use protorune::message::MessageContext;
use protorune::protostone::Protostones;
use protorune::tables::RuneTable;
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune_support::balance_sheet::BalanceSheet;
use protorune_support::network::{set_network, NetworkParams};
use protorune_support::protostone::{Protostone, ProtostoneEdict};
use std::str::FromStr;

#[cfg(test)]
use crate::tests::std::alkanes_std_test_build;

#[cfg(all(
    not(feature = "mainnet"),
    not(feature = "dogecoin"),
    not(feature = "bellscoin"),
    not(feature = "fractal"),
    not(feature = "luckycoin")
))]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("bcrt"),
        p2pkh_prefix: 0x64,
        p2sh_prefix: 0xc4,
    });
}
#[cfg(feature = "mainnet")]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("bc"),
        p2sh_prefix: 0x05,
        p2pkh_prefix: 0x00,
    });
}
#[cfg(feature = "testnet")]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("tb"),
        p2pkh_hash: 0x6f,
        p2sh_hash: 0xc4,
    });
}
#[cfg(feature = "luckycoin")]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("tb"),
        p2pkh_hash: 0x6f,
        p2sh_hash: 0xc4,
    });
}

#[cfg(feature = "dogecoin")]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("dc"),
        p2pkh_hash: 0x6f,
        p2sh_hash: 0xc4,
    });
}
#[cfg(feature = "bellscoin")]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("bel"),
        p2pkh_hash: 0x6f,
        p2sh_hash: 0xc4,
    });
}

pub fn clear() {
    clear_base();
    configure_network();
}

#[cfg(test)]
pub fn init_test_with_cellpack(cellpack: Cellpack) -> Block {
    let block_height = 840000;
    let mut test_block = create_block_with_coinbase_tx(block_height);

    let wasm_binary = alkanes_std_test_build::get_bytes();
    let raw_envelope = RawEnvelope::from(wasm_binary);

    let witness = raw_envelope.to_gzipped_witness();

    // Create a transaction input

    test_block
        .txdata
        .push(create_cellpack_with_witness(witness, cellpack));
    test_block
}

pub fn init_with_multiple_cellpacks_with_tx(
    binaries: Vec<Vec<u8>>,
    cellpacks: Vec<Cellpack>,
) -> Block {
    let block_height = 840000;
    let mut test_block = create_block_with_coinbase_tx(block_height);
    let mut previous_out: Option<OutPoint> = None;
    let mut txs = binaries
        .into_iter()
        .zip(cellpacks.into_iter())
        .map(|i| {
            let (binary, cellpack) = i;
            let witness = if binary.len() == 0 {
                Witness::new()
            } else {
                RawEnvelope::from(binary).to_gzipped_witness()
            };
            if let Some(previous_output) = previous_out {
                let tx = create_multiple_cellpack_with_witness_and_in(
                    witness,
                    [cellpack].into(),
                    previous_output,
                    false,
                );
                previous_out = Some(OutPoint {
                    txid: tx.compute_txid(),
                    vout: 0,
                });
                tx
            } else {
                let tx = create_multiple_cellpack_with_witness(witness, [cellpack].into(), false);
                previous_out = Some(OutPoint {
                    txid: tx.compute_txid(),
                    vout: 0,
                });
                tx
            }
        })
        .collect::<Vec<Transaction>>();
    test_block.txdata.append(&mut txs);
    test_block
}

pub fn init_with_multiple_cellpacks(binary: Vec<u8>, cellpacks: Vec<Cellpack>) -> Block {
    let block_height = 840000;

    let mut test_block = create_block_with_coinbase_tx(block_height);

    let raw_envelope = RawEnvelope::from(binary);
    let witness = raw_envelope.to_gzipped_witness();
    test_block
        .txdata
        .push(create_multiple_cellpack_with_witness(
            witness, cellpacks, false,
        ));
    test_block
}

pub fn create_protostone_tx_with_inputs_and_default_pointer(
    inputs: Vec<TxIn>,
    outputs: Vec<TxOut>,
    protostone: Protostone,
    default_pointer: u32,
) -> Transaction {
    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: Some(default_pointer), // points to the OP_RETURN, so therefore targets the protoburn
        edicts: Vec::new(),
        mint: None,
        protocol: vec![protostone].encipher().ok(),
    })
    .encipher();
    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };
    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());
    let _script_pubkey = address.script_pubkey();
    let mut _outputs = outputs.clone();
    _outputs.push(op_return);
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: inputs,
        output: _outputs,
    }
}

pub fn create_protostone_tx_with_inputs(
    inputs: Vec<TxIn>,
    outputs: Vec<TxOut>,
    protostone: Protostone,
) -> Transaction {
    create_protostone_tx_with_inputs_and_default_pointer(inputs, outputs, protostone, 1)
}

pub fn create_multiple_cellpack_with_witness_and_in(
    witness: Witness,
    cellpacks: Vec<Cellpack>,
    previous_output: OutPoint,
    etch: bool,
) -> Transaction {
    let input_script = ScriptBuf::new();
    let txin = TxIn {
        previous_output,
        script_sig: input_script,
        sequence: Sequence::MAX,
        witness,
    };
    create_multiple_cellpack_with_witness_and_txins_edicts(cellpacks, vec![txin], etch, vec![])
}

pub fn create_multiple_cellpack_with_witness_and_txins_edicts(
    cellpacks: Vec<Cellpack>,
    txins: Vec<TxIn>,
    etch: bool,
    edicts: Vec<ProtostoneEdict>,
) -> Transaction {
    let protocol_id = 1;
    let protostones = [
        match etch {
            true => vec![Protostone {
                burn: Some(protocol_id),
                edicts: vec![],
                pointer: Some(4),
                refund: None,
                from: None,
                protocol_tag: 13, // this value must be 13 if protoburn
                message: vec![],
            }],
            false => vec![],
        },
        cellpacks
            .into_iter()
            .map(|cellpack| Protostone {
                message: cellpack.encipher(),
                pointer: Some(0),
                refund: Some(0),
                edicts: edicts.clone(),
                from: None,
                burn: None,
                protocol_tag: protocol_id as u128,
            })
            .collect(),
    ]
    .concat();
    let etching = if etch {
        Some(Etching {
            divisibility: Some(2),
            premine: Some(1000),
            rune: Some(Rune::from_str("TESTTESTTESTTEST").unwrap()),
            spacers: Some(0),
            symbol: Some(char::from_str("A").unwrap()),
            turbo: true,
            terms: None,
        })
    } else {
        None
    };
    let runestone: ScriptBuf = (Runestone {
        etching,
        pointer: match etch {
            true => Some(1),
            false => Some(0),
        }, // points to the OP_RETURN, so therefore targets the protoburn
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
    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());

    let script_pubkey = address.script_pubkey();
    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey,
    };
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: txins,
        output: vec![txout, op_return],
    }
}

pub fn create_cellpack_with_witness(witness: Witness, cellpack: Cellpack) -> Transaction {
    create_multiple_cellpack_with_witness(witness, [cellpack].into(), false)
}

pub fn create_multiple_cellpack_with_witness(
    witness: Witness,
    cellpacks: Vec<Cellpack>,
    etch: bool,
) -> Transaction {
    let previous_output = OutPoint {
        txid: bitcoin::Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        vout: 0,
    };
    create_multiple_cellpack_with_witness_and_in(witness, cellpacks, previous_output, etch)
}

pub fn assert_binary_deployed_to_id(token_id: AlkaneId, binary: Vec<u8>) -> Result<()> {
    let binary_1 = IndexPointer::from_keyword("/alkanes/")
        .select(&token_id.into())
        .get()
        .as_ref()
        .clone();
    let binary_2: Vec<u8> = compress(binary)?;
    assert_eq!(binary_1.len(), binary_2.len());
    //    assert_eq!(binary_1, binary_2);
    return Ok(());
}

pub fn assert_token_id_has_no_deployment(token_id: AlkaneId) -> Result<()> {
    let binary = IndexPointer::from_keyword("/alkanes/")
        .select(&token_id.into())
        .get()
        .as_ref()
        .clone();
    assert_eq!(binary.len(), 0);
    return Ok(());
}

fn get_sheet_for_outpoint(
    test_block: &Block,
    tx_num: usize,
    vout: u32,
) -> Result<BalanceSheet<IndexPointer>> {
    let outpoint = OutPoint {
        txid: test_block.txdata[tx_num].compute_txid(),
        vout,
    };
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(&outpoint)?);
    let sheet = load_sheet(&ptr);
    println!(
        "balances at outpoint tx {} vout {}: {:?}",
        tx_num, vout, sheet
    );
    Ok(sheet)
}

pub fn get_sheet_for_runtime() -> BalanceSheet<IndexPointer> {
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag()).RUNTIME_BALANCE;
    let sheet = load_sheet(&ptr);
    println!("runtime balances: {:?}", sheet);
    sheet
}

pub fn get_lazy_sheet_for_runtime() -> BalanceSheet<IndexPointer> {
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag()).RUNTIME_BALANCE;
    let sheet = BalanceSheet::new_ptr_backed(ptr);
    sheet
}

pub fn get_last_outpoint_sheet(test_block: &Block) -> Result<BalanceSheet<IndexPointer>> {
    let len = test_block.txdata.len();
    get_sheet_for_outpoint(test_block, len - 1, 0)
}

pub fn assert_revert_context(outpoint: &OutPoint, expected_error_message: &str) -> Result<()> {
    // This is a convenience wrapper around assert_revert_context_at_index that checks the last event
    assert_revert_context_at_index(outpoint, expected_error_message, None)
}

pub fn assert_revert_context_at_index(
    outpoint: &OutPoint,
    expected_error_message: &str,
    index: Option<isize>,
) -> Result<()> {
    let trace_data: Trace = view::trace(outpoint)?.try_into()?;
    let trace_events = trace_data.0.lock().expect("Mutex poisoned");

    if trace_events.is_empty() {
        panic!("No trace events found");
    }

    // Determine which event to check
    let event_index = match index {
        Some(idx) if idx >= 0 => idx as usize,
        Some(idx) => {
            // Handle negative indices (counting from the end)
            let abs_idx = idx.abs() as usize;
            if abs_idx > trace_events.len() {
                panic!(
                    "Index out of bounds: requested event {} but only {} events available",
                    idx,
                    trace_events.len()
                );
            }
            trace_events.len() - abs_idx
        }
        None => trace_events.len() - 1, // Default to last event
    };

    // Get the event at the calculated index
    let event = trace_events
        .get(event_index)
        .cloned()
        .unwrap_or_else(|| panic!("Failed to get trace event at index {}", event_index));

    match event {
        TraceEvent::RevertContext(trace_response) => {
            let data = String::from_utf8_lossy(&trace_response.inner.data);
            assert!(
                data.contains(expected_error_message),
                "Expected error message '{}' not found in: '{}'",
                expected_error_message,
                data
            );
            Ok(())
        }
        _ => panic!(
            "Expected RevertContext variant at index {}, but got a different variant: {:?}",
            event_index, event
        ),
    }
}
