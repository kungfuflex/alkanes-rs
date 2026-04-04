//! In-memory blockchain testing framework.
//!
//! Provides `TestChain`, a self-contained in-memory blockchain that allows
//! downstream projects to create, mine, and manipulate a chain in tests
//! without touching disk or the network.
//!
//! The chain uses regtest parameters (very low proof-of-work difficulty,
//! 150-block halving interval) and an `EmptyCoinsView`-backed
//! `CoinsViewCache` for the UTXO set.

use std::sync::Arc;

use qubitcoin_common::chainparams::ChainParams;
use qubitcoin_common::coins::{add_coins, CoinsView, CoinsViewCache, EmptyCoinsView};
use qubitcoin_common::keys::{Key, PubKey};
use qubitcoin_consensus::block::{Block, BlockHeader};
use qubitcoin_consensus::check::{get_block_subsidy, COINBASE_MATURITY};
use qubitcoin_consensus::merkle::block_merkle_root;
use qubitcoin_consensus::transaction::{
    OutPoint, Transaction, TransactionRef, TxIn, TxOut, Witness, SEQUENCE_FINAL,
};
use qubitcoin_primitives::arith_uint256::uint256_to_arith;
use qubitcoin_primitives::{Amount, ArithUint256, BlockHash, COIN as SAT_PER_COIN};
use qubitcoin_script::{build_p2pkh, Script};

/// In-memory blockchain for testing.
///
/// Uses regtest parameters with an `EmptyCoinsView` backend. Automatically
/// mines a genesis block on construction so the chain starts at height 0.
pub struct TestChain {
    /// Chain parameters (regtest).
    params: ChainParams,
    /// All blocks in chain order (index == height).
    blocks: Vec<Block>,
    /// Block headers for lookup by height.
    headers: Vec<BlockHeader>,
    /// UTXO set.
    coins: CoinsViewCache,
    /// Current chain height (-1 means no blocks yet).
    height: i32,
    /// Hash of the current chain tip.
    tip_hash: BlockHash,
    /// Coinbase private key (for spending coinbase outputs).
    coinbase_key: Key,
    /// Coinbase public key.
    coinbase_pubkey: PubKey,
    /// Coinbase script (P2PKH paying to `coinbase_pubkey`).
    coinbase_script: Script,
    /// All coinbase transactions, one per block, for looking up spendable outputs.
    coinbase_txns: Vec<TransactionRef>,
}

impl TestChain {
    /// Create a new `TestChain` with a specific coinbase key (P2PKH outputs).
    pub fn new_with_key(coinbase_key: Key) -> Self {
        let params = ChainParams::regtest();
        let coinbase_pubkey = coinbase_key.get_pubkey();
        let pubkey_hash = coinbase_pubkey.get_id();
        let coinbase_script = build_p2pkh(&pubkey_hash);

        let coins = CoinsViewCache::new(Box::new(EmptyCoinsView));

        let mut chain = TestChain {
            params,
            blocks: Vec::new(),
            headers: Vec::new(),
            coins,
            height: -1,
            tip_hash: BlockHash::ZERO,
            coinbase_key,
            coinbase_pubkey,
            coinbase_script,
            coinbase_txns: Vec::new(),
        };

        chain.initialize_genesis();
        chain
    }

    /// Create a new `TestChain` with P2WPKH coinbase outputs (native segwit).
    ///
    /// Use this when the coinbase outputs need to be spendable by wallets
    /// that expect BIP84 native segwit addresses (bcrt1q...).
    pub fn new_with_key_wpkh(coinbase_key: Key) -> Self {
        use qubitcoin_script::build_p2wpkh;
        let params = ChainParams::regtest();
        let coinbase_pubkey = coinbase_key.get_pubkey();
        let pubkey_hash = coinbase_pubkey.get_id();
        let coinbase_script = build_p2wpkh(&pubkey_hash);

        let coins = CoinsViewCache::new(Box::new(EmptyCoinsView));

        let mut chain = TestChain {
            params,
            blocks: Vec::new(),
            headers: Vec::new(),
            coins,
            height: -1,
            tip_hash: BlockHash::ZERO,
            coinbase_key,
            coinbase_pubkey,
            coinbase_script,
            coinbase_txns: Vec::new(),
        };

        chain.initialize_genesis();
        chain
    }

    /// Create a new `TestChain` with a custom coinbase script.
    ///
    /// The key is still needed for signing, but the script determines which
    /// address receives mining rewards.
    pub fn new_with_key_and_script(coinbase_key: Key, coinbase_script: Script) -> Self {
        let params = ChainParams::regtest();
        let coinbase_pubkey = coinbase_key.get_pubkey();

        let coins = CoinsViewCache::new(Box::new(EmptyCoinsView));

        let mut chain = TestChain {
            params,
            blocks: Vec::new(),
            headers: Vec::new(),
            coins,
            height: -1,
            tip_hash: BlockHash::ZERO,
            coinbase_key,
            coinbase_pubkey,
            coinbase_script,
            coinbase_txns: Vec::new(),
        };

        chain.initialize_genesis();
        chain
    }

    /// Create a new `TestChain` with regtest parameters and a random key.
    ///
    /// Automatically mines the genesis block so the chain starts at height 0.
    #[cfg(feature = "native-deps")]
    pub fn new() -> Self {
        Self::new_with_key(Key::generate())
    }

    // --- Public accessors ------------------------------------------------

    /// Get the current chain height (0 after genesis).
    pub fn height(&self) -> i32 {
        self.height
    }

    /// Get the tip block hash.
    pub fn tip_hash(&self) -> &BlockHash {
        &self.tip_hash
    }

    /// Get a block by height, or `None` if out of range.
    pub fn block_at(&self, height: i32) -> Option<&Block> {
        if height < 0 {
            return None;
        }
        self.blocks.get(height as usize)
    }

    /// Get a reference to the coinbase private key.
    pub fn coinbase_key(&self) -> &Key {
        &self.coinbase_key
    }

    /// Get a reference to the coinbase public key.
    pub fn coinbase_pubkey(&self) -> &PubKey {
        &self.coinbase_pubkey
    }

    /// Get a reference to the coinbase P2PKH script.
    pub fn coinbase_script(&self) -> &Script {
        &self.coinbase_script
    }

    /// Get a reference to the chain parameters.
    pub fn params(&self) -> &ChainParams {
        &self.params
    }

    /// Get a reference to the UTXO set.
    pub fn coins(&self) -> &CoinsViewCache {
        &self.coins
    }

    /// Return the number of mature coinbase outputs that could theoretically
    /// be spent (i.e., have at least `COINBASE_MATURITY` confirmations).
    ///
    /// Note: some of these may already have been spent. Use
    /// [`get_spendable_output`](Self::get_spendable_output) to find one that
    /// is actually unspent.
    pub fn mature_coinbase_count(&self) -> usize {
        let maturity = COINBASE_MATURITY;
        if self.height < maturity {
            0
        } else {
            (self.height - maturity + 1) as usize
        }
    }

    // --- Mining -----------------------------------------------------------

    /// Mine a new block containing the given transactions (on top of the
    /// coinbase that is created automatically).
    ///
    /// Returns the newly mined block.
    pub fn mine_block(&mut self, txs: Vec<TransactionRef>) -> Block {
        let height = self.height + 1;
        let subsidy = get_block_subsidy(height, &self.params.consensus);

        // Build coinbase transaction.
        let coinbase_tx = self.create_coinbase(height, subsidy);
        let coinbase_ref: TransactionRef = Arc::new(coinbase_tx);

        let mut all_txs = vec![coinbase_ref.clone()];
        all_txs.extend(txs);

        // Compute merkle root.
        let mut mutated = false;
        let merkle_root = block_merkle_root(&all_txs, &mut mutated);

        // Build the header.
        let prev_hash = self.tip_hash;
        let time = if self.height >= 0 {
            self.headers[self.height as usize].time + 1
        } else {
            1_296_688_602 // regtest genesis timestamp
        };

        let bits = self.pow_limit_compact();

        let mut header = BlockHeader {
            version: 4,
            prev_blockhash: prev_hash,
            merkle_root,
            time,
            bits,
            nonce: 0,
        };

        // Mine (find a valid nonce).
        self.solve_header(&mut header);

        let block = Block {
            header: header.clone(),
            vtx: all_txs.clone(),
        };

        // Update the UTXO set.
        for (tx_idx, tx) in all_txs.iter().enumerate() {
            let is_coinbase = tx_idx == 0;
            add_coins(&self.coins, tx, height as u32, is_coinbase);

            // Spend inputs (skip coinbase -- it has no real inputs).
            if !is_coinbase {
                for input in &tx.vin {
                    self.coins.spend_coin(&input.prevout);
                }
            }
        }

        // Update chain state.
        self.height = height;
        self.tip_hash = block.header.block_hash();
        self.blocks.push(block.clone());
        self.headers.push(header);
        self.coinbase_txns.push(coinbase_ref);

        block
    }

    /// Mine `count` empty blocks (no user transactions).
    ///
    /// Returns the list of mined blocks.
    /// Mine a block with extra outputs in the coinbase transaction.
    ///
    /// This allows testing metaprotocol features that depend on coinbase outputs
    /// (e.g., ftrBTC creation from coinbase frBTC mints) without making
    /// qubitcoin aware of any specific metaprotocol.
    ///
    /// `coinbase_extra_outputs`: additional TxOut entries appended to the
    /// coinbase after the standard miner reward output.
    pub fn mine_block_with_coinbase_outputs(
        &mut self,
        txs: Vec<TransactionRef>,
        coinbase_extra_outputs: Vec<TxOut>,
    ) -> Block {
        let height = self.height + 1;
        let subsidy = get_block_subsidy(height, &self.params.consensus);

        let coinbase_tx = self.create_coinbase_with_extras(height, subsidy, coinbase_extra_outputs);
        let coinbase_ref: TransactionRef = Arc::new(coinbase_tx);

        let mut all_txs = vec![coinbase_ref.clone()];
        all_txs.extend(txs);

        let mut mutated = false;
        let merkle_root = block_merkle_root(&all_txs, &mut mutated);

        let prev_hash = self.tip_hash;
        let time = if self.height >= 0 {
            self.headers[self.height as usize].time + 1
        } else {
            1_296_688_602
        };

        let bits = self.pow_limit_compact();

        let mut header = BlockHeader {
            version: 4,
            prev_blockhash: prev_hash,
            merkle_root,
            time,
            bits,
            nonce: 0,
        };

        self.solve_header(&mut header);

        let block = Block {
            header: header.clone(),
            vtx: all_txs.clone(),
        };

        for (tx_idx, tx) in all_txs.iter().enumerate() {
            let is_coinbase = tx_idx == 0;
            add_coins(&self.coins, tx, height as u32, is_coinbase);
            if !is_coinbase {
                for input in &tx.vin {
                    self.coins.spend_coin(&input.prevout);
                }
            }
        }

        self.height = height;
        self.tip_hash = block.header.block_hash();
        self.blocks.push(block.clone());
        self.headers.push(header);
        self.coinbase_txns.push(coinbase_ref);

        block
    }

    pub fn mine_empty_blocks(&mut self, count: usize) -> Vec<Block> {
        (0..count).map(|_| self.mine_block(vec![])).collect()
    }

    // --- Transaction helpers -----------------------------------------------

    /// Create a simple transaction that spends a single UTXO at `input` and
    /// creates one or two outputs.
    ///
    /// * `value` -- amount to send to `dest_script`.
    /// * Remaining funds (minus a 1000-sat fee) are returned to the coinbase
    ///   address as change.
    ///
    /// Returns `None` if the input coin does not exist or has insufficient
    /// funds.
    ///
    /// **Note:** The transaction is not signed. For test purposes this is
    /// usually acceptable because the framework does not enforce script
    /// validation.
    pub fn create_transaction(
        &self,
        input: &OutPoint,
        value: Amount,
        dest_script: &Script,
    ) -> Option<TransactionRef> {
        let coin = self.coins.get_coin(input)?;

        let fee = Amount::from_sat(1000);
        let input_value = coin.tx_out.value;

        // Ensure input covers value + fee.
        if input_value.to_sat() < (value + fee).to_sat() {
            return None;
        }

        let change = input_value - value - fee;

        let tx_in = TxIn {
            prevout: input.clone(),
            script_sig: Script::new(),
            sequence: SEQUENCE_FINAL,
            witness: Witness::new(),
        };

        let mut vout = vec![TxOut::new(value, dest_script.clone())];

        if change > Amount::ZERO {
            vout.push(TxOut::new(change, self.coinbase_script.clone()));
        }

        let tx = Transaction::new(2, vec![tx_in], vout, 0);
        Some(Arc::new(tx))
    }

    /// Find the first spendable (mature and unspent) coinbase output.
    ///
    /// Returns the outpoint and its value, or `None` if none are available.
    pub fn get_spendable_output(&self) -> Option<(OutPoint, Amount)> {
        let maturity = COINBASE_MATURITY;
        if self.height < maturity {
            return None;
        }

        // The latest height whose coinbase is mature.
        let max_mature_height = (self.height - maturity) as usize;

        for i in 0..=max_mature_height {
            let tx = &self.coinbase_txns[i];
            let outpoint = OutPoint::new(*tx.txid(), 0);
            if self.coins.have_coin(&outpoint) {
                return Some((outpoint, tx.vout[0].value));
            }
        }
        None
    }

    /// Return all unspent transaction outputs whose scriptPubKey matches `script`.
    ///
    /// Iterates all blocks, all transactions, all outputs, and checks the
    /// UTXO set. Suitable for devnet where the chain is small.
    pub fn utxos_for_script(&self, script: &Script) -> Vec<(OutPoint, Amount, i32)> {
        let mut result = Vec::new();
        for (h, block) in self.blocks.iter().enumerate() {
            let height = h as i32;
            for (tx_idx, tx) in block.vtx.iter().enumerate() {
                for (vout_idx, txout) in tx.vout.iter().enumerate() {
                    if txout.script_pubkey == *script {
                        let outpoint = OutPoint::new(*tx.txid(), vout_idx as u32);
                        if self.coins.have_coin(&outpoint) {
                            // For coinbase outputs, check maturity
                            if tx_idx == 0 {
                                // Coinbase: must have COINBASE_MATURITY confirmations
                                let depth = self.height - height;
                                if depth < COINBASE_MATURITY {
                                    continue;
                                }
                            }
                            result.push((outpoint, txout.value, height));
                        }
                    }
                }
            }
        }
        result
    }

    // --- Internal helpers --------------------------------------------------

    /// Mine the genesis block and add it to the chain.
    fn initialize_genesis(&mut self) {
        let genesis = self.build_genesis_block();

        // Add coinbase outputs to the UTXO set.
        let coinbase_ref = genesis.vtx[0].clone();
        add_coins(&self.coins, &coinbase_ref, 0, true);

        self.height = 0;
        self.tip_hash = genesis.header.block_hash();
        self.headers.push(genesis.header.clone());
        self.coinbase_txns.push(coinbase_ref);
        self.blocks.push(genesis);
    }

    /// Construct a custom genesis block for the test chain.
    fn build_genesis_block(&self) -> Block {
        let subsidy = Amount::from_sat(50 * SAT_PER_COIN);
        let coinbase_tx = self.create_coinbase(0, subsidy);
        let coinbase_ref: TransactionRef = Arc::new(coinbase_tx);

        let mut mutated = false;
        let merkle_root = block_merkle_root(&[coinbase_ref.clone()], &mut mutated);

        let bits = self.pow_limit_compact();

        let mut header = BlockHeader {
            version: 1,
            prev_blockhash: BlockHash::ZERO,
            merkle_root,
            time: 1_296_688_602, // regtest genesis timestamp
            bits,
            nonce: 0,
        };

        self.solve_header(&mut header);

        Block {
            header,
            vtx: vec![coinbase_ref],
        }
    }

    /// Build a coinbase transaction for the given `height` and `subsidy`.
    fn create_coinbase(&self, height: i32, subsidy: Amount) -> Transaction {
        self.create_coinbase_with_extras(height, subsidy, vec![])
    }

    /// Create a coinbase transaction with optional extra outputs.
    ///
    /// Extra outputs are appended after the standard miner reward output.
    /// This is metaprotocol-agnostic — callers can include any outputs
    /// (e.g., OP_RETURN with protostones, P2TR outputs for token creation).
    /// The subsidy goes to the first output (miner), extras get 0 value
    /// unless they already have a value set.
    fn create_coinbase_with_extras(
        &self,
        height: i32,
        subsidy: Amount,
        extra_outputs: Vec<TxOut>,
    ) -> Transaction {
        // BIP34: encode the block height in the coinbase scriptSig.
        let mut sig_script = Script::new();
        sig_script.push_int(height as i64);
        // Pad to ensure coinbase scriptSig is at least 2 bytes (consensus rule).
        if sig_script.len() < 2 {
            sig_script.push_int(0);
        }

        let tx_in = TxIn {
            prevout: OutPoint::null(),
            script_sig: sig_script,
            sequence: SEQUENCE_FINAL,
            witness: Witness::new(),
        };

        let mut outputs = vec![TxOut::new(subsidy, self.coinbase_script.clone())];
        outputs.extend(extra_outputs);

        Transaction::new(2, vec![tx_in], outputs, 0)
    }

    /// Get the compact (nBits) representation of the regtest PoW limit.
    fn pow_limit_compact(&self) -> u32 {
        let arith = uint256_to_arith(&self.params.consensus.pow_limit);
        arith.get_compact(false)
    }

    /// Increment `header.nonce` until the block hash satisfies the PoW target.
    ///
    /// For regtest the target is extremely permissive (0x7fff...); the first
    /// nonce almost always works.
    fn solve_header(&self, header: &mut BlockHeader) {
        let mut target = ArithUint256::zero();
        target.set_compact(header.bits);

        loop {
            let hash = header.block_hash();
            let hash_arith = uint256_to_arith(&hash.into_uint256());
            if hash_arith <= target {
                return;
            }
            header.nonce = header.nonce.wrapping_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- 1. Genesis -------------------------------------------------------

    #[test]
    fn test_new_creates_genesis() {
        let chain = TestChain::new();

        assert_eq!(
            chain.height(),
            0,
            "Chain should start at height 0 after genesis"
        );
        assert_ne!(
            *chain.tip_hash(),
            BlockHash::ZERO,
            "Tip hash should not be zero"
        );

        let genesis = chain.block_at(0).expect("Genesis block must exist");
        assert_eq!(
            genesis.vtx.len(),
            1,
            "Genesis block should have exactly one tx (coinbase)"
        );
        assert!(
            genesis.vtx[0].is_coinbase(),
            "The sole transaction in genesis should be a coinbase"
        );
    }

    // -- 2. Mine empty blocks ---------------------------------------------

    #[test]
    fn test_mine_empty_blocks() {
        let mut chain = TestChain::new();
        let blocks = chain.mine_empty_blocks(10);

        assert_eq!(blocks.len(), 10);
        assert_eq!(chain.height(), 10);

        // Each block should have exactly one transaction (the coinbase).
        for blk in &blocks {
            assert_eq!(blk.vtx.len(), 1);
        }

        // Headers should chain together.
        for h in 1..=10 {
            let blk = chain.block_at(h).unwrap();
            let prev = chain.block_at(h - 1).unwrap();
            assert_eq!(
                blk.header.prev_blockhash,
                prev.header.block_hash(),
                "Block at height {} should reference block at height {}",
                h,
                h - 1,
            );
        }
    }

    // -- 3. Coinbase maturity ---------------------------------------------

    #[test]
    fn test_mine_101_blocks_mature_coinbase() {
        let mut chain = TestChain::new();

        // At height 0 (genesis only), no mature coinbase.
        assert_eq!(chain.mature_coinbase_count(), 0);
        assert!(chain.get_spendable_output().is_none());

        // Mine 100 more blocks to reach height 100.
        chain.mine_empty_blocks(100);
        assert_eq!(chain.height(), 100);

        // COINBASE_MATURITY is 100, so the genesis coinbase (height 0) is now
        // exactly mature (100 confirmations).
        assert_eq!(chain.mature_coinbase_count(), 1);
        let (outpoint, value) = chain
            .get_spendable_output()
            .expect("Should have one mature coinbase");

        // The genesis coinbase pays 50 BTC.
        assert_eq!(value, Amount::from_sat(50 * SAT_PER_COIN));

        // The outpoint should be output 0 of the genesis coinbase tx.
        let genesis = chain.block_at(0).unwrap();
        assert_eq!(outpoint.hash, *genesis.vtx[0].txid());
        assert_eq!(outpoint.n, 0);
    }

    // -- 4. Create and mine a transaction ---------------------------------

    #[test]
    fn test_create_and_mine_transaction() {
        let mut chain = TestChain::new();

        // Mine 100 blocks so the genesis coinbase matures.
        chain.mine_empty_blocks(100);
        assert_eq!(chain.height(), 100);

        let (outpoint, value) = chain
            .get_spendable_output()
            .expect("Mature coinbase should be available");

        // Create a transaction sending 10 BTC to a fresh script.
        let dest_script = Script::from_bytes(vec![0x51]); // OP_1 (anyone can spend)
        let send_amount = Amount::from_sat(10 * SAT_PER_COIN);
        let tx = chain
            .create_transaction(&outpoint, send_amount, &dest_script)
            .expect("Transaction creation should succeed");

        // Verify the transaction structure.
        assert_eq!(tx.vin.len(), 1);
        assert_eq!(tx.vin[0].prevout, outpoint);
        assert_eq!(tx.vout[0].value, send_amount);
        assert_eq!(tx.vout[0].script_pubkey, dest_script);

        // Change output: value - send_amount - 1000 sat fee
        let expected_change = value - send_amount - Amount::from_sat(1000);
        assert!(tx.vout.len() >= 2, "Should have a change output");
        assert_eq!(tx.vout[1].value, expected_change);

        // Mine the transaction.
        let block = chain.mine_block(vec![Arc::clone(&tx)]);
        assert_eq!(chain.height(), 101);
        assert_eq!(block.vtx.len(), 2, "Block should contain coinbase + our tx");

        // The spent outpoint should no longer be in the UTXO set.
        assert!(
            !chain.coins().have_coin(&outpoint),
            "Spent outpoint should be removed from the UTXO set"
        );

        // The new outputs should be in the UTXO set.
        let new_outpoint_0 = OutPoint::new(*tx.txid(), 0);
        let new_outpoint_1 = OutPoint::new(*tx.txid(), 1);
        assert!(
            chain.coins().have_coin(&new_outpoint_0),
            "Destination output should exist in UTXO set"
        );
        assert!(
            chain.coins().have_coin(&new_outpoint_1),
            "Change output should exist in UTXO set"
        );

        // Verify the coin values.
        let dest_coin = chain.coins().get_coin(&new_outpoint_0).unwrap();
        assert_eq!(dest_coin.tx_out.value, send_amount);

        let change_coin = chain.coins().get_coin(&new_outpoint_1).unwrap();
        assert_eq!(change_coin.tx_out.value, expected_change);
    }

    // -- 5. Block subsidy -------------------------------------------------

    #[test]
    fn test_block_subsidy() {
        let chain = TestChain::new();

        // Genesis block should have exactly 50 BTC subsidy.
        let genesis = chain.block_at(0).expect("Genesis block must exist");
        let coinbase_value = genesis.vtx[0].vout[0].value;
        assert_eq!(
            coinbase_value,
            Amount::from_sat(50 * SAT_PER_COIN),
            "Genesis coinbase should be 50 BTC"
        );

        // Verify get_block_subsidy directly for regtest parameters.
        let params = &chain.params().consensus;
        assert_eq!(get_block_subsidy(0, params).to_sat(), 50 * SAT_PER_COIN);
        assert_eq!(get_block_subsidy(149, params).to_sat(), 50 * SAT_PER_COIN);
        // Regtest halving at height 150.
        assert_eq!(get_block_subsidy(150, params).to_sat(), 25 * SAT_PER_COIN);
        assert_eq!(
            get_block_subsidy(300, params).to_sat(),
            (125 * SAT_PER_COIN) / 10
        );
    }

    // -- Additional tests for robustness -----------------------------------

    #[test]
    fn test_blocks_have_valid_pow() {
        let mut chain = TestChain::new();
        chain.mine_empty_blocks(5);

        let params = &chain.params().consensus;
        for h in 0..=chain.height() {
            let blk = chain.block_at(h).unwrap();
            let hash = blk.header.block_hash();
            assert!(
                qubitcoin_consensus::check::check_proof_of_work(
                    &hash.into_uint256(),
                    blk.header.bits,
                    params,
                ),
                "Block at height {} should have valid PoW",
                h,
            );
        }
    }

    #[test]
    fn test_block_hash_uniqueness() {
        let mut chain = TestChain::new();
        chain.mine_empty_blocks(20);

        let mut hashes = std::collections::HashSet::new();
        for h in 0..=chain.height() {
            let blk = chain.block_at(h).unwrap();
            let hash = blk.header.block_hash();
            assert!(
                hashes.insert(hash),
                "Block hash at height {} should be unique",
                h,
            );
        }
    }

    #[test]
    fn test_create_transaction_insufficient_funds() {
        let mut chain = TestChain::new();
        chain.mine_empty_blocks(100);

        let (outpoint, _value) = chain.get_spendable_output().unwrap();

        // Try to spend more than the coin holds.
        let dest = Script::from_bytes(vec![0x51]);
        let too_much = Amount::from_sat(51 * SAT_PER_COIN);
        let result = chain.create_transaction(&outpoint, too_much, &dest);
        assert!(result.is_none(), "Should fail when value exceeds input");
    }

    #[test]
    fn test_spend_multiple_coinbases() {
        let mut chain = TestChain::new();
        // Mine enough blocks so multiple coinbases are mature.
        chain.mine_empty_blocks(104);
        assert_eq!(chain.height(), 104);

        // We should have 5 mature coinbases (heights 0..=4).
        assert_eq!(chain.mature_coinbase_count(), 5);

        let dest = Script::from_bytes(vec![0x51]);
        let send = Amount::from_sat(1 * SAT_PER_COIN);

        // Spend them one by one.
        for _ in 0..5 {
            let (outpoint, _value) = chain
                .get_spendable_output()
                .expect("Should have a spendable output");
            let tx = chain
                .create_transaction(&outpoint, send, &dest)
                .expect("Transaction creation should succeed");
            chain.mine_block(vec![tx]);
        }

        // All 5 original mature coinbases should be spent.
        // But mining 5 more blocks doesn't create new mature coinbases
        // beyond the original set (heights 5..9 are not yet mature).
        // Heights 5..9 need 100 more confirmations.
        // Current height: 104 + 5 = 109. Height 5 coinbase needs height 105.
        // So height 5 coinbase at height 109 has 104 confirmations -- mature!
        // Actually mature count = 109 - 100 + 1 = 10, but we spent 5.
        // get_spendable_output skips spent ones.
        let next = chain.get_spendable_output();
        assert!(
            next.is_some(),
            "Newly matured coinbases should be available"
        );
    }
}
