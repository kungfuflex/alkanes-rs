//! Benchmarks for qubitcoin-consensus hot-path operations.
//!
//! Covers transaction serialization/deserialization, merkle root computation,
//! and block header hashing -- the three most performance-critical consensus
//! operations during block validation.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use qubitcoin_consensus::block::BlockHeader;
use qubitcoin_consensus::merkle::compute_merkle_root;
use qubitcoin_consensus::transaction::{
    deserialize_transaction, serialize_transaction, OutPoint, Transaction, TxIn, TxOut, Witness,
    SEQUENCE_FINAL,
};
use qubitcoin_primitives::{Amount, BlockHash, Txid, Uint256};
use qubitcoin_script::Script;

// ---------------------------------------------------------------------------
// Helpers to construct realistic test transactions
// ---------------------------------------------------------------------------

/// Create a simple non-witness transaction with the given number of inputs/outputs.
fn make_simple_tx(num_inputs: usize, num_outputs: usize) -> Transaction {
    let vin: Vec<TxIn> = (0..num_inputs)
        .map(|i| {
            TxIn::new(
                OutPoint::new(Txid::from_bytes([i as u8; 32]), 0),
                Script::from_bytes(vec![0x48; 72]), // typical DER sig length
                SEQUENCE_FINAL,
            )
        })
        .collect();

    let vout: Vec<TxOut> = (0..num_outputs)
        .map(|_| {
            TxOut::new(
                Amount::from_sat(50_000),
                Script::from_bytes(vec![
                    0x76, 0xa9, 0x14, // OP_DUP OP_HASH160 PUSH20
                    0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11,
                    0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
                    0xaa, 0xbb, 0xcc, 0xdd,
                    0x88, 0xac, // OP_EQUALVERIFY OP_CHECKSIG
                ]),
            )
        })
        .collect();

    Transaction::new(2, vin, vout, 0)
}

/// Create a segwit transaction with witness data.
fn make_witness_tx(num_inputs: usize, num_outputs: usize) -> Transaction {
    let vin: Vec<TxIn> = (0..num_inputs)
        .map(|i| {
            let mut input = TxIn::new(
                OutPoint::new(Txid::from_bytes([i as u8; 32]), 0),
                Script::new(), // empty scriptSig for segwit
                SEQUENCE_FINAL,
            );
            input.witness = Witness {
                stack: vec![
                    vec![0x30; 72], // DER signature
                    vec![0x02; 33], // compressed pubkey
                ],
            };
            input
        })
        .collect();

    let vout: Vec<TxOut> = (0..num_outputs)
        .map(|_| {
            TxOut::new(
                Amount::from_sat(50_000),
                // P2WPKH output
                Script::from_bytes(vec![
                    0x00, 0x14,
                    0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11,
                    0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
                    0xaa, 0xbb, 0xcc, 0xdd,
                ]),
            )
        })
        .collect();

    Transaction::new(2, vin, vout, 0)
}

// ---------------------------------------------------------------------------
// Transaction serialization
// ---------------------------------------------------------------------------

fn bench_tx_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("tx_serialize");

    // Simple 1-in 1-out (smallest common tx)
    let tx_1_1 = make_simple_tx(1, 1);
    let size_1_1 = serialize_transaction(&tx_1_1, false).len();
    group.throughput(Throughput::Bytes(size_1_1 as u64));
    group.bench_function("1in_1out", |b| {
        b.iter(|| serialize_transaction(black_box(&tx_1_1), false));
    });

    // Typical 2-in 2-out
    let tx_2_2 = make_simple_tx(2, 2);
    let size_2_2 = serialize_transaction(&tx_2_2, false).len();
    group.throughput(Throughput::Bytes(size_2_2 as u64));
    group.bench_function("2in_2out", |b| {
        b.iter(|| serialize_transaction(black_box(&tx_2_2), false));
    });

    // Larger 5-in 5-out
    let tx_5_5 = make_simple_tx(5, 5);
    group.bench_function("5in_5out", |b| {
        b.iter(|| serialize_transaction(black_box(&tx_5_5), false));
    });

    // Segwit transaction with witness
    let tx_wit = make_witness_tx(2, 2);
    group.bench_function("2in_2out_witness", |b| {
        b.iter(|| serialize_transaction(black_box(&tx_wit), true));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Transaction deserialization
// ---------------------------------------------------------------------------

fn bench_tx_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("tx_deserialize");

    // Simple 1-in 1-out
    let tx_1_1 = make_simple_tx(1, 1);
    let data_1_1 = serialize_transaction(&tx_1_1, false);
    group.throughput(Throughput::Bytes(data_1_1.len() as u64));
    group.bench_function("1in_1out", |b| {
        b.iter(|| {
            let mut cursor = black_box(data_1_1.as_slice());
            deserialize_transaction(&mut cursor, false).unwrap()
        });
    });

    // Typical 2-in 2-out
    let tx_2_2 = make_simple_tx(2, 2);
    let data_2_2 = serialize_transaction(&tx_2_2, false);
    group.bench_function("2in_2out", |b| {
        b.iter(|| {
            let mut cursor = black_box(data_2_2.as_slice());
            deserialize_transaction(&mut cursor, false).unwrap()
        });
    });

    // Segwit transaction
    let tx_wit = make_witness_tx(2, 2);
    let data_wit = serialize_transaction(&tx_wit, true);
    group.bench_function("2in_2out_witness", |b| {
        b.iter(|| {
            let mut cursor = black_box(data_wit.as_slice());
            deserialize_transaction(&mut cursor, true).unwrap()
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Transaction serialization roundtrip (serialize + deserialize)
// ---------------------------------------------------------------------------

fn bench_tx_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("tx_roundtrip");

    let tx = make_simple_tx(2, 2);

    group.bench_function("2in_2out_non_witness", |b| {
        b.iter(|| {
            let data = serialize_transaction(black_box(&tx), false);
            let mut cursor = data.as_slice();
            deserialize_transaction(&mut cursor, false).unwrap()
        });
    });

    let tx_wit = make_witness_tx(2, 2);
    group.bench_function("2in_2out_witness", |b| {
        b.iter(|| {
            let data = serialize_transaction(black_box(&tx_wit), true);
            let mut cursor = data.as_slice();
            deserialize_transaction(&mut cursor, true).unwrap()
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Merkle root computation -- called for every block during validation
// ---------------------------------------------------------------------------

fn bench_merkle_root(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_root");

    for num_txs in [1, 4, 16, 64, 256, 1024] {
        // Build a vector of plausible tx hashes
        let hashes: Vec<Uint256> = (0..num_txs)
            .map(|i: usize| {
                let mut bytes = [0u8; 32];
                bytes[0] = (i & 0xff) as u8;
                bytes[1] = ((i >> 8) & 0xff) as u8;
                bytes[2] = ((i >> 16) & 0xff) as u8;
                Uint256::from_bytes(bytes)
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_txs),
            &hashes,
            |b, hashes| {
                b.iter(|| {
                    let mut mutated = false;
                    compute_merkle_root(black_box(hashes.clone()), &mut mutated)
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Block header hashing -- called for every header during IBD and tip updates
// ---------------------------------------------------------------------------

fn bench_block_header_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_header_hash");

    // Genesis-like block header
    let mut header = BlockHeader::new();
    header.version = 1;
    header.prev_blockhash = BlockHash::ZERO;
    header.merkle_root =
        Uint256::from_hex("4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b")
            .unwrap();
    header.time = 1231006505;
    header.bits = 0x1d00ffff;
    header.nonce = 2083236893;

    group.bench_function("genesis_header", |b| {
        b.iter(|| black_box(&header).block_hash());
    });

    // Modern block header
    let mut modern_header = BlockHeader::new();
    modern_header.version = 0x20000000;
    modern_header.prev_blockhash = BlockHash::from_bytes([0xab; 32]);
    modern_header.merkle_root = Uint256::from_bytes([0xcd; 32]);
    modern_header.time = 1700000000;
    modern_header.bits = 0x17034567;
    modern_header.nonce = 0x12345678;

    group.bench_function("modern_header", |b| {
        b.iter(|| black_box(&modern_header).block_hash());
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Block header serialization roundtrip (80 bytes)
// ---------------------------------------------------------------------------

fn bench_block_header_serde(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_header_serde");

    let mut header = BlockHeader::new();
    header.version = 0x20000000;
    header.prev_blockhash = BlockHash::from_bytes([0xab; 32]);
    header.merkle_root = Uint256::from_bytes([0xcd; 32]);
    header.time = 1700000000;
    header.bits = 0x17034567;
    header.nonce = 0x12345678;

    group.throughput(Throughput::Bytes(80));

    group.bench_function("serialize", |b| {
        b.iter(|| qubitcoin_serialize::serialize(black_box(&header)).unwrap());
    });

    let encoded = qubitcoin_serialize::serialize(&header).unwrap();
    group.bench_function("deserialize", |b| {
        b.iter(|| {
            qubitcoin_serialize::deserialize::<BlockHeader>(black_box(&encoded)).unwrap()
        });
    });

    group.bench_function("roundtrip", |b| {
        b.iter(|| {
            let data = qubitcoin_serialize::serialize(black_box(&header)).unwrap();
            qubitcoin_serialize::deserialize::<BlockHeader>(&data).unwrap()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_tx_serialize,
    bench_tx_deserialize,
    bench_tx_roundtrip,
    bench_merkle_root,
    bench_block_header_hash,
    bench_block_header_serde
);
criterion_main!(benches);
