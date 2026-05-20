//! Native (Rust) reimplementation of the DIESEL alkane (id 2:0).
//!
//! Drop-in replacement for the wasm-compiled `alkanes-std-genesis-alkane-upgraded-eoa`
//! binary, intended for v2.2.0-rc.1's hot-path optimization (DIESEL alone is
//! responsible for most wasmi instantiations on mainnet).
//!
//! Design contract: every call through `run_diesel_eoa()` MUST produce the
//! same observable side effects as `run_after_special()` would for the same
//! `(context, binary)` — namely:
//!   * the returned `ExtendedCallResponse` is byte-identical to what the wasm
//!     `__execute` would have serialized;
//!   * the returned `gas_used` matches the wasm path's internal gas number
//!     (because `FuelTank::consume_fuel(gas_used)` mutates block-level fuel
//!     allocation for later txs in the same block — divergence here causes
//!     consensus drift on later mints);
//!   * any atomic mutations produced by the wasm path (via
//!     `pipe_storagemap_to(response.storage, …)` in `message.rs`) are
//!     equivalent under the precompile.
//!
//! The `gas_used` returned by the precompile is *looked up* from a
//! `DieselPathGas` table. The consensus-safety property is that the table
//! values match what wasmi charges for the same path on the same binary; that
//! invariant is enforced by `tests::diesel_sidebyside`.

// Only used by the cfg-gated shadow-test code below; the
// dispatcher in `crate::precompile` is what adds storage_byte_fuel
// for the production path.
#[cfg(any(test, feature = "test-utils"))]
use crate::vm::fuel::fuel_per_store_byte;
use crate::vm::runtime::AlkanesRuntimeContext;
use alkanes_support::{
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::ExtendedCallResponse,
    storage::StorageMap,
};
#[cfg(any(test, feature = "test-utils"))]
use std::sync::Mutex as StdMutex;
use anyhow::{anyhow, Result};
use bitcoin::hashes::Hash;
use metashrew_core::index_pointer::IndexPointer;
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::{Arc, Mutex};

/// Gas constants per DIESEL code path. Values come from the
/// `diesel_gas_paths_exhaustive` test against the binary the precompile
/// shadows. Per-chain (mainnet vs regtest etc.) values may differ because the
/// binary is rebuilt per feature set; the side-by-side test calibrates and
/// asserts these in-process for whatever binary is active.
#[derive(Debug, Clone, Copy, Default)]
pub struct DieselPathGas {
    pub communist_first_of_block: u64,
    pub communist_subsequent: u64,
    pub communist_revert_one_per_tx: u64,
    pub communist_revert_legacy_seen: u64,
    pub legacy_first_of_block: u64,
    pub legacy_revert_already_minted: u64,
    pub view_get_name: u64,
    pub view_get_symbol: u64,
    pub view_get_total_supply: u64,
}

/// Identifies which DIESEL execution path was taken. Used by tests to label
/// observed gas values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DieselPath {
    CommunistFirstOfBlock,
    CommunistSubsequent,
    CommunistRevertOnePerTx,
    CommunistRevertLegacySeen,
    LegacyFirstOfBlock,
    LegacyRevertAlreadyMinted,
    ViewGetName,
    ViewGetSymbol,
    ViewGetTotalSupply,
    NotEoa,
}

pub const DIESEL_ID: AlkaneId = AlkaneId { block: 2, tx: 0 };

/// Per-chain calibrated gas constants. Each chain has a distinct DIESEL
/// wasm binary (built with the matching feature flag), so each needs its
/// own calibration. Values populated below are placeholders calibrated
/// against the regtest EOA binary in tests::diesel_shadow; production
/// chains MUST run the shadow test on their own binary and update these
/// before flipping `diesel-precompile` on.
#[cfg(not(any(
    feature = "mainnet",
    feature = "dogecoin",
    feature = "bellscoin",
    feature = "fractal",
    feature = "luckycoin"
)))]
pub const CHAIN_GAS: DieselPathGas = DieselPathGas {
    // From tests::diesel_shadow cross-height sweep, regtest EOA binary,
    // height-invariant across {2, 10, 50, 250}.
    communist_first_of_block: 374598,    // 380838 total - 6240 storage_fuel @h=2 (40/byte * 156)
    communist_subsequent: 325403,        // 329083 total - 3680 storage_fuel (40 * 92)
    communist_revert_one_per_tx: 200678, // observed in diesel_gas_paths E1
    communist_revert_legacy_seen: 220750,
    legacy_first_of_block: 162266,
    legacy_revert_already_minted: 131377,
    view_get_name: 74636,
    view_get_symbol: 74636,
    view_get_total_supply: 84074,
};
#[cfg(feature = "mainnet")]
pub const CHAIN_GAS: DieselPathGas = DieselPathGas {
    // Calibrated against the mainnet EOA binary by tests::diesel_shadow at
    // heights TEST_BASE_HEIGHT (925_000) + {2, 10, 50, 250} — invariant
    // across every sampled height/N-mint combination. Total samples per
    // path: c1=5, c2=12, p1=1, v99/v100=2 each, v101=2.
    //
    // These are *internal* gas values (excluding storage_byte_fuel which
    // run_diesel_eoa adds dynamically based on the StorageMap size).
    communist_first_of_block: 370742,
    communist_subsequent: 321466,
    // Revert paths: precompile's reported gas is moot because the message
    // handler calls FuelTank::drain_fuel() on revert, zeroing the per-tx
    // budget regardless of the value the precompile returned. We still
    // need a value so shadow_compare can compare; revert success/failure
    // is governed by error-string matching elsewhere.
    communist_revert_one_per_tx: 0,
    communist_revert_legacy_seen: 0,
    legacy_first_of_block: 159324,
    legacy_revert_already_minted: 0,
    view_get_name: 72812,
    view_get_symbol: 72812,
    view_get_total_supply: 82166,
};
#[cfg(any(feature = "dogecoin", feature = "bellscoin", feature = "fractal", feature = "luckycoin"))]
pub const CHAIN_GAS: DieselPathGas = DieselPathGas {
    communist_first_of_block: 0,
    communist_subsequent: 0,
    communist_revert_one_per_tx: 0,
    communist_revert_legacy_seen: 0,
    legacy_first_of_block: 0,
    legacy_revert_already_minted: 0,
    view_get_name: 0,
    view_get_symbol: 0,
    view_get_total_supply: 0,
};

const STORAGE_PREFIX: &[u8] = b"/storage/";

/// Returns true iff the given target+opcode is a DIESEL call the precompile
/// is willing to handle for its target+opcode. Other opcodes (Initialize,
/// Upgrade, CollectFees, Burn, etc.) fall through to the wasm path.
pub fn matches_precompile(target: &AlkaneId, opcode: u128) -> bool {
    if target != &DIESEL_ID {
        return false;
    }
    matches!(opcode, 77 | 99 | 100 | 101)
}

/// More restrictive predicate that also rejects parcels whose tx contains
/// more than one mint protostone. The wasm path's gas in that case
/// includes extra consensus-decode + sha256 work proportional to the
/// larger tx_size, which a precompile constant cannot match. Such txs are
/// rare in practice (a 2nd mint protostone in the same tx always reverts
/// via enforce_one_mint_per_tx) and we let the wasm path handle them.
pub fn matches_precompile_for_ctx(ctx: &AlkanesRuntimeContext) -> bool {
    let opcode = ctx.inputs.first().copied().unwrap_or(0);
    if !matches_precompile(&ctx.myself, opcode) {
        return false;
    }
    if opcode == 77 {
        // Count mint protostones in this transaction
        if count_mint_protostones_in_tx(&ctx.message.transaction)
            .map(|n| n != 1)
            .unwrap_or(true)
        {
            return false;
        }
    }
    true
}

fn count_mint_protostones_in_tx(tx: &bitcoin::Transaction) -> anyhow::Result<u32> {
    use ordinals::{Artifact, Runestone};
    use protorune_support::protostone::Protostone;
    use protorune_support::utils::decode_varint_list;
    use alkanes_support::cellpack::Cellpack;
    use std::io::Cursor;
    let mut count = 0u32;
    if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(tx) {
        let protostones = Protostone::from_runestone(runestone)?;
        for p in protostones {
            if p.protocol_tag != 1 {
                continue;
            }
            let calldata: Vec<u8> = p.message.iter().flat_map(|v| v.to_be_bytes()).collect();
            if calldata.is_empty() {
                continue;
            }
            let list = match decode_varint_list(&mut Cursor::new(calldata)) {
                Ok(l) => l,
                Err(_) => continue,
            };
            if list.len() < 2 {
                continue;
            }
            if let Ok(cp) = TryInto::<Cellpack>::try_into(list) {
                if cp.target == DIESEL_ID && cp.inputs.first().copied() == Some(77) {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

/// Reads a storage value at `key` under DIESEL's storage subtree (i.e.
/// `/alkanes/<2:0>/storage/<key>`), checking the in-call pending writes
/// `pending` first to mirror wasmi's `_CACHE` semantics.
fn read_storage(
    ctx: &AlkanesRuntimeContext,
    pending: &StorageMap,
    key: &[u8],
) -> Vec<u8> {
    if let Some(v) = pending.get(key) {
        return v.clone();
    }
    let bytes = (&ctx.message.atomic)
        .keyword("/alkanes/")
        .select(&DIESEL_ID.into())
        .keyword("/storage/")
        .select(&key.to_vec())
        .get();
    let _ = STORAGE_PREFIX; // silence unused warning if cfg drops the path
    (*bytes).clone()
}

fn read_u128(ctx: &AlkanesRuntimeContext, pending: &StorageMap, key: &[u8]) -> u128 {
    let bytes = read_storage(ctx, pending, key);
    if bytes.is_empty() {
        0
    } else {
        u128::from_le_bytes(bytes.as_slice().try_into().expect("u128 slot has wrong length"))
    }
}

fn write_u128(pending: &mut StorageMap, key: &[u8], v: u128) {
    pending.set(key, v.to_le_bytes());
}

fn read_len(ctx: &AlkanesRuntimeContext, pending: &StorageMap, key: &[u8]) -> usize {
    read_storage(ctx, pending, key).len()
}

/// Block reward formula for the regtest / mainnet chain configuration. Mirrors
/// `ChainConfiguration::block_reward` in the EOA wasm. For non-regtest
/// chains the exponent base / shift period differs — when the precompile is
/// wired in for those chains, this must branch on the active feature.
#[cfg(not(any(
    feature = "mainnet",
    feature = "dogecoin",
    feature = "bellscoin",
    feature = "fractal",
    feature = "luckycoin"
)))]
fn block_reward(height: u64) -> u128 {
    (50e8 as u128) / (1u128 << ((height as u128) / 210000u128))
}
#[cfg(feature = "mainnet")]
fn block_reward(height: u64) -> u128 {
    (50e8 as u128) / (1u128 << ((height as u128) / 210000u128))
}

fn max_supply() -> u128 {
    // Regtest: u128::MAX (per ChainConfiguration impl). Mainnet:
    // mirrors `GenesisAlkane::max_supply` from
    // `crates/alkanes-std-genesis-alkane-upgraded-eoa/src/lib.rs:106`.
    #[cfg(feature = "mainnet")]
    {
        156_250_000_000_000u128
    }
    #[cfg(not(feature = "mainnet"))]
    {
        u128::MAX
    }
}

/// Test-only accessor for the precompile's compile-time `max_supply`
/// constant. Used by `tests::diesel_divergence_repro` to assert
/// byte-equivalence with the wasm `GenesisAlkane::max_supply` value
/// without standing up a full wasm runtime.
#[cfg(any(test, feature = "test-utils"))]
pub fn _test_only_max_supply() -> u128 {
    max_supply()
}

/// Test-only accessor for `number_diesel_mints`. Used by
/// `tests::diesel_divergence_repro` to exercise the
/// decode-error-propagation behavior of the precompile's mint counter
/// without going through the full `run_diesel_eoa` dispatch.
#[cfg(any(test, feature = "test-utils"))]
pub fn _test_only_number_diesel_mints(
    ctx: &AlkanesRuntimeContext,
) -> Result<u128> {
    number_diesel_mints(ctx)
}

/// Sums the value of every output in the block's coinbase tx. Mirrors
/// `_get_total_miner_fee` from `vm/host_functions.rs`.
fn total_miner_fee(ctx: &AlkanesRuntimeContext) -> u128 {
    let coinbase = match ctx.message.block.txdata.first() {
        Some(tx) => tx,
        None => return 0,
    };
    coinbase
        .output
        .iter()
        .map(|out| out.value.to_sat() as u128)
        .sum()
}

/// Counts unique txs in the block that contain at least one DIESEL mint
/// protostone (target=2:0, opcode=77). Mirrors `_get_number_diesel_mints`
/// from `vm/host_functions.rs`.
fn number_diesel_mints(ctx: &AlkanesRuntimeContext) -> Result<u128> {
    use ordinals::{Artifact, Runestone};
    use protorune_support::protostone::Protostone;
    use protorune_support::utils::decode_varint_list;
    use alkanes_support::cellpack::Cellpack;
    use std::io::Cursor;

    let mut counter: u128 = 0;
    for tx in &ctx.message.block.txdata {
        if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(tx) {
            let protostones = Protostone::from_runestone(runestone)?;
            for protostone in protostones {
                if protostone.protocol_tag != 1 {
                    continue;
                }
                let calldata: Vec<u8> = protostone
                    .message
                    .iter()
                    .flat_map(|v| v.to_be_bytes())
                    .collect();
                if calldata.is_empty() {
                    continue;
                }
                // v3 divergence fix: propagate the varint decode error
                // instead of skipping the malformed protostone. The wasm
                // path's `_get_number_diesel_mints` in
                // `vm/host_functions.rs:659` uses `?` to propagate the
                // same error; pre-fix the precompile used
                // `Err(_) => continue` which silently produced a lower
                // count → larger `value_per_mint` → divergent
                // `/totalsupply`. Suspected cause of the h=949478
                // divergence documented in
                // `.fastpath-bug-investigation/`.
                let list = decode_varint_list(&mut Cursor::new(calldata))?;
                if list.len() < 2 {
                    continue;
                }
                if let Ok(cellpack) = TryInto::<Cellpack>::try_into(list) {
                    if cellpack.target == DIESEL_ID
                        && !cellpack.inputs.is_empty()
                        && cellpack.inputs[0] == 77
                    {
                        counter += 1;
                        break;
                    }
                }
            }
        }
    }
    Ok(counter)
}

fn current_txid_bytes(ctx: &AlkanesRuntimeContext) -> Vec<u8> {
    ctx.message
        .transaction
        .compute_txid()
        .as_byte_array()
        .to_vec()
}

/// Run the DIESEL precompile for a call whose target is 2:0. Caller is
/// responsible for ensuring `matches_precompile(target, opcode)` returned
/// true before dispatching here.
///
/// The returned `ExtendedCallResponse.storage` should be piped to atomic by
/// the caller exactly as the wasm path's storage map is piped — i.e. via
/// `pipe_storagemap_to(response.storage, atomic.derive(/alkanes/2:0))`.
///
/// Phase 2 of the PrecompiledAlkane abstraction: this is now a thin
/// wrapper around `try_dispatch_precompile::<DieselEoa>`. The trait
/// `DieselEoa` lives below; the body of dispatch lives in
/// `crate::precompile`. Behavior is byte-equivalent — verified by the
/// `diesel_sidebyside` + `diesel_shadow` tests.
pub fn run_diesel_eoa(
    ctx_arc: Arc<Mutex<AlkanesRuntimeContext>>,
    gas: &DieselPathGas,
) -> Result<(ExtendedCallResponse, u64, DieselPath)> {
    match crate::precompile::try_dispatch_precompile::<DieselEoa>(ctx_arc, gas) {
        Some(r) => r,
        None => Err(anyhow!(
            "diesel precompile: dispatcher declined (caller should fall through to wasm)"
        )),
    }
}

/// Routes a DIESEL call by opcode. Caller (the
/// [`crate::precompile::try_dispatch_precompile`] generic dispatcher)
/// has already verified the call is dispatchable; this is purely the
/// per-opcode body.
fn execute_diesel(
    ctx: &AlkanesRuntimeContext,
    pending: &mut StorageMap,
) -> Result<(ExtendedCallResponse, DieselPath)> {
    let opcode = ctx
        .inputs
        .first()
        .copied()
        .ok_or_else(|| anyhow!("diesel precompile: missing opcode"))?;
    match opcode {
        77 => run_mint(ctx, pending),
        99 => run_view_name(ctx, DieselPath::ViewGetName, "DIESEL"),
        100 => run_view_name(ctx, DieselPath::ViewGetSymbol, "DIESEL"),
        101 => run_view_total_supply(ctx),
        op => Err(anyhow!("diesel precompile: unsupported opcode {}", op)),
    }
}

fn forward_response(ctx: &AlkanesRuntimeContext) -> ExtendedCallResponse {
    ExtendedCallResponse {
        alkanes: ctx.incoming_alkanes.clone(),
        storage: StorageMap::default(),
        data: Vec::new(),
    }
}

fn run_view_name(
    ctx: &AlkanesRuntimeContext,
    path: DieselPath,
    text: &str,
) -> Result<(ExtendedCallResponse, DieselPath)> {
    debug_assert!(matches!(
        path,
        DieselPath::ViewGetName | DieselPath::ViewGetSymbol
    ));
    let mut response = forward_response(ctx);
    response.data = text.as_bytes().to_vec();
    Ok((response, path))
}

fn run_view_total_supply(
    ctx: &AlkanesRuntimeContext,
) -> Result<(ExtendedCallResponse, DieselPath)> {
    let pending = StorageMap::default();
    let total_supply = read_u128(ctx, &pending, b"/totalsupply");
    let mut response = forward_response(ctx);
    response.data = total_supply.to_le_bytes().to_vec();
    Ok((response, DieselPath::ViewGetTotalSupply))
}

fn run_mint(
    ctx: &AlkanesRuntimeContext,
    pending: &mut StorageMap,
) -> Result<(ExtendedCallResponse, DieselPath)> {
    let upgrade_initialized = read_len(ctx, pending, b"/upgrade_initialized") > 0;
    if upgrade_initialized {
        run_mint_communist(ctx, pending)
    } else {
        run_mint_legacy(ctx, pending)
    }
}

fn run_mint_legacy(
    ctx: &AlkanesRuntimeContext,
    pending: &mut StorageMap,
) -> Result<(ExtendedCallResponse, DieselPath)> {
    let height = ctx.message.height;
    let height_key: Vec<u8> = {
        let mut k = b"/seen/".to_vec();
        k.extend_from_slice(&height.to_le_bytes());
        k
    };
    let already_minted = read_len(ctx, pending, &height_key) > 0;
    if already_minted {
        return Err(anyhow!(format!(
            "already minted for block {}",
            hex::encode(height.to_le_bytes())
        )));
    }
    pending.set(&height_key, &[1u8, 0, 0, 0]);

    let total_supply_now = read_u128(ctx, pending, b"/totalsupply");
    if total_supply_now >= max_supply() {
        return Err(anyhow!("total supply has been reached"));
    }
    let reward = block_reward(height);
    let new_supply = total_supply_now
        .checked_add(reward)
        .ok_or_else(|| anyhow!("total supply overflow"))?;
    write_u128(pending, b"/totalsupply", new_supply);

    let mut response = forward_response(ctx);
    response.alkanes.0.push(AlkaneTransfer {
        id: DIESEL_ID,
        value: reward,
    });
    Ok((response, DieselPath::LegacyFirstOfBlock))
}

fn run_mint_communist(
    ctx: &AlkanesRuntimeContext,
    pending: &mut StorageMap,
) -> Result<(ExtendedCallResponse, DieselPath)> {
    let caller = ctx.caller.clone();
    if caller != AlkaneId::new(0, 0) {
        return Err(anyhow!(
            "Diesel mint must be called from EOA (first call in a protostone)"
        ));
    }

    let height = ctx.message.height;

    // enforce_one_mint_per_tx
    let txid = current_txid_bytes(ctx);
    let mut tx_hash_key: Vec<u8> = b"/tx-hashes/".to_vec();
    tx_hash_key.extend_from_slice(&txid);
    if read_len(ctx, pending, &tx_hash_key) > 0 {
        return Err(anyhow!("Transaction already used for minting"));
    }
    pending.set(&tx_hash_key, &[1u8]);

    // enforce_no_upgraded_mints_with_legacy_mints
    let mut seen_key: Vec<u8> = b"/seen/".to_vec();
    seen_key.extend_from_slice(&height.to_le_bytes());
    if read_len(ctx, pending, &seen_key) > 0 {
        return Err(anyhow!("upgraded mint in the same block as legacy mint"));
    }

    // Precompiled extcalls: number_diesel_mints, total_miner_fee
    let total_mints = number_diesel_mints(ctx)?;
    if total_mints == 0 {
        return Err(anyhow!("diesel precompile: no mint protostones in block"));
    }
    let miner_fee = total_miner_fee(ctx);
    let reward = block_reward(height);
    let total_tx_fee = if miner_fee > reward {
        miner_fee - reward
    } else {
        0
    };
    let diesel_fee = std::cmp::min(reward / 2, total_tx_fee);
    let value_per_mint = (reward - diesel_fee) / total_mints;

    // observe_upgraded_mint
    let mut upgraded_seen_key: Vec<u8> = b"/upgraded_seen/".to_vec();
    upgraded_seen_key.extend_from_slice(&height.to_le_bytes());
    let upgraded_seen_present = read_len(ctx, pending, &upgraded_seen_key) > 0;

    let path;
    if !upgraded_seen_present {
        pending.set(&upgraded_seen_key, &[1u8, 0, 0, 0]);
        // claimable_fees: if empty, set to 0 (no-op-ish), then increase by diesel_fee
        let fees_now = read_u128(ctx, pending, b"/fees");
        let new_fees = fees_now
            .checked_add(diesel_fee)
            .ok_or_else(|| anyhow!("claimable_fees overflow"))?;
        write_u128(pending, b"/fees", new_fees);
        // increase_total_supply(diesel_fee)
        let ts_after_fee_inc = read_u128(ctx, pending, b"/totalsupply")
            .checked_add(diesel_fee)
            .ok_or_else(|| anyhow!("totalsupply overflow"))?;
        write_u128(pending, b"/totalsupply", ts_after_fee_inc);
        path = DieselPath::CommunistFirstOfBlock;
    } else {
        path = DieselPath::CommunistSubsequent;
    }

    // supply check + final increase
    let ts_now = read_u128(ctx, pending, b"/totalsupply");
    if ts_now >= max_supply() {
        return Err(anyhow!("total supply has been reached"));
    }
    let new_ts = ts_now
        .checked_add(value_per_mint)
        .ok_or_else(|| anyhow!("totalsupply overflow"))?;
    write_u128(pending, b"/totalsupply", new_ts);

    let mut response = forward_response(ctx);
    response.alkanes.0.push(AlkaneTransfer {
        id: DIESEL_ID,
        value: value_per_mint,
    });
    Ok((response, path))
}

// ===========================================================================
// DieselEoa: PrecompiledAlkane impl
// ===========================================================================

/// ZST implementing the [`crate::precompile::PrecompiledAlkane`] trait
/// for the DIESEL EOA alkane (2:0). Phase 2 of the abstraction rollout:
/// existing tests (`diesel_sidebyside`, `diesel_shadow`) call into the
/// helpers below via `run_diesel_eoa` and stay byte-equivalent.
///
/// Phase 3 will flip `requires_eoa(77) = true` and
/// `requires_solo_cellpack(77) = true` once shadow-tests for the
/// fall-through behavior land — those will eliminate divergence
/// classes #3 and #4 from the audit by routing problematic calls to
/// wasm where the gas accounting is canonical.
pub struct DieselEoa;

impl crate::precompile::PrecompiledAlkane for DieselEoa {
    const TARGET_ID: AlkaneId = DIESEL_ID;
    type Path = DieselPath;
    type GasTable = DieselPathGas;

    fn handles_opcode(opcode: u128) -> bool {
        matches!(opcode, 77 | 99 | 100 | 101)
    }

    fn is_mint_opcode(opcode: u128) -> bool {
        opcode == 77
    }

    // Phase 2 leaves `requires_eoa` and `requires_solo_cellpack` at
    // their trait defaults (false) so existing tests' caller shapes
    // continue to dispatch through. Phase 3 flips both to true for
    // opcode 77 alongside the proof-tests for cases 3 + 4 of the
    // divergence audit.

    fn execute(
        ctx: &AlkanesRuntimeContext,
        pending: &mut StorageMap,
    ) -> Result<(ExtendedCallResponse, Self::Path)> {
        execute_diesel(ctx, pending)
    }

    fn gas_for_path(path: Self::Path, gas: &Self::GasTable) -> u64 {
        match path {
            DieselPath::CommunistFirstOfBlock => gas.communist_first_of_block,
            DieselPath::CommunistSubsequent => gas.communist_subsequent,
            DieselPath::CommunistRevertOnePerTx => gas.communist_revert_one_per_tx,
            DieselPath::CommunistRevertLegacySeen => gas.communist_revert_legacy_seen,
            DieselPath::LegacyFirstOfBlock => gas.legacy_first_of_block,
            DieselPath::LegacyRevertAlreadyMinted => gas.legacy_revert_already_minted,
            DieselPath::ViewGetName => gas.view_get_name,
            DieselPath::ViewGetSymbol => gas.view_get_symbol,
            DieselPath::ViewGetTotalSupply => gas.view_get_total_supply,
            // NotEoa is a tag for shadow-trace classification; its gas
            // is meaningless (the dispatcher would have fallen through
            // before reaching this path). Returning 0 keeps the match
            // total without introducing a spurious gas-table field.
            DieselPath::NotEoa => 0,
        }
    }
}

impl DieselPath {
    pub fn tag(self) -> &'static str {
        match self {
            DieselPath::CommunistFirstOfBlock => "diesel:c1",
            DieselPath::CommunistSubsequent => "diesel:c2",
            DieselPath::CommunistRevertOnePerTx => "diesel:e1",
            DieselPath::CommunistRevertLegacySeen => "diesel:e2",
            DieselPath::LegacyFirstOfBlock => "diesel:p1",
            DieselPath::LegacyRevertAlreadyMinted => "diesel:p2",
            DieselPath::ViewGetName => "diesel:v99",
            DieselPath::ViewGetSymbol => "diesel:v100",
            DieselPath::ViewGetTotalSupply => "diesel:v101",
            DieselPath::NotEoa => "diesel:not-eoa",
        }
    }
}

#[allow(dead_code)]
fn _unused_silence_imports() {
    let _ = AlkaneTransferParcel::default();
}

// ============================================================================
// Shadow comparison harness — test-only.
// ============================================================================
//
// In shadow mode, run_after_special runs BOTH wasm and precompile for every
// DIESEL call, and records observations here. Production code doesn't touch
// this — it's only for the side-by-side equivalence test, so every item is
// cfg-gated to test/test-utils so the production wasm stays small.

/// Outcome of one shadow comparison.
#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug, Clone)]
pub struct ShadowRecord {
    pub path: Option<DieselPath>,
    pub height: u32,
    pub opcode: u128,
    pub wasm_gas: u64,
    pub precomp_gas: u64,
    pub wasm_response: Option<ExtendedCallResponse>,
    pub precomp_response: Option<ExtendedCallResponse>,
    pub wasm_error: Option<String>,
    pub precomp_error: Option<String>,
    pub diverged: bool,
    pub divergence_reason: Option<String>,
}

#[cfg(any(test, feature = "test-utils"))]
static SHADOW_RECORDS: StdMutex<Vec<ShadowRecord>> = StdMutex::new(Vec::new());
#[cfg(any(test, feature = "test-utils"))]
static SHADOW_ENABLED: StdMutex<bool> = StdMutex::new(false);

#[cfg(any(test, feature = "test-utils"))]
pub fn shadow_enable() {
    *SHADOW_ENABLED.lock().unwrap() = true;
}
#[cfg(any(test, feature = "test-utils"))]
pub fn shadow_disable() {
    *SHADOW_ENABLED.lock().unwrap() = false;
}
#[cfg(any(test, feature = "test-utils"))]
pub fn shadow_is_enabled() -> bool {
    *SHADOW_ENABLED.lock().unwrap()
}
#[cfg(any(test, feature = "test-utils"))]
pub fn shadow_clear() {
    SHADOW_RECORDS.lock().unwrap().clear();
}
#[cfg(any(test, feature = "test-utils"))]
pub fn shadow_snapshot() -> Vec<ShadowRecord> {
    SHADOW_RECORDS.lock().unwrap().clone()
}
#[cfg(any(test, feature = "test-utils"))]
pub fn shadow_push(r: ShadowRecord) {
    SHADOW_RECORDS.lock().unwrap().push(r);
}

/// Compare the wasm result (post-`run_after_special`) against a fresh
/// precompile run for the same context. Records a `ShadowRecord` with the
/// diff, never panics. The wasm result is unchanged.
///
/// `ctx` should be a fresh clone of the context the wasm ran against — the
/// wasm has already mutated its own AlkanesInstance, but the runtime
/// context's `message.atomic` is shared, and the wasm doesn't pipe storage
/// during execution, so reading atomic now reflects the pre-call state.
#[cfg(any(test, feature = "test-utils"))]
pub fn shadow_compare(
    ctx_arc: std::sync::Arc<std::sync::Mutex<AlkanesRuntimeContext>>,
    wasm_result: &Result<(ExtendedCallResponse, u64), anyhow::Error>,
) {
    if !shadow_is_enabled() {
        return;
    }
    let (opcode, height, skip_for_ctx) = {
        let ctx = ctx_arc.lock().unwrap();
        let op = ctx.inputs.first().copied().unwrap_or(0);
        let skip = !matches_precompile_for_ctx(&ctx);
        (op, ctx.message.height as u32, skip)
    };
    if skip_for_ctx {
        return;
    }

    // Build a one-entry table calibrated from the wasm's gas. The precompile
    // returns total_gas = internal_gas_from_table + storage_byte_fuel; for
    // the two totals to match we need:
    //   internal_gas_from_table = wasm_total_gas - storage_fuel(height, wasm_storage_len)
    // We don't know wasm_storage_len on the error path; for reverts the
    // storage map is unapplied so storage_len=0.
    let (wasm_internal_gas, wasm_storage_fuel) = match wasm_result {
        Ok((resp, total)) => {
            let sl = resp.storage.serialize().len() as u64;
            let sf = fuel_per_store_byte(height).saturating_mul(sl);
            (total.saturating_sub(sf), sf)
        }
        Err(_) => (0, 0),
    };
    let mut table = DieselPathGas::default();
    // Populate every field with wasm_internal_gas so the precompile uses
    // the right value regardless of which path it classifies into.
    table.communist_first_of_block = wasm_internal_gas;
    table.communist_subsequent = wasm_internal_gas;
    table.communist_revert_one_per_tx = wasm_internal_gas;
    table.communist_revert_legacy_seen = wasm_internal_gas;
    table.legacy_first_of_block = wasm_internal_gas;
    table.legacy_revert_already_minted = wasm_internal_gas;
    table.view_get_name = wasm_internal_gas;
    table.view_get_symbol = wasm_internal_gas;
    table.view_get_total_supply = wasm_internal_gas;
    let _ = wasm_storage_fuel; // captured for diagnostics
    let precomp_outcome = run_diesel_eoa(ctx_arc, &table);

    let mut record = ShadowRecord {
        path: None,
        height,
        opcode,
        wasm_gas: 0,
        precomp_gas: 0,
        wasm_response: None,
        precomp_response: None,
        wasm_error: None,
        precomp_error: None,
        diverged: false,
        divergence_reason: None,
    };

    match (wasm_result, &precomp_outcome) {
        (Ok((wr, wg)), Ok((pr, pg, path))) => {
            record.path = Some(*path);
            record.wasm_gas = *wg;
            record.precomp_gas = *pg;
            record.wasm_response = Some(wr.clone());
            record.precomp_response = Some(pr.clone());
            if wr.alkanes != pr.alkanes {
                record.diverged = true;
                record.divergence_reason = Some(format!(
                    "alkanes diverged: wasm={:?} precomp={:?}",
                    wr.alkanes, pr.alkanes
                ));
            } else if wr.data != pr.data {
                record.diverged = true;
                record.divergence_reason = Some(format!(
                    "data diverged: wasm={} precomp={}",
                    hex::encode(&wr.data),
                    hex::encode(&pr.data)
                ));
            } else if wr.storage != pr.storage {
                record.diverged = true;
                record.divergence_reason = Some(format!(
                    "storage diverged: wasm_keys={:?} precomp_keys={:?}",
                    wr.storage.0.keys().map(|k| String::from_utf8_lossy(k).to_string()).collect::<Vec<_>>(),
                    pr.storage.0.keys().map(|k| String::from_utf8_lossy(k).to_string()).collect::<Vec<_>>(),
                ));
            } else if *wg != *pg {
                record.diverged = true;
                record.divergence_reason = Some(format!(
                    "gas diverged: wasm={} precomp={}",
                    wg, pg
                ));
            }
        }
        (Err(we), Err(pe)) => {
            record.wasm_error = Some(we.to_string());
            record.precomp_error = Some(pe.to_string());
            // Revert paths must produce the SAME error message because the
            // message ends up in the on-wire revert trace (response.data is
            // built from e.to_string() in message.rs). Strip "ALKANES:
            // revert: " prefixes that the host adds for wasm panics.
            let wasm_msg = strip_revert_prefix(we.to_string());
            let precomp_msg = strip_revert_prefix(pe.to_string());
            if wasm_msg != precomp_msg {
                record.diverged = true;
                record.divergence_reason = Some(format!(
                    "error string diverged: wasm={:?} precomp={:?}",
                    wasm_msg, precomp_msg
                ));
            }
        }
        (Ok((wr, _)), Err(pe)) => {
            record.wasm_response = Some(wr.clone());
            record.precomp_error = Some(pe.to_string());
            record.diverged = true;
            record.divergence_reason =
                Some("wasm succeeded but precompile reverted".to_string());
        }
        (Err(we), Ok((pr, _, _))) => {
            record.wasm_error = Some(we.to_string());
            record.precomp_response = Some(pr.clone());
            record.diverged = true;
            record.divergence_reason =
                Some("wasm reverted but precompile succeeded".to_string());
        }
    }

    shadow_push(record);
}

#[cfg(any(test, feature = "test-utils"))]
fn strip_revert_prefix(s: String) -> String {
    // The wasm SDK macro wraps every Err as `Error: <inner>`, then the host
    // wraps the data again as `ALKANES: revert: Error: <inner>`. The
    // precompile returns just `<inner>`. To compare, strip both layers.
    let s = s
        .strip_prefix("ALKANES: revert: ")
        .map(|s| s.to_string())
        .unwrap_or(s);
    s.strip_prefix("Error: ")
        .map(|s| s.to_string())
        .unwrap_or(s)
}
