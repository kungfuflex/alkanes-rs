use crate::message::AlkaneMessageContext;
#[allow(unused_imports)]
use crate::precompiled::{
    alkanes_std_genesis_alkane_dogecoin_build, alkanes_std_genesis_alkane_fractal_build,
    alkanes_std_genesis_alkane_luckycoin_build, alkanes_std_genesis_alkane_mainnet_build,
    alkanes_std_genesis_alkane_regtest_build,
};
use crate::utils::pipe_storagemap_to;
use crate::view::simulate_parcel;
use crate::vm::utils::sequence_pointer;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::gz::compress;
use alkanes_support::id::AlkaneId;
use alkanes_support::parcel::AlkaneTransferParcel;
use anyhow::Result;
use bitcoin::hashes::Hash;
use bitcoin::{Block, OutPoint, Transaction, Txid};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::balance_sheet::PersistentRecord;
use protorune::message::{MessageContext, MessageContextParcel};
#[allow(unused_imports)]
use protorune::tables::{RuneTable, RUNES};
use protorune_support::balance_sheet::BalanceSheet;
use protorune_support::utils::outpoint_encode;
use std::sync::Arc;

#[allow(unused_imports)]
use {
    metashrew_core::{println, stdio::stdout},
    std::fmt::Write,
};

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
}

#[cfg(feature = "mainnet")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 880_000;
    pub const GENESIS_OUTPOINT: &str =
        "3977b30a97c9b9d609afb4b7cc138e17b21d1e0c5e360d25debf1441de933bf4";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 872_101;
}

#[cfg(feature = "fractal")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 400_000;
    pub const GENESIS_OUTPOINT: &str =
        "cf2b52ffaaf1c094df22f190b888fb0e474fe62990547a34e144ec9f8e135b07";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 228_194;
}

#[cfg(feature = "dogecoin")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 6_000_000;
    pub const GENESIS_OUTPOINT: &str =
        "cf2b52ffaaf1c094df22f190b888fb0e474fe62990547a34e144ec9f8e135b07";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 872_101;
}

#[cfg(feature = "luckycoin")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 400_000;
    pub const GENESIS_OUTPOINT: &str =
        "cf2b52ffaaf1c094df22f190b888fb0e474fe62990547a34e144ec9f8e135b07";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 872_101;
}

#[cfg(feature = "bellscoin")]
pub mod genesis {
    pub const GENESIS_BLOCK: u64 = 500_000;
    pub const GENESIS_OUTPOINT: &str =
        "2c58484a86e117a445c547d8f3acb56b569f7ea036637d909224d52a5b990259";
    pub const GENESIS_OUTPOINT_BLOCK_HEIGHT: u64 = 288_906;
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
    println!("has_not_seen_genesis: {}", has_not_seen_genesis);
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

pub fn genesis(block: &Block) -> Result<()> {
    IndexPointer::from_keyword("/alkanes/")
        .select(&(AlkaneId { block: 2, tx: 0 }).into())
        .set(Arc::new(compress(genesis_alkane_bytes())?));
    let mut atomic: AtomicPointer = AtomicPointer::default();
    sequence_pointer(&atomic).set_value::<u128>(1);
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
        txid: Txid::from_byte_array(
            <Vec<u8> as AsRef<[u8]>>::as_ref(
                &hex::decode(genesis::GENESIS_OUTPOINT)?
                    .iter()
                    .cloned()
                    .rev()
                    .collect::<Vec<u8>>(),
            )
            .try_into()?,
        ),
        vout: 0,
    })?;
    <AlkaneTransferParcel as Into<BalanceSheet<AtomicPointer>>>::into(response.alkanes.into())
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
