//! Benchmarks for qubitcoin-primitives hot-path operations.
//!
//! Covers ArithUint256 arithmetic (multiply, divide, shift) used in
//! difficulty calculations, and Uint256 hex encoding/decoding used in
//! RPC and logging.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use qubitcoin_primitives::{
    arith_to_uint256, uint256_to_arith, ArithUint256, Uint256,
};

// ---------------------------------------------------------------------------
// ArithUint256 multiplication -- used in difficulty adjustment
// ---------------------------------------------------------------------------

fn bench_arith_mul(c: &mut Criterion) {
    let mut group = c.benchmark_group("arith_uint256_mul");

    // Small * small (common: multiplying by small constants)
    let a_small = ArithUint256::from_u64(0xdeadbeef);
    let b_small = ArithUint256::from_u64(0xcafebabe);
    group.bench_function("small_x_small", |b| {
        b.iter(|| black_box(a_small) * black_box(b_small));
    });

    // Large * large (worst case: full 256-bit operands)
    let mut a_large = ArithUint256::from_u64(0xffffffffffffffff);
    a_large <<= 128;
    a_large += ArithUint256::from_u64(0xffffffffffffffff);
    let mut b_large = ArithUint256::from_u64(0x1234567890abcdef);
    b_large <<= 64;
    b_large += ArithUint256::from_u64(0xfedcba0987654321);
    group.bench_function("large_x_large", |b| {
        b.iter(|| black_box(a_large) * black_box(b_large));
    });

    // Scalar multiply (u32) -- common in target adjustments
    group.bench_function("scalar_u32", |b| {
        b.iter(|| black_box(a_large) * black_box(600u32));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// ArithUint256 division -- used in difficulty target computation
// ---------------------------------------------------------------------------

fn bench_arith_div(c: &mut Criterion) {
    let mut group = c.benchmark_group("arith_uint256_div");

    let mut numerator = ArithUint256::from_u64(0xffffffffffffffff);
    numerator <<= 192;
    let divisor = ArithUint256::from_u64(0x00000000ffff) << 208;

    group.bench_function("large_div", |b| {
        b.iter(|| black_box(numerator) / black_box(divisor));
    });

    // Small division (common in retarget)
    let num_small = ArithUint256::from_u64(1_209_600);
    let div_small = ArithUint256::from_u64(600);
    group.bench_function("small_div", |b| {
        b.iter(|| black_box(num_small) / black_box(div_small));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// ArithUint256 shift operations -- used heavily in set_compact / get_compact
// ---------------------------------------------------------------------------

fn bench_arith_shift(c: &mut Criterion) {
    let mut group = c.benchmark_group("arith_uint256_shift");

    let val = ArithUint256::from_u64(0xdeadbeefcafebabe);

    group.bench_function("shl_64", |b| {
        b.iter(|| black_box(val) << black_box(64u32));
    });

    group.bench_function("shl_200", |b| {
        b.iter(|| black_box(val) << black_box(200u32));
    });

    group.bench_function("shr_64", |b| {
        let shifted = val << 200u32;
        b.iter(|| black_box(shifted) >> black_box(64u32));
    });

    group.bench_function("shr_200", |b| {
        let shifted = val << 200u32;
        b.iter(|| black_box(shifted) >> black_box(200u32));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// ArithUint256 compact encoding -- set_compact / get_compact (nBits)
// ---------------------------------------------------------------------------

fn bench_arith_compact(c: &mut Criterion) {
    let mut group = c.benchmark_group("arith_uint256_compact");

    // Genesis block nBits
    group.bench_function("set_compact", |b| {
        b.iter(|| {
            let mut target = ArithUint256::zero();
            target.set_compact(black_box(0x1d00ffff));
            target
        });
    });

    group.bench_function("get_compact", |b| {
        let mut target = ArithUint256::zero();
        target.set_compact(0x1d00ffff);
        b.iter(|| black_box(target).get_compact(false));
    });

    group.bench_function("roundtrip", |b| {
        b.iter(|| {
            let mut target = ArithUint256::zero();
            target.set_compact(black_box(0x1d00ffff));
            target.get_compact(false)
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Uint256 hex encode/decode -- used in RPC, logging, debug output
// ---------------------------------------------------------------------------

fn bench_uint256_hex(c: &mut Criterion) {
    let mut group = c.benchmark_group("uint256_hex");

    let hex_str = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";
    let u = Uint256::from_hex(hex_str).unwrap();

    group.bench_function("encode", |b| {
        b.iter(|| black_box(u).to_hex());
    });

    group.bench_function("decode", |b| {
        b.iter(|| Uint256::from_hex(black_box(hex_str)));
    });

    group.bench_function("roundtrip", |b| {
        b.iter(|| {
            let decoded = Uint256::from_hex(black_box(hex_str)).unwrap();
            decoded.to_hex()
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// ArithUint256 <-> Uint256 conversion -- used at consensus/RPC boundary
// ---------------------------------------------------------------------------

fn bench_arith_uint256_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("arith_uint256_conversion");

    let arith = ArithUint256::from_u64(0xdeadbeefcafebabe);
    let uint = arith_to_uint256(&arith);

    group.bench_function("arith_to_uint256", |b| {
        b.iter(|| arith_to_uint256(black_box(&arith)));
    });

    group.bench_function("uint256_to_arith", |b| {
        b.iter(|| uint256_to_arith(black_box(&uint)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_arith_mul,
    bench_arith_div,
    bench_arith_shift,
    bench_arith_compact,
    bench_uint256_hex,
    bench_arith_uint256_conversion
);
criterion_main!(benches);
