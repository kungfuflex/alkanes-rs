//! Bridge between alkanes-cli-common's MockProvider and the TestRuntime.
//!
//! This module enables end-to-end testing of the CLI transaction building
//! pipeline: MockProvider + EnhancedAlkanesExecutor build a real transaction,
//! which is then fed through the wasmtime indexer for verification.

use alkanes_cli_common::mock_provider::MockProvider;
use alkanes_cli_common::alkanes::execute::EnhancedAlkanesExecutor;
use alkanes_cli_common::alkanes::types::EnhancedExecuteParams;
use anyhow::{anyhow, Context, Result};
use bitcoin::{consensus, Block, Network, OutPoint, Transaction, TxOut};

use crate::runtime::TestRuntime;

/// Alkane balance info for pre-populating MockProvider.
pub struct AlkaneBalance {
    pub outpoint: OutPoint,
    pub block: u64,
    pub tx: u64,
    pub amount: u64,
}

/// Bridge for executing CLI commands and indexing the results.
pub struct CliBridge {
    pub provider: MockProvider,
}

impl CliBridge {
    /// Create a new bridge with a regtest MockProvider.
    pub fn new() -> Self {
        Self {
            provider: MockProvider::new(Network::Regtest),
        }
    }

    /// Add a single UTXO to the mock provider's wallet.
    pub fn add_utxo(&self, outpoint: OutPoint, txout: TxOut) {
        self.provider
            .utxos
            .lock()
            .unwrap()
            .push((outpoint, txout));
    }

    /// Populate the mock provider with all spendable UTXOs from a block.
    pub fn add_utxos_from_block(&self, block: &Block) {
        let mut utxos = self.provider.utxos.lock().unwrap();
        for tx in &block.txdata {
            let txid = tx.compute_txid();
            for (vout, txout) in tx.output.iter().enumerate() {
                if txout.value.to_sat() > 0 && !txout.script_pubkey.is_op_return() {
                    utxos.push((OutPoint::new(txid, vout as u32), txout.clone()));
                }
            }
        }
    }

    /// Tell the mock provider that an outpoint carries alkane tokens.
    pub fn set_alkane_balance(&self, outpoint: &OutPoint, block: u64, tx: u64, amount: u64) {
        let key = format!("{}:{}", outpoint.txid, outpoint.vout);
        let mut balances = self.provider.alkane_balances.lock().unwrap();
        balances
            .entry(key)
            .or_default()
            .push((block, tx, amount));
    }

    /// Get the mock provider's taproot address.
    pub fn address(&self) -> String {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            use alkanes_cli_common::traits::WalletProvider;
            self.provider.get_address().await.unwrap()
        })
    }

    /// Execute a CLI command and extract the raw signed transaction.
    ///
    /// This runs the full `execute_full` pipeline through MockProvider:
    /// - UTXO selection
    /// - Protostone construction (including auto-change if needed)
    /// - PSBT creation and signing
    /// - Transaction broadcasting (stored in MockProvider.broadcasted_txs)
    pub fn execute_and_extract_tx(
        &mut self,
        params: EnhancedExecuteParams,
    ) -> Result<Transaction> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let result = rt.block_on(async {
            let mut executor = EnhancedAlkanesExecutor::new(&mut self.provider);
            executor.execute_full(params).await
        }).context("execute_full failed")?;

        // Extract the broadcast transaction from MockProvider
        let txid = &result.reveal_txid;
        let broadcasted = self.provider.broadcasted_txs.lock().unwrap();
        let tx_hex = broadcasted
            .get(txid)
            .ok_or_else(|| anyhow!("Transaction {} not in broadcasted_txs", txid))?;
        let tx_bytes = hex::decode(tx_hex)
            .context("failed to decode broadcasted tx hex")?;
        let tx: Transaction = consensus::deserialize(&tx_bytes)
            .context("failed to deserialize broadcasted tx")?;
        Ok(tx)
    }

    /// Full pipeline: execute CLI → extract tx → wrap in block → index.
    pub fn execute_and_index(
        &mut self,
        params: EnhancedExecuteParams,
        runtime: &TestRuntime,
        height: u32,
    ) -> Result<Block> {
        let tx = self.execute_and_extract_tx(params)?;
        let mut block = protorune::test_helpers::create_block_with_coinbase_tx(height);
        block.txdata.push(tx);
        runtime.index_block(&block, height)?;
        Ok(block)
    }

    /// Execute a commit/reveal deployment and index both transactions.
    ///
    /// Returns (commit_block, reveal_block).
    pub fn execute_deploy_and_index(
        &mut self,
        params: EnhancedExecuteParams,
        runtime: &TestRuntime,
        commit_height: u32,
    ) -> Result<(Block, Block)> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let result = rt.block_on(async {
            let mut executor = EnhancedAlkanesExecutor::new(&mut self.provider);
            executor.execute_full(params).await
        }).context("execute_full (deploy) failed")?;

        let broadcasted = self.provider.broadcasted_txs.lock().unwrap();

        // Extract commit tx
        let commit_txid = result
            .commit_txid
            .as_ref()
            .ok_or_else(|| anyhow!("no commit_txid in deploy result"))?;
        let commit_hex = broadcasted
            .get(commit_txid)
            .ok_or_else(|| anyhow!("commit tx {} not in broadcasted_txs", commit_txid))?;
        let commit_tx: Transaction =
            consensus::deserialize(&hex::decode(commit_hex)?)?;

        // Extract reveal tx
        let reveal_hex = broadcasted
            .get(&result.reveal_txid)
            .ok_or_else(|| anyhow!("reveal tx {} not in broadcasted_txs", result.reveal_txid))?;
        let reveal_tx: Transaction =
            consensus::deserialize(&hex::decode(reveal_hex)?)?;

        drop(broadcasted);

        // Index commit block
        let mut commit_block = protorune::test_helpers::create_block_with_coinbase_tx(commit_height);
        commit_block.txdata.push(commit_tx);
        runtime.index_block(&commit_block, commit_height)?;

        // Index reveal block
        let reveal_height = commit_height + 1;
        let mut reveal_block =
            protorune::test_helpers::create_block_with_coinbase_tx(reveal_height);
        reveal_block.txdata.push(reveal_tx);
        runtime.index_block(&reveal_block, reveal_height)?;

        Ok((commit_block, reveal_block))
    }
}
