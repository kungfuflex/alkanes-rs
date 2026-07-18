//! Benchmarks for qubitcoin-crypto hot-path operations.
//!
//! Covers SHA256d, Hash160, and SipHash -- the most latency-sensitive
//! cryptographic primitives used throughout consensus validation and
//! mempool processing.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use qubitcoin_crypto::hash::{hash160, hash256, sha256_hash};
use qubitcoin_crypto::siphash::{sip_hash, sip_hash_uint256};

// ---------------------------------------------------------------------------
// SHA256d (double SHA-256) -- used for block hashes, txids, merkle nodes
// ---------------------------------------------------------------------------

fn bench_sha256d(c: &mut Criterion) {
    let mut group = c.benchmark_group("sha256d");

    for size in [32, 256, 1024, 4096] {
        let data = vec![0xab_u8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| hash256(black_box(data)));
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Single SHA-256 -- used in tagged hashes and as an intermediate step
// ---------------------------------------------------------------------------

fn bench_sha256(c: &mut Criterion) {
    let mut group = c.benchmark_group("sha256");

    for size in [32, 256, 1024, 4096] {
        let data = vec![0xcd_u8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| sha256_hash(black_box(data)));
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Hash160 (RIPEMD160(SHA256(x))) -- used for P2PKH / P2SH addresses
// ---------------------------------------------------------------------------

fn bench_hash160(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash160");

    for size in [32, 256, 1024, 4096] {
        let data = vec![0xef_u8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, data| {
            b.iter(|| hash160(black_box(data)));
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// SipHash-2-4 -- used for compact block short IDs, mempool ordering,
// hash table randomization
// ---------------------------------------------------------------------------

fn bench_siphash(c: &mut Criterion) {
    let mut group = c.benchmark_group("siphash");

    let k0: u64 = 0x0706050403020100;
    let k1: u64 = 0x0f0e0d0c0b0a0908;

    // Variable-length general SipHash
    for size in [8, 32, 64, 256] {
        let data = vec![0x42_u8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("general", size),
            &data,
            |b, data| {
                b.iter(|| sip_hash(black_box(k0), black_box(k1), black_box(data)));
            },
        );
    }

    // Specialized uint256 (32-byte) SipHash -- most common path
    let hash_data: [u8; 32] = [0xaa; 32];
    group.throughput(Throughput::Bytes(32));
    group.bench_function("uint256", |b| {
        b.iter(|| sip_hash_uint256(black_box(k0), black_box(k1), black_box(&hash_data)));
    });

    group.finish();
}

criterion_group!(benches, bench_sha256d, bench_sha256, bench_hash160, bench_siphash);
criterion_main!(benches);
