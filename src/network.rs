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
    alkanes_std_genesis_alkane_upgraded_regtest_build, fr_btc_build, fr_sigil_build,
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
use protorune::balance_sheet::PersistentRecord;
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
    // networks where V220_FORK_HEIGHT=0), `setup_frbtc` must initialize
    // against the slim binary directly — initializing bulky and then
    // executing slim would leave slim running against an unfamiliar
    // storage layout, and it fuel-exhausts.
    // For mainnet (V220_FORK_HEIGHT=950_000 > 0), this returns bulky to
    // replicate the historical setup at block 880_000.
    if genesis::V220_FORK_HEIGHT == 0 {
        // Genesis-coincident chains (regtest + alt-coins): use the latest
        // slim build straight away. v1.3.1 is storage-init compatible with the
        // v1.2.0/v1.3.0 slim builds these chains already ran against, so this
        // preserves their post-V220 behaviour and matches frbtc_wasm_for_height
        // (FRBTC_V131_FORK_HEIGHT=0 on these chains → v1.3.1 executes).
        fr_btc_build_v1_3_1::get_bytes()
    } else {
        fr_btc_build::get_bytes()
    }
}

/// Height-versioned static frBTC (`32:0`) code map — the "load a precompiled
/// built-in directly from static program storage" path.
///
/// frBTC code is resolved by block height from bytes compiled into this binary,
/// instead of read from indexed state. A binary upgrade that adds a new version
/// + fork height therefore activates on every node at that height without a
/// state migration, and can never be left inert on a rolled pod that already
/// indexed past the fork height.
///
/// Boundaries are `>=` and mirror the historical one-shot byte-swaps that used
/// to rewrite indexed state at these heights (each swap ran *before* tx
/// indexing of its activation block), so the bytes returned here are
/// byte-identical to what execution historically saw at every height.
/// The `else` arm reuses `fr_btc_bytes()`: bulky at mainnet genesis, and the
/// unreachable-on-genesis-coincident-chains fallback (there `FRBTC_V130 == 0`
/// so the first arm always matches).
pub fn frbtc_wasm_for_height(height: u32) -> Vec<u8> {
    if height >= genesis::FRBTC_V131_FORK_HEIGHT {
        // v1.3.1: caches the tweaked P2TR signer script (skips the ~2M
        // tap_tweak per wrap that starved chained wrap+swap under v1.3.0) +
        // unconditional owner-gated set_signer. Mainnet fork @ 960_000.
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

pub fn fr_sigil_bytes() -> Vec<u8> {
    fr_sigil_build::get_bytes()
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

/// Height-versioned static genesis-alkane / DIESEL (`2:0`) code map.
///
/// Boundaries are `>=` and mirror the historical one-shot byte-swaps that used
/// to rewrite indexed state at `GENESIS_UPGRADE_BLOCK_HEIGHT` and
/// `GENESIS_UPGRADE_EOA_BLOCK_HEIGHT` (each ran *before* tx indexing of its
/// activation block), so the bytes returned here are byte-identical to what
/// execution historically saw at every height.
pub fn genesis_alkane_wasm_for_height(height: u32) -> Vec<u8> {
    if height >= genesis::GENESIS_UPGRADE_EOA_BLOCK_HEIGHT {
        genesis_alkane_upgrade_bytes_eoa()
    } else if height >= genesis::GENESIS_UPGRADE_BLOCK_HEIGHT {
        genesis_alkane_upgrade_bytes()
    } else {
        genesis_alkane_bytes()
    }
}

/// The single load path for precompiled built-ins: resolves the alkane's code
/// for `height` from the static in-binary version maps, or `None` when the id
/// is not a precompile. Precompiled code is never read from (or written to)
/// the `/alkanes/` bytecode table.
pub fn precompiled_alkane_wasm_for_height(id: &AlkaneId, height: u32) -> Option<Vec<u8>> {
    if *id == (AlkaneId { block: 2, tx: 0 }) {
        Some(genesis_alkane_wasm_for_height(height))
    } else if *id == (AlkaneId { block: 32, tx: 0 }) {
        Some(frbtc_wasm_for_height(height))
    } else if *id == (AlkaneId { block: 32, tx: 1 }) {
        Some(fr_sigil_bytes())
    } else {
        None
    }
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
    /// v2.2.1-alpha.3 fork: activates fr_btc v1.3.0. Genesis-coincident on
    /// non-mainnet chains, so the static frBTC version map resolves to v1.3.0
    /// from genesis here.
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 0;
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 0;
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
    /// v2.2.1-alpha.3 mainnet fork: activates fr_btc v1.3.0. Future block —
    /// coordinated hard fork; all indexers MUST ship this activation (roll
    /// v2.2.1-alpha.3) before this height or they diverge at 32:0.
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 957_000;
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 960_000;
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
    /// v2.2.1-alpha.3 fork: activates fr_btc v1.3.0. Genesis-coincident on
    /// non-mainnet chains, so the static frBTC version map resolves to v1.3.0
    /// from genesis here.
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 0;
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 0;
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
    /// v2.2.1-alpha.3 fork: activates fr_btc v1.3.0. Genesis-coincident on
    /// non-mainnet chains, so the static frBTC version map resolves to v1.3.0
    /// from genesis here.
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 0;
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 0;
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
    /// v2.2.1-alpha.3 fork: activates fr_btc v1.3.0. Genesis-coincident on
    /// non-mainnet chains, so the static frBTC version map resolves to v1.3.0
    /// from genesis here.
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 0;
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 0;
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
    /// v2.2.1-alpha.3 fork: activates fr_btc v1.3.0. Genesis-coincident on
    /// non-mainnet chains, so the static frBTC version map resolves to v1.3.0
    /// from genesis here.
    pub const FRBTC_V130_FORK_HEIGHT: u32 = 0;
    pub const FRBTC_V131_FORK_HEIGHT: u32 = 0;
}

pub fn is_active(height: u64) -> bool {
    height >= genesis::GENESIS_BLOCK
}

static mut _VIEW: bool = false;

pub fn set_view_mode() {
    unsafe {
        _VIEW = true;
    }
}

pub fn get_view_mode() -> bool {
    unsafe { _VIEW }
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

/// On the `regtest_frsigil` build the 1-unit frSIGIL auth-token premine is
/// deferred out of genesis and re-homed onto the coinbase of this height — a
/// b8-controlled, bitcoin-spendable regtest outpoint (driven from
/// `index_block` via `premine_frsigil`). Stock builds premine at genesis to
/// the fixed `GENESIS_OUTPOINT`, which is a phantom (unspendable) outpoint on
/// regtest, so frSIGIL can never be moved to authorize `set_signer`.
#[cfg(feature = "regtest_frsigil")]
pub const FRSIGIL_PREMINE_HEIGHT: u32 = 1;

pub fn setup_frsigil(block: &Block) -> Result<()> {
    // Byte presence at /alkanes/32:1 is the deployed-detection marker (kept
    // for backwards compatibility with already-synced DBs). Execution never
    // reads these bytes — `get_alkane_binary` resolves `32:1` from the static
    // in-binary code via `precompiled_alkane_wasm_for_height`.
    let mut ptr =
        IndexPointer::from_keyword("/alkanes/").select(&(AlkaneId { block: 32, tx: 1 }).into());
    if ptr.get().len() == 0 {
        ptr.set(Arc::new(compress(fr_sigil_bytes())?));
    } else {
        return Ok(());
    }
    // Stock: premine the frSIGIL auth token to the fixed genesis outpoint
    // immediately. On `regtest_frsigil` the premine is deferred to
    // `FRSIGIL_PREMINE_HEIGHT`'s coinbase (a spendable regtest UTXO b8 owns),
    // done from `index_block` after protorune's own block indexing.
    #[cfg(not(feature = "regtest_frsigil"))]
    {
        let outpoint = OutPoint {
            txid: tx_hex_to_txid(genesis::GENESIS_OUTPOINT)?,
            vout: 0,
        };
        premine_frsigil(block, outpoint)?;
    }
    let _ = block;
    Ok(())
}

/// Mint the single frSIGIL (32:1) auth-token unit and premine it to
/// `outpoint`. Idempotent: a `/frsigil/premined` flag guards double-premine
/// across re-scans. Split out of `setup_frsigil` so the `regtest_frsigil`
/// build can point the premine at a b8-controlled coinbase outpoint.
pub fn premine_frsigil(block: &Block, outpoint: OutPoint) -> Result<()> {
    let mut flag = IndexPointer::from_keyword("/frsigil/premined");
    if flag.get().len() != 0 {
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
    let outpoint_bytes = outpoint_encode(&outpoint)?;
    <AlkaneTransferParcel as TryInto<BalanceSheet<AtomicPointer>>>::try_into(
        response2.alkanes.into(),
    )?
    .save(
        &mut atomic.derive(
            &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
                .OUTPOINT_TO_RUNES
                .select(&outpoint_bytes),
        ),
        false,
    );
    pipe_storagemap_to(
        &response2.storage,
        &mut atomic
            .derive(&IndexPointer::from_keyword("/alkanes/").select(&fr_sigil.clone().into())),
    );
    flag.set_value::<u8>(1);
    atomic.commit();
    Ok(())
}

pub fn setup_frbtc(block: &Block) -> Result<()> {
    // Byte presence at /alkanes/32:0 is the deployed-detection marker (kept
    // for backwards compatibility with already-synced DBs). Execution never
    // reads these bytes — `get_alkane_binary` resolves `32:0` from the static
    // in-binary version map via `precompiled_alkane_wasm_for_height`.
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

pub fn setup_diesel(block: &Block) -> Result<()> {
    // Byte presence at /alkanes/2:0 is the deployed-detection marker (kept
    // for backwards compatibility with already-synced DBs). Execution never
    // reads these bytes — `get_alkane_binary` resolves `2:0` from the static
    // in-binary version map via `precompiled_alkane_wasm_for_height`.
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
    <AlkaneTransferParcel as TryInto<BalanceSheet<AtomicPointer>>>::try_into(
        response.alkanes.into(),
    )?
    .save(
        &mut atomic.derive(
            &RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
                .OUTPOINT_TO_RUNES
                .select(&outpoint_bytes),
        ),
        false,
    );
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
