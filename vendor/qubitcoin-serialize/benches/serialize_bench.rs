//! Benchmarks for qubitcoin-serialize hot-path operations.
//!
//! Covers CompactSize encoding/decoding (used in every transaction and block
//! serialization) and DataStream read/write operations.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use qubitcoin_serialize::compact_size::{read_compact_size, write_compact_size};
use qubitcoin_serialize::data_stream::DataStream;
use qubitcoin_serialize::{deserialize, serialize};
use std::io::Cursor;

// ---------------------------------------------------------------------------
// CompactSize encode -- called for every vector/list in the wire protocol
// ---------------------------------------------------------------------------

fn bench_compact_size_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("compact_size_encode");

    let test_cases: &[(&str, u64)] = &[
        ("1byte_0", 0),
        ("1byte_252", 252),
        ("3byte_253", 253),
        ("3byte_65535", 0xffff),
        ("5byte_65536", 0x10000),
        ("5byte_max32", 0xffffffff),
        ("9byte_large", 0x100000000),
    ];

    for (name, value) in test_cases {
        group.bench_with_input(BenchmarkId::new("write", name), value, |b, &val| {
            let mut buf = Vec::with_capacity(9);
            b.iter(|| {
                buf.clear();
                write_compact_size(&mut buf, black_box(val)).unwrap();
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// CompactSize decode -- called for every vector/list during deserialization
// ---------------------------------------------------------------------------

fn bench_compact_size_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("compact_size_decode");

    let test_cases: &[(&str, u64)] = &[
        ("1byte_0", 0),
        ("1byte_252", 252),
        ("3byte_253", 253),
        ("3byte_65535", 0xffff),
        ("5byte_65536", 0x10000),
        ("5byte_max32", 0xffffffff),
    ];

    for (name, value) in test_cases {
        let mut encoded = Vec::new();
        write_compact_size(&mut encoded, *value).unwrap();

        group.bench_with_input(BenchmarkId::new("read", name), &encoded, |b, encoded| {
            b.iter(|| {
                let mut cursor = Cursor::new(black_box(encoded.as_slice()));
                read_compact_size(&mut cursor).unwrap()
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// CompactSize roundtrip -- encode then decode
// ---------------------------------------------------------------------------

fn bench_compact_size_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("compact_size_roundtrip");

    let values: &[u64] = &[0, 100, 252, 253, 1000, 0xffff, 0x10000, 0xffffffff];

    for &val in values {
        group.bench_with_input(BenchmarkId::from_parameter(val), &val, |b, &val| {
            let mut buf = Vec::with_capacity(9);
            b.iter(|| {
                buf.clear();
                write_compact_size(&mut buf, black_box(val)).unwrap();
                let mut cursor = Cursor::new(buf.as_slice());
                read_compact_size(&mut cursor).unwrap()
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// DataStream write/read operations -- the primary in-memory serialization
// buffer used throughout the codebase
// ---------------------------------------------------------------------------

fn bench_datastream_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("datastream_write");

    // Writing a sequence of u32 values
    group.bench_function("100_u32s", |b| {
        b.iter(|| {
            let mut ds = DataStream::new();
            for i in 0u32..100 {
                ds.write_obj(&black_box(i)).unwrap();
            }
            ds
        });
    });

    // Writing mixed types (simulates a transaction header)
    group.bench_function("mixed_types", |b| {
        b.iter(|| {
            let mut ds = DataStream::new();
            ds.write_obj(&black_box(2u32)).unwrap(); // version
            ds.write_obj(&black_box(0u32)).unwrap(); // locktime
            ds.write_obj(&black_box(0xffffffffu32)).unwrap(); // sequence
            ds.write_obj(&black_box(50000i64)).unwrap(); // amount
            ds
        });
    });

    group.finish();
}

fn bench_datastream_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("datastream_read");

    // Prepare a DataStream with 100 u32s
    let mut prepared = DataStream::new();
    for i in 0u32..100 {
        prepared.write_obj(&i).unwrap();
    }
    let bytes = prepared.as_bytes().to_vec();

    group.bench_function("100_u32s", |b| {
        b.iter(|| {
            let mut ds = DataStream::from_bytes(black_box(bytes.clone()));
            for _ in 0..100 {
                let _: u32 = ds.read_obj().unwrap();
            }
        });
    });

    // Prepare mixed types
    let mut mixed = DataStream::new();
    mixed.write_obj(&2u32).unwrap();
    mixed.write_obj(&0u32).unwrap();
    mixed.write_obj(&0xffffffffu32).unwrap();
    mixed.write_obj(&50000i64).unwrap();
    let mixed_bytes = mixed.as_bytes().to_vec();

    group.bench_function("mixed_types", |b| {
        b.iter(|| {
            let mut ds = DataStream::from_bytes(black_box(mixed_bytes.clone()));
            let _: u32 = ds.read_obj().unwrap();
            let _: u32 = ds.read_obj().unwrap();
            let _: u32 = ds.read_obj().unwrap();
            let _: i64 = ds.read_obj().unwrap();
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Primitive type serialization roundtrips via the serialize/deserialize API
// ---------------------------------------------------------------------------

fn bench_primitive_serde(c: &mut Criterion) {
    let mut group = c.benchmark_group("primitive_serde");

    // u32 roundtrip
    group.bench_function("u32_roundtrip", |b| {
        b.iter(|| {
            let encoded = serialize(&black_box(0x12345678u32)).unwrap();
            let _: u32 = deserialize(&encoded).unwrap();
        });
    });

    // u64 roundtrip
    group.bench_function("u64_roundtrip", |b| {
        b.iter(|| {
            let encoded = serialize(&black_box(0x1234567890abcdefu64)).unwrap();
            let _: u64 = deserialize(&encoded).unwrap();
        });
    });

    // Uint256 roundtrip
    group.throughput(Throughput::Bytes(32));
    group.bench_function("uint256_roundtrip", |b| {
        let val = qubitcoin_primitives::Uint256::from_bytes([0xab; 32]);
        b.iter(|| {
            let encoded = serialize(&black_box(val)).unwrap();
            let _: qubitcoin_primitives::Uint256 = deserialize(&encoded).unwrap();
        });
    });

    // Vec<u8> roundtrip (simulates script serialization)
    group.bench_function("vec_u8_256_roundtrip", |b| {
        let val = vec![0x42u8; 256];
        b.iter(|| {
            let encoded = serialize(black_box(&val)).unwrap();
            let _: Vec<u8> = deserialize(&encoded).unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_compact_size_encode,
    bench_compact_size_decode,
    bench_compact_size_roundtrip,
    bench_datastream_write,
    bench_datastream_read,
    bench_primitive_serde
);
criterion_main!(benches);
