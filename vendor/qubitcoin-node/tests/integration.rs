//! Integration tests for the full Qubitcoin node stack.
//!
//! These tests exercise the TestChain framework, consensus validation,
//! merkle root computation, UTXO tracking, and block subsidy halving
//! across the qubitcoin-node, qubitcoin-consensus, and qubitcoin-common
//! crates working together.

use std::sync::Arc;

use qubitcoin_common::coins::CoinsView;
use qubitcoin_consensus::check::{check_proof_of_work, get_block_subsidy};
use qubitcoin_consensus::merkle::block_merkle_root;
use qubitcoin_node::test_framework::TestChain;
use qubitcoin_primitives::{Amount, BlockHash, COIN};

// ---------------------------------------------------------------------------
// 1. Genesis block tests
// ---------------------------------------------------------------------------

#[test]
fn test_full_chain_from_genesis() {
    let chain = TestChain::new();
    assert_eq!(chain.height(), 0);
    assert_ne!(*chain.tip_hash(), BlockHash::ZERO);
}

#[test]
fn test_genesis_block_has_coinbase() {
    let chain = TestChain::new();
    let genesis = chain.block_at(0).expect("Genesis block must exist");
    assert_eq!(genesis.vtx.len(), 1, "Genesis should have exactly one tx");
    assert!(
        genesis.vtx[0].is_coinbase(),
        "Genesis tx should be a coinbase"
    );
}

#[test]
fn test_genesis_coinbase_value() {
    let chain = TestChain::new();
    let genesis = chain.block_at(0).unwrap();
    let coinbase_value = genesis.vtx[0].vout[0].value;
    assert_eq!(
        coinbase_value,
        Amount::from_sat(50 * COIN),
        "Genesis coinbase should be 50 BTC"
    );
}

// ---------------------------------------------------------------------------
// 2. Mine and spend workflow
// ---------------------------------------------------------------------------

#[test]
fn test_mine_and_spend_workflow() {
    let mut chain = TestChain::new();

    // Mine 100 blocks to mature first coinbase
    chain.mine_empty_blocks(100);
    assert_eq!(chain.height(), 100);

    // Get a spendable output
    let (outpoint, amount) = chain
        .get_spendable_output()
        .expect("should have spendable output");
    assert_eq!(amount.to_sat(), 50 * COIN); // 50 BTC

    // Create a transaction spending the coinbase
    let dest_script = chain.coinbase_script().clone();
    let tx = chain
        .create_transaction(&outpoint, Amount::from_sat(49 * COIN), &dest_script)
        .expect("should create tx");

    // Mine the transaction
    let block = chain.mine_block(vec![tx]);
    assert_eq!(chain.height(), 101);
    assert_eq!(block.vtx.len(), 2); // coinbase + our tx

    // Verify the block has valid PoW
    let hash_uint = block.header.block_hash().into_uint256();
    assert!(check_proof_of_work(
        &hash_uint,
        block.header.bits,
        &chain.params().consensus,
    ));
}

// ---------------------------------------------------------------------------
// 3. Block subsidy halving (regtest halves at 150)
// ---------------------------------------------------------------------------

#[test]
fn test_block_subsidy_halving() {
    let chain = TestChain::new();
    let params = &chain.params().consensus;

    // Blocks 0-149 should have 50 BTC subsidy
    let subsidy_before = get_block_subsidy(149, params);
    assert_eq!(subsidy_before.to_sat(), 50 * COIN);

    // Block 150 should have halved subsidy
    let subsidy_after = get_block_subsidy(150, params);
    assert_eq!(subsidy_after.to_sat(), 25 * COIN);

    // Block 300 should have quartered subsidy
    let subsidy_300 = get_block_subsidy(300, params);
    assert_eq!(subsidy_300.to_sat(), (125 * COIN) / 10);
}

#[test]
fn test_halving_affects_mined_blocks() {
    let mut chain = TestChain::new();

    // Mine up to block 149 (still pre-halving)
    chain.mine_empty_blocks(149);
    assert_eq!(chain.height(), 149);

    // Block 149's coinbase should be 50 BTC
    let block_149 = chain.block_at(149).unwrap();
    assert_eq!(block_149.vtx[0].vout[0].value.to_sat(), 50 * COIN);

    // Block 150 is the first halved block
    chain.mine_block(vec![]);
    let block_150 = chain.block_at(150).unwrap();
    assert_eq!(block_150.vtx[0].vout[0].value.to_sat(), 25 * COIN);
}

// ---------------------------------------------------------------------------
// 4. Merkle root consistency
// ---------------------------------------------------------------------------

#[test]
fn test_merkle_root_consistency() {
    let mut chain = TestChain::new();
    chain.mine_empty_blocks(100);

    // Mine a block with a transaction
    let (outpoint, _) = chain.get_spendable_output().unwrap();
    let dest = chain.coinbase_script().clone();
    let tx = chain
        .create_transaction(&outpoint, Amount::from_sat(1 * COIN), &dest)
        .unwrap();
    let block = chain.mine_block(vec![tx]);

    // Verify merkle root matches
    let mut mutated = false;
    let computed_root = block_merkle_root(&block.vtx, &mut mutated);
    assert_eq!(computed_root, block.header.merkle_root);
    assert!(!mutated);
}

#[test]
fn test_merkle_root_single_coinbase() {
    let mut chain = TestChain::new();
    let block = chain.mine_block(vec![]);

    let mut mutated = false;
    let computed = block_merkle_root(&block.vtx, &mut mutated);
    assert_eq!(computed, block.header.merkle_root);
    assert!(!mutated);
}

// ---------------------------------------------------------------------------
// 5. UTXO tracking across blocks
// ---------------------------------------------------------------------------

#[test]
fn test_utxo_tracking_across_blocks() {
    let mut chain = TestChain::new();
    chain.mine_empty_blocks(100);

    // Spend first mature coinbase
    let (outpoint, _) = chain.get_spendable_output().unwrap();
    let dest = chain.coinbase_script().clone();
    let tx = chain
        .create_transaction(&outpoint, Amount::from_sat(10 * COIN), &dest)
        .unwrap();
    chain.mine_block(vec![tx]);

    // The original outpoint should no longer be spendable
    assert!(
        !chain.coins().have_coin(&outpoint),
        "Spent outpoint should be removed from UTXO set"
    );
}

#[test]
fn test_utxo_new_outputs_appear() {
    let mut chain = TestChain::new();
    chain.mine_empty_blocks(100);

    let (outpoint, _) = chain.get_spendable_output().unwrap();
    let dest = chain.coinbase_script().clone();
    let tx = chain
        .create_transaction(&outpoint, Amount::from_sat(10 * COIN), &dest)
        .unwrap();

    let txid = *tx.txid();
    chain.mine_block(vec![Arc::clone(&tx)]);

    // The new outputs should exist in the UTXO set
    let new_outpoint_0 = qubitcoin_consensus::transaction::OutPoint::new(txid, 0);
    assert!(
        chain.coins().have_coin(&new_outpoint_0),
        "New output 0 should exist in UTXO set"
    );

    // Change output (if it exists)
    if tx.vout.len() > 1 {
        let new_outpoint_1 = qubitcoin_consensus::transaction::OutPoint::new(txid, 1);
        assert!(
            chain.coins().have_coin(&new_outpoint_1),
            "Change output should exist in UTXO set"
        );
    }
}

// ---------------------------------------------------------------------------
// 6. Multiple transactions in a block
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_transactions_in_block() {
    let mut chain = TestChain::new();
    chain.mine_empty_blocks(104); // Mature 5 coinbases (heights 0..=4)

    // To build multiple transactions for a single block, we need distinct
    // outpoints. Since get_spendable_output() always returns the first unspent
    // one, we look up known coinbase transactions by height directly.
    let dest = chain.coinbase_script().clone();
    let mut txs = vec![];

    // Collect distinct mature coinbase outpoints from blocks 0, 1, 2.
    // At height 104, blocks 0..=4 have mature coinbases (100+ confirmations).
    for h in 0..3i32 {
        let block = chain.block_at(h).unwrap();
        let coinbase_txid = *block.vtx[0].txid();
        let outpoint = qubitcoin_consensus::transaction::OutPoint::new(coinbase_txid, 0);

        if chain.coins().have_coin(&outpoint) {
            if let Some(tx) = chain.create_transaction(&outpoint, Amount::from_sat(1 * COIN), &dest)
            {
                txs.push(tx);
            }
        }
    }

    let num_user_txs = txs.len();
    assert!(
        num_user_txs >= 2,
        "Should have at least 2 transactions to mine"
    );

    let block = chain.mine_block(txs);
    // Block should have coinbase + our transactions
    assert_eq!(
        block.vtx.len(),
        1 + num_user_txs,
        "Block should contain coinbase + {} user txs",
        num_user_txs
    );
}

// ---------------------------------------------------------------------------
// 7. Chain continuity
// ---------------------------------------------------------------------------

#[test]
fn test_chain_continuity() {
    let mut chain = TestChain::new();

    for i in 0..20 {
        chain.mine_block(vec![]);
        let block = chain.block_at(i + 1).unwrap();
        let prev_block = chain.block_at(i).unwrap();

        // Each block's prev_blockhash should point to the previous block
        assert_eq!(
            block.header.prev_blockhash,
            prev_block.header.block_hash(),
            "Block at height {} should reference block at height {}",
            i + 1,
            i,
        );
    }
}

#[test]
fn test_tip_hash_updates() {
    let mut chain = TestChain::new();

    for _ in 0..10 {
        let prev_tip = *chain.tip_hash();
        chain.mine_block(vec![]);
        let new_tip = *chain.tip_hash();
        assert_ne!(
            prev_tip, new_tip,
            "Tip hash should change after mining a block"
        );
    }
}

// ---------------------------------------------------------------------------
// 8. All blocks have valid PoW
// ---------------------------------------------------------------------------

#[test]
fn test_all_blocks_have_valid_pow() {
    let mut chain = TestChain::new();
    chain.mine_empty_blocks(50);

    let params = &chain.params().consensus;
    for h in 0..=chain.height() {
        let block = chain.block_at(h).unwrap();
        let hash_uint = block.header.block_hash().into_uint256();
        assert!(
            check_proof_of_work(&hash_uint, block.header.bits, params),
            "Block at height {} has invalid PoW",
            h
        );
    }
}

// ---------------------------------------------------------------------------
// 9. Block timestamps increase
// ---------------------------------------------------------------------------

#[test]
fn test_block_timestamps_increase() {
    let mut chain = TestChain::new();
    chain.mine_empty_blocks(20);

    for h in 1..=chain.height() {
        let block = chain.block_at(h).unwrap();
        let prev_block = chain.block_at(h - 1).unwrap();
        assert!(
            block.header.time >= prev_block.header.time,
            "Block {} time {} < block {} time {}",
            h,
            block.header.time,
            h - 1,
            prev_block.header.time
        );
    }
}

// ---------------------------------------------------------------------------
// 10. Block hash uniqueness
// ---------------------------------------------------------------------------

#[test]
fn test_block_hash_uniqueness() {
    let mut chain = TestChain::new();
    chain.mine_empty_blocks(50);

    let mut hashes = std::collections::HashSet::new();
    for h in 0..=chain.height() {
        let block = chain.block_at(h).unwrap();
        let hash = block.header.block_hash();
        assert!(
            hashes.insert(hash),
            "Block hash at height {} should be unique",
            h,
        );
    }
}

// ---------------------------------------------------------------------------
// 11. Transaction validation: insufficient funds
// ---------------------------------------------------------------------------

#[test]
fn test_create_transaction_insufficient_funds() {
    let mut chain = TestChain::new();
    chain.mine_empty_blocks(100);

    let (outpoint, _value) = chain.get_spendable_output().unwrap();

    // Try to spend more than the coin holds
    let dest = qubitcoin_script::Script::from_bytes(vec![0x51]);
    let too_much = Amount::from_sat(51 * COIN);
    let result = chain.create_transaction(&outpoint, too_much, &dest);
    assert!(result.is_none(), "Should fail when value exceeds input");
}

// ---------------------------------------------------------------------------
// 12. Spend multiple coinbases
// ---------------------------------------------------------------------------

#[test]
fn test_spend_multiple_coinbases() {
    let mut chain = TestChain::new();
    // Mine enough blocks so multiple coinbases are mature
    chain.mine_empty_blocks(104);
    assert_eq!(chain.height(), 104);

    // We should have 5 mature coinbases (heights 0..=4)
    assert_eq!(chain.mature_coinbase_count(), 5);

    let dest = chain.coinbase_script().clone();
    let send = Amount::from_sat(1 * COIN);

    // Spend them one by one
    for _ in 0..5 {
        let (outpoint, _value) = chain
            .get_spendable_output()
            .expect("Should have a spendable output");
        let tx = chain
            .create_transaction(&outpoint, send, &dest)
            .expect("Transaction creation should succeed");
        chain.mine_block(vec![tx]);
    }

    // After spending 5 and mining 5 more blocks, new coinbases should be available
    let next = chain.get_spendable_output();
    assert!(
        next.is_some(),
        "Newly matured coinbases should be available"
    );
}

// ---------------------------------------------------------------------------
// 13. Coinbase maturity enforcement
// ---------------------------------------------------------------------------

#[test]
fn test_coinbase_maturity_boundary() {
    let mut chain = TestChain::new();

    // At height 0, no mature coinbases
    assert_eq!(chain.mature_coinbase_count(), 0);
    assert!(chain.get_spendable_output().is_none());

    // Mine 99 blocks (height 99): genesis coinbase has 99 confirmations, not yet mature
    chain.mine_empty_blocks(99);
    assert_eq!(chain.height(), 99);
    assert_eq!(chain.mature_coinbase_count(), 0);
    assert!(chain.get_spendable_output().is_none());

    // Mine 1 more block (height 100): genesis coinbase now has 100 confirmations, mature!
    chain.mine_block(vec![]);
    assert_eq!(chain.height(), 100);
    assert_eq!(chain.mature_coinbase_count(), 1);

    let (outpoint, value) = chain
        .get_spendable_output()
        .expect("Should have one mature coinbase");
    assert_eq!(value, Amount::from_sat(50 * COIN));

    // The outpoint should be output 0 of the genesis coinbase tx
    let genesis = chain.block_at(0).unwrap();
    assert_eq!(outpoint.hash, *genesis.vtx[0].txid());
    assert_eq!(outpoint.n, 0);
}

// ---------------------------------------------------------------------------
// 14. Large chain test
// ---------------------------------------------------------------------------

#[test]
fn test_mine_200_blocks() {
    let mut chain = TestChain::new();
    chain.mine_empty_blocks(200);
    assert_eq!(chain.height(), 200);

    // All blocks should exist
    for h in 0..=200 {
        assert!(
            chain.block_at(h).is_some(),
            "Block at height {} should exist",
            h
        );
    }
}

// ---------------------------------------------------------------------------
// 15. Block at out-of-range returns None
// ---------------------------------------------------------------------------

#[test]
fn test_block_at_out_of_range() {
    let chain = TestChain::new();
    assert!(chain.block_at(-1).is_none());
    assert!(chain.block_at(1).is_none()); // only genesis at height 0
    assert!(chain.block_at(100).is_none());
}

// ---------------------------------------------------------------------------
// 16. Chain uses regtest parameters
// ---------------------------------------------------------------------------

#[test]
fn test_chain_uses_regtest() {
    let chain = TestChain::new();
    let params = chain.params();
    assert_eq!(
        params.network,
        qubitcoin_common::chainparams::Network::Regtest
    );
    assert_eq!(params.default_port, 18444);
    assert!(params.consensus.pow_allow_min_difficulty_blocks);
    assert!(params.consensus.pow_no_retargeting);
    assert_eq!(params.consensus.subsidy_halving_interval, 150);
}

// ---------------------------------------------------------------------------
// 17. Transaction output values are correct
// ---------------------------------------------------------------------------

#[test]
fn test_transaction_output_values() {
    let mut chain = TestChain::new();
    chain.mine_empty_blocks(100);

    let (outpoint, value) = chain.get_spendable_output().unwrap();
    let send_amount = Amount::from_sat(10 * COIN);
    let fee = Amount::from_sat(1000);
    let expected_change = value - send_amount - fee;

    let dest = chain.coinbase_script().clone();
    let tx = chain
        .create_transaction(&outpoint, send_amount, &dest)
        .unwrap();

    assert_eq!(
        tx.vout[0].value, send_amount,
        "Destination output value mismatch"
    );
    assert!(tx.vout.len() >= 2, "Should have a change output");
    assert_eq!(
        tx.vout[1].value, expected_change,
        "Change output value mismatch"
    );
}

// ---------------------------------------------------------------------------
// 18. Merkle roots for all mined blocks are valid
// ---------------------------------------------------------------------------

#[test]
fn test_all_merkle_roots_valid() {
    let mut chain = TestChain::new();
    chain.mine_empty_blocks(30);

    for h in 0..=chain.height() {
        let block = chain.block_at(h).unwrap();
        let mut mutated = false;
        let computed = block_merkle_root(&block.vtx, &mut mutated);
        assert_eq!(
            computed, block.header.merkle_root,
            "Merkle root mismatch at height {}",
            h
        );
        assert!(
            !mutated,
            "Merkle root should not be mutated at height {}",
            h
        );
    }
}

// ---------------------------------------------------------------------------
// 19. Genesis block has valid PoW
// ---------------------------------------------------------------------------

#[test]
fn test_genesis_has_valid_pow() {
    let chain = TestChain::new();
    let genesis = chain.block_at(0).unwrap();
    let hash_uint = genesis.header.block_hash().into_uint256();
    assert!(
        check_proof_of_work(&hash_uint, genesis.header.bits, &chain.params().consensus),
        "Genesis block should have valid PoW"
    );
}

// ---------------------------------------------------------------------------
// 20. Full workflow: mine, spend, verify, repeat
// ---------------------------------------------------------------------------

#[test]
fn test_full_lifecycle() {
    let mut chain = TestChain::new();

    // Phase 1: Mine blocks to maturity
    chain.mine_empty_blocks(100);
    assert_eq!(chain.height(), 100);
    assert!(chain.get_spendable_output().is_some());

    // Phase 2: Spend coins
    let (outpoint, _) = chain.get_spendable_output().unwrap();
    let dest = chain.coinbase_script().clone();
    let tx = chain
        .create_transaction(&outpoint, Amount::from_sat(25 * COIN), &dest)
        .unwrap();

    let block = chain.mine_block(vec![Arc::clone(&tx)]);
    assert_eq!(chain.height(), 101);
    assert_eq!(block.vtx.len(), 2);

    // Phase 3: Verify the transaction was mined
    assert!(!chain.coins().have_coin(&outpoint));

    let new_out = qubitcoin_consensus::transaction::OutPoint::new(*tx.txid(), 0);
    let coin = chain.coins().get_coin(&new_out).unwrap();
    assert_eq!(coin.tx_out.value, Amount::from_sat(25 * COIN));

    // Phase 4: Verify chain integrity
    for h in 1..=chain.height() {
        let blk = chain.block_at(h).unwrap();
        let prev = chain.block_at(h - 1).unwrap();
        assert_eq!(blk.header.prev_blockhash, prev.header.block_hash());
    }

    // Phase 5: Verify PoW for all blocks
    let params = &chain.params().consensus;
    for h in 0..=chain.height() {
        let blk = chain.block_at(h).unwrap();
        let hash_uint = blk.header.block_hash().into_uint256();
        assert!(check_proof_of_work(&hash_uint, blk.header.bits, params));
    }

    // Phase 6: Continue mining
    chain.mine_empty_blocks(50);
    assert_eq!(chain.height(), 151);

    // Block 150 should have halved subsidy
    let block_150 = chain.block_at(150).unwrap();
    assert_eq!(block_150.vtx[0].vout[0].value.to_sat(), 25 * COIN);
}
