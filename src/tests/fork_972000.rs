//! The 972_000 crossing on a MAINNET build, with mainnet traffic.
//!
//! `mint_gate` locks in the DIESEL v3 design on regtest with a test gate and
//! one to three mints per block. This module answers a narrower question: what
//! happens on mainnet at `DIESEL_V3_BLOCK_HEIGHT` = `POST_AUDIT_FORK_HEIGHT` =
//! 972_000 when the blocks look like the blocks we actually see.
//!
//! Everything that matters is the mainnet thing, not a stand-in:
//!   - `--features mainnet`, so the fork heights, block reward, max supply,
//!     precompile dispatch (v2 fastpath below the fork, wasm from it) and the
//!     static DIESEL code map are the production ones;
//!   - SUBMINER-GATE at 4:1001 and the SUBMINER treasury at 2:96782 are the
//!     mainnet bytecode (sha256 830462828a6fa723… / e214acc8965d3177…, fetched
//!     with `getbytecode` from rockshrew-a), initialized with the mainnet args
//!     (diesel 2:0, treasury 2:96782, sigil 4:866, caller_bp 192);
//!   - DIESEL's storage is seeded with the live `/totalsupply` and `/fees`
//!     read with `getstorageat` (rockshrew-a, tip 970_968), and
//!     DIESEL's `/upgrade_initialized` = 1 and `/auth` = 2:69805 (DSIGIL) as
//!     on mainnet, with a real auth token at 2:69805;
//!   - block shapes come from `static/mainnet_diesel_census_970826_970969.csv`:
//!     144 consecutive mainnet blocks, each with its DIESEL mint count (counted
//!     like `number_diesel_mints`: unique txs with a tag-1 protostone calling
//!     2:0 opcode 77) and its coinbase total (`total_miner_fee`). Every one of
//!     the 551_255 mints in that window used the calldata `[77, 0 x 12]`, so
//!     that is the calldata used here.
//!
//! For every replayed block the assertions are exact, not "> 0": what each
//! minter receives, what the treasury is credited, what the gate routed, what
//! accrues to claimable fees, and that supply grows by exactly the reward
//! (v3) or by n * value_per_mint + diesel_fee (v2, remainder dropped).
//!
//! The full 144-block census replay (551k mints) is opt-in at compile time:
//! `FORK972_FULL=1 cargo test ...`.
#![cfg(feature = "mainnet")]

use crate::index_block;
use crate::network::genesis;
use crate::tests::helpers::{
    self as alkane_helpers, assert_return_context, assert_revert_context, clear,
    get_sheet_for_outpoint, CellpackOrEdict,
};
use crate::tests::std::alkanes_std_auth_token_build;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::constants::AUTH_TOKEN_FACTORY_ID;
use alkanes_support::envelope::RawEnvelope;
use alkanes_support::id::AlkaneId;
use alkanes_support::utils::string_to_u128_list;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::{Amount, Block, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Txid, Witness};
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::test_helpers::{create_block_with_coinbase_tx, create_coinbase_transaction};
use protorune_support::balance_sheet::{BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::protostone::{Protostone, ProtostoneEdict};
use std::sync::Arc;
use wasm_bindgen_test::wasm_bindgen_test;

const DIESEL: AlkaneId = AlkaneId { block: 2, tx: 0 };
const DIESEL_RUNE: ProtoruneRuneId = ProtoruneRuneId { block: 2, tx: 0 };
const GATE: AlkaneId = AlkaneId { block: 4, tx: 1001 };
const SIGIL: AlkaneId = AlkaneId { block: 4, tx: 866 };
const TREASURY: AlkaneId = AlkaneId { block: 2, tx: 96782 };
/// DIESEL's owner token on mainnet (`/auth` of 2:0).
const DSIGIL: AlkaneId = AlkaneId { block: 2, tx: 69805 };
const CALLER_BP: u128 = 192;
const BP: u128 = 10_000;
const FORK: u32 = genesis::DIESEL_V3_BLOCK_HEIGHT;

/// Live mainnet DIESEL state at 970_968 (`getstorageat`, rockshrew-a).
const MAINNET_TOTAL_SUPPLY: u128 = 70_873_008_905_224;
const MAINNET_CLAIMABLE_FEES: u128 = 125_695_008_831;

const MAINNET_SUBMINER_GATE: &[u8] = include_bytes!("static/mainnet_4_1001_subminer_gate.wasm");
const MAINNET_SUBMINER_TREASURY: &[u8] =
    include_bytes!("static/mainnet_2_96782_subminer_treasury.wasm");
const CENSUS: &str = include_str!("static/mainnet_diesel_census_970826_970969.csv");

fn console(msg: String) {
    wasm_bindgen_test::console_log!("{}", msg);
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

// ------------------------------------------------------------- census ----

#[derive(Clone, Copy, Debug)]
struct Shape {
    height: u32,
    mints: u32,
    coinbase_sats: u64,
}

fn census() -> Vec<Shape> {
    CENSUS
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            Shape {
                height: f[0].parse().unwrap(),
                mints: f[2].parse().unwrap(),
                coinbase_sats: f[6].parse().unwrap(),
            }
        })
        .collect()
}

fn shape_at(height: u32) -> Shape {
    *census().iter().find(|s| s.height == height).expect("height in census")
}

// ------------------------------------------------------------- storage ----

fn storage(id: AlkaneId, key: &str) -> IndexPointer {
    IndexPointer::default()
        .keyword("/alkanes/")
        .select(&id.into())
        .keyword("/storage/")
        .select(&key.as_bytes().to_vec())
}

fn read_u128(id: AlkaneId, key: &str) -> u128 {
    storage(id, key).get_value::<u128>()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ledger {
    supply: u128,
    fees: u128,
    deposited: u128,
    routed_total: u128,
    routed_to_treasury: u128,
}

fn ledger() -> Ledger {
    Ledger {
        supply: read_u128(DIESEL, "/totalsupply"),
        fees: read_u128(DIESEL, "/fees"),
        deposited: read_u128(TREASURY, "/deposited"),
        routed_total: read_u128(GATE, "/routed-total"),
        routed_to_treasury: read_u128(GATE, "/routed-to-treasury"),
    }
}

// --------------------------------------------------------------- setup ----

fn reward(height: u32) -> u128 {
    (50e8 as u128) / (1u128 << ((height as u128) / 210_000u128))
}

thread_local! {
    /// Test-side model of the emission ledger: the first height from
    /// FORK on whose reward has not been issued yet. Kept independently of
    /// the contract's `/emission/*` storage so the tests check it, not echo it.
    static NEXT_UNSETTLED: std::cell::Cell<u32> = std::cell::Cell::new(FORK);
}

/// What the first successful mint at `height` must settle into claimable fees
/// (the full reward of every earlier block since the last minting block that
/// issued nothing), and advance the model past `height`.
fn take_owed(height: u32) -> u128 {
    NEXT_UNSETTLED.with(|c| {
        let owed = (c.get()..height).map(reward).sum();
        c.set(height + 1);
        owed
    })
}

/// Deploy `binary` with `cellpack` in its own block at `height`.
fn deploy(height: u32, binary: &[u8], cellpack: Cellpack) -> Result<OutPoint> {
    let mut block = create_block_with_coinbase_tx(height);
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        RawEnvelope::from(binary.to_vec()).to_witness(true),
        vec![cellpack],
        OutPoint::new(create_coinbase_transaction(height).compute_txid(), 0),
        false,
    );
    block.txdata.push(tx.clone());
    index_block(&block, height)?;
    Ok(OutPoint {
        txid: tx.compute_txid(),
        vout: 0,
    })
}

struct Env {
    /// DIESEL's own auth token (DSIGIL, 2:69805) units.
    dsigil: OutPoint,
    /// The cFIRE-SIGIL stand-in at 4:866 (an auth token; the real 4:866 is
    /// only consulted by governance, never on the mint path).
    sigil: OutPoint,
}

/// Mainnet-shaped state just below the fork:
///   971_900 auth-token factory template (DIESEL's setup_diesel also runs on
///           this first active block)
///   971_901 DSIGIL auth token CREATEd at 2:69805 (its mainnet id) and set as
///           DIESEL's `/auth`; `/upgrade_initialized` = 1 (mainnet state)
///   971_902 cFIRE-SIGIL at 4:866
///   971_903 SUBMINER treasury at 2:96782 (sequence pinned so CREATE lands
///           there), initialize(asset 2:0, payee 4:866)
///   971_904 SUBMINER-GATE at 4:1001, initialize(2:0, 2:96782, 4:866, 192)
/// then DIESEL's supply and claimable fees are set to the live mainnet values.
fn setup() -> Result<Env> {
    clear();
    NEXT_UNSETTLED.with(|c| c.set(FORK));
    let h0 = FORK - 100;
    deploy(
        h0,
        &alkanes_std_auth_token_build::get_bytes(),
        Cellpack {
            target: AlkaneId {
                block: 3,
                tx: AUTH_TOKEN_FACTORY_ID,
            },
            inputs: vec![100],
        },
    )?;

    // Mainnet crossed the premine upgrade long ago; what 972_000 inherits is
    // its RESULT: `/upgrade_initialized` = 1 and DIESEL's owner token (DSIGIL)
    // at `/auth`, read from mainnet with getstorageat: `/auth` = 2:69805.
    // Recreate exactly that: a real auth token CREATEd at 2:69805 holding 5
    // units, registered as DIESEL's auth token.
    IndexPointer::from_keyword("/alkanes/sequence").set_value::<u128>(DSIGIL.tx);
    let mut dsigil_inputs = vec![0u128];
    dsigil_inputs.extend(string_to_u128_list("DSIGIL".into()));
    dsigil_inputs.extend(string_to_u128_list("DSIGIL".into()));
    dsigil_inputs.push(5);
    let dsigil = deploy(
        h0 + 1,
        &alkanes_std_auth_token_build::get_bytes(),
        Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: dsigil_inputs,
        },
    )?;
    assert!(
        IndexPointer::default().keyword("/alkanes/").select(&DSIGIL.into()).get().len() > 0,
        "DSIGIL landed on 2:69805"
    );
    let auth_bytes: Vec<u8> = DSIGIL.into();
    storage(DIESEL, "/auth").set(Arc::new(auth_bytes));
    storage(DIESEL, "/upgrade_initialized").set_value::<u8>(0x01);

    let mut sigil_inputs = vec![0u128];
    sigil_inputs.extend(string_to_u128_list("cFIRE-SIGIL".into()));
    sigil_inputs.extend(string_to_u128_list("CFS".into()));
    sigil_inputs.push(5);
    let sigil = deploy(
        h0 + 2,
        &alkanes_std_auth_token_build::get_bytes(),
        Cellpack {
            target: AlkaneId { block: 3, tx: SIGIL.tx },
            inputs: sigil_inputs,
        },
    )?;

    // Land the treasury on its mainnet id: CREATE takes the next sequence.
    IndexPointer::from_keyword("/alkanes/sequence").set_value::<u128>(TREASURY.tx);
    deploy(
        h0 + 3,
        MAINNET_SUBMINER_TREASURY,
        Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0, DIESEL.block, DIESEL.tx, SIGIL.block, SIGIL.tx],
        },
    )?;
    deploy(
        h0 + 4,
        MAINNET_SUBMINER_GATE,
        Cellpack {
            target: AlkaneId { block: 3, tx: GATE.tx },
            inputs: vec![
                0,
                DIESEL.block,
                DIESEL.tx,
                TREASURY.block,
                TREASURY.tx,
                SIGIL.block,
                SIGIL.tx,
                CALLER_BP,
            ],
        },
    )?;
    assert_eq!(read_u128(GATE, "/caller-bp"), CALLER_BP, "gate initialized");
    assert!(
        IndexPointer::default()
            .keyword("/alkanes/")
            .select(&TREASURY.into())
            .get()
            .len()
            > 0,
        "treasury landed on 2:96782"
    );

    storage(DIESEL, "/totalsupply").set(Arc::new(MAINNET_TOTAL_SUPPLY.to_le_bytes().to_vec()));
    storage(DIESEL, "/fees").set(Arc::new(MAINNET_CLAIMABLE_FEES.to_le_bytes().to_vec()));
    Ok(Env { dsigil, sigil })
}

// -------------------------------------------------------------- blocks ----

/// The calldata of every mainnet mint in the census window.
fn real_mint() -> Cellpack {
    let mut inputs = vec![77u128];
    inputs.extend(std::iter::repeat(0u128).take(12));
    Cellpack {
        target: DIESEL,
        inputs,
    }
}

fn block_with_coinbase(height: u32, coinbase_sats: u64) -> Block {
    // The harness coinbase carries many 10 BTC outputs and `total_miner_fee`
    // sums them all: keep one output, carrying the real mainnet total.
    let mut block = create_block_with_coinbase_tx(height);
    block.txdata[0].output.truncate(1);
    block.txdata[0].output[0].value = Amount::from_sat(coinbase_sats);
    block
}

/// A distinct fake funding outpoint per tx so every mint has its own txid.
fn funding(height: u32, i: u32) -> OutPoint {
    let mut b = [0u8; 32];
    b[0..4].copy_from_slice(&height.to_le_bytes());
    b[4..8].copy_from_slice(&i.to_le_bytes());
    b[31] = 0xd1;
    OutPoint::new(Txid::from_byte_array(b), 0)
}

/// A block shaped like `shape`: the real coinbase total and `shape.mints`
/// mint txs with the real calldata, indexed at `height`. Returns the block
/// (mint txs at 1..=mints) and the wall time to index it.
fn replay(height: u32, shape: Shape) -> Result<(Block, f64)> {
    let mut block = block_with_coinbase(height, shape.coinbase_sats);
    for i in 0..shape.mints {
        block.txdata.push(alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![real_mint()],
            funding(height, i),
            false,
        ));
    }
    let t = now_ms();
    index_block(&block, height)?;
    Ok((block, now_ms() - t))
}

fn diesel_at(block: &Block, tx: usize, vout: u32) -> Result<u128> {
    Ok(get_sheet_for_outpoint(block, tx, vout)?.get(&DIESEL_RUNE))
}


/// A plain DIESEL transfer as wallets build it: one protostone, no message, an
/// edict sending `amount` to real output 1 and the rest by pointer to output 0.
fn edict_transfer(input: OutPoint, amount: u128) -> bitcoin::Transaction {
    let spk = create_coinbase_transaction(FORK).output[0].script_pubkey.clone();
    alkane_helpers::create_protostone_tx_with_inputs_and_default_pointer(
        vec![TxIn {
            previous_output: input,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        vec![
            TxOut { value: Amount::from_sat(546), script_pubkey: spk.clone() },
            TxOut { value: Amount::from_sat(546), script_pubkey: spk },
        ],
        Protostone {
            burn: None,
            message: vec![],
            edicts: vec![ProtostoneEdict { id: DIESEL_RUNE, amount, output: 1 }],
            refund: Some(0),
            pointer: Some(0),
            from: None,
            protocol_tag: 1,
        },
        0,
    )
}

/// Where DIESEL sits on outputs 0..4 of `block.txdata[tx]` (shadow vouts
/// included), so a transfer's whole outcome can be compared.
fn diesel_layout(block: &Block, tx: usize) -> Result<Vec<u128>> {
    (0..5u32).map(|v| diesel_at(block, tx, v)).collect()
}

struct Expect {
    value_per_mint: u128,
    diesel_fee: u128,
    remainder: u128,
}

fn expect(height: u32, mints: u32, coinbase_sats: u64) -> Expect {
    let reward = reward(height);
    let miner = coinbase_sats as u128;
    let total_tx_fee = if miner > reward { miner - reward } else { 0 };
    let diesel_fee = std::cmp::min(reward / 2, total_tx_fee);
    let n = mints as u128;
    Expect {
        value_per_mint: (reward - diesel_fee) / n,
        diesel_fee,
        remainder: (reward - diesel_fee) % n,
    }
}

/// Exact v3 accounting for a block of `mints` valid real-shape mints.
fn assert_v3_block(block: &Block, height: u32, shape: Shape, before: Ledger) -> Result<()> {
    let after = ledger();
    if shape.mints == 0 {
        assert_eq!(after, before, "a block with no mints touches nothing");
        return Ok(());
    }
    let e = expect(height, shape.mints, shape.coinbase_sats);
    let n = shape.mints as u128;
    let caller = e.value_per_mint * CALLER_BP / BP;
    let to_treasury = e.value_per_mint - caller;
    let owed = take_owed(height);
    for i in 1..=shape.mints as usize {
        assert_eq!(
            diesel_at(block, i, 0)?,
            caller,
            "h={} mint #{}: the minter gets floor(v * 192 / 10000)",
            height,
            i
        );
    }
    assert_eq!(
        after.supply - before.supply,
        reward(height) + owed,
        "h={}: supply grows by the reward plus what earlier mint-free blocks owed ({})",
        height,
        owed
    );
    assert_eq!(
        after.fees - before.fees,
        e.diesel_fee + e.remainder + owed,
        "h={}: fee + remainder + owed spill-over to claimable fees",
        height
    );
    assert_eq!(after.deposited - before.deposited, n * to_treasury, "h={}: treasury credited via deposit()", height);
    assert_eq!(after.routed_total - before.routed_total, n * e.value_per_mint, "h={}: gate routed every mint", height);
    assert_eq!(after.routed_to_treasury - before.routed_to_treasury, n * to_treasury, "h={}", height);
    // Every unit minted lands somewhere: minters + treasury + claimable fees.
    assert_eq!(
        n * caller + n * to_treasury + (after.fees - before.fees),
        after.supply - before.supply,
        "h={}: conservation",
        height
    );
    Ok(())
}

/// Exact v2 (eoa, below the fork) accounting: minter paid in full, gate untouched.
fn assert_v2_block(block: &Block, height: u32, shape: Shape, before: Ledger) -> Result<()> {
    let after = ledger();
    let e = expect(height, shape.mints, shape.coinbase_sats);
    let n = shape.mints as u128;
    for i in 1..=shape.mints as usize {
        assert_eq!(diesel_at(block, i, 0)?, e.value_per_mint, "h={} mint #{}: v2 pays the minter v", height, i);
    }
    assert_eq!(after.supply - before.supply, n * e.value_per_mint + e.diesel_fee, "h={}: v2 drops the remainder", height);
    assert_eq!(after.fees - before.fees, e.diesel_fee, "h={}", height);
    assert_eq!((after.deposited, after.routed_total), (before.deposited, before.routed_total), "below the fork the gate is never called");
    Ok(())
}

fn run_v3(height: u32, shape: Shape, label: &str) -> Result<f64> {
    let before = ledger();
    let (block, ms) = replay(height, shape)?;
    assert_v3_block(&block, height, shape, before)?;
    console(format!(
        "[v3] h={} {} (mainnet {}): {} mints, coinbase {} sats -> indexed in {:.0} ms ({:.2} ms/mint)",
        height,
        label,
        shape.height,
        shape.mints,
        shape.coinbase_sats,
        ms,
        if shape.mints > 0 { ms / shape.mints as f64 } else { 0.0 }
    ));
    Ok(ms)
}

// --------------------------------------------------------------- tests ----

/// The crossing itself: the last v2 block, then 972_000 carrying the HEAVIEST
/// block of the census window (7_492 mints), then the same median-shaped block
/// on both sides of the fork so v2 and v3 are compared on identical input.
#[wasm_bindgen_test]
fn fork_972000_crossing_with_real_traffic() -> Result<()> {
    setup()?;
    let median = shape_at(970893);
    let max = shape_at(970912);
    assert_eq!(FORK, 972_000);
    assert_eq!(protorune::POST_AUDIT_FORK_HEIGHT, 972_000);

    let before = ledger();
    let (block, v2_ms) = replay(FORK - 1, median)?;
    assert_v2_block(&block, FORK - 1, median, before)?;
    console(format!("[v2] h={} median (mainnet 970893): {} mints in {:.0} ms", FORK - 1, median.mints, v2_ms));

    run_v3(FORK, max, "HEAVIEST, on the fork block")?;
    let v3_ms = run_v3(FORK + 1, median, "median, same shape as the last v2 block")?;
    console(format!(
        "same {}-mint shape: v2 {:.0} ms vs v3 {:.0} ms ({:.2}x)",
        median.mints,
        v2_ms,
        v3_ms,
        v3_ms / v2_ms
    ));
    Ok(())
}

/// The spread of real blocks, back to back after the fork: no mints, the
/// fewest mints, median, p90, max, the highest miner fee, a zero-fee block
/// (the coinbase is exactly the reward, as at mainnet 970_917) and the census
/// mint count with the cap-hitting fee. Exact accounting on every block, and
/// cumulative conservation across the run.
#[wasm_bindgen_test]
fn fork_972000_real_block_spectrum() -> Result<()> {
    setup()?;
    let start = ledger();
    let median = shape_at(970893);
    let run: Vec<(Shape, &str)> = vec![
        (shape_at(970842), "no mints"),
        (shape_at(970909), "fewest mints"),
        (median, "median"),
        (shape_at(970929), "p90"),
        (shape_at(970912), "max"),
        (shape_at(970861), "highest miner fee"),
        (Shape { coinbase_sats: 312_500_000, ..median }, "zero tx fee"),
        (Shape { coinbase_sats: 1_000_000_000, ..median }, "fee at the 50% cap"),
    ];
    for (i, (shape, label)) in run.iter().enumerate() {
        run_v3(FORK + i as u32, *shape, label)?;
    }
    let end = ledger();
    // The run ends on a minting block, so every block from FORK on, the
    // mint-free one included, has issued exactly its reward.
    let all: u128 = (FORK..FORK + run.len() as u32).map(reward).sum();
    assert_eq!(end.supply - start.supply, all, "cumulative supply = reward of EVERY block");
    assert_eq!(
        (end.deposited - start.deposited) + (end.fees - start.fees) <= end.supply - start.supply,
        true
    );
    Ok(())
}

/// Opt-in: replay all 144 census blocks (551_255 mints) as 972_000.. .
/// `FORK972_FULL=1` at compile time.
#[wasm_bindgen_test]
fn fork_972000_full_census_replay() -> Result<()> {
    if option_env!("FORK972_FULL").is_none() {
        console("fork_972000_full_census_replay: skipped (set FORK972_FULL=1 to run)".into());
        return Ok(());
    }
    setup()?;
    let start = ledger();
    let mut total_ms = 0f64;
    let mut last_minting = FORK;
    let shapes = census();
    for (i, shape) in shapes.iter().enumerate() {
        let h = FORK + i as u32;
        total_ms += run_v3(h, *shape, "census")?;
        if shape.mints > 0 {
            last_minting = h;
        }
    }
    let expected_supply: u128 = (FORK..=last_minting).map(reward).sum();
    assert_eq!(ledger().supply - start.supply, expected_supply);
    console(format!("full census: {} blocks in {:.0} s", shapes.len(), total_ms / 1000.0));
    Ok(())
}

/// The fork block with the odd traffic real blocks can also carry, alongside
/// real-shape mints: a bare `[77]`, a tx with TWO mint protostones (counted
/// once, second one rejected), a mint whose tail tries to steer the gate, a
/// plain DIESEL edict transfer of v2-minted DIESEL, and a non-DIESEL call.
#[wasm_bindgen_test]
fn fork_972000_mixed_fork_block() -> Result<()> {
    setup()?;
    // DIESEL to move at the fork: one v2 mint at 971_999.
    let before = ledger();
    let (v2_block, _) = replay(FORK - 1, Shape { height: 0, mints: 1, coinbase_sats: 313_000_000 })?;
    assert_v2_block(&v2_block, FORK - 1, Shape { height: 0, mints: 1, coinbase_sats: 313_000_000 }, before)?;
    let held = diesel_at(&v2_block, 1, 0)?;
    assert!(held > 0);

    let h = FORK;
    let coinbase = 313_000_000u64;
    let mut block = block_with_coinbase(h, coinbase);
    let real = 20u32;
    for i in 0..real {
        block.txdata.push(alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![real_mint()],
            funding(h, i),
            false,
        ));
    }
    let bare_ix = block.txdata.len();
    block.txdata.push(alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack { target: DIESEL, inputs: vec![77] }],
        funding(h, 1000),
        false,
    ));
    let steer_ix = block.txdata.len();
    block.txdata.push(alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack { target: DIESEL, inputs: vec![77, 2, 2, 0, 0] }],
        funding(h, 1001),
        false,
    ));
    let double_ix = block.txdata.len();
    block.txdata.push(alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![real_mint(), real_mint()],
        funding(h, 1002),
        false,
    ));
    let transfer_ix = block.txdata.len();
    block.txdata.push(edict_transfer(
        OutPoint { txid: v2_block.txdata[1].compute_txid(), vout: 0 },
        held / 2,
    ));
    let other_ix = block.txdata.len();
    // A non-DIESEL call: the gate's own read opcode (no alkanes moved).
    block.txdata.push(alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack { target: TREASURY, inputs: vec![102] }],
        funding(h, 1003),
        false,
    ));

    let before = ledger();
    index_block(&block, h)?;
    let after = ledger();

    // Counted like number_diesel_mints: unique txs with a mint protostone.
    let counted = (real + 3) as u128; // real + bare + steer + double (once)
    let owed = take_owed(h);
    assert_eq!(owed, 0, "972_000 is the first v3 block: nothing owed yet");
    let e = expect(h, counted as u32, coinbase);
    let caller = e.value_per_mint * CALLER_BP / BP;
    for i in 1..=real as usize {
        assert_eq!(diesel_at(&block, i, 0)?, caller);
    }
    assert_eq!(diesel_at(&block, bare_ix, 0)?, caller, "bare [77] = real-shape mint");
    assert_eq!(diesel_at(&block, steer_ix, 0)?, caller, "the tail cannot steer the gate");
    // The double-mint tx: the first protostone mints, the second is rejected.
    assert_revert_context(
        &OutPoint { txid: block.txdata[double_ix].compute_txid(), vout: 4 },
        "Transaction already used for minting",
    )?;
    assert_eq!(diesel_at(&block, double_ix, 0)?, caller, "first mint of the tx still pays");
    // Every counted mint executed, so the block still emits exactly the reward.
    assert_eq!(after.supply - before.supply, reward(h));
    assert_eq!(after.fees - before.fees, e.diesel_fee + e.remainder);
    assert_eq!(after.deposited - before.deposited, counted * (e.value_per_mint - caller));
    // The v2-minted DIESEL moves by edict at the fork block, intact.
    assert_eq!(diesel_at(&block, transfer_ix, 1)?, held / 2, "edict leg");
    assert_eq!(diesel_at(&block, transfer_ix, 0)?, held - held / 2, "pointer leg");
    // The non-DIESEL call ran.
    assert_return_context(
        &OutPoint { txid: block.txdata[other_ix].compute_txid(), vout: 3 },
        |_| Ok(()),
    )?;
    Ok(())
}

/// Governance right after the fork: the cFIRE-SIGIL clears the gate (mints
/// accrue to claimable fees), points it back at 4:1001 (split resumes), and
/// collects the fees, including the live mainnet balance accrued under v2.
/// DSIGIL (2:69805) is frozen out: its claim reverts and the v3 upgrade path,
/// the only writer of `/auth`, is retired.
#[wasm_bindgen_test]
fn fork_972000_governance_after_fork() -> Result<()> {
    let env = setup()?;
    let median = shape_at(970893);
    run_v3(FORK, median, "median")?;

    let call = |h: u32, inputs: Vec<u128>, input: OutPoint| -> Result<(Block, OutPoint)> {
        let mut block = create_block_with_coinbase_tx(h);
        let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![Cellpack { target: DIESEL, inputs }],
            input,
            false,
        );
        block.txdata.push(tx.clone());
        index_block(&block, h)?;
        Ok((block, OutPoint { txid: tx.compute_txid(), vout: 0 }))
    };

    // Clear the gate: mints accrue to claimable fees, minters get nothing.
    let (_, sigil) = call(FORK + 1, vec![80, 0, 0], env.sigil)?;
    let before = ledger();
    let shape = Shape { height: 0, mints: 50, coinbase_sats: 313_000_000 };
    let (block, _) = replay(FORK + 2, shape)?;
    let after = ledger();
    for i in 1..=50 {
        assert_eq!(diesel_at(&block, i, 0)?, 0);
    }
    let owed = take_owed(FORK + 2);
    assert_eq!(owed, reward(FORK + 1), "the governance block had no mints: its reward spills over");
    assert_eq!(after.supply - before.supply, reward(FORK + 2) + owed);
    assert_eq!(after.fees - before.fees, reward(FORK + 2) + owed, "the whole block, plus the spill-over, accrues to fees");
    assert_eq!(after.deposited, before.deposited, "gate bypassed");

    // Back to 4:1001: the split resumes.
    let (_, sigil) = call(FORK + 3, vec![80, GATE.block, GATE.tx], sigil)?;
    run_v3(FORK + 4, median, "median, gate restored")?;

    // DSIGIL is frozen out: its claim reverts, nothing moves.
    let fees = ledger().fees;
    assert!(fees > MAINNET_CLAIMABLE_FEES);
    let (block, _) = call(FORK + 5, vec![78], env.dsigil)?;
    assert_revert_context(
        &OutPoint { txid: block.txdata[1].compute_txid(), vout: 3 },
        "cFIRE-SIGIL not present",
    )?;
    assert_eq!(ledger().fees, fees, "DSIGIL cannot claim from 972_000");
    // ...and the v3 upgrade path (the only writer of `/auth`) is retired.
    let (block, _) = call(FORK + 6, vec![1], funding(FORK + 6, 0))?;
    assert_revert_context(
        &OutPoint { txid: block.txdata[1].compute_txid(), vout: 3 },
        "upgrade is retired",
    )?;
    let auth_bytes: Vec<u8> = DSIGIL.into();
    assert_eq!(storage(DIESEL, "/auth").get().as_ref().clone(), auth_bytes, "/auth frozen");

    // The cFIRE-SIGIL collects everything claimable.
    let (block, out) = call(FORK + 7, vec![78], sigil)?;
    assert_eq!(ledger().fees, 0, "claimable fees drained");
    let sheet = get_sheet_for_outpoint(&block, 1, out.vout)?;
    assert_eq!(sheet.get(&DIESEL_RUNE), fees, "cFIRE-SIGIL claims the full balance");
    assert_eq!(
        sheet.get(&ProtoruneRuneId { block: SIGIL.block, tx: SIGIL.tx }),
        5,
        "the sigil comes back with the payout"
    );
    Ok(())
}

/// Spill-over: blocks with NO mints after the fork owe their full reward
/// (3.125 DIESEL each), which the next successful mint settles into claimable
/// fees for the cFIRE-SIGIL. Also the first-ever ledger: if 972_000 itself has
/// no mints, its reward is not lost either.
#[wasm_bindgen_test]
fn fork_972000_unminted_blocks_spill_to_cfire_sigil() -> Result<()> {
    let env = setup()?;
    let none = Shape { height: 0, mints: 0, coinbase_sats: 313_000_000 };
    let few = Shape { height: 0, mints: 3, coinbase_sats: 313_000_000 };
    // 972_000 .. 972_004: five empty blocks, starting ON the fork block.
    for h in FORK..FORK + 5 {
        run_v3(h, none, "empty")?;
    }
    let fees0 = ledger().fees;
    run_v3(FORK + 5, few, "first mint after 5 empty blocks")?;
    let fees1 = ledger().fees;
    let five: u128 = (FORK..FORK + 5).map(reward).sum();
    assert_eq!(five, 5 * 312_500_000, "5 x 3.125 DIESEL");
    assert!(fees1 - fees0 >= five, "the 5 empty blocks spilled to fees");

    // A longer gap later on, then a claim.
    for h in FORK + 6..FORK + 20 {
        run_v3(h, none, "empty")?;
    }
    run_v3(FORK + 20, few, "first mint after 14 empty blocks")?;
    let fees = ledger().fees;
    let mut block = create_block_with_coinbase_tx(FORK + 21);
    let tx = alkane_helpers::create_multiple_cellpack_with_witness_and_in(
        Witness::new(),
        vec![Cellpack { target: DIESEL, inputs: vec![78] }],
        env.sigil,
        false,
    );
    block.txdata.push(tx.clone());
    index_block(&block, FORK + 21)?;
    assert_eq!(get_sheet_for_outpoint(&block, 1, 0)?.get(&DIESEL_RUNE), fees);
    assert_eq!(ledger().fees, 0);
    Ok(())
}


/// DIESEL transfers on both sides of the fork. POST_AUDIT_FORK_HEIGHT gates
/// the protorune tag-1 fixes at 972_000 too, so the same transfers are made
/// at 971_999 and at 972_000 and their outcomes compared output by output:
///   - a normal wallet transfer (edict to a real output, rest by pointer);
///   - the edge case of an edict aimed at a protomessage that does not exist
///     (the test helper's shadow-vout mapping with no following protostone).
#[wasm_bindgen_test]
fn fork_972000_edict_transfers_both_sides() -> Result<()> {
    setup()?;
    let four = Shape { height: 0, mints: 4, coinbase_sats: 313_000_000 };
    let (minted, _) = replay(FORK - 3, four)?;
    let held = diesel_at(&minted, 1, 0)?;
    assert!(held > 0);
    let holder = |i: usize| OutPoint { txid: minted.txdata[i].compute_txid(), vout: 0 };

    let mut outcomes = vec![];
    for (h, normal, edge) in [(FORK - 1, holder(1), holder(2)), (FORK, holder(3), holder(4))] {
        let mut block = create_block_with_coinbase_tx(h);
        block.txdata.push(edict_transfer(normal, held / 2));
        block.txdata.push(alkane_helpers::create_multiple_cellpack_with_witness_and_in_with_edicts(
            Witness::new(),
            vec![CellpackOrEdict::Edict(vec![ProtostoneEdict { id: DIESEL_RUNE, amount: held, output: 0 }])],
            edge,
            false,
        ));
        index_block(&block, h)?;
        let (n, e) = (diesel_layout(&block, 1)?, diesel_layout(&block, 2)?);
        console(format!("h={} normal transfer layout {:?}; edge-case edict layout {:?}", h, n, e));
        outcomes.push((n, e));
    }
    let (pre, post) = (&outcomes[0], &outcomes[1]);
    assert_eq!(pre.0, vec![held - held / 2, held / 2, 0, 0, 0], "normal transfer before the fork");
    assert_eq!(post.0, pre.0, "a normal DIESEL transfer is identical across 972_000");
    assert_eq!(
        post.1.iter().sum::<u128>(),
        pre.1.iter().sum::<u128>(),
        "edge-case edict: same amount preserved on both sides (layout logged above)"
    );
    Ok(())
}

/// The mainnet gate on the least fuel a mainnet mint tx can carry at 972_000,
/// with the real calldata. 4x margin, as in mint_gate's regtest probe.
#[wasm_bindgen_test]
fn fork_972000_real_gate_fuel_on_minimum_tx() -> Result<()> {
    setup()?;
    crate::fuel_probe::clear();
    let (block, _) = replay(FORK, Shape { height: 0, mints: 1, coinbase_sats: 313_000_000 })?;
    let records = crate::fuel_probe::snapshot();
    use alkanes_support::trace::{Trace, TraceEvent};
    let trace: Trace = crate::view::trace(&OutPoint {
        txid: block.txdata[1].compute_txid(),
        vout: 3,
    })?
    .try_into()?;
    let events = trace.0.lock().expect("Mutex poisoned").clone();
    let mut start = None;
    let mut allotted = None;
    for ev in events.iter() {
        if let TraceEvent::EnterCall(c) = ev {
            if start.is_none() {
                start = Some(c.fuel);
            } else if c.target == GATE {
                allotted = Some(c.fuel);
                break;
            }
        }
    }
    let (start, allotted) = (start.unwrap(), allotted.expect("gate called"));
    let gas = |t: AlkaneId, op: u128| -> u64 {
        records.iter().filter(|r| r.target == t && r.opcode == op).map(|r| r.gas_used).sum()
    };
    let (gate, treasury) = (gas(GATE, 1), gas(TREASURY, 1));
    console(format!(
        "h=972000 minimum tx fuel {} (minimum_fuel {}), gate allotted {}, gate wasm {} + treasury wasm {}",
        start,
        crate::vm::fuel::minimum_fuel(FORK),
        allotted,
        gate,
        treasury
    ));
    assert!(gate > 0 && treasury > 0);
    assert!((gate + treasury) * 4 <= allotted, "gate+treasury {} vs allotted {}", gate + treasury, allotted);
    Ok(())
}

// ----------------------------------------------------------- profiling ----
// One block of 1_000 real-shape mints each side of the fork, run alone under
// `NODE_OPTIONS=--cpu-prof` to see where per-mint time goes (module
// decompress/parse/link vs execution vs host/storage). Not assertions.

#[wasm_bindgen_test]
fn fork_972000_profile_v2_block() -> Result<()> {
    setup()?;
    let (_, ms) = replay(FORK - 1, Shape { height: 0, mints: 1_000, coinbase_sats: 313_000_000 })?;
    console(format!("PROFILE v2 1000 mints: {:.0} ms", ms));
    Ok(())
}

#[wasm_bindgen_test]
fn fork_972000_profile_v3_block() -> Result<()> {
    setup()?;
    let (_, ms) = replay(FORK, Shape { height: 0, mints: 1_000, coinbase_sats: 313_000_000 })?;
    console(format!("PROFILE v3 1000 mints: {:.0} ms", ms));
    Ok(())
}

#[wasm_bindgen_test]
fn fork_972000_profile_v3_heaviest_block() -> Result<()> {
    setup()?;
    let max = shape_at(970912);
    let (_, ms) = replay(FORK, max)?;
    console(format!("PROFILE v3 {} mints: {:.0} ms ({:.2} ms/mint)", max.mints, ms, ms / max.mints as f64));
    Ok(())
}
