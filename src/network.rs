// Copyright 2024-present, Fractal Industries, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Network Configuration and Genesis
//!
//! This module handles network-specific configurations, including the definition
//! of genesis block details for various Bitcoin-like chains (mainnet, testnet, etc.).
//! It also provides the `genesis` function, which initializes the protocol's state
//! at the designated genesis block.

use crate::message::AlkaneMessageContext;
#[allow(unused_imports)]
use crate::precompiled::{
    alkanes_std_genesis_alkane_dogecoin_build, alkanes_std_genesis_alkane_fractal_build,
    alkanes_std_genesis_alkane_luckycoin_build, alkanes_std_genesis_alkane_mainnet_build,
    alkanes_std_genesis_alkane_regtest_build, alkanes_std_genesis_alkane_upgraded_mainnet_build,
    alkanes_std_genesis_alkane_upgraded_regtest_build, fr_btc_build, fr_sigil_build,
};
use crate::vm::utils::sequence_pointer;
use alkanes_support::gz::compress;
use crate::WasmHost;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::Block;
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune_support::message::MessageContext;
#[allow(unused_imports)]
use protorune_support::tables::{RuneTable, RUNES};
use std::sync::Arc;

#[allow(unused_imports)]
use {
    metashrew_core::{println, stdio::stdout},
    std::fmt::Write,
};

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

pub fn genesis(_block: &Block) -> Result<()> {
    IndexPointer::from_keyword("/alkanes/")
        .select(&(AlkaneId { block: 2, tx: 0 }).into())
        .set(Arc::new(compress(genesis_alkane_bytes())?));
    IndexPointer::from_keyword("/alkanes/")
        .select(&(AlkaneId { block: 32, tx: 1 }).into())
        .set(Arc::new(compress(fr_sigil_bytes())?));
    IndexPointer::from_keyword("/alkanes/")
        .select(&(AlkaneId { block: 32, tx: 0 }).into())
        .set(Arc::new(compress(fr_btc_bytes())?));
    let mut atomic: AtomicPointer = AtomicPointer::default();
    sequence_pointer(&atomic).set_value::<u128>(1);

    let host = WasmHost::default();

    // Manually trigger the genesis for the Alkanes protocol itself.
    // This is a simplified, special-case indexing since it's the first one.
    let _ = protorune_support::Protorune::<WasmHost>::index_transaction_ids(
        _block,
        genesis::GENESIS_BLOCK,
    );

    atomic.commit();
    Ok(())
}

pub fn get_network() -> alkanes_support::network::Network {
    if cfg!(feature = "mainnet") {
        alkanes_support::network::Network::Bitcoin
    } else if cfg!(feature = "testnet") {
        alkanes_support::network::Network::Testnet
    } else if cfg!(feature = "dogecoin") {
        alkanes_support::network::Network::Dogecoin
    } else if cfg!(feature = "bellscoin") {
        alkanes_support::network::Network::Bellscoin
    } else if cfg!(feature = "fractal") {
        alkanes_support::network::Network::Fractal
    } else if cfg!(feature = "luckycoin") {
        alkanes_support::network::Network::Luckycoin
    } else {
        alkanes_support::network::Network::Regtest
    }
}
