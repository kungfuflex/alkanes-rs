//! Compatibility conversions between qubitcoin types and rust-bitcoin 0.32 types.
//!
//! These conversions go through raw serialized bytes to ensure byte-identical
//! round-trip fidelity. This is the safest approach since both libraries
//! implement the same wire format.
//!
//! For composite types (Transaction, Block, BlockHeader), the standard
//! `TryFrom` trait is implemented directly, since at least one side of each
//! conversion is defined in this crate.
//!
//! For primitive types (Txid, BlockHash, Amount) that live in external crates
//! on both sides, standalone conversion functions are provided because Rust's
//! orphan rules prevent `From`/`Into` implementations between two foreign types.
//!
//! Gated behind the `rust-bitcoin-compat` feature.

use bitcoin::hashes::Hash;

use crate::block::{Block, BlockHeader};
use crate::transaction::Transaction;
use qubitcoin_primitives::{Amount, BlockHash, Txid};
use qubitcoin_serialize::{Decodable, Encodable};

/// Error type for conversion failures between qubitcoin and rust-bitcoin types.
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    /// Qubitcoin serialization produced an error.
    #[error("Serialization error: {0}")]
    Serialize(String),
    /// Deserialization of the foreign type produced an error.
    #[error("Deserialization error: {0}")]
    Deserialize(String),
}

// ===========================================================================
// Transaction conversions (via raw bytes)
// ===========================================================================

impl TryFrom<&Transaction> for bitcoin::Transaction {
    type Error = ConvertError;

    fn try_from(tx: &Transaction) -> Result<bitcoin::Transaction, ConvertError> {
        let mut buf = Vec::new();
        tx.encode(&mut buf)
            .map_err(|e| ConvertError::Serialize(e.to_string()))?;
        bitcoin::consensus::deserialize(&buf).map_err(|e| ConvertError::Deserialize(e.to_string()))
    }
}

impl TryFrom<&bitcoin::Transaction> for Transaction {
    type Error = ConvertError;

    fn try_from(tx: &bitcoin::Transaction) -> Result<Transaction, ConvertError> {
        let bytes = bitcoin::consensus::serialize(tx);
        let mut cursor = std::io::Cursor::new(&bytes);
        Transaction::decode(&mut cursor).map_err(|e| ConvertError::Deserialize(e.to_string()))
    }
}

// ===========================================================================
// Block conversions (via raw bytes)
// ===========================================================================

impl TryFrom<&Block> for bitcoin::Block {
    type Error = ConvertError;

    fn try_from(block: &Block) -> Result<bitcoin::Block, ConvertError> {
        let mut buf = Vec::new();
        block
            .encode(&mut buf)
            .map_err(|e| ConvertError::Serialize(e.to_string()))?;
        bitcoin::consensus::deserialize(&buf).map_err(|e| ConvertError::Deserialize(e.to_string()))
    }
}

impl TryFrom<&bitcoin::Block> for Block {
    type Error = ConvertError;

    fn try_from(block: &bitcoin::Block) -> Result<Block, ConvertError> {
        let bytes = bitcoin::consensus::serialize(block);
        let mut cursor = std::io::Cursor::new(&bytes);
        Block::decode(&mut cursor).map_err(|e| ConvertError::Deserialize(e.to_string()))
    }
}

// ===========================================================================
// BlockHeader conversions (via raw bytes)
// ===========================================================================

impl TryFrom<&BlockHeader> for bitcoin::block::Header {
    type Error = ConvertError;

    fn try_from(header: &BlockHeader) -> Result<bitcoin::block::Header, ConvertError> {
        let mut buf = Vec::new();
        header
            .encode(&mut buf)
            .map_err(|e| ConvertError::Serialize(e.to_string()))?;
        bitcoin::consensus::deserialize(&buf).map_err(|e| ConvertError::Deserialize(e.to_string()))
    }
}

impl TryFrom<&bitcoin::block::Header> for BlockHeader {
    type Error = ConvertError;

    fn try_from(header: &bitcoin::block::Header) -> Result<BlockHeader, ConvertError> {
        let bytes = bitcoin::consensus::serialize(header);
        let mut cursor = std::io::Cursor::new(&bytes);
        BlockHeader::decode(&mut cursor).map_err(|e| ConvertError::Deserialize(e.to_string()))
    }
}

// ===========================================================================
// Simple / primitive type conversions (standalone functions)
//
// These cannot use From/Into due to Rust orphan rules: both sides are
// defined in external crates relative to qubitcoin-consensus.
// ===========================================================================

/// Convert a qubitcoin [`Txid`] to a rust-bitcoin [`bitcoin::Txid`].
pub fn txid_to_bitcoin(txid: &Txid) -> bitcoin::Txid {
    bitcoin::Txid::from_byte_array(*txid.data())
}

/// Convert a rust-bitcoin [`bitcoin::Txid`] to a qubitcoin [`Txid`].
pub fn txid_from_bitcoin(txid: &bitcoin::Txid) -> Txid {
    Txid::from_bytes(txid.to_byte_array())
}

/// Convert a qubitcoin [`BlockHash`] to a rust-bitcoin [`bitcoin::BlockHash`].
pub fn blockhash_to_bitcoin(hash: &BlockHash) -> bitcoin::BlockHash {
    bitcoin::BlockHash::from_byte_array(*hash.data())
}

/// Convert a rust-bitcoin [`bitcoin::BlockHash`] to a qubitcoin [`BlockHash`].
pub fn blockhash_from_bitcoin(hash: &bitcoin::BlockHash) -> BlockHash {
    BlockHash::from_bytes(hash.to_byte_array())
}

/// Convert a qubitcoin [`Amount`] to a rust-bitcoin [`bitcoin::Amount`].
///
/// Qubitcoin uses signed i64 satoshis; rust-bitcoin uses unsigned u64.
/// Negative values are saturated to zero.
pub fn amount_to_bitcoin(amount: Amount) -> bitcoin::Amount {
    let sat = amount.to_sat();
    if sat < 0 {
        bitcoin::Amount::ZERO
    } else {
        bitcoin::Amount::from_sat(sat as u64)
    }
}

/// Convert a rust-bitcoin [`bitcoin::Amount`] to a qubitcoin [`Amount`].
pub fn amount_from_bitcoin(amount: bitcoin::Amount) -> Amount {
    Amount::from_sat(amount.to_sat() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{OutPoint, TxIn, TxOut, Witness, SEQUENCE_FINAL};
    use qubitcoin_primitives::Uint256;
    use qubitcoin_script::Script;

    /// Helper: build a simple non-witness transaction for testing.
    fn make_simple_tx() -> Transaction {
        let input = TxIn::new(
            OutPoint::new(Txid::from_bytes([0xaa; 32]), 0),
            Script::from_bytes(vec![0x51]), // OP_TRUE
            SEQUENCE_FINAL,
        );
        let output = TxOut::new(
            Amount::from_sat(50_000),
            Script::from_bytes(vec![0x76, 0xa9, 0x14]),
        );
        Transaction::new(1, vec![input], vec![output], 0)
    }

    /// Helper: build a witness (segwit) transaction for testing.
    fn make_witness_tx() -> Transaction {
        let mut input = TxIn::new(
            OutPoint::new(Txid::from_bytes([0xbb; 32]), 1),
            Script::new(),
            SEQUENCE_FINAL,
        );
        input.witness = Witness {
            stack: vec![vec![0x30, 0x44], vec![0x02, 0x21]],
        };
        let output = TxOut::new(
            Amount::from_sat(100_000),
            Script::from_bytes(vec![0x00, 0x14, 0xab]),
        );
        Transaction::new(2, vec![input], vec![output], 0)
    }

    /// Helper: build a block header resembling the genesis block.
    fn make_header() -> BlockHeader {
        let mut header = BlockHeader::new();
        header.version = 1;
        header.time = 1231006505;
        header.bits = 0x1d00ffff;
        header.nonce = 2083236893;
        header.merkle_root =
            Uint256::from_hex("4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b")
                .unwrap();
        header
    }

    // --- Transaction round-trip tests ---

    #[test]
    fn test_transaction_roundtrip_qubitcoin_to_bitcoin() {
        let qb_tx = make_simple_tx();

        // qubitcoin -> bitcoin
        let btc_tx = bitcoin::Transaction::try_from(&qb_tx).unwrap();
        // bitcoin -> qubitcoin
        let roundtrip = Transaction::try_from(&btc_tx).unwrap();

        // Byte-level equality via serialized form.
        let qb_bytes = qubitcoin_serialize::serialize(&qb_tx).unwrap();
        let rt_bytes = qubitcoin_serialize::serialize(&roundtrip).unwrap();
        assert_eq!(qb_bytes, rt_bytes, "non-witness tx round-trip mismatch");
    }

    #[test]
    fn test_transaction_roundtrip_bitcoin_to_qubitcoin() {
        let qb_tx = make_simple_tx();
        let btc_tx = bitcoin::Transaction::try_from(&qb_tx).unwrap();

        // bitcoin -> qubitcoin -> bitcoin
        let qb_roundtrip = Transaction::try_from(&btc_tx).unwrap();
        let btc_roundtrip = bitcoin::Transaction::try_from(&qb_roundtrip).unwrap();

        let btc_bytes = bitcoin::consensus::serialize(&btc_tx);
        let rt_bytes = bitcoin::consensus::serialize(&btc_roundtrip);
        assert_eq!(btc_bytes, rt_bytes, "bitcoin tx round-trip mismatch");
    }

    #[test]
    fn test_witness_transaction_roundtrip() {
        let qb_tx = make_witness_tx();
        assert!(qb_tx.has_witness());

        let btc_tx = bitcoin::Transaction::try_from(&qb_tx).unwrap();
        let roundtrip = Transaction::try_from(&btc_tx).unwrap();
        assert!(roundtrip.has_witness());

        let qb_bytes = qubitcoin_serialize::serialize(&qb_tx).unwrap();
        let rt_bytes = qubitcoin_serialize::serialize(&roundtrip).unwrap();
        assert_eq!(qb_bytes, rt_bytes, "witness tx round-trip mismatch");
    }

    // --- Block round-trip tests ---

    #[test]
    fn test_block_roundtrip() {
        let tx = make_simple_tx();
        let mut block = Block::with_header(make_header());
        block.vtx.push(std::sync::Arc::new(tx));

        let btc_block = bitcoin::Block::try_from(&block).unwrap();
        let roundtrip = Block::try_from(&btc_block).unwrap();

        let qb_bytes = qubitcoin_serialize::serialize(&block).unwrap();
        let rt_bytes = qubitcoin_serialize::serialize(&roundtrip).unwrap();
        assert_eq!(qb_bytes, rt_bytes, "block round-trip mismatch");
    }

    #[test]
    fn test_empty_block_roundtrip() {
        let block = Block::with_header(make_header());

        let btc_block = bitcoin::Block::try_from(&block).unwrap();
        let roundtrip = Block::try_from(&btc_block).unwrap();

        let qb_bytes = qubitcoin_serialize::serialize(&block).unwrap();
        let rt_bytes = qubitcoin_serialize::serialize(&roundtrip).unwrap();
        assert_eq!(qb_bytes, rt_bytes, "empty block round-trip mismatch");
    }

    // --- BlockHeader round-trip tests ---

    #[test]
    fn test_block_header_roundtrip() {
        let header = make_header();

        let btc_header = bitcoin::block::Header::try_from(&header).unwrap();
        let roundtrip = BlockHeader::try_from(&btc_header).unwrap();

        assert_eq!(header, roundtrip, "block header round-trip mismatch");
    }

    #[test]
    fn test_block_header_field_fidelity() {
        let header = make_header();
        let btc_header = bitcoin::block::Header::try_from(&header).unwrap();

        // Verify specific fields survived the conversion.
        assert_eq!(btc_header.time, header.time);
        assert_eq!(btc_header.nonce, header.nonce);
    }

    // --- Txid conversion tests ---

    #[test]
    fn test_txid_roundtrip() {
        let bytes = [0xab_u8; 32];
        let qb_txid = Txid::from_bytes(bytes);

        let btc_txid = txid_to_bitcoin(&qb_txid);
        let roundtrip = txid_from_bitcoin(&btc_txid);

        assert_eq!(qb_txid, roundtrip, "Txid round-trip mismatch");
        assert_eq!(*qb_txid.data(), *roundtrip.data());
    }

    #[test]
    fn test_txid_zero() {
        let qb_txid = Txid::ZERO;
        let btc_txid = txid_to_bitcoin(&qb_txid);
        let roundtrip = txid_from_bitcoin(&btc_txid);
        assert!(roundtrip.is_null());
    }

    // --- BlockHash conversion tests ---

    #[test]
    fn test_blockhash_roundtrip() {
        let bytes = [0xcd_u8; 32];
        let qb_hash = BlockHash::from_bytes(bytes);

        let btc_hash = blockhash_to_bitcoin(&qb_hash);
        let roundtrip = blockhash_from_bitcoin(&btc_hash);

        assert_eq!(qb_hash, roundtrip, "BlockHash round-trip mismatch");
    }

    #[test]
    fn test_blockhash_zero() {
        let qb_hash = BlockHash::ZERO;
        let btc_hash = blockhash_to_bitcoin(&qb_hash);
        let roundtrip = blockhash_from_bitcoin(&btc_hash);
        assert!(roundtrip.is_null());
    }

    // --- Amount conversion tests ---

    #[test]
    fn test_amount_roundtrip_positive() {
        let qb_amount = Amount::from_sat(123_456_789);
        let btc_amount = amount_to_bitcoin(qb_amount);
        assert_eq!(btc_amount.to_sat(), 123_456_789);

        let roundtrip = amount_from_bitcoin(btc_amount);
        assert_eq!(roundtrip, qb_amount);
    }

    #[test]
    fn test_amount_zero() {
        let qb_amount = Amount::ZERO;
        let btc_amount = amount_to_bitcoin(qb_amount);
        assert_eq!(btc_amount, bitcoin::Amount::ZERO);
    }

    #[test]
    fn test_amount_negative_saturates() {
        // Negative qubitcoin Amount should saturate to bitcoin::Amount::ZERO
        // since bitcoin::Amount is unsigned.
        let qb_amount = Amount::from_sat(-100);
        let btc_amount = amount_to_bitcoin(qb_amount);
        assert_eq!(btc_amount, bitcoin::Amount::ZERO);
    }

    #[test]
    fn test_amount_max_money() {
        let qb_amount = Amount::MAX;
        let btc_amount = amount_to_bitcoin(qb_amount);
        assert_eq!(btc_amount.to_sat(), qubitcoin_primitives::MAX_MONEY as u64);
    }
}
