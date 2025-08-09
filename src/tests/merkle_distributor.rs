use crate::index_block;
use crate::tests::helpers::{
    assert_binary_deployed_to_id, clear, create_multiple_cellpack_with_witness_and_in,
    init_with_multiple_cellpacks_with_tx,
};
use crate::tests::std::alkanes_std_merkle_distributor_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::envelope::RawEnvelope;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, Witness};
use protorune::test_helpers::{create_block_with_coinbase_tx, ADDRESS1, ADDRESS2};
use rs_merkle::{Hasher, MerkleTree};
use wasm_bindgen_test::wasm_bindgen_test;

use borsh::{BorshDeserialize, BorshSerialize};
use sha2::{Digest, Sha256};

#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};

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

#[derive(Clone)]
pub struct Sha256Algorithm;

impl Hasher for Sha256Algorithm {
    type Hash = [u8; 32];

    fn hash(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }
}

#[wasm_bindgen_test]
fn test_merkle_distributor() -> Result<()> {
    clear();
    let block_height = 840_000;

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
        Sha256Algorithm::hash(&leaf1),
        Sha256Algorithm::hash(&leaf2),
        Sha256Algorithm::hash(&leaf3),
        Sha256Algorithm::hash(&leaf4),
    ];

    let merkle_tree = MerkleTree::<Sha256Algorithm>::from_leaves(&leaf_hashes);
    let root = merkle_tree.root().expect("Failed to calculate merkle root");

    let root_first_half = u128::from_le_bytes(root[0..16].try_into()?);
    let root_second_half = u128::from_le_bytes(root[16..32].try_into()?);
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![
            0,       // opcode 0 = initialize
            4,       // length of the merkle tree
            900_000, // block deadline
            root_first_half,
            root_second_half,
        ],
    };

    let mint_diesel = Cellpack {
        target: AlkaneId { block: 2, tx: 0 },
        inputs: vec![77],
    };

    let test_block = init_with_multiple_cellpacks_with_tx(
        vec![[].into(), alkanes_std_merkle_distributor_build::get_bytes()],
        vec![mint_diesel, init_cellpack],
    );

    index_block(&test_block, block_height)?;

    let merkle_distributor_id = AlkaneId { block: 2, tx: 1 };
    assert_binary_deployed_to_id(
        merkle_distributor_id.clone(),
        alkanes_std_merkle_distributor_build::get_bytes(),
    )?;

    let proof = merkle_tree.proof(&[0]);
    let merkle_proof = SchemaMerkleProof {
        leaf: leaf1,
        proofs: proof.proof_hashes().iter().map(|v| v.to_vec()).collect(),
    };
    println!("merkle_proof: {:?}", merkle_proof);
    let witness_data = borsh::to_vec(&merkle_proof)?;

    let witness = RawEnvelope::from(witness_data).to_witness(false);

    let claim_cellpack = Cellpack {
        target: merkle_distributor_id.clone(),
        inputs: vec![1],
    };

    let mut claim_block = create_block_with_coinbase_tx(block_height + 1);
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

    index_block(&claim_block, block_height + 1)?;

    Ok(())
}

// #[wasm_bindgen_test]
// fn test_merkle_distributor_invalid_proof() -> Result<()> {
//     clear();
//     let block_height = 840_000;

//     let leaf1 = [0u8; 40];
//     let leaf2 = [1u8; 40];
//     let leaf3 = [2u8; 40];
//     let leaf4 = [3u8; 40];

//     let leaf_hashes: Vec<[u8; 32]> = vec![
//         Sha256Algorithm::hash(&leaf1),
//         Sha256Algorithm::hash(&leaf2),
//         Sha256Algorithm::hash(&leaf3),
//         Sha256Algorithm::hash(&leaf4),
//     ];

//     let merkle_tree = MerkleTree::<Sha256Algorithm>::from_leaves(&leaf_hashes);
//     let root = merkle_tree.root().expect("Failed to calculate merkle root");
//     let proof = merkle_tree.proof(&[0]);
//     let proof_bytes = proof.to_bytes();

//     let root_first_half = u128::from_le_bytes(root[0..16].try_into()?);
//     let root_second_half = u128::from_le_bytes(root[16..32].try_into()?);
//     let init_cellpack = Cellpack {
//         target: AlkaneId { block: 2, tx: 0 },
//         inputs: vec![
//             0,       // opcode 0 = initialize
//             4,       // length of the merkle tree
//             900_000, // block deadline
//             root_first_half,
//             root_second_half,
//         ],
//     };

//     let test_block = init_with_multiple_cellpacks_with_tx(
//         vec![alkanes_std_merkle_distributor_build::get_bytes()],
//         vec![init_cellpack],
//     );

//     index_block(&test_block, block_height)?;

//     let merkle_distributor_id = AlkaneId { block: 2, tx: 1 };
//     let _ = assert_binary_deployed_to_id(
//         merkle_distributor_id.clone(),
//         alkanes_std_merkle_distributor_build::get_bytes(),
//     );

//     let mut witness_data = Vec::new();
//     witness_data.extend_from_slice(&leaf2);
//     witness_data.extend_from_slice(&proof_bytes);

//     let mut witness = Witness::new();
//     witness.push(witness_data);

//     let claim_cellpack = Cellpack {
//         target: merkle_distributor_id.clone(),
//         inputs: vec![1],
//     };

//     let mut claim_block = create_block_with_coinbase_tx(block_height + 1);
//     claim_block
//         .txdata
//         .push(create_multiple_cellpack_with_witness_and_in(
//             witness,
//             vec![claim_cellpack],
//             OutPoint {
//                 txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
//                 vout: 0,
//             },
//             false,
//         ));

//     index_block(&claim_block, block_height + 1)?;

//     Ok(())
// }
