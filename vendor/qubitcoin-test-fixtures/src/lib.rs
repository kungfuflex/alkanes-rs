//! In-process TestChain + reorg harness primitives.
//!
//! Test code across `subfrost-mobile`, `alkanes-rs`, and any other
//! project that builds on qubitcoin shares this surface so a
//! reorg-safety invariant can be exercised inside `cargo test`
//! without spinning up a bitcoind+indexer container pair.
//!
//! The two primary types are:
//!
//! * [`TestChain`] — owns a stack of mined blocks (each a `Vec<MockTx>`)
//!   plus a UTXO set keyed by `OutPoint`. Supports linear mining,
//!   in-place invalidation (`reorg`), and competing-chain mining
//!   (`mine_alternate`) without touching disk.
//! * [`UtxoSnapshot`] — the iterator type returned by
//!   [`TestChain::candidate_utxos`]. Each item is a `Utxo` struct
//!   that downstream harnesses (e.g. multisend planner unit tests)
//!   convert into their domain shapes via a small adapter.
//!
//! ## Reorg model
//!
//! Bitcoin Core's `invalidateblock` semantics drop a block + every
//! descendant from the active chain and return their non-coinbase
//! txs to the mempool. [`TestChain::reorg(n)`] mirrors that:
//!
//! 1. Pop the last `n` blocks from the active stack.
//! 2. For each popped block, restore its inputs to spendable UTXOs
//!    and demote its outputs to mempool.
//! 3. The mempool now carries the rolled-back txs; callers test
//!    against the new active tip + the mempool overflow.
//!
//! Then [`TestChain::mine_alternate(txs)`] mines a competing chain
//! starting at the new tip, optionally including a subset of the
//! mempool — the standard re-attach flow.

#![allow(dead_code)]

// We use bitcoin-rs's OutPoint/Txid here instead of qubitcoin-primitives'
// — qubitcoin-primitives only re-exports the hash types (Txid /
// Wtxid / BlockHash) and OutPoint lives downstream. bitcoin::OutPoint
// is the lingua franca for downstream harnesses (subfrost-mobile-
// integ-tests, alkanes-rs tests) which all already pull bitcoin-rs.
use bitcoin::{OutPoint, Txid};
use std::collections::HashMap;

/// A minimal transaction shape sufficient for chain-state simulation.
/// Real production txs carry witness data + script_sigs; the test
/// harness omits both since it never validates signatures (the goal
/// is to exercise planner / UTXO-selection invariants, not consensus).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MockTx {
    pub txid:    Txid,
    /// Outpoints consumed by this tx — removed from the UTXO set on
    /// inclusion, restored on reorg.
    pub inputs:  Vec<OutPoint>,
    /// One `Utxo` per output. `outpoint.txid == self.txid` for every
    /// entry; `outpoint.vout` is the index into `outputs`.
    pub outputs: Vec<Utxo>,
}

/// One UTXO in the test chain. The `confirmations` field is
/// recomputed on each `mine()` / `reorg()` call so harness code can
/// snapshot the chain mid-test and read realistic confirmation
/// counts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Utxo {
    pub outpoint:      OutPoint,
    pub value_sats:    u64,
    /// Set on mining; updated on reorg-rewind to track how many
    /// blocks of confirmation each UTXO has at the current tip.
    pub confirmations: u32,
    /// True when the UTXO is still in the mempool / unconfirmed
    /// (post-reorg-rewind, freshly-broadcast).
    pub mempool:       bool,
    /// Owner address — opaque string keyed by what the caller
    /// supplies in `mine()`. The fixture doesn't parse it; that's
    /// the harness adapter's job.
    pub address:       String,
    /// Opaque "asset balance" metadata (e.g. alkane balances) the
    /// harness wants to thread through. The fixture treats this as
    /// a black box — it's preserved across reorgs and exposed via
    /// [`TestChain::candidate_utxos`].
    pub aux:           Vec<(String, u128)>,
}

/// A mined block bundle the chain stack carries between mines and
/// reorgs. We snapshot the consumed inputs at mine-time so reorg
/// can restore them losslessly (with their original address +
/// value + aux metadata).
#[derive(Clone, Debug)]
struct MinedBlock {
    txs:                Vec<MockTx>,
    /// One entry per consumed outpoint, in input-order. Stored
    /// alongside the block so reorg() can restore the exact UTXO
    /// the input pointed at (instead of synthesising a stub with
    /// empty fields).
    consumed_snapshots: Vec<Utxo>,
}

/// In-process chain stack. Each `MinedBlock` is one mined block at
/// the corresponding index; index 0 is the genesis (typically empty
/// or a single coinbase). The mempool holds txs that were either
/// freshly broadcast or rolled back by a reorg.
#[derive(Debug, Default)]
pub struct TestChain {
    blocks:  Vec<MinedBlock>,
    mempool: Vec<MockTx>,
    /// Live UTXO set, keyed by outpoint. Mirrors the spendable
    /// outputs of every block + mempool tx, less their consumed
    /// inputs. Recomputed on reorg.
    utxos:   HashMap<OutPoint, Utxo>,
}

impl TestChain {
    /// Empty chain — height 0, no UTXOs, no mempool.
    pub fn new() -> Self { Self::default() }

    /// Mine a single block with the given txs. Each tx's outputs
    /// land in the UTXO set with `confirmations = 1`; previously
    /// mined UTXOs increment by 1. Consumed inputs are removed AND
    /// snapshotted onto the block so reorg() can restore them
    /// losslessly (with their original address + value + aux).
    pub fn mine(&mut self, txs: Vec<MockTx>) -> Result<(), TestChainError> {
        // Verify + snapshot consumed inputs in input-order, then
        // remove them. Verifying first means we can fail cleanly
        // without leaving the UTXO set in a half-mutated state.
        let mut consumed: Vec<Utxo> = Vec::new();
        for tx in &txs {
            for inp in &tx.inputs {
                let snap = self.utxos.get(inp)
                    .ok_or_else(|| TestChainError::UnknownOutpoint(format!("{:?}", inp)))?
                    .clone();
                consumed.push(snap);
            }
        }
        for tx in &txs {
            for inp in &tx.inputs {
                self.utxos.remove(inp);
            }
        }
        // Bump confirmations on every existing UTXO.
        for u in self.utxos.values_mut() { u.confirmations += 1; }
        // Add new outputs at conf=1.
        for tx in &txs {
            for out in &tx.outputs {
                let mut u = out.clone();
                u.confirmations = 1;
                u.mempool       = false;
                self.utxos.insert(u.outpoint, u);
            }
        }
        // Drop any mempool entries whose txids are now mined.
        let confirmed: Vec<Txid> = txs.iter().map(|t| t.txid).collect();
        self.mempool.retain(|t| !confirmed.contains(&t.txid));
        self.blocks.push(MinedBlock { txs, consumed_snapshots: consumed });
        Ok(())
    }

    /// Convenience: mine `n` empty blocks. Bumps every existing
    /// UTXO's `confirmations` by `n` without changing the UTXO set.
    pub fn mine_empty_blocks(&mut self, n: usize) -> Result<(), TestChainError> {
        for _ in 0..n {
            self.mine(Vec::new())?;
        }
        Ok(())
    }

    /// Broadcast a tx to the mempool without mining it. Useful when
    /// a harness wants to exercise the "uses-pending-utxo" planner
    /// path — the tx's outputs are visible with `mempool: true,
    /// confirmations: 0`.
    pub fn broadcast(&mut self, tx: MockTx) -> Result<(), TestChainError> {
        for inp in &tx.inputs {
            if !self.utxos.contains_key(inp) && !self.is_in_mempool(inp) {
                return Err(TestChainError::UnknownOutpoint(format!("{:?}", inp)));
            }
        }
        for out in &tx.outputs {
            let mut u = out.clone();
            u.confirmations = 0;
            u.mempool       = true;
            self.utxos.insert(u.outpoint.clone(), u);
        }
        self.mempool.push(tx);
        Ok(())
    }

    /// Mirror of `bitcoin-cli invalidateblock` for the last `n`
    /// blocks. Pops them off the active chain, returns their
    /// non-coinbase txs to the mempool, and reduces every surviving
    /// UTXO's `confirmations` by `n` (saturating at 0).
    pub fn reorg(&mut self, n: usize) -> Result<(), TestChainError> {
        if n > self.blocks.len() {
            return Err(TestChainError::ReorgDepthExceedsChain {
                requested: n, have: self.blocks.len(),
            });
        }
        for _ in 0..n {
            let popped = self.blocks.pop().unwrap();
            // Remove this block's outputs from the UTXO set.
            for tx in &popped.txs {
                for out in &tx.outputs {
                    self.utxos.remove(&out.outpoint);
                }
            }
            // Restore consumed inputs from the per-block snapshot.
            // We saved each consumed UTXO at mine-time, so the
            // restored entries carry their original address +
            // value + aux metadata — no loss across reorg.
            for snap in &popped.consumed_snapshots {
                self.utxos.insert(snap.outpoint, snap.clone());
            }
            // Return the popped txs to the mempool. Coinbase txs
            // (no inputs) can't be remixed into another chain so
            // they're dropped — BUT their outputs ARE restored as
            // mempool UTXOs so harness code can observe the
            // "rolled-back coinbase value sits in the mempool"
            // semantic (mirrors real-world behaviour where the
            // wallet's confirmed coinbase rolls back to unconfirmed).
            for tx in popped.txs.into_iter() {
                if tx.inputs.is_empty() {
                    for out in &tx.outputs {
                        let mut u = out.clone();
                        u.confirmations = 0;
                        u.mempool       = true;
                        self.utxos.insert(u.outpoint, u);
                    }
                } else {
                    self.mempool.push(tx);
                }
            }
        }
        // Reduce confirmations on everyone else.
        for u in self.utxos.values_mut() {
            u.confirmations = u.confirmations.saturating_sub(n as u32);
            if u.confirmations == 0 { u.mempool = true; }
        }
        Ok(())
    }

    /// Mine a competing chain starting at the current tip. Equivalent
    /// to a fresh `mine(txs)` call but semantically tagged so harness
    /// code can assert it's the alternate-tip branch of a reorg
    /// scenario.
    pub fn mine_alternate(&mut self, txs: Vec<MockTx>) -> Result<(), TestChainError> {
        self.mine(txs)
    }

    /// Current tip height (= number of mined blocks, 0 for a
    /// just-constructed TestChain).
    pub fn height(&self) -> u32 { self.blocks.len() as u32 }

    /// Iterator over every spendable UTXO at the given address. The
    /// caller's harness adapter converts these into its domain
    /// shape (e.g. `multisend::CandidateUtxo`).
    pub fn candidate_utxos<'a>(&'a self, address: &'a str) -> UtxoSnapshot<'a> {
        UtxoSnapshot {
            inner: self.utxos.values(),
            filter: address,
        }
    }

    /// Mempool snapshot — txs that were either freshly broadcast or
    /// rolled back by the most recent reorg.
    pub fn mempool(&self) -> &[MockTx] { &self.mempool }

    fn is_in_mempool(&self, outpoint: &OutPoint) -> bool {
        self.mempool.iter().any(|tx|
            tx.outputs.iter().any(|o| &o.outpoint == outpoint))
    }
}

/// Borrowed iterator over a [`TestChain`]'s UTXO set filtered by
/// address. Lazy so the harness can `.take(n)` without materialising
/// the whole snapshot.
pub struct UtxoSnapshot<'a> {
    inner:  std::collections::hash_map::Values<'a, OutPoint, Utxo>,
    filter: &'a str,
}

impl<'a> Iterator for UtxoSnapshot<'a> {
    type Item = Utxo;
    fn next(&mut self) -> Option<Utxo> {
        for u in self.inner.by_ref() {
            if u.address == self.filter { return Some(u.clone()); }
        }
        None
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TestChainError {
    #[error("outpoint not in UTXO set: {0}")]
    UnknownOutpoint(String),
    #[error("reorg depth {requested} exceeds chain height {have}")]
    ReorgDepthExceedsChain { requested: usize, have: usize },
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::{OutPoint, Txid};
    use bitcoin::hashes::Hash;

    fn mk_txid(byte: u8) -> Txid {
        let mut b = [0u8; 32];
        b[0] = byte;
        Txid::from_byte_array(b)
    }
    fn mk_utxo(txid: Txid, vout: u32, sats: u64, addr: &str) -> Utxo {
        Utxo {
            outpoint:      OutPoint { txid, vout },
            value_sats:    sats,
            confirmations: 0,
            mempool:       false,
            address:       addr.into(),
            aux:           Vec::new(),
        }
    }

    #[test]
    fn empty_chain_has_no_utxos() {
        let chain = TestChain::new();
        assert_eq!(chain.height(), 0);
        assert_eq!(chain.candidate_utxos("alice").count(), 0);
    }

    #[test]
    fn mining_grows_chain_and_creates_utxos() {
        let mut chain = TestChain::new();
        let txid = mk_txid(1);
        chain.mine(vec![MockTx {
            txid: txid.clone(),
            inputs: vec![],
            outputs: vec![mk_utxo(txid.clone(), 0, 100_000, "alice")],
        }]).unwrap();
        assert_eq!(chain.height(), 1);
        let utxos: Vec<_> = chain.candidate_utxos("alice").collect();
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].confirmations, 1);
        assert_eq!(utxos[0].value_sats, 100_000);
    }

    #[test]
    fn empty_blocks_bump_confirmations_without_changing_set() {
        let mut chain = TestChain::new();
        let txid = mk_txid(2);
        chain.mine(vec![MockTx {
            txid: txid.clone(), inputs: vec![],
            outputs: vec![mk_utxo(txid.clone(), 0, 50_000, "bob")],
        }]).unwrap();
        chain.mine_empty_blocks(10).unwrap();
        let utxos: Vec<_> = chain.candidate_utxos("bob").collect();
        assert_eq!(utxos[0].confirmations, 11);
        assert_eq!(utxos[0].value_sats, 50_000);
    }

    #[test]
    fn reorg_rolls_back_blocks_and_decreases_confirmations() {
        let mut chain = TestChain::new();
        let txid1 = mk_txid(3);
        chain.mine(vec![MockTx {
            txid: txid1.clone(), inputs: vec![],
            outputs: vec![mk_utxo(txid1.clone(), 0, 70_000, "carol")],
        }]).unwrap();
        chain.mine_empty_blocks(5).unwrap();
        // Pre-reorg: carol's UTXO has 6 confirmations.
        let pre = chain.candidate_utxos("carol").next().unwrap();
        assert_eq!(pre.confirmations, 6);
        // Reorg-rewind the last 3 blocks.
        chain.reorg(3).unwrap();
        let post = chain.candidate_utxos("carol").next().unwrap();
        assert_eq!(post.confirmations, 3);
        assert_eq!(chain.height(), 3);
    }

    #[test]
    fn reorg_returns_txs_to_mempool() {
        let mut chain = TestChain::new();
        let g_txid = mk_txid(4);
        chain.mine(vec![MockTx {
            txid: g_txid.clone(), inputs: vec![],
            outputs: vec![mk_utxo(g_txid.clone(), 0, 200_000, "dave")],
        }]).unwrap();
        let spend_txid = mk_txid(5);
        chain.mine(vec![
            // Synthetic coinbase to keep skip(1) honest.
            MockTx { txid: mk_txid(255), inputs: vec![],
                outputs: vec![mk_utxo(mk_txid(255), 0, 1, "miner")] },
            MockTx {
                txid: spend_txid.clone(),
                inputs: vec![OutPoint { txid: g_txid.clone(), vout: 0 }],
                outputs: vec![mk_utxo(spend_txid.clone(), 0, 199_000, "dave")],
            },
        ]).unwrap();
        assert_eq!(chain.height(), 2);
        // Reorg-rewind block 2.
        chain.reorg(1).unwrap();
        // The spending tx is now in the mempool.
        assert!(chain.mempool().iter().any(|t| t.txid == spend_txid));
        // The original UTXO is back (restored from input).
        assert!(chain.candidate_utxos("dave").next().is_some());
    }

    #[test]
    fn reorg_deeper_than_chain_errs() {
        let mut chain = TestChain::new();
        chain.mine(vec![]).unwrap();
        let err = chain.reorg(5).unwrap_err();
        assert!(matches!(err,
            TestChainError::ReorgDepthExceedsChain { requested: 5, have: 1 }));
    }

    #[test]
    fn mine_alternate_grows_height_post_reorg() {
        let mut chain = TestChain::new();
        chain.mine_empty_blocks(5).unwrap();
        chain.reorg(2).unwrap();
        assert_eq!(chain.height(), 3);
        chain.mine_alternate(vec![]).unwrap();
        assert_eq!(chain.height(), 4);
    }

    #[test]
    fn broadcast_yields_mempool_utxos() {
        let mut chain = TestChain::new();
        let txid = mk_txid(7);
        chain.broadcast(MockTx {
            txid: txid.clone(),
            inputs:  vec![],
            outputs: vec![mk_utxo(txid.clone(), 0, 10_000, "erin")],
        }).unwrap();
        let utxos: Vec<_> = chain.candidate_utxos("erin").collect();
        assert_eq!(utxos.len(), 1);
        assert!(utxos[0].mempool);
        assert_eq!(utxos[0].confirmations, 0);
    }

    #[test]
    fn candidate_utxos_filters_by_address() {
        let mut chain = TestChain::new();
        let txid = mk_txid(8);
        chain.mine(vec![MockTx {
            txid: txid.clone(), inputs: vec![],
            outputs: vec![
                mk_utxo(txid.clone(), 0, 100_000, "alice"),
                mk_utxo(txid.clone(), 1,  50_000, "bob"),
                mk_utxo(txid.clone(), 2,  25_000, "alice"),
            ],
        }]).unwrap();
        assert_eq!(chain.candidate_utxos("alice").count(), 2);
        assert_eq!(chain.candidate_utxos("bob").count(),   1);
        assert_eq!(chain.candidate_utxos("nobody").count(), 0);
    }
}
