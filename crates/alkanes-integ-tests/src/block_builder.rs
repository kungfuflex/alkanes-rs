//! Block construction helpers for integration tests.
//!
//! Ports patterns from `crates/alkanes/src/tests/helpers.rs` for use
//! with the native wasmtime test harness.

use alkanes_support::cellpack::Cellpack;
use alkanes_support::envelope::RawEnvelope;
use bitcoin::address::NetworkChecked;
use bitcoin::{
    transaction::Version, Address, Amount, Block, OutPoint, ScriptBuf, Sequence, Transaction, TxIn,
    TxOut, Witness,
};
use ordinals::Runestone;
use protorune::protostone::Protostones;
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune_support::protostone::Protostone;

/// WASM binary + cellpack pair for contract deployment.
pub struct DeployPair {
    pub binary: Vec<u8>,
    pub cellpack: Cellpack,
}

impl DeployPair {
    pub fn new(binary: impl Into<Vec<u8>>, cellpack: Cellpack) -> Self {
        Self {
            binary: binary.into(),
            cellpack,
        }
    }

    /// Cellpack-only (no WASM deployment), for calling existing contracts.
    pub fn call_only(cellpack: Cellpack) -> Self {
        Self {
            binary: Vec::new(),
            cellpack,
        }
    }
}

fn default_address() -> Address<NetworkChecked> {
    get_address(&ADDRESS1().as_str())
}

fn default_txout() -> TxOut {
    TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey: default_address().script_pubkey(),
    }
}

/// Create a block with chained deploy+cellpack transactions.
///
/// Each `DeployPair` becomes one transaction. Transactions chain:
/// tx[n].input[0] = tx[n-1].output[0].
///
/// This matches `init_with_multiple_cellpacks_with_tx` from
/// `crates/alkanes/src/tests/helpers.rs`.
pub fn create_block_with_deploys(height: u32, pairs: Vec<DeployPair>) -> Block {
    create_block_with_deploys_and_input(height, pairs, OutPoint::null())
}

pub fn create_block_with_deploys_and_input(
    height: u32,
    pairs: Vec<DeployPair>,
    initial_outpoint: OutPoint,
) -> Block {
    let mut block = create_block_with_coinbase_tx(height);
    let mut prev_outpoint = initial_outpoint;

    for pair in pairs {
        let witness = if pair.binary.is_empty() {
            Witness::new()
        } else {
            RawEnvelope::from(pair.binary).to_witness(true)
        };

        let txin = TxIn {
            previous_output: prev_outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness,
        };

        let protostone = Protostone {
            message: pair.cellpack.encipher(),
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
            from: None,
            burn: None,
            protocol_tag: 1,
        };

        let protostones = vec![protostone];
        let runestone_script = (Runestone {
            edicts: vec![],
            etching: None,
            mint: None,
            pointer: Some(0),
            protocol: protostones.encipher().ok(),
        })
        .encipher();

        let tx = Transaction {
            version: Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![txin],
            output: vec![
                default_txout(),
                TxOut {
                    value: Amount::from_sat(0),
                    script_pubkey: runestone_script,
                },
            ],
        };

        prev_outpoint = OutPoint {
            txid: tx.compute_txid(),
            vout: 0,
        };
        block.txdata.push(tx);
    }

    block
}

/// Like `create_block_with_deploys_and_input` but sends outputs to a specific address.
pub fn create_block_with_deploys_to_address(
    height: u32,
    pairs: Vec<DeployPair>,
    initial_outpoint: OutPoint,
    to_address: &str,
) -> Block {
    let addr = get_address(to_address);
    let mut block = create_block_with_coinbase_tx(height);
    let mut prev_outpoint = initial_outpoint;

    for pair in pairs {
        let witness = if pair.binary.is_empty() {
            Witness::new()
        } else {
            RawEnvelope::from(pair.binary).to_witness(true)
        };

        let txin = TxIn {
            previous_output: prev_outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness,
        };

        let protostone = Protostone {
            message: pair.cellpack.encipher(),
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![],
            from: None,
            burn: None,
            protocol_tag: 1,
        };

        let protostones = vec![protostone];
        let runestone_script = (Runestone {
            edicts: vec![],
            etching: None,
            mint: None,
            pointer: Some(0),
            protocol: protostones.encipher().ok(),
        })
        .encipher();

        let tx = Transaction {
            version: Version::ONE,
            lock_time: bitcoin::absolute::LockTime::ZERO,
            input: vec![txin],
            output: vec![
                TxOut {
                    value: Amount::from_sat(100_000_000),
                    script_pubkey: addr.script_pubkey(),
                },
                TxOut {
                    value: Amount::from_sat(0),
                    script_pubkey: runestone_script,
                },
            ],
        };

        prev_outpoint = OutPoint {
            txid: tx.compute_txid(),
            vout: 0,
        };
        block.txdata.push(tx);
    }

    block
}

/// Create a block with a raw multi-protostone transaction.
///
/// This is the key function for testing the auto-change protostone pattern.
/// The caller constructs the exact Protostone vec (with shadow output indices
/// already resolved) and this function wraps them in a Runestone.
pub fn create_block_with_protostones(
    height: u32,
    txins: Vec<TxIn>,
    extra_txouts: Vec<TxOut>,
    protostones: Vec<Protostone>,
) -> Block {
    let mut block = create_block_with_coinbase_tx(height);

    let runestone_script = (Runestone {
        edicts: vec![],
        etching: None,
        mint: None,
        pointer: Some(0),
        protocol: protostones.encipher().ok(),
    })
    .encipher();

    let mut outputs = extra_txouts;
    // Always ensure at least one regular output before the OP_RETURN
    if outputs.is_empty() {
        outputs.push(default_txout());
    }
    outputs.push(TxOut {
        value: Amount::from_sat(0),
        script_pubkey: runestone_script,
    });

    let tx = Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: txins,
        output: outputs,
    };

    block.txdata.push(tx);
    block
}

/// Create a simple call block (no WASM deployment) spending a specific outpoint.
pub fn create_call_block(height: u32, prev_outpoint: OutPoint, cellpack: Cellpack) -> Block {
    create_block_with_deploys_and_input(
        height,
        vec![DeployPair::call_only(cellpack)],
        prev_outpoint,
    )
}

/// Get the outpoint of the last transaction's first output in a block.
pub fn last_tx_outpoint(block: &Block) -> OutPoint {
    let last_tx = block.txdata.last().expect("block has no transactions");
    OutPoint {
        txid: last_tx.compute_txid(),
        vout: 0,
    }
}
