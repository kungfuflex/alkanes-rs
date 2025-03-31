use crate::balance_sheet::load_sheet;
use crate::protostone::Protostones;
use crate::tables;
use bitcoin::address::NetworkChecked;
use bitcoin::blockdata::block::{Block, Header};
use bitcoin::blockdata::script::ScriptBuf;
use bitcoin::blockdata::transaction::Version;
use bitcoin::blockdata::transaction::{Transaction, TxIn, TxOut};
use bitcoin::hashes::Hash;
use bitcoin::{Address, Amount, BlockHash, Network, OutPoint, Script, Sequence, Witness};
use byteorder::{ByteOrder, LittleEndian};
use core::str::FromStr;
use hex::decode;
use metashrew::{println, stdio::stdout};
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::format_key;
use ordinals::{Edict, Etching, Rune, RuneId, Runestone};
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::network::{set_network, to_address_str, NetworkParams};
use protorune_support::protostone::{Protostone, ProtostoneEdict};
use protorune_support::utils::consensus_encode;
use std::fmt::Write;
use std::sync::Arc;

#[cfg(feature = "mainnet")]
pub fn get_btc_network() -> Network {
    Network::Bitcoin
}

#[cfg(not(feature = "mainnet"))]
pub fn get_btc_network() -> Network {
    Network::Regtest
}

#[cfg(feature = "mainnet")]
pub fn init_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("bc"),
        p2sh_prefix: 0x05,
        p2pkh_prefix: 0x00,
    });
}

#[cfg(not(feature = "mainnet"))]
pub fn init_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("bcrt"),
        p2pkh_prefix: 0x64,
        p2sh_prefix: 0xc4,
    });
}

pub fn clear() {
    metashrew::clear();
    init_network();
}

// TODO: This module should probably not be compiled into the prod indexer wasm

pub const ADDRESS1_BYTES: &'static str = "a914ad8028e0e0f9b863174e0efc67f65223c3b7ab5387";
pub const ADDRESS2_BYTES: &'static str = "a914000000000000000000000000000000000000000087";

pub fn get_address_from_bytes(hex: &str) -> String {
    let bytes = decode(hex.to_string()).unwrap();
    let pk = Script::from_bytes(bytes.as_slice());
    let address = to_address_str(pk);
    address.unwrap()
}

#[allow(non_snake_case)]
pub fn ADDRESS1() -> String {
    get_address_from_bytes(ADDRESS1_BYTES)
}

#[allow(non_snake_case)]
pub fn ADDRESS2() -> String {
    get_address_from_bytes(ADDRESS2_BYTES)
}

// Removed print_cache function as get_cache is not available in the new metashrew version

pub fn display_vec_as_hex(data: Vec<u8>) -> String {
    let mut hex_string = String::new();
    for byte in data {
        write!(&mut hex_string, "{:02x}", byte).expect("Unable to write");
    }
    hex_string
}

pub fn display_list_as_hex(data: Vec<Arc<Vec<u8>>>) -> String {
    let mut hex_string = String::new();

    for arc_data in data {
        for byte in arc_data.to_vec().iter() {
            write!(&mut hex_string, "{:02x}", byte).expect("Unable to write");
        }
    }

    hex_string
}

pub fn serialize_u32_little_endian(value: u32) -> Vec<u8> {
    let mut buf = vec![0u8; 4]; // Create a buffer of 4 bytes
    LittleEndian::write_u32(&mut buf, value); // Write the value in little-endian order
    buf
}

pub fn create_coinbase_transaction(height: u32) -> Transaction {
    // Create the script for the coinbase transaction
    let script_pubkey = Address::from_str(ADDRESS1().as_str())
        .unwrap()
        .require_network(get_btc_network())
        .unwrap()
        .script_pubkey();
    // Create a coinbase transaction input
    let coinbase_input = TxIn {
        previous_output: Default::default(),
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX, // sequence for coinbase
        witness: Witness::new(),
    };

    // Create the coinbase transaction output
    let coinbase_output = TxOut {
        value: Amount::from_sat(50_000_000), // 50 BTC in satoshis
        script_pubkey,
    };

    let locktime = bitcoin::absolute::LockTime::from_height(height).unwrap();

    // Create the coinbase transaction
    Transaction {
        version: Version::TWO,
        lock_time: locktime,
        input: vec![coinbase_input],
        output: vec![coinbase_output],
    }
}

pub fn serialize_block(block: &Block) -> [u8; 32] {
    block.block_hash().to_raw_hash().to_byte_array()
}

pub fn create_test_transaction() -> Transaction {
    create_test_transaction_with_witness(vec![])
}

pub fn create_test_transaction_with_witness(script: Vec<u8>) -> Transaction {
    let previous_output = OutPoint {
        txid: bitcoin::Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        vout: 0,
    };
    let input_script = ScriptBuf::new();

    let mut witness = Witness::new();
    witness.push(&script);

    // Create a transaction input
    let txin = TxIn {
        previous_output,
        script_sig: input_script,
        sequence: Sequence::MAX,
        witness,
    };

    let address_str = ADDRESS1();

    let address: Address<NetworkChecked> = Address::from_str(&address_str)
        .unwrap()
        .require_network(get_btc_network())
        .unwrap();

    let script_pubkey = address.script_pubkey();

    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey,
    };

    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO, // no locktime
        input: vec![txin],
        output: vec![txout],
    }
}
#[derive(Debug, Clone)]
pub struct RunesTestingConfig {
    pub address1: String,
    pub address2: String,
    pub rune_name: Option<String>,
    pub rune_symbol: Option<String>,
    pub rune_etch_height: u64,
    pub rune_etch_vout: u32,
    pub rune_pointer: Option<u32>,
}

impl RunesTestingConfig {
    pub fn new(
        address1: &str,
        address2: &str,
        rune_name: Option<&str>,
        rune_symbol: Option<&str>,
        rune_etch_height: u64,
        rune_etch_vout: u32,
        rune_pointer: Option<u32>,
    ) -> RunesTestingConfig {
        RunesTestingConfig {
            address1: address1.into(),
            address2: address2.into(),
            rune_name: match rune_name {
                Some(_name) => Some(_name.into()),
                None => None,
            },
            rune_symbol: match rune_symbol {
                Some(_symbol) => Some(_symbol.into()),
                None => None,
            },
            rune_etch_height,
            rune_etch_vout,
            rune_pointer,
        }
    }

    pub fn default() -> RunesTestingConfig {
        RunesTestingConfig::new(
            ADDRESS1().as_str(),
            ADDRESS2().as_str(),
            Some("AAAAAAAAAAAAATESTER"),
            Some("Z"),
            840001,
            0,
            Some(1),
        )
    }

    pub fn default_with_pointer(rune_pointer: Option<u32>) -> RunesTestingConfig {
        RunesTestingConfig::new(
            ADDRESS1().as_str(),
            ADDRESS2().as_str(),
            Some("AAAAAAAAAAAAATESTER"),
            Some("Z"),
            840001,
            0,
            rune_pointer,
        )
    }
}

pub fn get_address(address: &str) -> Address<NetworkChecked> {
    Address::from_str(address)
        .unwrap()
        .require_network(get_btc_network())
        .unwrap()
}

pub fn get_rune_balance_by_outpoint(
    outpoint: OutPoint,
    protorune_ids: Vec<ProtoruneRuneId>,
) -> Vec<u128> {
    let mint_sheet = load_sheet(
        &tables::RUNES
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint).unwrap()),
    );
    let stored_amount = protorune_ids
        .into_iter()
        .map(|id| mint_sheet.get_cached(&id))
        .collect();
    return stored_amount;
}

pub fn get_protorune_balance_by_outpoint(
    protocol_id: u128,
    outpoint: OutPoint,
    protorune_ids: Vec<ProtoruneRuneId>,
) -> Vec<u128> {
    let mint_sheet = load_sheet(
        &tables::RuneTable::for_protocol(protocol_id.into())
            .OUTPOINT_TO_RUNES
            .select(&consensus_encode(&outpoint).unwrap()),
    );
    let stored_amount = protorune_ids
        .into_iter()
        .map(|id| mint_sheet.get_cached(&id))
        .collect();
    return stored_amount;
}

pub fn get_mock_outpoint(n: u32) -> OutPoint {
    OutPoint {
        txid: bitcoin::Txid::from_str(&format!(
            "000000000000000000000000000000000000000000000000000000000000000{}",
            n
        ))
        .unwrap(),
        vout: 0,
    }
}

pub fn get_mock_txin(n: u32) -> TxIn {
    let previous_output = get_mock_outpoint(n);
    return get_txin_from_outpoint(previous_output);
}

pub fn get_txin_from_outpoint(previous_output: OutPoint) -> TxIn {
    let input_script = ScriptBuf::new();

    // Create a transaction input
    let txin = TxIn {
        previous_output,
        script_sig: input_script,
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    return txin;
}

pub fn get_txout_transfer_to_address(address: &String, amount: u64) -> TxOut {
    let _address: Address<NetworkChecked> = get_address(address);

    let script_pubkey = _address.script_pubkey();

    TxOut {
        value: Amount::from_sat(amount),
        script_pubkey,
    }
}

pub fn create_tx_from_runestone(
    runestone: Runestone,
    txins: Vec<TxIn>,
    additional_txouts: Vec<TxOut>,
) -> Transaction {
    let runestone_script: ScriptBuf = runestone.encipher();

    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone_script,
    };

    let mut txouts = additional_txouts.clone();
    txouts.push(op_return);

    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: txins,
        output: txouts,
    }
}

/// TODO: Convert all these create_*_transaction functions into sdk functions
/// Create a rune etching transferring all runes to vout 0 in the tx
/// Mocks a dummy outpoint for the previous outpoint
pub fn create_rune_etching_transaction(config: &RunesTestingConfig) -> Transaction {
    // Create a transaction input
    let txin = get_mock_txin(0);

    let address: Address<NetworkChecked> = get_address(&config.address1);

    let script_pubkey = address.script_pubkey();

    // tx vout 0 will hold all 1000 of the runes
    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey,
    };

    let rune = match &config.rune_name {
        Some(rune_name) => Some(Rune::from_str(rune_name).unwrap()),
        None => None,
    };

    let symbol = match &config.rune_symbol {
        Some(rune_symbol) => Some(char::from_str(rune_symbol).unwrap()),
        None => None,
    };

    let runestone: ScriptBuf = (Runestone {
        etching: Some(Etching {
            divisibility: Some(2),
            premine: Some(1000),
            rune,
            spacers: Some(0),
            symbol,
            turbo: true,
            terms: None,
        }),
        pointer: Some(0),
        edicts: Vec::new(),
        mint: None,
        protocol: None,
    })
    .encipher();

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

///
/// TODO: Convert all these create_*_transaction functions into sdk functions
pub fn create_rune_transfer_transaction(
    config: &RunesTestingConfig,
    previous_output: OutPoint,
    edicts: Vec<Edict>,
) -> Transaction {
    let input_script = ScriptBuf::new();

    // Create a transaction input
    let txin = TxIn {
        previous_output,
        script_sig: input_script,
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    let address1 = get_address(&config.address1);
    let address2 = get_address(&config.address2);

    let script_pubkey1 = address1.script_pubkey();
    let script_pubkey2 = address2.script_pubkey();

    // tx vout 0 corresponds to address2 will hold all 200 of the runes
    let txout0 = TxOut {
        value: Amount::from_sat(1),
        script_pubkey: script_pubkey2,
    };

    // tx vout 1 corresponds to address1 and will hold 800 of the runes
    let txout1 = TxOut {
        value: Amount::from_sat(99_999_999),
        script_pubkey: script_pubkey1,
    };

    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: config.rune_pointer,
        edicts,
        mint: None,
        protocol: None,
    })
    .encipher();

    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };

    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin],
        output: vec![txout0, txout1, op_return],
    }
}

pub fn create_block_with_txs(txdata: Vec<Transaction>) -> Block {
    // Define block header fields
    let _version = Version::ONE;
    let previous_blockhash =
        BlockHash::from_str("00000000000000000005c3b409b4f17f9b3a97ed46d1a63d3f660d24168b2b3e")
            .unwrap();

    // let merkle_root_hash = bitcoin::merkle_tree::calculate_root(&[coinbase_tx.clone()]);
    let merkle_root = bitcoin::hash_types::TxMerkleNode::from_str(
        "4e07408562b4b5a9c0555f0671e0d2b6c5764c1d2a5e97c1d7f36f7c91e4c77a",
    )
    .unwrap();
    let time = 1231006505; // Example timestamp (January 3 2009)
    let bits = bitcoin::CompactTarget::from_consensus(0x1234); // Example bits (difficulty)
    let nonce = 2083236893; // Example nonce

    // Create the block header
    let header = Header {
        version: bitcoin::blockdata::block::Version::from_consensus(1),
        prev_blockhash: previous_blockhash,
        merkle_root,
        time,
        bits,
        nonce,
    };

    // Create the block with the coinbase transaction
    Block { header, txdata }
}

/// Create a transaction with OP_RETURN in the middle (output index 1)
/// Output 0: Normal output
/// Output 1: OP_RETURN with runestone
/// Output 2: Normal output
pub fn create_transaction_with_middle_op_return(
    previous_output: OutPoint,
    protocol_id: u128,
) -> Transaction {
    let input_script = ScriptBuf::new();

    // Create a transaction input
    let txin = TxIn {
        previous_output,
        script_sig: input_script,
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());
    let script_pubkey = address.script_pubkey();

    // Output 0: Normal output
    let txout0 = TxOut {
        value: Amount::from_sat(50_000_000),
        script_pubkey: script_pubkey.clone(),
    };

    // Output 1: OP_RETURN with runestone
    let runestone: ScriptBuf = (Runestone {
        etching: Some(Etching {
            divisibility: Some(2),
            premine: Some(1000),
            rune: Some(Rune::from_str("MIDDLERUNEOPRET").unwrap()),
            spacers: Some(0),
            symbol: Some(char::from_str("A").unwrap()),
            turbo: true,
            terms: None,
        }),
        pointer: Some(0), // Point to output 0 (before OP_RETURN)
        edicts: Vec::new(),
        mint: None,
        protocol: match vec![Protostone {
            burn: Some(protocol_id),
            edicts: vec![],
            pointer: Some(0), // Point to output 0 (before OP_RETURN)
            refund: None,
            from: None,
            protocol_tag: protocol_id,
            message: vec![],
        }]
        .encipher()
        {
            Ok(v) => Some(v),
            Err(_) => None,
        },
    })
    .encipher();

    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };

    // Output 2: Another normal output
    let txout2 = TxOut {
        value: Amount::from_sat(40_000_000),
        script_pubkey: script_pubkey.clone(),
    };

    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin],
        output: vec![txout0, op_return, txout2], // OP_RETURN in the middle
    }
}

pub fn create_block_with_sample_tx() -> Block {
    return create_block_with_txs(vec![create_test_transaction()]);
}

pub fn create_block_with_rune_tx(
    config: Option<RunesTestingConfig>,
) -> (Block, RunesTestingConfig) {
    let final_config = config.unwrap_or(RunesTestingConfig::default());

    let rune = match &final_config.rune_name {
        Some(rune_name) => Some(Rune::from_str(rune_name).unwrap()),
        None => None,
    };

    let symbol = match &final_config.rune_symbol {
        Some(rune_symbol) => Some(char::from_str(rune_symbol).unwrap()),
        None => None,
    };

    let tx = create_tx_from_runestone(
        Runestone {
            etching: Some(Etching {
                divisibility: Some(2),
                premine: Some(1000),
                rune,
                spacers: Some(0),
                symbol,
                turbo: true,
                terms: None,
            }),
            pointer: Some(0),
            edicts: Vec::new(),
            mint: None,
            protocol: None,
        },
        vec![get_mock_txin(0)],
        vec![get_txout_transfer_to_address(
            &final_config.address1,
            100_000_000,
        )],
    );
    let test_block = create_block_with_txs(vec![tx]);
    return (test_block, final_config);
}

pub fn create_block_with_coinbase_tx(height: u32) -> Block {
    return create_block_with_txs(vec![create_coinbase_transaction(height)]);
}

/// Fixture with the following block:
///  - tx0:
///     - inputs:
///         - [0]: dummy outpoint
///     - outputs:
///         - [0]: ptpkh (?) address1
///         - [1]: runestone with etch 1000 runes to vout0
///  - tx1:
///     - inputs:
///         - [0]: outpoint(tx0, vout0)
///     - outputs:
///         - [0]: ptpkh address2
///         - [1]: ptpkh address1
///         - [2]: runestone with edict to transfer to vout0, default to vout1
pub fn create_block_with_rune_transfer(config: &RunesTestingConfig, edicts: Vec<Edict>) -> Block {
    let tx0 = create_rune_etching_transaction(config);
    let outpoint_with_runes = OutPoint {
        txid: tx0.compute_txid(),
        vout: 0,
    };

    let tx1 = create_rune_transfer_transaction(config, outpoint_with_runes, edicts);
    return create_block_with_txs(vec![tx0, tx1]);
}

pub fn create_protostone_encoded_tx(
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

    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());

    let script_pubkey = address.script_pubkey();

    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey,
    };

    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: None, // points to the OP_RETURN so therefore targets the protoburn
        edicts: vec![Edict {
            id: RuneId {
                block: 840000,
                tx: 1,
            },
            amount: 500,
            output: 2,
        }],
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

pub fn create_multi_protoburn_transaction(
    previous_output: OutPoint,
    burn_protocol_ids: &[u128],
) -> Transaction {
    let input_script = ScriptBuf::new();

    // Create a transaction input
    let txin = TxIn {
        previous_output,
        script_sig: input_script,
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());

    let script_pubkey = address.script_pubkey();

    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey,
    };

    let etching = Some(Etching {
        divisibility: Some(2),
        premine: Some(1000),
        rune: Some(Rune::from_str("TESTTESTTESTTEST").unwrap()),
        spacers: Some(0),
        symbol: Some(char::from_str("A").unwrap()),
        turbo: true,
        terms: None,
    });

    let runestone: ScriptBuf = (Runestone {
        etching,
        pointer: Some(1),
        edicts: Vec::new(),
        mint: None,
        protocol: match burn_protocol_ids
            .into_iter()
            .enumerate()
            .map(|(i, id)| Protostone {
                burn: Some(*id),
                edicts: vec![],
                pointer: Some(i as u32),
                refund: None,
                from: None,
                protocol_tag: 13,
                message: vec![],
            })
            .collect::<Vec<Protostone>>()
            .encipher()
        {
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

    let mut output = burn_protocol_ids
        .into_iter()
        .map(|_| txout.clone())
        .collect::<Vec<TxOut>>();
    output.push(op_return);
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![txin],
        output,
    }
}

pub fn create_default_protoburn_transaction(
    previous_output: OutPoint,
    burn_protocol_id: u128,
) -> Transaction {
    // output rune pointer points to the OP_RETURN so therefore targets the protoburn
    return create_protostone_transaction(
        previous_output,
        Some(burn_protocol_id),
        true,
        1,
        // protoburn and give protorunes to output 0
        0,
        13, // this value must be 13 if protoburn
        vec![],
    );
}

/// Create a protoburn given an input that holds runes
/// Outpoint with protorunes is the txid and vout 0
/// This outpoint holds 1000 protorunes
pub fn create_protostone_transaction(
    previous_output: OutPoint,
    burn_protocol_id: Option<u128>,
    etch: bool,
    output_rune_pointer: u32,
    output_protostone_pointer: u32,
    protocol_tag: u128,
    protostone_edicts: Vec<ProtostoneEdict>,
) -> Transaction {
    let input_script = ScriptBuf::new();

    // Create a transaction input
    let txin = TxIn {
        previous_output,
        script_sig: input_script,
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());

    let script_pubkey = address.script_pubkey();

    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey,
    };

    let etching = if etch {
        Some(Etching {
            divisibility: Some(2),
            premine: Some(1000),
            rune: Some(Rune::from_str("ENDRUNEOPRETURN").unwrap()),
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
        pointer: Some(output_rune_pointer),
        edicts: Vec::new(),
        mint: None,
        protocol: match vec![Protostone {
            burn: burn_protocol_id,
            edicts: protostone_edicts,
            pointer: Some(output_protostone_pointer),
            refund: None,
            from: None,
            protocol_tag: protocol_tag,
            message: vec![],
        }]
        .encipher()
        {
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

//creates a tx with multiple protomessages and multiple protocols
pub fn create_multiple_protomessage_from_edict_tx(
    previous_outputs: Vec<OutPoint>,
    protocol_id: Vec<u128>,
    protostone_edicts: Vec<Vec<ProtostoneEdict>>,
) -> Transaction {
    let input_script = ScriptBuf::new();
    let txins = previous_outputs
        .clone()
        .into_iter()
        .map(|previous_output| TxIn {
            previous_output,
            script_sig: input_script.clone(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        })
        .collect::<Vec<TxIn>>();

    let address: Address<NetworkChecked> = get_address(&ADDRESS1().as_str());
    let script_pubkey = address.script_pubkey();

    // Create normal outputs
    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey: script_pubkey.clone(),
    };

    // Create protostones for each protocol
    let protostones = protocol_id
        .iter()
        .zip(protostone_edicts.iter())
        .enumerate()
        .map(|(i, (id, edicts))| Protostone {
            burn: None,
            edicts: edicts.clone(),
            pointer: Some(i as u32), // Point to the corresponding output
            refund: None,
            from: None,
            protocol_tag: *id,
            message: vec![],
        })
        .collect::<Vec<Protostone>>();

    // Create runestone with protocol data
    let runestone: ScriptBuf = (Runestone {
        etching: None,
        pointer: Some(0), // Default pointer
        edicts: Vec::new(),
        mint: None,
        protocol: match protostones.encipher() {
            Ok(v) => Some(v),
            Err(_) => None,
        },
    })
    .encipher();

    // Create OP_RETURN output
    let op_return = TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone,
    };

    // Create normal outputs for each protocol plus the OP_RETURN
    let mut outputs = vec![txout; protocol_id.len()];
    outputs.push(op_return);

    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: txins,
        output: outputs,
    }
}
