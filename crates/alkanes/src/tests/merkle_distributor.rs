use crate::index_block;
use crate::message::AlkaneMessageContext;
use crate::tests::helpers::{
    self as alkane_helpers,
    assert_binary_deployed_to_id,
    assert_revert_context,
    create_multiple_cellpack_with_witness_and_in,
    get_last_outpoint_sheet,
    get_sheet_for_outpoint,
    init_with_multiple_cellpacks_with_tx,
};

use crate::tests::test_runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::envelope::RawEnvelope;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{Block, OutPoint, Witness};
use borsh::{BorshDeserialize, BorshSerialize};

use metashrew_support::environment::RuntimeEnvironment;
use protorune::test_helpers::{create_block_with_coinbase_tx, ADDRESS1, ADDRESS2};
use protorune_support::balance_sheet::BalanceSheetOperations;
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::protostone::ProtostoneEdict;
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

// This function generates a proof for a leaf at a given index.
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

fn helper_test_merkle_distributor<E: RuntimeEnvironment + Clone + Default + 'static>(
    block_height: u32,
    deadline: u128,
    leaf_address: String,
    output_address_index: u32,
) -> Result<Block> {
    let leaf1 = borsh::to_vec(&SchemaMerkleLeaf {
        address: leaf_address,
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
            deadline, // block deadline
            root_first_half,
            root_second_half,
        ],
    };

    let mint_diesel = Cellpack {
        target: AlkaneId { block: 2, tx: 0 },
        inputs: vec![77],
    };

    let merkle_testnet_build = include_bytes!(
        "../../../../target/alkanes/wasm32-unknown-unknown/release/alkanes_std_merkle_distributor_regtest.wasm"
    )
    .to_vec();

    let test_block = init_with_multiple_cellpacks_with_tx(
        vec![[].into(), merkle_testnet_build.clone()],
        vec![mint_diesel, init_cellpack],
    );

    index_block::<E>(&test_block, block_height)?;

    let merkle_distributor_id = AlkaneId { block: 2, tx: 1 };
    assert_binary_deployed_to_id(merkle_distributor_id.clone(), merkle_testnet_build.clone())?;

    let mut proof_hashes = generate_proof(&leaf_hashes, 0);

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

    let mut claim_block = create_block_with_coinbase_tx(block_height + 1);
    claim_block.txdata.push(create_multiple_cellpack_with_witness_and_in(
        witness,
        vec![claim_cellpack],
        OutPoint {
            txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
            vout: output_address_index,
        },
        false,
    ));

    index_block::<E>(&claim_block, block_height + 1)?;

    let sheet = get_last_outpoint_sheet(&claim_block)?;
    assert_eq!(
        sheet.get_cached(&(ProtoruneRuneId { block: 2, tx: 0 }).into()),
        1_000_000
    );

    Ok(claim_block)
}

#[test]
fn test_merkle_distributor() -> Result<()> {
    alkane_helpers::clear::<TestRuntime>();
    helper_test_merkle_distributor(840_000, 900_000, ADDRESS1(), 0)?;
    Ok(())
}

#[test]
fn test_merkle_distributor_admin_collect() -> Result<()> {
    alkane_helpers::clear::<TestRuntime>();
    let init_block =
        helper_test_merkle_distributor(840_000, 900_000, ADDRESS1(), 0)?;
    let auth_outpoint = OutPoint {
        txid: init_block.txdata.last().unwrap().compute_txid(),
        vout: 0,
    };
    let merkle_distributor_id = AlkaneId { block: 2, tx: 1 };
    let auth_sheet =
        get_sheet_for_outpoint(&init_block, init_block.txdata.len() - 1, 0)?;
    assert_eq!(
        auth_sheet.get_cached(&merkle_distributor_id.clone().into()),
        5
    );
    let block_height = 840_001;
    let mut spend_block = create_block_with_coinbase_tx(block_height);
    let collect_tx =
        alkane_helpers::create_multiple_cellpack_with_witness_and_in_with_edicts(
            Witness::new(),
            vec![
                alkane_helpers::CellpackOrEdict::Edict(vec![ProtostoneEdict {
                    id: merkle_distributor_id.into(),
                    amount: 1,
                    output: 0,
                }]),
                alkane_helpers::CellpackOrEdict::Cellpack(Cellpack {
                    target: merkle_distributor_id.clone().into(),
                    inputs: vec![2, 2, 0],
                }),
            ],
            auth_outpoint.clone(),
            false,
        );
    spend_block.txdata.push(collect_tx.clone());
    index_block::<TestRuntime>(&spend_block, block_height)?;
    let sheet = get_last_outpoint_sheet(&spend_block)?;
    assert_eq!(
        sheet.get_cached(&merkle_distributor_id.clone().into()),
        5
    );
    assert_eq!(
        sheet.get_cached(&(ProtoruneRuneId { block: 2, tx: 0 }).into()),
        312500000
    );

    Ok(())
}
#[test]
fn test_merkle_distributor_admin_collect_no_auth() -> Result<()> {
    alkane_helpers::clear::<TestRuntime>();
    let init_block =
        helper_test_merkle_distributor(840_000, 900_000, ADDRESS1(), 0)?;
    let merkle_distributor_id = AlkaneId { block: 2, tx: 1 };
    let block_height = 840_001;
    let mut spend_block = create_block_with_coinbase_tx(block_height);
    let collect_tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack {
            target: merkle_distributor_id.clone().into(),
            inputs: vec![2],
        }],
        OutPoint::default(),
        false,
    );
    spend_block.txdata.push(collect_tx.clone());
    index_block::<TestRuntime>(&spend_block, block_height)?;
    let new_outpoint = OutPoint {
        txid: collect_tx.compute_txid(),
        vout: 3,
    };
    assert_revert_context(&new_outpoint, "Auth token is not in incoming alkanes")?;
    Ok(())
}