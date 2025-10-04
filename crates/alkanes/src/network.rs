use crate::message::AlkaneMessageContext;
#[allow(unused_imports)]
use crate::precompiled::{
    alkanes_std_genesis_alkane_dogecoin_build, alkanes_std_genesis_alkane_fractal_build,
    alkanes_std_genesis_alkane_luckycoin_build, alkanes_std_genesis_alkane_mainnet_build,
    alkanes_std_genesis_alkane_regtest_build, alkanes_std_genesis_alkane_upgraded_mainnet_build,
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
use metashrew_support::environment::RuntimeEnvironment;

pub const DIESEL_ID: AlkaneId = AlkaneId { block: 2, tx: 0 };
pub const FRBTC_ID: AlkaneId = AlkaneId { block: 32, tx: 0 };
pub const FRSIGIL_ID: AlkaneId = AlkaneId { block: 32, tx: 1 };
use metashrew_support::index_pointer::{AtomicPointer, IndexPointer, KeyValuePointer};

use protorune::balance_sheet::PersistentRecord;
use protorune::message::{MessageContext, MessageContextParcel};
#[allow(unused_imports)]
use protorune::tables::{RuneTable};
use protorune_support::balance_sheet::BalanceSheet;
use protorune_support::utils::{outpoint_encode, tx_hex_to_txid};
use std::marker::PhantomData;
use std::sync::Arc;


pub fn fr_btc_bytes() -> Vec<u8> {
    fr_btc_build::get_bytes()
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
}

#[cfg(feature = "mainnet")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 880_000;
    pub const GENESIS_OUTPOINT: &str =
        "3977b30a97c9b9d609afb4b7cc138e17b21d1e0c5e360d25debf1441de933bf4";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 872_101;
    pub const GENESIS_UPGRADE_BLOCK_HEIGHT: u32 = 908_888;
}

#[cfg(feature = "fractal")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 400_000;
    pub const GENESIS_OUTPOINT: &str =
        "cf2b52ffaaf1c094df22f190b888fb0e474fe62990547a34e144ec9f8e135b07";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 228_194;
    pub const GENESIS_UPGRADE_BLOCK_HEIGHT: u32 = 759_865;
}

#[cfg(feature = "dogecoin")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 6_000_000;
    pub const GENESIS_OUTPOINT: &str =
        "cf2b52ffaaf1c094df22f190b888fb0e474fe62990547a34e144ec9f8e135b07";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 872_101;
    pub const GENESIS_UPGRADE_BLOCK_HEIGHT: u32 = 5_730_675;
}

#[cfg(feature = "luckycoin")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 400_000;
    pub const GENESIS_OUTPOINT: &str =
        "cf2b52ffaaf1c094df22f190b888fb0e474fe62990547a34e144ec9f8e135b07";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 872_101;
    pub const GENESIS_UPGRADE_BLOCK_HEIGHT: u32 = 1_664_317;
}

#[cfg(feature = "bellscoin")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 500_000;
    pub const GENESIS_OUTPOINT: &str =
        "2c58484a86e117a445c547d8f3acb56b569f7ea036637d909224d52a5b990259";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 288_906;
    pub const GENESIS_UPGRADE_BLOCK_HEIGHT: u32 = 533_970;
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

pub fn is_genesis<E: RuntimeEnvironment + Clone + Default>(env: &mut E, height: u64) -> bool {
    let mut init_ptr = IndexPointer::<E>::from_keyword("/seen-genesis");
    let has_not_seen_genesis = init_ptr.get(env).len() == 0;
    let is_genesis = if has_not_seen_genesis {
        get_view_mode() || height >= genesis::GENESIS_BLOCK
    } else {
        false
    };
    if is_genesis {
        init_ptr.set_value(env, 0x01_u8);
    }
    is_genesis
}

pub fn setup_frsigil<E: RuntimeEnvironment + Clone + Default + 'static>(
    env: &mut E,
    block: &Block,
) -> Result<BalanceSheet<E, AtomicPointer<E>>> {
    let mut atomic: AtomicPointer<E> = AtomicPointer::default();
    let fr_sigil = FRSIGIL_ID;

    let parcel3 = MessageContextParcel::<E> {
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
        sheets: Box::<BalanceSheet<E, AtomicPointer<E>>>::new(BalanceSheet::default()),
        txindex: 0,
        vout: 0,
        runtime_balances: Box::<BalanceSheet<E, AtomicPointer<E>>>::new(BalanceSheet::default()),
        _phantom: PhantomData,
    };
    let (response2, _gas_used2) = (match simulate_parcel(env, &parcel3, u64::MAX) {
        Ok((a, b)) => Ok((a, b)),
        Err(e) => {
            env.log(&format!("{:?}", e));
            Err(e)
        }
    })?;
    pipe_storagemap_to(
        &response2.storage,
        &mut atomic
            .derive(&IndexPointer::from_keyword("/alkanes/").select(&fr_sigil.clone().into())),
        env,
    );
    atomic.commit(env);
    let result = <AlkaneTransferParcel as TryInto<BalanceSheet<E, AtomicPointer<E>>>>::try_into(
        response2.alkanes.into(),
    );
    env.log(&format!("setup_frsigil result: {:?}", result));
    result
}

pub fn setup_frbtc<E: RuntimeEnvironment + Clone + Default + 'static>(
    env: &mut E,
    block: &Block,
) -> Result<BalanceSheet<E, AtomicPointer<E>>> {
    let mut atomic: AtomicPointer<E> = AtomicPointer::default();
    let fr_btc = FRBTC_ID;
    let parcel2 = MessageContextParcel::<E> {
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
        sheets: Box::<BalanceSheet<E, AtomicPointer<E>>>::new(BalanceSheet::default()),
        txindex: 0,
        vout: 0,
        runtime_balances: Box::<BalanceSheet<E, AtomicPointer<E>>>::new(BalanceSheet::default()),
        _phantom: PhantomData,
    };
    let (response3, _gas_used3) = (match simulate_parcel(env, &parcel2, u64::MAX) {
        Ok((a, b)) => Ok((a, b)),
        Err(e) => {
            env.log(&format!("{:?}", e));
            Err(e)
        }
    })?;
    pipe_storagemap_to(
        &response3.storage,
        &mut atomic.derive(&IndexPointer::from_keyword("/alkanes/").select(&fr_btc.clone().into())),
        env,
    );
    atomic.commit(env);
    let result = <AlkaneTransferParcel as TryInto<BalanceSheet<E, AtomicPointer<E>>>>::try_into(
        response3.alkanes.into(),
    );
    env.log(&format!("setup_frbtc result: {:?}", result));
    result
}

pub fn check_and_upgrade_diesel<E: RuntimeEnvironment + Clone + Default + 'static>(
    env: &mut E,
    height: u32,
) -> Result<()> {
    if height >= genesis::GENESIS_UPGRADE_BLOCK_HEIGHT {
        let mut upgrade_ptr = IndexPointer::<E>::from_keyword("/genesis-upgraded");
        if upgrade_ptr.get(env).len() == 0 {
            upgrade_ptr.set_value(env, 0x01_u8);
            IndexPointer::<E>::from_keyword("/alkanes/")
                .select(&(AlkaneId { block: 2, tx: 0 }).into())
                .set(env, Arc::new(compress(genesis_alkane_upgrade_bytes())?));
        }
    }
    Ok(())
}

pub fn setup_diesel<E: RuntimeEnvironment + Clone + Default + 'static>(
    env: &mut E,
    block: &Block,
) -> Result<BalanceSheet<E, AtomicPointer<E>>> {
    let mut atomic: AtomicPointer<E> = AtomicPointer::default();
    let myself = DIESEL_ID;
    let parcel = MessageContextParcel::<E> {
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
        sheets: Box::<BalanceSheet<E, AtomicPointer<E>>>::new(BalanceSheet::default()),
        txindex: 0,
        vout: 0,
        runtime_balances: Box::<BalanceSheet<E, AtomicPointer<E>>>::new(BalanceSheet::default()),
        _phantom: PhantomData,
    };
    let (response, _gas_used) = (match simulate_parcel(env, &parcel, u64::MAX) {
        Ok((a, b)) => Ok((a, b)),
        Err(e) => {
            env.log(&format!("{:?}", e));
            Err(e)
        }
    })?;
    pipe_storagemap_to(
        &response.storage,
        &mut atomic.derive(&IndexPointer::from_keyword("/alkanes/").select(&myself.clone().into())),
        env,
    );
    atomic.commit(env);
    let result = <AlkaneTransferParcel as TryInto<BalanceSheet<E, AtomicPointer<E>>>>::try_into(
        response.alkanes.into(),
    );
    env.log(&format!("setup_diesel result: {:?}", result));
    result
}

pub fn genesis<E: RuntimeEnvironment + Clone + Default + 'static>(env: &mut E) -> Result<()> {
    let mut atomic: AtomicPointer<E> = AtomicPointer::default();
    sequence_pointer(&atomic).set_value(env, 1_u128);
    IndexPointer::<E>::from_keyword("/alkanes/")
        .select(&DIESEL_ID.into())
        .set(env, Arc::new(compress(genesis_alkane_bytes())?));
    IndexPointer::<E>::from_keyword("/alkanes/")
        .select(&FRBTC_ID.into())
        .set(env, Arc::new(compress(fr_btc_bytes())?));
    IndexPointer::<E>::from_keyword("/alkanes/")
        .select(&FRSIGIL_ID.into())
        .set(env, Arc::new(compress(fr_sigil_bytes())?));
    let outpoint_bytes = outpoint_encode(&OutPoint {
        txid: tx_hex_to_txid(genesis::GENESIS_OUTPOINT)?,
        vout: 0,
    })?;
    atomic
        .derive(&RuneTable::<E>::new().OUTPOINT_TO_HEIGHT.select(&outpoint_bytes))
        .set_value(env, genesis::GENESIS_OUTPOINT_BLOCK_HEIGHT);
    atomic
        .derive(
            &RuneTable::<E>::new()
                .HEIGHT_TO_TRANSACTION_IDS
                .select_value::<u64>(genesis::GENESIS_OUTPOINT_BLOCK_HEIGHT),
        )
        .append(
            env,
            Arc::new(
                hex::decode(genesis::GENESIS_OUTPOINT)?
                    .iter()
                    .cloned()
                    .rev()
                    .collect::<Vec<u8>>(),
            ),
        );
    atomic.commit(env);
    Ok(())
}