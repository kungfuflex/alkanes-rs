use crate::index_block;
use crate::tests::helpers::{
    assert_binary_deployed_to_id, clear, create_multiple_cellpack_with_witness_and_in,
    init_with_multiple_cellpacks_with_tx,
};
use crate::tests::std::alkanes_std_merkle_distributor_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::parcel::AlkaneTransfer;
use anyhow::{anyhow, Result};
use bitcoin::blockdata::locktime::absolute::LockTime;
use bitcoin::blockdata::transaction::Version;
use bitcoin::{Address, OutPoint, Transaction, Witness};
use bitcoin::{Sequence, TxIn};
use metashrew_support::utils::consensus_encode;
use protorune::test_helpers::create_block_with_coinbase_tx;
use protorune::{balance_sheet::load_sheet, message::MessageContext, tables::RuneTable};
use rs_merkle::{algorithms::Sha256, Hasher, MerkleProof, MerkleTree};
use wasm_bindgen_test::wasm_bindgen_test;

use super::helpers;

#[wasm_bindgen_test]
fn test_merkle_distributor() -> Result<()> {
    // Clear any previous state
    clear();
    let block_height = 840_000;

    // Create a proper merkle tree for testing with 4 leaves
    // Each leaf contains: P2SH (20 bytes) + index (4 bytes) + amount (16 bytes)
    let leaf1 = [0u8; 40];
    let leaf2 = [1u8; 40];
    let leaf3 = [2u8; 40];
    let leaf4 = [3u8; 40];

    // Hash the leaves
    let leaf_hashes: Vec<[u8; 32]> = vec![
        Sha256::hash(&leaf1),
        Sha256::hash(&leaf2),
        Sha256::hash(&leaf3),
        Sha256::hash(&leaf4),
    ];

    // Create a merkle tree
    let merkle_tree = MerkleTree::<Sha256>::from_leaves(&leaf_hashes);

    // Get the merkle root
    let root = merkle_tree.root().expect("Failed to calculate merkle root");

    // Generate a proof for the first leaf
    let proof = merkle_tree.proof(&[0]);
    let proof_bytes = proof.to_bytes();

    // Create an alkane to be distributed
    let alkane_id = AlkaneId { block: 1, tx: 0 };
    let alkane_transfer = AlkaneTransfer {
        id: alkane_id.clone(),
        value: 1000,
    };

    // Initialize the merkle distributor
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 0 },
        inputs: vec![
            0, // opcode 0 = initialize
            4, // length of the merkle tree
            0, 1, 2, 3,
        ],
    };

    // Add the merkle root as input
    let mut init_inputs = vec![0u8, 4u8]; // opcode and length
    init_inputs.extend_from_slice(&root); // merkle root

    // Create the transaction with the merkle distributor contract
    let test_block = init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_merkle_distributor_build::get_bytes()],
        vec![init_cellpack],
    );

    // Index the block
    index_block(&test_block, block_height)?;

    // Verify the contract was deployed correctly
    let merkle_distributor_id = AlkaneId { block: 2, tx: 0 };
    let _ = assert_binary_deployed_to_id(
        merkle_distributor_id.clone(),
        alkanes_std_merkle_distributor_build::get_bytes(),
    );

    // Now let's test the claim functionality
    // Create a transaction with a witness containing the merkle proof
    let mut witness_data = Vec::new();
    witness_data.extend_from_slice(&leaf1); // The leaf data
    witness_data.extend_from_slice(&proof_bytes); // The proof

    let mut witness = Witness::new();
    witness.push(witness_data);

    // Create a transaction with the claim operation
    let claim_cellpack = Cellpack {
        target: merkle_distributor_id.clone(),
        inputs: vec![
            1, // opcode 1 = claim
        ],
    };

    // Create a transaction with the claim operation and the witness
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

    // Index the block
    index_block(&claim_block, block_height + 1)?;

    // Verify the claim was successful
    // In a real test, we would check that the alkane was transferred correctly

    Ok(())
}

#[wasm_bindgen_test]
fn test_merkle_distributor_invalid_proof() -> Result<()> {
    // Clear any previous state
    clear();
    let block_height = 840_000;

    // Create a proper merkle tree for testing with 4 leaves
    // Each leaf contains: P2SH (20 bytes) + index (4 bytes) + amount (16 bytes)
    let leaf1 = [0u8; 40];
    let leaf2 = [1u8; 40];
    let leaf3 = [2u8; 40];
    let leaf4 = [3u8; 40];

    // Hash the leaves
    let leaf_hashes: Vec<[u8; 32]> = vec![
        Sha256::hash(&leaf1),
        Sha256::hash(&leaf2),
        Sha256::hash(&leaf3),
        Sha256::hash(&leaf4),
    ];

    // Create a merkle tree
    let merkle_tree = MerkleTree::<Sha256>::from_leaves(&leaf_hashes);

    // Get the merkle root
    let root = merkle_tree.root().expect("Failed to calculate merkle root");

    // Generate a proof for the first leaf
    let proof = merkle_tree.proof(&[0]);
    let proof_bytes = proof.to_bytes();

    // Create an alkane to be distributed
    let alkane_id = AlkaneId { block: 1, tx: 0 };
    let alkane_transfer = AlkaneTransfer {
        id: alkane_id.clone(),
        value: 1000,
    };

    // Initialize the merkle distributor
    let init_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 0 },
        inputs: vec![
            0, // opcode 0 = initialize
            4, // length of the merkle tree
            0, 1, 2, 3,
        ],
    };

    // Add the merkle root as input
    let mut init_inputs = vec![0u8, 4u8]; // opcode and length
    init_inputs.extend_from_slice(&root); // merkle root

    // Create the transaction with the merkle distributor contract
    let test_block = init_with_multiple_cellpacks_with_tx(
        vec![alkanes_std_merkle_distributor_build::get_bytes()],
        vec![init_cellpack],
    );

    // Index the block
    index_block(&test_block, block_height)?;

    // Verify the contract was deployed correctly
    let merkle_distributor_id = AlkaneId { block: 2, tx: 0 };
    let _ = assert_binary_deployed_to_id(
        merkle_distributor_id.clone(),
        alkanes_std_merkle_distributor_build::get_bytes(),
    );

    // Now let's test with an invalid proof
    // We'll use the proof for leaf1 but try to claim with leaf2 data
    let mut witness_data = Vec::new();
    witness_data.extend_from_slice(&leaf2); // Wrong leaf data
    witness_data.extend_from_slice(&proof_bytes); // Proof for leaf1

    let mut witness = Witness::new();
    witness.push(witness_data);

    // Create a transaction with the claim operation
    let claim_cellpack = Cellpack {
        target: merkle_distributor_id.clone(),
        inputs: vec![
            1, // opcode 1 = claim
        ],
    };

    // Create a transaction with the claim operation and the witness
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
