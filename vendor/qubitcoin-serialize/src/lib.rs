//! Serialization framework for Qubitcoin.
//!
//! Maps to: `src/serialize.h`, `src/streams.h` in Bitcoin Core.
//!
//! This crate provides the building blocks for consensus-compatible binary
//! serialization and deserialization of all on-wire and on-disk data structures:
//!
//! - [`Encodable`] / [`Decodable`] traits -- the Rust equivalent of Bitcoin
//!   Core's `Serialize` / `Unserialize` template methods.
//! - [`CompactSize`](compact_size) encoding/decoding -- variable-length unsigned
//!   integers used for vector lengths and counts.
//! - [`VarInt`](varint) encoding/decoding -- MSB base-128 encoding used in the
//!   UTXO set and block index databases.
//! - [`DataStream`] -- an in-memory buffer with a read cursor, mirroring
//!   Bitcoin Core's `DataStream` (formerly `CDataStream`).

/// CompactSize variable-length integer encoding/decoding.
pub mod compact_size;
/// In-memory data stream with sequential read cursor.
pub mod data_stream;
/// Core `Encodable`/`Decodable` traits and primitive-type implementations.
pub mod encode;
/// VarInt (MSB base-128) variable-length integer encoding/decoding.
pub mod varint;

pub use compact_size::{compact_size_len, read_compact_size, write_compact_size};
pub use data_stream::DataStream;
pub use encode::{
    decode_vec, deserialize, encode_vec, serialize, Decodable, Encodable, Error, MAX_SIZE,
};
pub use varint::{read_varint, varint_len, write_varint};
