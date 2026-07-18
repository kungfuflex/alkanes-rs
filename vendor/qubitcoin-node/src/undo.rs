//! Block undo data for chain reorganization.
//!
//! Maps to: `src/undo.h` (`CTxUndo`, `CBlockUndo`) in Bitcoin Core.
//!
//! When a block is connected, each non-coinbase transaction spends one or more
//! UTXO entries.  Before those entries are removed from the UTXO set we snapshot
//! them into a `TxUndo` (one per non-coinbase tx).  All per-tx undo records
//! for a single block are collected into a `BlockUndo`.
//!
//! During a chain reorganization the undo data is replayed in reverse to
//! restore the UTXO set to its pre-connection state.

use qubitcoin_common::coins::Coin;
use qubitcoin_serialize::{
    read_compact_size, write_compact_size, Decodable, Encodable, Error as SerError,
};

use std::io::{Read, Write};

// ---------------------------------------------------------------------------
// TxUndo
// ---------------------------------------------------------------------------

/// Undo data for a single transaction.
///
/// Stores the coins that were consumed by each input, in order.
/// Port of Bitcoin Core's `CTxUndo`.
#[derive(Debug, Clone)]
pub struct TxUndo {
    /// The coins that were spent (one per input, in input order).
    pub prev_coins: Vec<Coin>,
}

impl TxUndo {
    /// Create a new empty `TxUndo`.
    pub fn new() -> Self {
        TxUndo {
            prev_coins: Vec::new(),
        }
    }

    /// Create a `TxUndo` pre-allocated for `n` inputs.
    pub fn with_capacity(n: usize) -> Self {
        TxUndo {
            prev_coins: Vec::with_capacity(n),
        }
    }
}

impl Default for TxUndo {
    fn default() -> Self {
        Self::new()
    }
}

impl Encodable for TxUndo {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, SerError> {
        let mut size = write_compact_size(w, self.prev_coins.len() as u64)?;
        for coin in &self.prev_coins {
            size += coin.encode(w)?;
        }
        Ok(size)
    }
}

impl Decodable for TxUndo {
    fn decode<R: Read>(r: &mut R) -> Result<Self, SerError> {
        let count = read_compact_size(r)? as usize;
        let mut prev_coins = Vec::with_capacity(count);
        for _ in 0..count {
            prev_coins.push(Coin::decode(r)?);
        }
        Ok(TxUndo { prev_coins })
    }
}

// ---------------------------------------------------------------------------
// BlockUndo
// ---------------------------------------------------------------------------

/// Undo data for an entire block.
///
/// Stores `TxUndo` for every transaction in the block **except** the
/// coinbase (which has no real inputs to undo).
///
/// Port of Bitcoin Core's `CBlockUndo`.
#[derive(Debug, Clone)]
pub struct BlockUndo {
    /// Undo data for each non-coinbase transaction, in the same order as they
    /// appear in the block's `vtx` vector (skipping `vtx[0]`).
    pub tx_undo: Vec<TxUndo>,
}

impl BlockUndo {
    /// Create a new empty `BlockUndo`.
    pub fn new() -> Self {
        BlockUndo {
            tx_undo: Vec::new(),
        }
    }

    /// Create a `BlockUndo` pre-allocated for `n` non-coinbase transactions.
    pub fn with_capacity(n: usize) -> Self {
        BlockUndo {
            tx_undo: Vec::with_capacity(n),
        }
    }
}

impl Default for BlockUndo {
    fn default() -> Self {
        Self::new()
    }
}

impl Encodable for BlockUndo {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, SerError> {
        let mut size = write_compact_size(w, self.tx_undo.len() as u64)?;
        for tx_undo in &self.tx_undo {
            size += tx_undo.encode(w)?;
        }
        Ok(size)
    }
}

impl Decodable for BlockUndo {
    fn decode<R: Read>(r: &mut R) -> Result<Self, SerError> {
        let count = read_compact_size(r)? as usize;
        let mut tx_undo = Vec::with_capacity(count);
        for _ in 0..count {
            tx_undo.push(TxUndo::decode(r)?);
        }
        Ok(BlockUndo { tx_undo })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use qubitcoin_common::coins::Coin;
    use qubitcoin_consensus::TxOut;
    use qubitcoin_primitives::Amount;
    use qubitcoin_script::Script;
    use qubitcoin_serialize::{deserialize, serialize};

    fn make_test_coin(value: i64, height: u32, coinbase: bool) -> Coin {
        Coin::new(
            TxOut::new(
                Amount::from_sat(value),
                Script::from_bytes(vec![0x76, 0xa9]),
            ),
            height,
            coinbase,
        )
    }

    #[test]
    fn test_tx_undo_empty_roundtrip() {
        let tx_undo = TxUndo::new();
        let encoded = serialize(&tx_undo).unwrap();
        let decoded: TxUndo = deserialize(&encoded).unwrap();
        assert_eq!(decoded.prev_coins.len(), 0);
    }

    #[test]
    fn test_tx_undo_single_coin_roundtrip() {
        let mut tx_undo = TxUndo::new();
        tx_undo.prev_coins.push(make_test_coin(50_000, 100, false));

        let encoded = serialize(&tx_undo).unwrap();
        let decoded: TxUndo = deserialize(&encoded).unwrap();

        assert_eq!(decoded.prev_coins.len(), 1);
        assert_eq!(decoded.prev_coins[0].tx_out.value.to_sat(), 50_000);
        assert_eq!(decoded.prev_coins[0].height, 100);
        assert!(!decoded.prev_coins[0].coinbase);
    }

    #[test]
    fn test_tx_undo_multiple_coins_roundtrip() {
        let mut tx_undo = TxUndo::new();
        tx_undo.prev_coins.push(make_test_coin(100_000, 50, true));
        tx_undo.prev_coins.push(make_test_coin(200_000, 75, false));
        tx_undo.prev_coins.push(make_test_coin(300_000, 99, false));

        let encoded = serialize(&tx_undo).unwrap();
        let decoded: TxUndo = deserialize(&encoded).unwrap();

        assert_eq!(decoded.prev_coins.len(), 3);
        assert_eq!(decoded.prev_coins[0].tx_out.value.to_sat(), 100_000);
        assert!(decoded.prev_coins[0].coinbase);
        assert_eq!(decoded.prev_coins[1].tx_out.value.to_sat(), 200_000);
        assert!(!decoded.prev_coins[1].coinbase);
        assert_eq!(decoded.prev_coins[2].tx_out.value.to_sat(), 300_000);
        assert_eq!(decoded.prev_coins[2].height, 99);
    }

    #[test]
    fn test_block_undo_empty_roundtrip() {
        let block_undo = BlockUndo::new();
        let encoded = serialize(&block_undo).unwrap();
        let decoded: BlockUndo = deserialize(&encoded).unwrap();
        assert_eq!(decoded.tx_undo.len(), 0);
    }

    #[test]
    fn test_block_undo_roundtrip() {
        let mut block_undo = BlockUndo::with_capacity(2);

        // First non-coinbase tx spent one coin.
        let mut tx_undo1 = TxUndo::new();
        tx_undo1
            .prev_coins
            .push(make_test_coin(1_000_000, 10, false));
        block_undo.tx_undo.push(tx_undo1);

        // Second non-coinbase tx spent two coins.
        let mut tx_undo2 = TxUndo::new();
        tx_undo2
            .prev_coins
            .push(make_test_coin(2_000_000, 20, true));
        tx_undo2
            .prev_coins
            .push(make_test_coin(3_000_000, 30, false));
        block_undo.tx_undo.push(tx_undo2);

        let encoded = serialize(&block_undo).unwrap();
        let decoded: BlockUndo = deserialize(&encoded).unwrap();

        assert_eq!(decoded.tx_undo.len(), 2);
        assert_eq!(decoded.tx_undo[0].prev_coins.len(), 1);
        assert_eq!(
            decoded.tx_undo[0].prev_coins[0].tx_out.value.to_sat(),
            1_000_000
        );
        assert_eq!(decoded.tx_undo[1].prev_coins.len(), 2);
        assert_eq!(
            decoded.tx_undo[1].prev_coins[0].tx_out.value.to_sat(),
            2_000_000
        );
        assert!(decoded.tx_undo[1].prev_coins[0].coinbase);
        assert_eq!(
            decoded.tx_undo[1].prev_coins[1].tx_out.value.to_sat(),
            3_000_000
        );
    }

    #[test]
    fn test_block_undo_with_capacity() {
        let undo = BlockUndo::with_capacity(5);
        assert_eq!(undo.tx_undo.len(), 0);
        assert!(undo.tx_undo.capacity() >= 5);
    }

    #[test]
    fn test_tx_undo_with_capacity() {
        let undo = TxUndo::with_capacity(3);
        assert_eq!(undo.prev_coins.len(), 0);
        assert!(undo.prev_coins.capacity() >= 3);
    }
}
