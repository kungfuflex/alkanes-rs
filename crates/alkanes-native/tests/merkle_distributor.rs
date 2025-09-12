#[path = "helpers.rs"]
mod helpers;

use anyhow::Result;
use bitcoin::{Block, OutPoint, Witness};
use helpers::{
    create_multiple_cellpack_with_witness_and_in, init_with_multiple_cellpacks_with_tx,
    TestHarness,
};

use alkanes_indexer::message::AlkaneMessageContext;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::envelope::RawEnvelope;
use alkanes_support::gz::compress;
use alkanes_support::id::AlkaneId;
use borsh::{BorshDeserialize, BorshSerialize};
use memshrew_runtime::{KeyValueStoreLike, MemStoreAdapter};
use metashrew_core::index_pointer::IndexPointer;
use metashrew_core::native_host::StorageAdapter as NativeStorageAdapter;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_encode;
use protorune::balance_sheet::load_sheet;
use protorune::message::MessageContext;
use protorune::tables::RuneTable;
use protorune::test_helpers::{create_block_with_coinbase_tx, ADDRESS1, ADDRESS2};
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use sha2::{Digest, Sha256};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct SchemaMerkleLeaf {
    pub address: String,
    pub amount: u128,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct SchemaMerkleProof {
    pub leaf: Vec<u8>,
    pub proofs: Vec<Vec<u8>>,
}

fn calculate_merkle_root(leaf_hashes: &[[u8; 32]]) -> [u8; 32] {
    if leaf_hashes.is_empty() {
        return [0; 32];
    }
    let mut nodes = leaf_hashes.to_vec();
    while nodes.len() > 1 {
        if nodes.len() % 2 != 0 {
            nodes.push(nodes.last().unwrap().clone());
        }
        let mut next_level = vec![];
        for chunk in nodes.chunks(2) {
            let left = chunk[0];
            let right = chunk[1];

            let (sorted_left, sorted_right) = if left <= right {
                (left, right)
            } else {
                (right, left)
            };

            let mut hasher = Sha256::new();
            hasher.update(&sorted_left);
            hasher.update(&sorted_right);
            let parent: [u8; 32] = hasher.finalize().into();
            next_level.push(parent);
        }
        nodes = next_level;
    }
    nodes[0]
}

fn generate_proof(leaf_hashes: &[[u8; 32]], leaf_index: usize) -> Vec<[u8; 32]> {
    if leaf_hashes.len() <= 1 {
        return vec![];
    }

    let mut proof = vec![];
    let mut nodes = leaf_hashes.to_vec();
    let mut current_index = leaf_index;

    while nodes.len() > 1 {
        if nodes.len() % 2 != 0 {
            nodes.push(nodes.last().unwrap().clone());
        }

        let sibling_index = if current_index % 2 == 0 {
            current_index + 1
        } else {
            current_index - 1
        };
        proof.push(nodes[sibling_index]);

        let mut next_level = vec![];
        for chunk in nodes.chunks(2) {
            let left = chunk[0];
            let right = chunk[1];

            let (sorted_left, sorted_right) = if left <= right {
                (left, right)
            } else {
                (right, left)
            };

            let mut hasher = Sha256::new();
            hasher.update(&sorted_left);
            hasher.update(&sorted_right);
            let parent: [u8; 32] = hasher.finalize().into();
            next_level.push(parent);
        }
        nodes = next_level;
        current_index /= 2;
    }
    proof
}

pub fn assert_binary_deployed_to_id(
    harness: &TestHarness,
    token_id: AlkaneId,
    binary: Vec<u8>,
) -> Result<()> {
    let key = IndexPointer::from_keyword("/alkanes/").select(&token_id.into()).key;
    let db = harness.runtime.context.lock().unwrap().db.clone();
    let runtime_binary = db.get(&key)?.unwrap();
    let compressed_binary = compress(binary)?;
    assert_eq!(runtime_binary.len(), compressed_binary.len());
    Ok(())
}

pub fn get_last_outpoint_sheet(
    harness: &TestHarness,
    test_block: &Block,
) -> Result<BalanceSheet> {
    let len = test_block.txdata.len();
    get_sheet_for_outpoint(harness, test_block, len - 1, 0)
}

pub fn get_sheet_for_outpoint(
    harness: &TestHarness,
    test_block: &Block,
    tx_num: usize,
    vout: u32,
) -> Result<BalanceSheet> {
    let outpoint = OutPoint {
        txid: test_block.txdata[tx_num].compute_txid(),
        vout,
    };
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(&outpoint)?);

    let db = harness.runtime.context.lock().unwrap().db.clone();
    let sheet = load_sheet(&ptr.with_storage(db.clone()));
    Ok(sheet)
}

#[tokio::test]
async fn test_merkle_distributor() -> Result<()> {
    let mut harness = TestHarness::new();

    let leaf1 = borsh::to_vec(&SchemaMerkleLeaf {
        address: ADDRESS1(),
        amount: 1_000_000,
    })?;
    let leaf2 = borsh::to_vec(&SchemaMerkleLeaf {
        address: ADDRESS2(),
        amount: 1_000_000,
    })?;
    let leaf3 = borsh::to_vec(&SchemaMerkleLeaf {
        address: ADDRESS1(),
        amount: 2_000_000,
    })?;
    let leaf4 = borsh::to_vec(&SchemaMerkleLeaf {
        address: ADDRESS2(),
        amount: 3_000_000,
    })?;

    let leaf_hashes: Vec<[u8; 32]> = vec![
        Sha256::digest(&leaf1).into(),
        Sha256::digest(&leaf2).into(),
        Sha256::digest(&leaf3).into(),
        Sha256::digest(&leaf4).into(),
    ];

    let root = calculate_merkle_root(&leaf_hashes);

    let root_first_half = u128::from_le_bytes(root[0..16].try_into()?);
    let root_second_half = u128::from_le_bytes(root[16..32].try_into()?);
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![
            0, // opcode 0 = initialize
            2,
            0, // alkane id of input alkane
            312500000,
            900_000, // block deadline
            root_first_half,
            root_second_half,
        ],
    };

    let mint_diesel = Cellpack {
        target: AlkaneId { block: 2, tx: 0 },
        inputs: vec![77],
    };

    let merkle_testnet_build = alkanes_indexer::tests::std::alkanes_std_merkle_distributor_regtest_build::get_bytes();

    let test_block = init_with_multiple_cellpacks_with_tx(
        vec![[].into(), merkle_testnet_build.clone()],
        vec![mint_diesel, init_cellpack],
    );

    harness.add_block(test_block.clone());
    harness.process_block().await;

    let merkle_distributor_id = AlkaneId { block: 2, tx: 1 };
    assert_binary_deployed_to_id(&harness, merkle_distributor_id.clone(), merkle_testnet_build)?;

    let proof_hashes = generate_proof(&leaf_hashes, 0);
    let merkle_proof = SchemaMerkleProof {
        leaf: leaf1,
        proofs: proof_hashes.iter().map(|v| v.to_vec()).collect(),
    };
    let witness_data = borsh::to_vec(&merkle_proof)?;

    let witness = RawEnvelope::from(witness_data).to_witness(false);

    let claim_cellpack = Cellpack {
        target: merkle_distributor_id.clone(),
        inputs: vec![1],
    };

    let mut claim_block =
        create_block_with_coinbase_tx(harness.node.blocks.lock().unwrap().len() as u32);
    claim_block
        .txdata
        .push(create_multiple_cellpack_with_witness_and_in(
            witness,
            vec![claim_cellpack],
            OutPoint {
                txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
                vout: 0,
            },
            false,
        ));

    harness.add_block(claim_block.clone());
    harness.process_block().await;

    let sheet = get_last_outpoint_sheet(&harness, &claim_block)?;
    assert_eq!(
        sheet.get(&ProtoruneRuneId { block: 2, tx: 0 }).unwrap_or(0),
        1_000_000
    );

    Ok(())
}