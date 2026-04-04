//! Transaction types: OutPoint, TxIn, TxOut, Transaction.
//! Maps to: src/primitives/transaction.h
//!
//! Includes full witness (segwit) serialization support.
//! Transaction is immutable with cached hashes.

use qubitcoin_crypto::hash::hash256;
use qubitcoin_primitives::{Amount, Txid, Uint256, Wtxid};
use qubitcoin_script::Script;
use qubitcoin_serialize::{
    decode_vec, encode_vec, read_compact_size, write_compact_size, Decodable, Encodable,
    Error as SerError,
};
use std::io::{Read, Write};
use std::sync::Arc;

// --- Sequence number constants (from CTxIn) ---

/// Setting nSequence to this value for every input disables nLockTime.
pub const SEQUENCE_FINAL: u32 = 0xffffffff;

/// Maximum sequence number that enables both nLockTime and CHECKLOCKTIMEVERIFY.
pub const MAX_SEQUENCE_NONFINAL: u32 = SEQUENCE_FINAL - 1;

/// If set, nSequence is NOT interpreted as a relative lock-time (BIP68).
pub const SEQUENCE_LOCKTIME_DISABLE_FLAG: u32 = 1 << 31;

/// If set and relative lock-time, units are 512 seconds; otherwise blocks (BIP68).
pub const SEQUENCE_LOCKTIME_TYPE_FLAG: u32 = 1 << 22;

/// Mask for extracting relative lock-time from nSequence (BIP68).
pub const SEQUENCE_LOCKTIME_MASK: u32 = 0x0000ffff;

/// Granularity shift for time-based relative lock-times (BIP68).
pub const SEQUENCE_LOCKTIME_GRANULARITY: u32 = 9;

/// Default transaction version (2). Version 2 enables BIP68 relative lock-time.
pub const CURRENT_TX_VERSION: u32 = 2;

// --- Types ---

/// An outpoint: reference to a specific output of a previous transaction.
///
/// Equivalent to `COutPoint` in Bitcoin Core (`src/primitives/transaction.h`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OutPoint {
    /// The transaction ID of the referenced transaction.
    pub hash: Txid,
    /// The index of the output within that transaction.
    pub n: u32,
}

impl OutPoint {
    /// Sentinel index value used in null/coinbase outpoints (`u32::MAX`).
    pub const NULL_INDEX: u32 = u32::MAX;

    /// Create a new outpoint referencing output `n` of transaction `hash`.
    pub fn new(hash: Txid, n: u32) -> Self {
        OutPoint { hash, n }
    }

    /// Create a null outpoint (zero hash, `NULL_INDEX`). Used in coinbase inputs.
    pub fn null() -> Self {
        OutPoint {
            hash: Txid::ZERO,
            n: Self::NULL_INDEX,
        }
    }

    /// Returns `true` if this is a null outpoint (coinbase marker).
    pub fn is_null(&self) -> bool {
        self.hash.is_null() && self.n == Self::NULL_INDEX
    }
}

impl Default for OutPoint {
    fn default() -> Self {
        Self::null()
    }
}

impl Encodable for OutPoint {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, SerError> {
        let mut size = self.hash.encode(w)?;
        size += self.n.encode(w)?;
        Ok(size)
    }
}

impl Decodable for OutPoint {
    fn decode<R: Read>(r: &mut R) -> Result<Self, SerError> {
        let hash = Txid::decode(r)?;
        let n = u32::decode(r)?;
        Ok(OutPoint { hash, n })
    }
}

/// Witness data for a transaction input (BIP141).
///
/// Equivalent to `CTxInWitness` / `CScriptWitness` in Bitcoin Core.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Witness {
    /// Stack items pushed onto the script evaluation stack before execution.
    pub stack: Vec<Vec<u8>>,
}

impl Witness {
    /// Create an empty witness with no stack items.
    pub fn new() -> Self {
        Witness { stack: Vec::new() }
    }

    /// Returns `true` if the witness stack is empty (no witness data).
    pub fn is_null(&self) -> bool {
        self.stack.is_empty()
    }

    /// Returns the number of items on the witness stack.
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Returns `true` if the witness stack contains no items.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

/// A transaction input.
///
/// Equivalent to `CTxIn` in Bitcoin Core (`src/primitives/transaction.h`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TxIn {
    /// The outpoint being spent (reference to a previous transaction output).
    pub prevout: OutPoint,
    /// Unlocking script (signature script). Empty for segwit spends.
    pub script_sig: Script,
    /// Sequence number. Controls relative lock-time (BIP68) and `nLockTime` behavior.
    pub sequence: u32,
    /// Witness data (BIP141). Only serialized as part of the full transaction.
    pub witness: Witness,
}

impl TxIn {
    /// Create a new transaction input with the given outpoint, script, and sequence.
    pub fn new(prevout: OutPoint, script_sig: Script, sequence: u32) -> Self {
        TxIn {
            prevout,
            script_sig,
            sequence,
            witness: Witness::new(),
        }
    }

    /// Create a coinbase input with a null outpoint and `SEQUENCE_FINAL`.
    pub fn coinbase(script_sig: Script) -> Self {
        TxIn {
            prevout: OutPoint::null(),
            script_sig,
            sequence: SEQUENCE_FINAL,
            witness: Witness::new(),
        }
    }
}

impl Default for TxIn {
    fn default() -> Self {
        TxIn {
            prevout: OutPoint::default(),
            script_sig: Script::new(),
            sequence: SEQUENCE_FINAL,
            witness: Witness::new(),
        }
    }
}

impl Encodable for TxIn {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, SerError> {
        let mut size = self.prevout.encode(w)?;
        size += self.script_sig.encode(w)?;
        size += self.sequence.encode(w)?;
        Ok(size)
    }
}

impl Decodable for TxIn {
    fn decode<R: Read>(r: &mut R) -> Result<Self, SerError> {
        let prevout = OutPoint::decode(r)?;
        let script_sig = Script::decode(r)?;
        let sequence = u32::decode(r)?;
        Ok(TxIn {
            prevout,
            script_sig,
            sequence,
            witness: Witness::new(),
        })
    }
}

/// A transaction output.
///
/// Equivalent to `CTxOut` in Bitcoin Core (`src/primitives/transaction.h`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TxOut {
    /// Output value in satoshis.
    pub value: Amount,
    /// Locking script (scriptPubKey) that defines the spending conditions.
    pub script_pubkey: Script,
}

impl TxOut {
    /// Create a new transaction output with the given value and scriptPubKey.
    pub fn new(value: Amount, script_pubkey: Script) -> Self {
        TxOut {
            value,
            script_pubkey,
        }
    }

    /// Create a null output (value = -1, empty script).
    ///
    /// Used as a placeholder in `SIGHASH_SINGLE` serialization.
    pub fn null() -> Self {
        TxOut {
            value: Amount::from_sat(-1),
            script_pubkey: Script::new(),
        }
    }

    /// Returns `true` if this is a null output (value == -1).
    pub fn is_null(&self) -> bool {
        self.value.to_sat() == -1
    }
}

impl Default for TxOut {
    fn default() -> Self {
        Self::null()
    }
}

impl Encodable for TxOut {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, SerError> {
        let mut size = self.value.to_sat().encode(w)?;
        size += self.script_pubkey.encode(w)?;
        Ok(size)
    }
}

impl Decodable for TxOut {
    fn decode<R: Read>(r: &mut R) -> Result<Self, SerError> {
        let value = Amount::from_sat(i64::decode(r)?);
        let script_pubkey = Script::decode(r)?;
        Ok(TxOut {
            value,
            script_pubkey,
        })
    }
}

/// An immutable transaction with cached hashes.
///
/// Equivalent to `CTransaction` in Bitcoin Core (`src/primitives/transaction.h`).
/// Once constructed, fields should not be modified (ensuring hash cache consistency).
#[derive(Clone, Debug)]
pub struct Transaction {
    /// Transaction format version. Currently 1 or 2 (BIP68 requires version >= 2).
    pub version: u32,
    /// Transaction inputs (coins being spent).
    pub vin: Vec<TxIn>,
    /// Transaction outputs (new coins being created).
    pub vout: Vec<TxOut>,
    /// Lock time. If non-zero, the transaction cannot be mined before this
    /// block height (if < 500,000,000) or Unix timestamp (if >= 500,000,000).
    pub lock_time: u32,
    // Cached hashes (computed once on construction)
    hash: Txid,
    witness_hash: Wtxid,
}

impl Transaction {
    /// Construct from components, computing hashes.
    pub fn new(version: u32, vin: Vec<TxIn>, vout: Vec<TxOut>, lock_time: u32) -> Self {
        let mut tx = Transaction {
            version,
            vin,
            vout,
            lock_time,
            hash: Txid::ZERO,
            witness_hash: Wtxid::ZERO,
        };
        tx.hash = tx.compute_hash();
        tx.witness_hash = tx.compute_witness_hash();
        tx
    }

    /// Get the transaction ID (double-SHA256 of non-witness serialization).
    pub fn txid(&self) -> &Txid {
        &self.hash
    }

    /// Get the witness transaction ID (double-SHA256 of witness serialization).
    pub fn wtxid(&self) -> &Wtxid {
        &self.witness_hash
    }

    /// Check if this is a coinbase transaction.
    pub fn is_coinbase(&self) -> bool {
        self.vin.len() == 1 && self.vin[0].prevout.is_null()
    }

    /// Check if the transaction has any witness data.
    pub fn has_witness(&self) -> bool {
        self.vin.iter().any(|input| !input.witness.is_null())
    }

    /// Check if the transaction is null (empty inputs and outputs).
    pub fn is_null(&self) -> bool {
        self.vin.is_empty() && self.vout.is_empty()
    }

    /// Calculate the total output value.
    pub fn get_value_out(&self) -> Amount {
        self.vout
            .iter()
            .fold(Amount::ZERO, |sum, out| sum + out.value)
    }

    /// Compute the total serialized size including witness data.
    pub fn get_total_size(&self) -> usize {
        let data = serialize_transaction(self, true);
        data.len()
    }

    /// Compute the virtual size (weight / 4, rounded up).
    pub fn get_virtual_size(&self) -> usize {
        let weight = self.get_weight();
        (weight + 3) / 4
    }

    /// Compute the transaction weight (BIP141).
    pub fn get_weight(&self) -> usize {
        let base_size = serialize_transaction(self, false).len();
        let total_size = serialize_transaction(self, true).len();
        base_size * 3 + total_size
    }

    fn compute_hash(&self) -> Txid {
        let data = serialize_transaction(self, false);
        Txid::from_bytes(hash256(&data))
    }

    fn compute_witness_hash(&self) -> Wtxid {
        let data = serialize_transaction(self, true);
        Wtxid::from_bytes(hash256(&data))
    }
}

impl PartialEq for Transaction {
    fn eq(&self, other: &Self) -> bool {
        self.witness_hash == other.witness_hash
    }
}

impl Eq for Transaction {}

impl std::hash::Hash for Transaction {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.witness_hash.hash(state);
    }
}

/// Arc-wrapped transaction reference (shared ownership).
pub type TransactionRef = Arc<Transaction>;

/// Create a TransactionRef from a Transaction.
pub fn make_transaction_ref(tx: Transaction) -> TransactionRef {
    Arc::new(tx)
}

// --- Serialization ---

/// Serialize a transaction with or without witness data.
pub fn serialize_transaction(tx: &Transaction, allow_witness: bool) -> Vec<u8> {
    let mut buf = Vec::new();
    serialize_transaction_to(tx, &mut buf, allow_witness).unwrap();
    buf
}

/// Serialize a transaction to a writer.
pub fn serialize_transaction_to<W: Write>(
    tx: &Transaction,
    w: &mut W,
    allow_witness: bool,
) -> Result<usize, SerError> {
    let mut size = tx.version.encode(w)?;

    let mut flags: u8 = 0;
    if allow_witness && tx.has_witness() {
        flags |= 1;
    }

    if flags != 0 {
        // Extended format: empty vin marker + flags byte
        size += write_compact_size(w, 0)?; // dummy empty vin
        size += flags.encode(w)?;
    }

    // Serialize vin
    size += write_compact_size(w, tx.vin.len() as u64)?;
    for input in &tx.vin {
        size += input.encode(w)?;
    }

    // Serialize vout
    size += write_compact_size(w, tx.vout.len() as u64)?;
    for output in &tx.vout {
        size += output.encode(w)?;
    }

    // Serialize witness data if present
    if flags & 1 != 0 {
        for input in &tx.vin {
            size += write_compact_size(w, input.witness.stack.len() as u64)?;
            for item in &input.witness.stack {
                size += write_compact_size(w, item.len() as u64)?;
                w.write_all(item)?;
                size += item.len();
            }
        }
    }

    size += tx.lock_time.encode(w)?;
    Ok(size)
}

/// Deserialize a transaction from a reader.
pub fn deserialize_transaction<R: Read>(
    r: &mut R,
    allow_witness: bool,
) -> Result<Transaction, SerError> {
    let version = u32::decode(r)?;
    let mut flags: u8 = 0;

    // Try to read vin
    let mut vin: Vec<TxIn> = decode_vec(r)?;

    let vout: Vec<TxOut>;

    if vin.is_empty() && allow_witness {
        // We read a dummy or empty vin. Read flags byte.
        flags = u8::decode(r)?;
        if flags != 0 {
            vin = decode_vec(r)?;
            vout = decode_vec(r)?;
        } else {
            vout = Vec::new();
        }
    } else {
        // Normal non-witness format: read vout
        vout = decode_vec(r)?;
    }

    // Read witness data
    if (flags & 1) != 0 && allow_witness {
        flags ^= 1;
        for input in &mut vin {
            let stack_count = read_compact_size(r)? as usize;
            let mut stack = Vec::with_capacity(stack_count);
            for _ in 0..stack_count {
                let item = Vec::<u8>::decode(r)?;
                stack.push(item);
            }
            input.witness = Witness { stack };
        }

        // Validate: witness flag set but no actual witness data is illegal
        let has_witness = vin.iter().any(|input| !input.witness.is_null());
        if !has_witness {
            return Err(SerError::InvalidEncoding(
                "Superfluous witness record".to_string(),
            ));
        }
    }

    if flags != 0 {
        return Err(SerError::InvalidEncoding(
            "Unknown transaction optional data".to_string(),
        ));
    }

    let lock_time = u32::decode(r)?;

    Ok(Transaction::new(version, vin, vout, lock_time))
}

// Implement Encodable/Decodable for Transaction (with witness by default)
impl Encodable for Transaction {
    fn encode<W: Write>(&self, w: &mut W) -> Result<usize, SerError> {
        serialize_transaction_to(self, w, true)
    }
}

impl Decodable for Transaction {
    fn decode<R: Read>(r: &mut R) -> Result<Self, SerError> {
        deserialize_transaction(r, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outpoint_null() {
        let op = OutPoint::null();
        assert!(op.is_null());
    }

    #[test]
    fn test_outpoint_not_null() {
        let op = OutPoint::new(Txid::from_bytes([1u8; 32]), 0);
        assert!(!op.is_null());
    }

    #[test]
    fn test_txout_null() {
        let out = TxOut::null();
        assert!(out.is_null());
        assert_eq!(out.value.to_sat(), -1);
    }

    #[test]
    fn test_coinbase_tx() {
        let coinbase_input = TxIn::coinbase(Script::from_bytes(vec![0x04, 0xff, 0xff, 0x00, 0x1d]));
        let output = TxOut::new(
            Amount::from_btc(50),
            Script::from_bytes(vec![0x76, 0xa9, 0x14]),
        );
        let tx = Transaction::new(1, vec![coinbase_input], vec![output], 0);
        assert!(tx.is_coinbase());
        assert!(!tx.has_witness());
    }

    #[test]
    fn test_transaction_hash_stability() {
        let tx = Transaction::new(
            1,
            vec![TxIn::default()],
            vec![TxOut::new(Amount::from_sat(100), Script::new())],
            0,
        );
        let hash1 = tx.txid().clone();
        let hash2 = tx.txid().clone();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_serialize_deserialize_simple_tx() {
        let tx = Transaction::new(
            1,
            vec![TxIn::new(
                OutPoint::new(Txid::from_bytes([0xaa; 32]), 0),
                Script::from_bytes(vec![0x00]),
                SEQUENCE_FINAL,
            )],
            vec![TxOut::new(
                Amount::from_sat(50_000),
                Script::from_bytes(vec![0x76, 0xa9]),
            )],
            0,
        );

        let encoded = serialize_transaction(&tx, false);
        let decoded = deserialize_transaction(&mut &encoded[..], false).unwrap();
        assert_eq!(tx.version, decoded.version);
        assert_eq!(tx.vin.len(), decoded.vin.len());
        assert_eq!(tx.vout.len(), decoded.vout.len());
        assert_eq!(tx.lock_time, decoded.lock_time);
        assert_eq!(tx.txid(), decoded.txid());
    }

    #[test]
    fn test_serialize_deserialize_witness_tx() {
        let mut input = TxIn::new(
            OutPoint::new(Txid::from_bytes([0xbb; 32]), 1),
            Script::new(),
            SEQUENCE_FINAL,
        );
        input.witness = Witness {
            stack: vec![vec![0x30, 0x44], vec![0x02, 0x21]],
        };

        let tx = Transaction::new(
            2,
            vec![input],
            vec![TxOut::new(
                Amount::from_sat(100_000),
                Script::from_bytes(vec![0x00, 0x14, 0xab]),
            )],
            0,
        );

        assert!(tx.has_witness());

        // Serialize with witness
        let with_witness = serialize_transaction(&tx, true);
        let decoded = deserialize_transaction(&mut &with_witness[..], true).unwrap();
        assert_eq!(tx.vin[0].witness.stack.len(), 2);
        assert_eq!(decoded.vin[0].witness.stack.len(), 2);

        // Non-witness serialization should differ
        let without_witness = serialize_transaction(&tx, false);
        assert_ne!(with_witness.len(), without_witness.len());

        // TXID is based on non-witness data
        assert_eq!(tx.txid(), decoded.txid());
    }

    #[test]
    fn test_value_out() {
        let tx = Transaction::new(
            1,
            vec![TxIn::default()],
            vec![
                TxOut::new(Amount::from_sat(100), Script::new()),
                TxOut::new(Amount::from_sat(200), Script::new()),
            ],
            0,
        );
        assert_eq!(tx.get_value_out().to_sat(), 300);
    }
}
