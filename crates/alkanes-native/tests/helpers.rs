use alkanes_indexer::indexer::AlkanesIndexer;
use alkanes_native::adapters::NativeRuntimeAdapter;
use bitcoin::hashes::Hash;
use memshrew_runtime::{MemStoreAdapter, MemStoreRuntime};
use metashrew_sync::{
    BitcoinNodeAdapter, BlockInfo, ChainTip, SnapshotMetashrewSync, StorageStats, SyncConfig,
    SyncEngine, SyncError, SyncMode, SyncResult,
};
use std::collections::HashMap;

pub fn setup_test_runtime() -> MemStoreRuntime<AlkanesIndexer> {
    let storage = MemStoreAdapter::default();
    MemStoreRuntime::new(storage, vec![]).unwrap()
}

pub struct TestHarness {
    pub runtime: MemStoreRuntime<AlkanesIndexer>,
    pub node: MockNodeAdapter,
    pub sync_config: SyncConfig,
    pub sync_mode: SyncMode,
}

impl TestHarness {
    pub fn new() -> Self {
        Self {
            runtime: setup_test_runtime(),
            node: MockNodeAdapter::default(),
            sync_config: SyncConfig::default(),
            sync_mode: SyncMode::Normal,
        }
    }

    pub fn add_block(&mut self, block: Block) {
        let height = self.node.blocks.lock().unwrap().len() as u32;
        self.node.blocks.lock().unwrap().insert(height, block);
    }

    pub async fn process_block(&mut self) {
        let mut engine = SnapshotMetashrewSync::new(
            self.node.clone(),
            self.runtime.context.lock().unwrap().db.clone(),
            NativeRuntimeAdapter,
            self.sync_config.clone(),
            self.sync_mode.clone(),
        );
        engine.start().await.unwrap();
    }
}
use async_trait::async_trait;
use bitcoin::{Block, BlockHash};
use metashrew_sync::StorageAdapter;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MemStorageAdapter {
    pub db: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    pub height: Arc<Mutex<u32>>,
}

impl Default for MemStorageAdapter {
    fn default() -> Self {
        Self {
            db: Arc::new(Mutex::new(HashMap::new())),
            height: Arc::new(Mutex::new(0)),
        }
    }
}

impl metashrew_core::native_host::StorageAdapter for MemStorageAdapter {
	fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, anyhow::Error> {
		Ok(self.db.lock().unwrap().get(key).cloned())
	}
}

#[async_trait]
impl StorageAdapter for MemStorageAdapter {
    async fn get_indexed_height(&self) -> SyncResult<u32> {
        Ok(*self.height.lock().unwrap())
    }
    async fn set_indexed_height(&mut self, height: u32) -> SyncResult<()> {
        *self.height.lock().unwrap() = height;
        Ok(())
    }
    async fn store_block_hash(&mut self, _height: u32, _hash: &[u8]) -> SyncResult<()> {
        Ok(())
    }
    async fn get_block_hash(&self, _height: u32) -> SyncResult<Option<Vec<u8>>> {
        Ok(None)
    }
    async fn store_state_root(&mut self, _height: u32, _root: &[u8]) -> SyncResult<()> {
        Ok(())
    }
    async fn get_state_root(&self, _height: u32) -> SyncResult<Option<Vec<u8>>> {
        Ok(None)
    }
    async fn rollback_to_height(&mut self, _height: u32) -> SyncResult<()> {
        Ok(())
    }
    async fn is_available(&self) -> bool {
        true
    }
    async fn get_stats(&self) -> SyncResult<StorageStats> {
        Ok(StorageStats {
            total_entries: 0,
            indexed_height: 0,
            storage_size_bytes: Some(0),
        })
    }
}

#[derive(Clone, Default)]
pub struct MockNodeAdapter {
    pub blocks: Arc<Mutex<HashMap<u32, Block>>>,
}

#[async_trait]
impl BitcoinNodeAdapter for MockNodeAdapter {
    async fn get_block_hash(&self, height: u32) -> SyncResult<Vec<u8>> {
        let blocks = self.blocks.lock().unwrap();
        let block = blocks.get(&height).ok_or(SyncError::BitcoinNode("Block not found".to_string()))?;
        Ok(block.block_hash()[..].to_vec())
    }

    async fn get_block_data(&self, height: u32) -> SyncResult<Vec<u8>> {
        let blocks = self.blocks.lock().unwrap();
        let block = blocks.get(&height).ok_or(SyncError::BitcoinNode("Block not found".to_string()))?;
        Ok(bitcoin::consensus::encode::serialize(block))
    }

    async fn get_block_info(&self, height: u32) -> SyncResult<BlockInfo> {
        let blocks = self.blocks.lock().unwrap();
        let block = blocks.get(&height).ok_or(SyncError::BitcoinNode("Block not found".to_string()))?;
        let hash = block.block_hash()[..].to_vec();
        let data = bitcoin::consensus::encode::serialize(block);
        Ok(BlockInfo { height, hash, data })
    }

    async fn get_tip_height(&self) -> SyncResult<u32> {
        Ok(0)
    }
    async fn get_chain_tip(&self) -> SyncResult<ChainTip> {
        Ok(ChainTip {
            height: 0,
            hash: BlockHash::all_zeros()[..].to_vec(),
        })
    }
    async fn is_connected(&self) -> bool {
        true
    }
}
use alkanes_support::cellpack::Cellpack;
use alkanes_support::envelope::RawEnvelope;
use bitcoin::{
    blockdata::transaction::Version,
    {Address, Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness},
};
use ordinals::{Etching, Rune, Runestone};
use protorune::{
    protostone::{Protostones},
    test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1},
};
use protorune_support::protostone::{Protostone, ProtostoneEdict};
use std::str::FromStr;

/// A struct that combines a binary and its corresponding cellpack for cleaner initialization
#[derive(Debug, Clone)]
pub struct BinaryAndCellpack {
    pub binary: Vec<u8>,
    pub cellpack: Cellpack,
}

impl BinaryAndCellpack {
    pub fn new(binary: Vec<u8>, cellpack: Cellpack) -> Self {
        Self { binary, cellpack }
    }

    /// Creates a BinaryAndCellpack with an empty binary (useful when only cellpack data is needed)
    pub fn cellpack_only(cellpack: Cellpack) -> Self {
        Self {
            binary: Vec::new(),
            cellpack,
        }
    }
}

/// Helper function that accepts a vector of BinaryAndCellpack structs and calls init_with_multiple_cellpacks_with_tx
pub fn init_with_cellpack_pairs(cellpack_pairs: Vec<BinaryAndCellpack>) -> bitcoin::Block {
    let (binaries, cellpacks): (Vec<Vec<u8>>, Vec<Cellpack>) = cellpack_pairs
        .into_iter()
        .map(|pair| (pair.binary, pair.cellpack))
        .unzip();

    init_with_multiple_cellpacks_with_tx(binaries, cellpacks)
}

/// Helper function that accepts a vector of BinaryAndCellpack structs and calls init_with_multiple_cellpacks_with_tx
pub fn init_with_cellpack_pairs_w_input(
    cellpack_pairs: Vec<BinaryAndCellpack>,
    previous_outpoint: OutPoint,
) -> bitcoin::Block {
    let (binaries, cellpacks): (Vec<Vec<u8>>, Vec<Cellpack>) = cellpack_pairs
        .into_iter()
        .map(|pair| (pair.binary, pair.cellpack))
        .unzip();

    init_with_multiple_cellpacks_with_tx_w_input(binaries, cellpacks, Some(previous_outpoint))
}

pub fn init_with_multiple_cellpacks_with_tx(
    binaries: Vec<Vec<u8>>,
    cellpacks: Vec<Cellpack>,
) -> bitcoin::Block {
    init_with_multiple_cellpacks_with_tx_w_input(binaries, cellpacks, None)
}

pub fn init_with_multiple_cellpacks_with_tx_w_input(
    binaries: Vec<Vec<u8>>,
    cellpacks: Vec<Cellpack>,
    _previous_out: Option<OutPoint>,
) -> bitcoin::Block {
    let block_height = 880_000;
    let mut test_block = create_block_with_coinbase_tx(block_height);
    let mut previous_out: Option<OutPoint> = _previous_out;
    let mut txs = binaries
        .into_iter()
        .zip(cellpacks.into_iter())
        .map(|i| {
            let (binary, cellpack) = i;
            let witness = if binary.len() == 0 {
                Witness::new()
            } else {
                RawEnvelope::from(binary).to_witness(true)
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
        .collect::<Vec<bitcoin::Transaction>>();
    test_block.txdata.append(&mut txs);
    test_block
}

pub fn create_multiple_cellpack_with_witness_and_in(
    witness: Witness,
    cellpacks: Vec<Cellpack>,
    previous_output: OutPoint,
    etch: bool,
) -> bitcoin::Transaction {
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
) -> bitcoin::Transaction {
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

    let address: Address = get_address(&ADDRESS1().as_str());

    let script_pubkey = address.script_pubkey();
    let txout = TxOut {
        value: Amount::from_sat(100_000_000),
        script_pubkey,
    };
    bitcoin::Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: txins,
        output: vec![txout, op_return],
    }
}

pub fn create_cellpack_with_witness(witness: Witness, cellpack: Cellpack) -> bitcoin::Transaction {
    create_multiple_cellpack_with_witness(witness, [cellpack].into(), false)
}

pub fn create_multiple_cellpack_with_witness(
    witness: Witness,
    cellpacks: Vec<Cellpack>,
    etch: bool,
) -> bitcoin::Transaction {
    let previous_output = OutPoint {
        txid: bitcoin::Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap(),
        vout: 0,
    };
    create_multiple_cellpack_with_witness_and_in(witness, cellpacks, previous_output, etch)
}