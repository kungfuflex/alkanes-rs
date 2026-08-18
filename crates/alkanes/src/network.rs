use crate::message::AlkaneMessageContext;
use crate::precompiled::fr_btc_build_v1_1_0;
use crate::precompiled::fr_btc_build_v1_2_0;
use crate::precompiled::fr_btc_build_v1_3_0;
use crate::precompiled::fr_btc_build_v1_3_1;
#[allow(unused_imports)]
use crate::precompiled::{
    alkanes_std_genesis_alkane_dogecoin_build, alkanes_std_genesis_alkane_fractal_build,
    alkanes_std_genesis_alkane_luckycoin_build, alkanes_std_genesis_alkane_mainnet_build,
    alkanes_std_genesis_alkane_regtest_build,
    alkanes_std_genesis_alkane_upgraded_eoa_mainnet_build,
    alkanes_std_genesis_alkane_upgraded_eoa_regtest_build,
    alkanes_std_genesis_alkane_upgraded_mainnet_build,
    alkanes_std_genesis_alkane_upgraded_regtest_build, dsigil_build, firevector_build,
    fr_btc_build, fr_sigil_build,
};
use crate::utils::pipe_storagemap_to;
use crate::view::simulate_parcel;
use crate::vm::utils::sequence_pointer;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::gz::compress;
use alkanes_support::id::AlkaneId;
use alkanes_support::parcel::AlkaneTransferParcel;
use anyhow::Result;
use bitcoin::{Block, OutPoint, Transaction};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::balance_sheet::{save_chunked, save_chunked_merging, PersistentRecord};
use protorune::message::{MessageContext, MessageContextParcel};
#[allow(unused_imports)]
use protorune::tables::{RuneTable, RUNES};
use protorune_support::balance_sheet::BalanceSheet;
use protorune_support::utils::{outpoint_encode, tx_hex_to_txid};
use std::sync::Arc;

#[allow(unused_imports)]
use {
    metashrew_core::{println, stdio::stdout},
    std::fmt::Write,
};

pub fn fr_btc_bytes() -> Vec<u8> {
    // On chains where V220 is active from genesis (regtest + alt-coin
    // networks where V220_FORK_HEIGHT=0), `setup_frbtc` must deploy and
    // initialize the slim binary directly. If we deploy bulky here and
    // later swap to slim via `check_and_upgrade_precompiled`, the swap
    // overwrites bytes but preserves bulky's init storage — slim then
    // executes against an unfamiliar storage layout and fuel-exhausts.
    // For mainnet (V220_FORK_HEIGHT=950_000 > 0), this returns bulky to
    // replicate the historical setup at block 880_000.
    if genesis::V220_FORK_HEIGHT == 0 {
        // Genesis-coincident chains (regtest + alt-coins): deploy the latest
        // slim build straight away. v1.3.1 (signer-script cache) is storage-init
        // compatible with the v1.2.0/v1.3.0 slim builds these chains ran against.
        fr_btc_build_v1_3_1::get_bytes()
    } else {
        fr_btc_build::get_bytes()
    }
}

/// Height-versioned static frBTC (`32:0`) code map — the "load a precompiled
/// built-in directly from static program storage" path. frBTC code is resolved
/// by block height from bytes compiled into this binary, not read from indexed
/// state, so a binary upgrade activates a new version at its fork height on
/// every node without a state migration (and can never be left inert on a
/// rolled node whose one-shot `check_and_upgrade_precompiled` swap already
/// fired). Boundaries are `>=` and MUST mirror those swaps (which run before tx
/// indexing), so the bytes here are byte-identical to indexed state for every
/// height < FRBTC_V130_FORK_HEIGHT — no divergence on the pre-fork range.
pub fn frbtc_wasm_for_height(height: u32) -> Vec<u8> {
    if height >= genesis::FRBTC_V131_FORK_HEIGHT {
        fr_btc_build_v1_3_1::get_bytes()
    } else if height >= genesis::FRBTC_V130_FORK_HEIGHT {
        fr_btc_build_v1_3_0::get_bytes()
    } else if height >= genesis::V220_FORK_HEIGHT {
        fr_btc_build_v1_2_0::get_bytes()
    } else if height >= genesis::GENESIS_UPGRADE_EOA_BLOCK_HEIGHT {
        fr_btc_build_v1_1_0::get_bytes()
    } else {
        fr_btc_bytes()
    }
}

/// Height-versioned static DIESEL genesis-alkane (`2:0`) code map — the same
/// "load a precompiled built-in from static program storage by height" path as
/// [`frbtc_wasm_for_height`]. The genesis alkane progresses base → upgraded → EOA
/// at `GENESIS_UPGRADE_BLOCK_HEIGHT` / `GENESIS_UPGRADE_EOA_BLOCK_HEIGHT`; the `>=`
/// boundaries mirror the one-shot swaps in [`check_and_upgrade_precompiled`], so
/// these bytes are byte-identical to indexed state for every pre-fork height.
///
/// Restores the rc.3 execution-path map that the develop refactor dropped from
/// [`crate::vm::utils::get_alkane_binary_from_context`] (which special-cased only
/// frBTC `32:0`). Without it, `2:0` resolved to the indexed binary — seeded to the
/// heavier upgraded-EOA build — and executed upgraded-EOA from genesis, over-
/// consuming the tight pre-`FUEL_CHANGE1_HEIGHT` (899_087) fuel budget so that
/// legitimate free-mint constructors reverted "all fuel consumed by WebAssembly",
/// rolled back the atomic pointer, and never incremented the sequence — silently
/// failing to create ~all `2:N` free-mint alkanes below block 908_888 vs rc.3.
pub fn genesis_alkane_wasm_for_height(height: u32) -> Vec<u8> {
    if height >= genesis::GENESIS_UPGRADE_EOA_BLOCK_HEIGHT {
        genesis_alkane_upgrade_bytes_eoa()
    } else if height >= genesis::GENESIS_UPGRADE_BLOCK_HEIGHT {
        genesis_alkane_upgrade_bytes()
    } else {
        genesis_alkane_bytes()
    }
}

pub fn fr_sigil_bytes() -> Vec<u8> {
    fr_sigil_build::get_bytes()
}

/// The FIREVECTOR VM contract, deployed at 12:0.
pub fn firevector_bytes() -> Vec<u8> {
    firevector_build::get_bytes()
}

/// DSIGIL, the governance capability, deployed at 12:1.
pub fn dsigil_bytes() -> Vec<u8> {
    dsigil_build::get_bytes()
}

#[cfg(feature = "mainnet")]
pub fn genesis_alkane_bytes() -> Vec<u8> {
    alkanes_std_genesis_alkane_mainnet_build::get_bytes()
}

//use if regtest
#[cfg(all(
    not(feature = "mainnet"),
    not(feature = "dogecoin"),
    not(feature = "bellscoin"),
    not(feature = "fractal"),
    not(feature = "luckycoin")
))]
pub fn genesis_alkane_bytes() -> Vec<u8> {
    alkanes_std_genesis_alkane_regtest_build::get_bytes()
}

#[cfg(feature = "dogecoin")]
pub fn genesis_alkane_bytes() -> Vec<u8> {
    alkanes_std_genesis_alkane_dogecoin_build::get_bytes()
}

#[cfg(feature = "bellscoin")]
pub fn genesis_alkane_bytes() -> Vec<u8> {
    alkanes_std_genesis_alkane_dogecoin_build::get_bytes()
}

#[cfg(feature = "fractal")]
pub fn genesis_alkane_bytes() -> Vec<u8> {
    alkanes_std_genesis_alkane_fractal_build::get_bytes()
}

#[cfg(feature = "luckycoin")]
pub fn genesis_alkane_bytes() -> Vec<u8> {
    alkanes_std_genesis_alkane_luckycoin_build::get_bytes()
}

#[cfg(feature = "mainnet")]
pub fn genesis_alkane_upgrade_bytes() -> Vec<u8> {
    alkanes_std_genesis_alkane_upgraded_mainnet_build::get_bytes()
}

//use if regtest
#[cfg(all(
    not(feature = "mainnet"),
    not(feature = "dogecoin"),
    not(feature = "bellscoin"),
    not(feature = "fractal"),
    not(feature = "luckycoin")
))]
pub fn genesis_alkane_upgrade_bytes() -> Vec<u8> {
    alkanes_std_genesis_alkane_upgraded_regtest_build::get_bytes()
}

#[cfg(feature = "mainnet")]
pub fn genesis_alkane_upgrade_bytes_eoa() -> Vec<u8> {
    alkanes_std_genesis_alkane_upgraded_eoa_mainnet_build::get_bytes()
}

//use if regtest
#[cfg(all(
    not(feature = "mainnet"),
    not(feature = "dogecoin"),
    not(feature = "bellscoin"),
    not(feature = "fractal"),
    not(feature = "luckycoin")
))]
pub fn genesis_alkane_upgrade_bytes_eoa() -> Vec<u8> {
    alkanes_std_genesis_alkane_upgraded_eoa_regtest_build::get_bytes()
}

//use if regtest
#[cfg(all(
    not(feature = "mainnet"),
    not(feature = "dogecoin"),
    not(feature = "bellscoin"),
    not(feature = "fractal"),
    not(feature = "luckycoin")
))]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 0;
    pub const GENESIS_OUTPOINT: &str =
        "3977b30a97c9b9d609afb4b7cc138e17b21d1e0c5e360d25debf1441de933bf4";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 0;
    pub const GENESIS_UPGRADE_BLOCK_HEIGHT: u32 = 0;
    pub const GENESIS_UPGRADE_EOA_BLOCK_HEIGHT: u32 = 0;
    /// v2.2.0 fork: activates the slim fr_btc.wasm precompile + the
    /// extcall-child-revert containment fix (ports of kungfuflex/v2.1.8 +
    /// kungfuflex/v2.1.8-slim-frbtc). On regtest the fork is genesis-coincident
    /// so all tests run against the post-fork behaviour by default.
    pub const V220_FORK_HEIGHT: u32 = 0;
    /// v2.2.1 fork: activates fr_btc v1.3.0. Genesis-coincident on non-mainnet
    /// chains, so the static frBTC version map resolves to v1.3.0 from genesis.
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 0;
    /// v2.2.1 fork: activates fr_btc v1.3.1. Genesis-coincident on non-mainnet
    /// chains, so the static frBTC version map resolves to v1.3.1 from genesis.
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 0;
    /// v3 fork: removes the `max_virtual_vout` protostone cap. Genesis-coincident
    /// on non-mainnet chains so tests exercise the post-fork (uncapped) behaviour
    /// by default.
    pub const MAX_VIRTUAL_VOUT_REMOVAL_HEIGHT: u64 = 0;
    /// FIREVECTOR: `800000000:2` starts returning a weightmap-derived
    /// denominator instead of the raw mint count. Genesis-coincident off mainnet
    /// so tests exercise the post-fork path by default; with no weightmap
    /// configured the returned value is identical either way.
    pub const FIREVECTOR_FORK_HEIGHT: u32 = 0;
    /// Height at which 12:0 and 12:1 are deployed. Must be <= the fork height:
    /// the code and the seeded identity vector have to exist before the
    /// denominator changes meaning.
    pub const FIREVECTOR_DEPLOY_HEIGHT: u32 = 0;
}

#[cfg(feature = "mainnet")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 880_000;
    pub const GENESIS_OUTPOINT: &str =
        "3977b30a97c9b9d609afb4b7cc138e17b21d1e0c5e360d25debf1441de933bf4";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 872_101;
    pub const GENESIS_UPGRADE_BLOCK_HEIGHT: u32 = 908_888;

    pub const GENESIS_UPGRADE_EOA_BLOCK_HEIGHT: u32 = 917_888;
    /// v2.2.0 mainnet fork: slim fr_btc.wasm + extcall revert containment.
    pub const V220_FORK_HEIGHT: u32 = 950_000;
    /// v2.2.1 mainnet fork: activates fr_btc v1.3.0. Future block — coordinated
    /// hard fork; must match the deployed rc.1 activation (kept at 957_000).
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 957_000;
    /// v2.2.1 mainnet fork: activates fr_btc v1.3.1 (signer-script cache; warm
    /// wrap <300k fuel). Future block > FRBTC_V130 — coordinated hard fork,
    /// matches the deployed rc.3 activation.
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 960_000;
    /// FIREVECTOR activation: the height at which `800000000:2` switches from
    /// returning the raw DIESEL mint count to returning a weightmap-derived
    /// denominator.
    ///
    /// DELIBERATELY UNSCHEDULED (`u32::MAX`). Picking this block is a governance
    /// decision, and it must not be set until (a) the weightmap slot on 2:0 is
    /// seeded with the identity vector, and (b) indexers have upgraded — an
    /// un-upgraded indexer diverges from this height on, which is what a fork
    /// gate is for.
    ///
    /// Note the activation itself is payout- and fuel-neutral: with the identity
    /// vector loaded, `W / w_i == N`, and the DIESEL contract is not modified at
    /// all, so its metered instruction count and the 16-byte reply size are
    /// unchanged.
    pub const FIREVECTOR_FORK_HEIGHT: u32 = u32::MAX;
    /// Height at which 12:0 (the VM) and 12:1 (DSIGIL) are deployed.
    ///
    /// Separate from, and necessarily earlier than, FIREVECTOR_FORK_HEIGHT: the
    /// contracts and the seeded identity vector must exist before the
    /// denominator changes meaning. Deploying is itself a state change — it
    /// writes two contracts and mints the single DSIGIL onto the genesis
    /// outpoint — so it is gated rather than unconditional, and is likewise
    /// UNSCHEDULED until governance picks a block.
    pub const FIREVECTOR_DEPLOY_HEIGHT: u32 = u32::MAX;
    /// v3 fork: removes the `max_virtual_vout = num_outputs + 100` protostone
    /// cap in `protorune::protostone::process_message`. That cap is an artifact
    /// of the old 80-byte OP_RETURN standardness limit and has no protocol
    /// purpose — VM work is already bounded by fuel. Below this height the cap
    /// still applies (history is byte-identical, and assets stranded by it are
    /// recoverable from the `8:dead` recycle bin); at/after it there is no
    /// bound on protostone count. Coordinated hard fork — every indexer must
    /// agree on this height.
    pub const MAX_VIRTUAL_VOUT_REMOVAL_HEIGHT: u64 = 975_000;
}

#[cfg(feature = "fractal")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 400_000;
    pub const GENESIS_OUTPOINT: &str =
        "cf2b52ffaaf1c094df22f190b888fb0e474fe62990547a34e144ec9f8e135b07";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 228_194;
    pub const GENESIS_UPGRADE_BLOCK_HEIGHT: u32 = 228_194;
    pub const GENESIS_UPGRADE_EOA_BLOCK_HEIGHT: u32 = 228_194;
    pub const V220_FORK_HEIGHT: u32 = 0;
    /// v2.2.1 fork: activates fr_btc v1.3.0. Genesis-coincident on non-mainnet
    /// chains, so the static frBTC version map resolves to v1.3.0 from genesis.
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 0;
    /// v2.2.1 fork: activates fr_btc v1.3.1. Genesis-coincident on non-mainnet
    /// chains, so the static frBTC version map resolves to v1.3.1 from genesis.
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 0;
    /// v3 fork: removes the `max_virtual_vout` protostone cap. Genesis-coincident
    /// on non-mainnet chains so tests exercise the post-fork (uncapped) behaviour
    /// by default.
    pub const MAX_VIRTUAL_VOUT_REMOVAL_HEIGHT: u64 = 0;
    /// FIREVECTOR: `800000000:2` starts returning a weightmap-derived
    /// denominator instead of the raw mint count. Genesis-coincident off mainnet
    /// so tests exercise the post-fork path by default; with no weightmap
    /// configured the returned value is identical either way.
    pub const FIREVECTOR_FORK_HEIGHT: u32 = 0;
    /// Height at which 12:0 and 12:1 are deployed. Must be <= the fork height:
    /// the code and the seeded identity vector have to exist before the
    /// denominator changes meaning.
    pub const FIREVECTOR_DEPLOY_HEIGHT: u32 = 0;
}

#[cfg(feature = "dogecoin")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 6_000_000;
    pub const GENESIS_OUTPOINT: &str =
        "cf2b52ffaaf1c094df22f190b888fb0e474fe62990547a34e144ec9f8e135b07";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 872_101;
    pub const GENESIS_UPGRADE_BLOCK_HEIGHT: u32 = 872_101;
    pub const GENESIS_UPGRADE_EOA_BLOCK_HEIGHT: u32 = 872_101;
    pub const V220_FORK_HEIGHT: u32 = 0;
    /// v2.2.1 fork: activates fr_btc v1.3.0. Genesis-coincident on non-mainnet
    /// chains, so the static frBTC version map resolves to v1.3.0 from genesis.
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 0;
    /// v2.2.1 fork: activates fr_btc v1.3.1. Genesis-coincident on non-mainnet
    /// chains, so the static frBTC version map resolves to v1.3.1 from genesis.
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 0;
    /// v3 fork: removes the `max_virtual_vout` protostone cap. Genesis-coincident
    /// on non-mainnet chains so tests exercise the post-fork (uncapped) behaviour
    /// by default.
    pub const MAX_VIRTUAL_VOUT_REMOVAL_HEIGHT: u64 = 0;
    /// FIREVECTOR: `800000000:2` starts returning a weightmap-derived
    /// denominator instead of the raw mint count. Genesis-coincident off mainnet
    /// so tests exercise the post-fork path by default; with no weightmap
    /// configured the returned value is identical either way.
    pub const FIREVECTOR_FORK_HEIGHT: u32 = 0;
    /// Height at which 12:0 and 12:1 are deployed. Must be <= the fork height:
    /// the code and the seeded identity vector have to exist before the
    /// denominator changes meaning.
    pub const FIREVECTOR_DEPLOY_HEIGHT: u32 = 0;
}

#[cfg(feature = "luckycoin")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 400_000;
    pub const GENESIS_OUTPOINT: &str =
        "cf2b52ffaaf1c094df22f190b888fb0e474fe62990547a34e144ec9f8e135b07";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 872_101;
    pub const GENESIS_UPGRADE_BLOCK_HEIGHT: u32 = 872_101;
    pub const GENESIS_UPGRADE_EOA_BLOCK_HEIGHT: u32 = 872_101;
    pub const V220_FORK_HEIGHT: u32 = 0;
    /// v2.2.1 fork: activates fr_btc v1.3.0. Genesis-coincident on non-mainnet
    /// chains, so the static frBTC version map resolves to v1.3.0 from genesis.
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 0;
    /// v2.2.1 fork: activates fr_btc v1.3.1. Genesis-coincident on non-mainnet
    /// chains, so the static frBTC version map resolves to v1.3.1 from genesis.
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 0;
    /// v3 fork: removes the `max_virtual_vout` protostone cap. Genesis-coincident
    /// on non-mainnet chains so tests exercise the post-fork (uncapped) behaviour
    /// by default.
    pub const MAX_VIRTUAL_VOUT_REMOVAL_HEIGHT: u64 = 0;
    /// FIREVECTOR: `800000000:2` starts returning a weightmap-derived
    /// denominator instead of the raw mint count. Genesis-coincident off mainnet
    /// so tests exercise the post-fork path by default; with no weightmap
    /// configured the returned value is identical either way.
    pub const FIREVECTOR_FORK_HEIGHT: u32 = 0;
    /// Height at which 12:0 and 12:1 are deployed. Must be <= the fork height:
    /// the code and the seeded identity vector have to exist before the
    /// denominator changes meaning.
    pub const FIREVECTOR_DEPLOY_HEIGHT: u32 = 0;
}

#[cfg(feature = "bellscoin")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 500_000;
    pub const GENESIS_OUTPOINT: &str =
        "2c58484a86e117a445c547d8f3acb56b569f7ea036637d909224d52a5b990259";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 288_906;
    pub const GENESIS_UPGRADE_BLOCK_HEIGHT: u32 = 288_906;
    pub const GENESIS_UPGRADE_EOA_BLOCK_HEIGHT: u32 = 288_906;
    pub const V220_FORK_HEIGHT: u32 = 0;
    /// v2.2.1 fork: activates fr_btc v1.3.0. Genesis-coincident on non-mainnet
    /// chains, so the static frBTC version map resolves to v1.3.0 from genesis.
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 0;
    /// v2.2.1 fork: activates fr_btc v1.3.1. Genesis-coincident on non-mainnet
    /// chains, so the static frBTC version map resolves to v1.3.1 from genesis.
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 0;
    /// v3 fork: removes the `max_virtual_vout` protostone cap. Genesis-coincident
    /// on non-mainnet chains so tests exercise the post-fork (uncapped) behaviour
    /// by default.
    pub const MAX_VIRTUAL_VOUT_REMOVAL_HEIGHT: u64 = 0;
    /// FIREVECTOR: `800000000:2` starts returning a weightmap-derived
    /// denominator instead of the raw mint count. Genesis-coincident off mainnet
    /// so tests exercise the post-fork path by default; with no weightmap
    /// configured the returned value is identical either way.
    pub const FIREVECTOR_FORK_HEIGHT: u32 = 0;
    /// Height at which 12:0 and 12:1 are deployed. Must be <= the fork height:
    /// the code and the seeded identity vector have to exist before the
    /// denominator changes meaning.
    pub const FIREVECTOR_DEPLOY_HEIGHT: u32 = 0;
}

pub fn is_active(height: u64) -> bool {
    height >= genesis::GENESIS_BLOCK
}

use std::sync::atomic::{AtomicBool, Ordering};

/// Thread-safe view mode flag.
/// When true, allows genesis checks to pass during view/query operations.
static VIEW_MODE: AtomicBool = AtomicBool::new(false);

/// Sets view mode to true. Called before view/query operations.
pub fn set_view_mode() {
    VIEW_MODE.store(true, Ordering::SeqCst);
}

/// Clears view mode. Should be called at the start of block indexing
/// to ensure deterministic behavior.
pub fn clear_view_mode() {
    VIEW_MODE.store(false, Ordering::SeqCst);
}

/// Gets the current view mode.
pub fn get_view_mode() -> bool {
    VIEW_MODE.load(Ordering::SeqCst)
}

pub fn is_genesis(height: u64) -> bool {
    let mut init_ptr = IndexPointer::from_keyword("/seen-genesis");
    let has_not_seen_genesis = init_ptr.get().len() == 0;
    let is_genesis = if has_not_seen_genesis {
        get_view_mode() || height >= genesis::GENESIS_BLOCK
    } else {
        false
    };
    if is_genesis {
        init_ptr.set_value::<u8>(0x01);
    }
    is_genesis
}

/// Deploy the FIREVECTOR system: the VM at 12:0 and DSIGIL at 12:1.
///
/// Two things worth being explicit about, because both are load-bearing:
///
/// **12:0 never has units.** It is a contract, not a token. Its `initialize`
/// mints nothing and pushes no transfer of itself, so no quantity of "12:0" can
/// ever exist to be held, traded, or required. It is addressable so wallets can
/// `simulate` and anyone can `disassemble` the active policy — not because it is
/// an asset.
///
/// **12:1 is the whole of the authority.** Exactly one unit, minted here and
/// never again, and it carries no `authenticate` method — so the auth-token
/// inflation bug (a callback that returns two tokens where one went in) is
/// unreachable rather than merely unused. `set_weightmap` checks possession
/// directly.
///
/// The unit lands on the genesis outpoint, merged into whatever is already
/// there. Note the merge rather than overwrite: `setup_diesel` wrote the DIESEL
/// premine to that outpoint first, and a plain chunked write would drop it —
/// which is exactly what happened once before and snowballed into supply drift.
pub fn setup_firevector(block: &Block, height: u64) -> Result<()> {
    if height < genesis::FIREVECTOR_DEPLOY_HEIGHT as u64 {
        return Ok(());
    }
    let firevector = AlkaneId { block: 12, tx: 0 };
    let dsigil = AlkaneId { block: 12, tx: 1 };

    let mut vm_ptr = IndexPointer::from_keyword("/alkanes/").select(&firevector.clone().into());
    if vm_ptr.get().len() != 0 {
        return Ok(());
    }
    vm_ptr.set(Arc::new(compress(firevector_bytes())?));

    let mut sigil_ptr = IndexPointer::from_keyword("/alkanes/").select(&dsigil.clone().into());
    sigil_ptr.set(Arc::new(compress(dsigil_bytes())?));

    let mut atomic: AtomicPointer = AtomicPointer::default();
    let empty_tx = Transaction {
        version: bitcoin::blockdata::transaction::Version::ONE,
        input: vec![],
        output: vec![],
        lock_time: bitcoin::absolute::LockTime::ZERO,
    };

    // DSIGIL first: it mints the single unit that authorises the VM's setter.
    let sigil_parcel = MessageContextParcel {
        atomic: atomic.derive(&IndexPointer::default()),
        runes: vec![],
        transaction: empty_tx.clone(),
        block: block.clone(),
        height: genesis::GENESIS_BLOCK,
        pointer: 0,
        refund_pointer: 0,
        calldata: (Cellpack {
            target: dsigil.clone(),
            inputs: vec![0],
        })
        .encipher(),
        sheets: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
        txindex: 0,
        vout: 0,
        runtime_balances: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
    };
    let (sigil_response, _) = simulate_parcel(&sigil_parcel, u64::MAX)?;

    // Then the VM, told which alkane holds authority over its weightmap. The
    // sigil id is set here and is not settable afterwards — a mutable setter
    // would leave a window in which anyone could claim the capability.
    let vm_parcel = MessageContextParcel {
        atomic: atomic.derive(&IndexPointer::default()),
        runes: vec![],
        transaction: empty_tx,
        block: block.clone(),
        height: genesis::GENESIS_BLOCK,
        pointer: 0,
        refund_pointer: 0,
        calldata: (Cellpack {
            target: firevector.clone(),
            inputs: vec![0, dsigil.block, dsigil.tx],
        })
        .encipher(),
        sheets: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
        txindex: 0,
        vout: 0,
        runtime_balances: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
    };
    let (vm_response, _) = simulate_parcel(&vm_parcel, u64::MAX)?;

    // The VM is a contract, not a token: it must not have handed anything back.
    if !vm_response.alkanes.0.is_empty() {
        return Err(anyhow::anyhow!(
            "firevector 12:0 must never mint units; got {} transfers",
            vm_response.alkanes.0.len()
        ));
    }

    // Merge the DSIGIL unit into the genesis outpoint's existing chunk. MERGE,
    // not overwrite: `setup_diesel` wrote the DIESEL premine to that outpoint
    // first, and a plain chunked write drops it — which happened once and
    // snowballed into supply drift.
    let outpoint_bytes = outpoint_encode(&OutPoint {
        txid: tx_hex_to_txid(genesis::GENESIS_OUTPOINT)?,
        vout: 0,
    })?;
    let sheet: BalanceSheet<AtomicPointer> =
        <AlkaneTransferParcel as TryInto<BalanceSheet<AtomicPointer>>>::try_into(
            sigil_response.alkanes.into(),
        )?;
    save_chunked_merging(
        &sheet,
        &mut atomic.derive(
            &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
                .OUTPOINT_TO_RUNES
                .select(&outpoint_bytes),
        ),
        false,
    )?;

    // Persist both contracts' storage: DSIGIL's supply, and the VM's sigil id
    // plus its seeded identity weightmap. Without this the initialize writes are
    // discarded and the VM comes up with no policy and no authority.
    pipe_storagemap_to(
        &sigil_response.storage,
        &mut atomic.derive(&IndexPointer::from_keyword("/alkanes/").select(&dsigil.clone().into())),
    );
    pipe_storagemap_to(
        &vm_response.storage,
        &mut atomic
            .derive(&IndexPointer::from_keyword("/alkanes/").select(&firevector.clone().into())),
    );

    atomic.commit();
    Ok(())
}

pub fn setup_frsigil(block: &Block) -> Result<()> {
    let mut ptr =
        IndexPointer::from_keyword("/alkanes/").select(&(AlkaneId { block: 32, tx: 1 }).into());
    if ptr.get().len() == 0 {
        ptr.set(Arc::new(compress(fr_sigil_bytes())?));
    } else {
        return Ok(());
    }
    let mut atomic: AtomicPointer = AtomicPointer::default();
    let fr_sigil = AlkaneId { block: 32, tx: 1 };

    let parcel3 = MessageContextParcel {
        atomic: atomic.derive(&IndexPointer::default()),
        runes: vec![],
        transaction: Transaction {
            version: bitcoin::blockdata::transaction::Version::ONE,
            input: vec![],
            output: vec![],
            lock_time: bitcoin::absolute::LockTime::ZERO,
        },
        block: block.clone(),
        height: genesis::GENESIS_BLOCK,
        pointer: 0,
        refund_pointer: 0,
        calldata: (Cellpack {
            target: fr_sigil.clone(),
            inputs: vec![0, 1],
        })
        .encipher(),
        sheets: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
        txindex: 0,
        vout: 0,
        runtime_balances: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
    };
    let (response2, _gas_used2) = (match simulate_parcel(&parcel3, u64::MAX) {
        Ok((a, b)) => Ok((a, b)),
        Err(e) => {
            println!("{:?}", e);
            Err(e)
        }
    })?;
    let outpoint_bytes = outpoint_encode(&OutPoint {
        txid: tx_hex_to_txid(genesis::GENESIS_OUTPOINT)?,
        vout: 0,
    })?;
    // v3 chunked-outpoint write: MERGE into the genesis outpoint's
    // chunk. `setup_diesel` ran first and wrote the 44T DIESEL
    // premine there; if we used plain `save_chunked` we'd overwrite
    // that with just the frSIGIL entry — which is exactly what was
    // happening pre-fix and caused the v10 pod to lose the DIESEL
    // premine on the genesis outpoint, snowballing into ~33 DIESEL
    // supply drift by h=922k (see bisect at h=913,043 upgrade tx).
    let sheet: BalanceSheet<AtomicPointer> =
        <AlkaneTransferParcel as TryInto<BalanceSheet<AtomicPointer>>>::try_into(
            response2.alkanes.into(),
        )?;
    save_chunked_merging(
        &sheet,
        &mut atomic.derive(
            &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
                .OUTPOINT_TO_RUNES
                .select(&outpoint_bytes),
        ),
        false,
    )?;
    pipe_storagemap_to(
        &response2.storage,
        &mut atomic
            .derive(&IndexPointer::from_keyword("/alkanes/").select(&fr_sigil.clone().into())),
    );
    atomic.commit();
    Ok(())
}

pub fn setup_frbtc(block: &Block) -> Result<()> {
    let mut ptr =
        IndexPointer::from_keyword("/alkanes/").select(&(AlkaneId { block: 32, tx: 0 }).into());
    if ptr.get().len() == 0 {
        ptr.set(Arc::new(compress(fr_btc_bytes())?));
    } else {
        return Ok(());
    }
    let mut atomic: AtomicPointer = AtomicPointer::default();
    let fr_btc = AlkaneId { block: 32, tx: 0 };
    let parcel2 = MessageContextParcel {
        atomic: atomic.derive(&IndexPointer::default()),
        runes: vec![],
        transaction: Transaction {
            version: bitcoin::blockdata::transaction::Version::ONE,
            input: vec![],
            output: vec![],
            lock_time: bitcoin::absolute::LockTime::ZERO,
        },
        block: block.clone(),
        height: genesis::GENESIS_BLOCK,
        pointer: 0,
        refund_pointer: 0,
        calldata: (Cellpack {
            target: fr_btc.clone(),
            inputs: vec![0],
        })
        .encipher(),
        sheets: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
        txindex: 0,
        vout: 0,
        runtime_balances: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
    };
    let (response3, _gas_used3) = (match simulate_parcel(&parcel2, u64::MAX) {
        Ok((a, b)) => Ok((a, b)),
        Err(e) => {
            println!("{:?}", e);
            Err(e)
        }
    })?;
    pipe_storagemap_to(
        &response3.storage,
        &mut atomic.derive(&IndexPointer::from_keyword("/alkanes/").select(&fr_btc.clone().into())),
    );
    atomic.commit();
    Ok(())
}

pub fn check_and_upgrade_precompiled(height: u32) -> Result<()> {
    if height >= genesis::GENESIS_UPGRADE_BLOCK_HEIGHT {
        let mut upgrade_ptr = IndexPointer::from_keyword("/genesis-upgraded");
        if upgrade_ptr.get().len() == 0 {
            upgrade_ptr.set_value::<u8>(0x01);
            IndexPointer::from_keyword("/alkanes/")
                .select(&(AlkaneId { block: 2, tx: 0 }).into())
                .set(Arc::new(compress(genesis_alkane_upgrade_bytes())?));
        }
    }
    if height >= genesis::GENESIS_UPGRADE_EOA_BLOCK_HEIGHT {
        let mut eoa_upgrade_ptr = IndexPointer::from_keyword("/genesis-upgraded-eoa");
        if eoa_upgrade_ptr.get().len() == 0 {
            eoa_upgrade_ptr.set_value::<u8>(0x01);
            IndexPointer::from_keyword("/alkanes/")
                .select(&(AlkaneId { block: 2, tx: 0 }).into())
                .set(Arc::new(compress(genesis_alkane_upgrade_bytes_eoa())?));
            // Only swap fr_btc to bulky v1.1.0 when V220 has not yet
            // activated. On chains where V220 is genesis-coincident
            // (V220_FORK_HEIGHT=0), `setup_frbtc` already deployed slim and
            // overwriting it here would leak bulky storage semantics back in.
            if genesis::V220_FORK_HEIGHT > genesis::GENESIS_UPGRADE_EOA_BLOCK_HEIGHT {
                IndexPointer::from_keyword("/alkanes/")
                    .select(&(AlkaneId { block: 32, tx: 0 }).into())
                    .set(Arc::new(compress(fr_btc_build_v1_1_0::get_bytes())?));
            }
        }
    }
    // v2.2.0 fork: replace the fr_btc precompile bytes (alkane id 32:0) with
    // the slim 281 KB build. The v1.1.0 (1.5 MB) bytes deployed at
    // GENESIS_UPGRADE_EOA_BLOCK_HEIGHT remain canonical pre-fork; nodes
    // re-syncing from before that height still walk through the same
    // historical wasm blobs in order. Behaviour is interface-compatible —
    // tap_tweak was restored in fr_btc_v1.2.0 so the slim build is a drop-in
    // at the precompile boundary.
    if height >= genesis::V220_FORK_HEIGHT {
        let mut v220_upgrade_ptr = IndexPointer::from_keyword("/genesis-upgraded-v220");
        if v220_upgrade_ptr.get().len() == 0 {
            v220_upgrade_ptr.set_value::<u8>(0x01);
            // When V220 is genesis-coincident, `setup_frbtc` already deployed
            // slim with the correct init storage; skip the byte-swap and just
            // record the upgrade in the pointer. For real fork heights
            // (V220_FORK_HEIGHT>0), perform the swap as normal — it replaces
            // the bulky v1.1.0 bytes with slim, leaving the historical
            // bulky-derived storage in place (slim is interface-compatible).
            if genesis::V220_FORK_HEIGHT > 0 {
                IndexPointer::from_keyword("/alkanes/")
                    .select(&(AlkaneId { block: 32, tx: 0 }).into())
                    .set(Arc::new(compress(fr_btc_build_v1_2_0::get_bytes())?));
            }
        }
    }
    // v2.2.1 fork: activate fr_btc v1.3.0. Keeps indexed state (read by
    // `getbytecode`) in lock-step with the static `frbtc_wasm_for_height`
    // execution map — both resolve `32:0` to v1.3.0 at/after this height.
    // Genesis-coincident chains (V220==0) already deployed v1.3.0 via
    // `setup_frbtc`, so record-only.
    if height >= genesis::FRBTC_V130_FORK_HEIGHT {
        let mut v130_upgrade_ptr = IndexPointer::from_keyword("/genesis-upgraded-frbtc-v130");
        if v130_upgrade_ptr.get().len() == 0 {
            v130_upgrade_ptr.set_value::<u8>(0x01);
            if genesis::V220_FORK_HEIGHT > 0 {
                IndexPointer::from_keyword("/alkanes/")
                    .select(&(AlkaneId { block: 32, tx: 0 }).into())
                    .set(Arc::new(compress(fr_btc_build_v1_3_0::get_bytes())?));
            }
        }
    }
    // v2.2.1 fork: activate fr_btc v1.3.1 (signer-script cache — warm wrap
    // <300k fuel). Keeps indexed state in lock-step with the static
    // `frbtc_wasm_for_height` map — both resolve `32:0` to v1.3.1 at/after
    // this height. Genesis-coincident chains (V220==0) already deployed v1.3.1
    // via `setup_frbtc`, so record-only.
    if height >= genesis::FRBTC_V131_FORK_HEIGHT {
        let mut v131_upgrade_ptr = IndexPointer::from_keyword("/genesis-upgraded-frbtc-v131");
        if v131_upgrade_ptr.get().len() == 0 {
            v131_upgrade_ptr.set_value::<u8>(0x01);
            if genesis::V220_FORK_HEIGHT > 0 {
                IndexPointer::from_keyword("/alkanes/")
                    .select(&(AlkaneId { block: 32, tx: 0 }).into())
                    .set(Arc::new(compress(fr_btc_build_v1_3_1::get_bytes())?));
            }
        }
    }
    Ok(())
}

pub fn setup_diesel(block: &Block) -> Result<()> {
    let mut ptr =
        IndexPointer::from_keyword("/alkanes/").select(&(AlkaneId { block: 2, tx: 0 }).into());
    if ptr.get().len() == 0 {
        ptr.set(Arc::new(compress(genesis_alkane_bytes())?));
    } else {
        return Ok(());
    }
    let mut atomic: AtomicPointer = AtomicPointer::default();
    let myself = AlkaneId { block: 2, tx: 0 };
    let parcel = MessageContextParcel {
        atomic: atomic.derive(&IndexPointer::default()),
        runes: vec![],
        transaction: Transaction {
            version: bitcoin::blockdata::transaction::Version::ONE,
            input: vec![],
            output: vec![],
            lock_time: bitcoin::absolute::LockTime::ZERO,
        },
        block: block.clone(),
        height: genesis::GENESIS_BLOCK,
        pointer: 0,
        refund_pointer: 0,
        calldata: (Cellpack {
            target: myself.clone(),
            inputs: vec![0],
        })
        .encipher(),
        sheets: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
        txindex: 0,
        vout: 0,
        runtime_balances: Box::<BalanceSheet<AtomicPointer>>::new(BalanceSheet::default()),
    };
    let (response, _gas_used) = (match simulate_parcel(&parcel, u64::MAX) {
        Ok((a, b)) => Ok((a, b)),
        Err(e) => {
            println!("{:?}", e);
            Err(e)
        }
    })?;
    let outpoint_bytes = outpoint_encode(&OutPoint {
        txid: tx_hex_to_txid(genesis::GENESIS_OUTPOINT)?,
        vout: 0,
    })?;
    // v3 chunked-outpoint write: MERGE into the genesis outpoint's
    // chunk so subsequent `setup_*` calls in the same block (notably
    // `setup_frsigil` for the frSIGIL precompile) don't wipe the
    // DIESEL premine. See `save_chunked_merging` for the rationale +
    // the upstream incident this fix prevents.
    let sheet: BalanceSheet<AtomicPointer> =
        <AlkaneTransferParcel as TryInto<BalanceSheet<AtomicPointer>>>::try_into(
            response.alkanes.into(),
        )?;
    save_chunked_merging(
        &sheet,
        &mut atomic.derive(
            &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
                .OUTPOINT_TO_RUNES
                .select(&outpoint_bytes),
        ),
        false,
    )?;
    pipe_storagemap_to(
        &response.storage,
        &mut atomic.derive(&IndexPointer::from_keyword("/alkanes/").select(&myself.clone().into())),
    );
    atomic.commit();
    Ok(())
}

pub fn genesis() -> Result<()> {
    let mut atomic: AtomicPointer = AtomicPointer::default();
    sequence_pointer(&atomic).set_value::<u128>(1);
    let outpoint_bytes = outpoint_encode(&OutPoint {
        txid: tx_hex_to_txid(genesis::GENESIS_OUTPOINT)?,
        vout: 0,
    })?;
    atomic
        .derive(&RUNES.OUTPOINT_TO_HEIGHT.select(&outpoint_bytes))
        .set_value(genesis::GENESIS_OUTPOINT_BLOCK_HEIGHT);
    atomic
        .derive(
            &RUNES
                .HEIGHT_TO_TRANSACTION_IDS
                .select_value::<u64>(genesis::GENESIS_OUTPOINT_BLOCK_HEIGHT),
        )
        .append(Arc::new(
            hex::decode(genesis::GENESIS_OUTPOINT)?
                .iter()
                .cloned()
                .rev()
                .collect::<Vec<u8>>(),
        ));
    atomic.commit();
    Ok(())
}
