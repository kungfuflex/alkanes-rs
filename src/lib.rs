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

//! # Alkanes
//!
//! This crate is the host-specific implementation for the Alkanes protocol,
//! designed to run in a WASM environment provided by `metashrew-core`.
//! It implements the `AlkanesHost` and `protorune_support::host::Host` traits.
//!
//! The core application logic is abstracted into the `alkanes-support` and
//! `protorune-support` crates, making it environment-agnostic. This crate
//!, `alkanes`, provides the concrete implementations that bridge the generic
//! logic to the specific capabilities of the WASM host.

use alkanes_proto::alkanes;
use anyhow::{Result};
use bitcoin::{hashes::Hash, Block, OutPoint, TxOut};
#[allow(unused_imports)]
use metashrew_core::{
    flush, input, println,
    stdio::{stdout, Write},
};
#[allow(unused_imports)]
use metashrew_support::block::AuxpowBlock;
use metashrew_support::compat::export_bytes;
#[allow(unused_imports)]
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::{consensus_decode, consume_sized_int, consume_to_end};
use protobuf::Message;
use std::collections::BTreeSet;
use std::io::Cursor;
use std::ops::{Deref, DerefMut};
use metashrew_core::index_pointer::AtomicPointer;

pub mod block;
pub mod etl;
pub mod into_proto;
pub mod message;
pub mod network;
pub mod precompiled;
pub mod tables;
#[cfg(any(test, feature = "test-utils"))]
pub mod tests;
pub mod trace;
pub mod unwrap;
pub mod utils;
pub mod view;
pub mod vm;

use alkanes_support::host::AlkanesHost;
use alkanes_support::view::{Balance, Rune, ViewHost};
use metashrew_core::index_pointer::{IndexPointer};
use protorune_support::host::Host;

#[derive(Clone)]
pub struct WasmHost(pub AtomicPointer);

impl PartialEq for WasmHost {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Default for WasmHost {
    fn default() -> Self {
        Self(AtomicPointer::default())
    }
}

impl Deref for WasmHost {
    type Target = AtomicPointer;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WasmHost {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl ViewHost for WasmHost {
    fn get_outpoints_by_address(&self, _address: &str) -> Result<Vec<OutPoint>> {
        todo!()
    }
    fn get_balances_by_outpoint(
        &self,
        _outpoint: &OutPoint,
        _protocol_tag: u128,
    ) -> Result<Vec<Balance>> {
        todo!()
    }
    fn get_output(&self, _outpoint: &OutPoint) -> Result<TxOut> {
        todo!()
    }
    fn get_height(&self, _outpoint: &OutPoint) -> Result<u32> {
        todo!()
    }
    fn get_txindex(&self, _outpoint: &OutPoint) -> Result<u32> {
        todo!()
    }
    fn get_runes_by_height(&self, _height: u32, _protocol_tag: u128) -> Result<Vec<Rune>> {
        todo!()
    }
    fn sequence_pointer(&self) -> Self::Pointer {
        todo!()
    }
    fn get_alkane_inventory(
        &self,
        _owner_id: &alkanes_support::id::AlkaneId,
    ) -> Result<Vec<alkanes_support::id::AlkaneId>> {
        todo!()
    }
    fn get_balance(
        &self,
        _owner_id: &alkanes_support::id::AlkaneId,
        _alkane_id: &alkanes_support::id::AlkaneId,
    ) -> Result<u128> {
        todo!()
    }
    fn get_alkane_storage_at(
        &self,
        _alkane_id: &alkanes_support::id::AlkaneId,
        _path: &[u8],
    ) -> Result<Vec<u8>> {
        todo!()
    }
    fn get_bytecode_by_alkane_id(
        &self,
        _alkane_id: &alkanes_support::id::AlkaneId,
    ) -> Result<Vec<u8>> {
        todo!()
    }
}

impl Host for WasmHost {
    type Pointer = metashrew_core::index_pointer::IndexPointer;
    fn get(&self, key: &[u8]) -> Result<Vec<u8>> {
        Ok(self.0.get_pointer().select(&key.to_vec()).get().as_ref().clone())
    }
    fn flush(&self) {
        metashrew_core::flush();
    }
    fn println(&self, msg: &str) {
        metashrew_core::println!("{}", msg);
    }
    fn save_balance_sheet(
        &self,
        _outpoint: &OutPoint,
        _balance_sheet: &protorune_support::balance_sheet::BalanceSheet<WasmHost>,
    ) -> Result<()> {
        todo!()
    }
    fn initialized_protocol_index(&self) -> Result<()> {
        todo!()
    }
    fn add_to_indexable_protocols(&self, _protocol_id: u128) -> Result<()> {
        todo!()
    }
    fn index_height_to_block_hash(
        &self,
        _height: u64,
        _block_hash: &bitcoin::BlockHash,
    ) -> Result<()> {
        todo!()
    }
    fn index_transaction_ids(&self, _block: &Block, _height: u64) -> Result<()> {
        todo!()
    }
    fn index_outpoints(&self, _block: &Block, _height: u64) -> Result<()> {
        todo!()
    }
    fn index_spendables(
        &self,
        _transactions: &Vec<bitcoin::Transaction>,
    ) -> Result<BTreeSet<Vec<u8>>> {
        todo!()
    }
    fn clear_balances(&self, _script_pubkey: &[u8]) -> Result<()> {
        todo!()
    }
    fn clear_balances_for_protocol(&self, _script_pubkey: &[u8], _protocol_id: u128) -> Result<()> {
        todo!()
    }
    fn set_rune_id_to_etching(&self, _rune_id: &[u8], _etching: &[u8]) -> Result<()> {
        todo!()
    }
    fn set_etching_to_rune_id(&self, _etching: &[u8], _rune_id: &[u8]) -> Result<()> {
        todo!()
    }
    fn set_rune_id_to_height(&self, _rune_id: &[u8], _height: u64) -> Result<()> {
        todo!()
    }
    fn set_divisibility(&self, _rune_id: &[u8], _divisibility: u128) -> Result<()> {
        todo!()
    }
    fn set_premine(&self, _rune_id: &[u8], _premine: u128) -> Result<()> {
        todo!()
    }
    fn set_amount(&self, _rune_id: &[u8], _amount: u128) -> Result<()> {
        todo!()
    }
    fn set_cap(&self, _rune_id: &[u8], _cap: u128) -> Result<()> {
        todo!()
    }
    fn set_mints_remaining(&self, _rune_id: &[u8], _mints_remaining: u128) -> Result<()> {
        todo!()
    }
    fn set_height_start(&self, _rune_id: &[u8], _height_start: u64) -> Result<()> {
        todo!()
    }
    fn set_height_end(&self, _rune_id: &[u8], _height_end: u64) -> Result<()> {
        todo!()
    }
    fn set_offset_start(&self, _rune_id: &[u8], _offset_start: u64) -> Result<()> {
        todo!()
    }
    fn set_offset_end(&self, _rune_id: &[u8], _offset_end: u64) -> Result<()> {
        todo!()
    }
    fn set_symbol(&self, _rune_id: &[u8], _symbol: u128) -> Result<()> {
        todo!()
    }
    fn set_spacers(&self, _rune_id: &[u8], _spacers: u128) -> Result<()> {
        todo!()
    }
    fn add_etching(&self, _etching: &[u8]) -> Result<()> {
        todo!()
    }
    fn add_rune_to_height(&self, _height: u64, _rune_id: &[u8]) -> Result<()> {
        todo!()
    }
    fn set_storage_auth(&self, _alkane_id: &[u8], _auth: &[u8]) -> Result<()> {
        todo!()
    }
    fn get_etching_from_rune_id(&self, _rune_id: &[u8]) -> Result<Vec<u8>> {
        todo!()
    }
    fn get_spacers(&self, _rune_id: &[u8]) -> Result<u128> {
        todo!()
    }
    fn get_divisibility(&self, _rune_id: &[u8]) -> Result<u128> {
        todo!()
    }
    fn get_symbol(&self, _rune_id: &[u8]) -> Result<u128> {
        todo!()
    }
    fn append_etching(&self, _etching: &[u8]) -> Result<()> {
        todo!()
    }
    fn index_protorune(
        &self,
        _outpoint: &[u8],
        _height: u64,
        _runes: &protorune_support::tables::RuneTable,
    ) -> Result<()> {
        todo!()
    }
    fn is_rune_mintable(
        &self,
        _rune_id: &protorune_support::ProtoruneRuneId,
    ) -> Result<bool> {
        todo!()
    }
    fn get_balance_sheet(
        &self,
        _script_pubkey: &[u8],
    ) -> Result<protorune_support::balance_sheet::BalanceSheet<WasmHost>> {
        todo!()
    }
}

impl AlkanesHost for WasmHost {
    fn index_block(&self, block: &Block, height: u32) -> Result<BTreeSet<Vec<u8>>> {
        let network = if cfg!(feature = "mainnet") {
            alkanes_support::network::Network::Bitcoin
        } else if cfg!(feature = "testnet") {
            alkanes_support::network::Network::Testnet
        } else if cfg!(feature = "regtest") {
            alkanes_support::network::Network::Regtest
        } else if cfg!(feature = "signet") {
            alkanes_support::network::Network::Signet
        } else if cfg!(feature = "luckycoin") {
            alkanes_support::network::Network::Luckycoin
        } else if cfg!(feature = "dogecoin") {
            alkanes_support::network::Network::Dogecoin
        } else if cfg!(feature = "bellscoin") {
            alkanes_support::network::Network::Bellscoin
        } else if cfg!(feature = "fractal") {
            alkanes_support::network::Network::Fractal
        } else {
            alkanes_support::network::Network::Regtest
        };
        alkanes_support::index_block::<_, crate::message::AlkaneMessageContext>(
            self, block, height, network,
        )
    }
}

/*
All the #[no_mangle] configs will fail during github action cargo test step
due to duplicate symbol:
  rust-lld: error: duplicate symbol: runesbyheight
  >>> defined in /home/runner/work/alkanes-rs/alkanes-rs/target/wasm32-unknown-unknown/debug/deps/alkanes-5b647d16704125c9.alkanes.7a19fa39330b2460-cgu.05.rcgu.o
  >>> defined in /home/runner/work/alkanes-rs/alkanes-rs/target/wasm32-unknown-unknown/debug/deps/libalkanes.rlib(alkanes.alkanes.2dae95da706e3a8c-cgu.09.rcgu.o)

This is because both
[lib]
crate-type = ["cdylib", "rlib"]

are defined in Cargo.toml since we want to build both the wasm and rust library.

Running cargo test will compile an additional test harness binary that:
Links libalkanes.rlib
Compiles #[no_mangle] functions again into the test binary
Then links everything together, leading to duplicate symbols

Thus, going to add not(test) to all these functions
*/

#[cfg(not(test))]
#[no_mangle]
pub fn runesbyaddress() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: alkanes::WalletResponse =
        crate::view::protorunes_by_address(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| alkanes::WalletResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn unwrap() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let height = consume_sized_int::<u32>(&mut data).unwrap();
    export_bytes(crate::unwrap::view(height.into()).unwrap().write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn runesbyoutpoint() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: alkanes::OutpointResponse =
        crate::view::protorunes_by_outpoint(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| alkanes::OutpointResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn spendablesbyaddress() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: alkanes::WalletResponse =
        crate::view::protorunes_by_address(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| alkanes::WalletResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn protorunesbyaddress() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let input_data = consume_to_end(&mut data).unwrap();
    //  let _request = protorune_support::proto::protorune::ProtorunesWalletRequest::parse_from_bytes(&input_data).unwrap();

    let mut result: alkanes::WalletResponse =
        crate::view::protorunes_by_address(&input_data)
            .unwrap_or_else(|_| alkanes::WalletResponse::new());

    result.outpoints = result
        .outpoints
        .into_iter()
        .filter_map(|v| {
            if v.balances.len() == 0 {
                None
            } else {
                Some(v)
            }
        })
        .collect::<Vec<alkanes::OutpointResponse>>();

    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn getblock() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let input_data = consume_to_end(&mut data).unwrap();
    export_bytes(crate::view::getblock(&input_data).unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn protorunesbyheight() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: alkanes::RunesResponse =
        crate::view::protorunes_by_height(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| alkanes::RunesResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn alkanes_id_to_outpoint() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    // first 4 bytes come in as height, not used
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let data_vec = consume_to_end(&mut data).unwrap();
    let result: alkanes::AlkaneIdToOutpointResponse =
        crate::view::alkanes_id_to_outpoint(&data_vec).unwrap_or_else(|err| {
            eprintln!("Error in alkanes_id_to_outpoint: {:?}", err);
            alkanes::AlkaneIdToOutpointResponse::new()
        });
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn traceblock() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let height = consume_sized_int::<u32>(&mut data).unwrap();
    export_bytes(crate::view::traceblock(height).unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn trace() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let outpoint_proto =
        alkanes::Outpoint::parse_from_bytes(&consume_to_end(&mut data).unwrap()).unwrap();
    let outpoint: OutPoint = OutPoint {
        txid: bitcoin::Txid::from_slice(&outpoint_proto.txid).unwrap(),
        vout: outpoint_proto.vout,
    };
    export_bytes(crate::view::trace(&outpoint).unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn getbytecode() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let host = WasmHost::default();
    export_bytes(
        crate::view::getbytecode(&host, &consume_to_end(&mut data).unwrap()).unwrap_or_default(),
    )
}

#[cfg(not(test))]
#[no_mangle]
pub fn protorunesbyoutpoint() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: alkanes::OutpointResponse =
        crate::view::protorunes_by_outpoint(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| alkanes::OutpointResponse::new());

    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn runesbyheight() -> i32 {
    let mut data: Cursor<Vec<u8>> = Cursor::new(input());
    let _height = consume_sized_int::<u32>(&mut data).unwrap();
    let result: alkanes::RunesResponse =
        crate::view::protorunes_by_height(&consume_to_end(&mut data).unwrap())
            .unwrap_or_else(|_| alkanes::RunesResponse::new());
    export_bytes(result.write_to_bytes().unwrap())
}

// TODO: this function needs to improve the way it stores all alkane ids, it doesn't handle duplicates right now
#[cfg(not(test))]
#[no_mangle]
pub fn getinventory() -> i32 {
    let data = input();
    let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let host = WasmHost::default();
    let result = crate::view::getinventory(
        &host,
        &alkanes::AlkaneInventoryRequest::parse_from_bytes(reader).unwrap(),
    )
    .unwrap();
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(not(test))]
#[no_mangle]
pub fn getstorageat() -> i32 {
    let data = input();
    let _height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let host = WasmHost::default();
    let result = crate::view::getstorageat(
        &host,
        &alkanes::AlkaneStorageRequest::parse_from_bytes(reader).unwrap(),
    )
    .unwrap();
    export_bytes(result.write_to_bytes().unwrap())
}

#[cfg(all(target_arch = "wasm32", not(test)))]
#[no_mangle]
pub fn _start() {
    let data = input();
    let height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    #[cfg(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin"))]
    let block: Block = AuxpowBlock::parse(&mut Cursor::<Vec<u8>>::new(reader.to_vec()))
        .unwrap()
        .to_consensus();
    #[cfg(not(any(feature = "dogecoin", feature = "luckycoin", feature = "bellscoin")))]
    let block: Block =
        consensus_decode::<Block>(&mut Cursor::<Vec<u8>>::new(reader.to_vec())).unwrap();

    let host = WasmHost::default();
    host.index_block(&block, height).unwrap();
    etl::index_extensions(height, &block);
    <WasmHost as Host>::flush(&host);
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::message::AlkaneMessageContext;
    use alkanes_proto::alkanes::{RunesByHeightRequest, Uint128, WalletRequest};
    use protobuf::{Message, SpecialFields};
    use protorune_support::view::{
        rune_outpoint_to_outpoint_response, runes_by_address, runes_by_height,
    };
    use std::fs;
    use std::path::PathBuf;

    #[test]
    pub fn test_decode_block() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("src/tests/static/849236.txt");
        let block_data = fs::read(&path).unwrap();

        assert!(block_data.len() > 0);

        let data = block_data;
        let height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
        let reader = &data[4..];
        let block: Block =
            consensus_decode::<Block>(&mut Cursor::<Vec<u8>>::new(reader.to_vec())).unwrap();
        assert!(height == 849236);

        let host = WasmHost::default();
        host.index_block(&block, height).unwrap();

        let req_height: Vec<u8> = (RunesByHeightRequest {
            height: 849236,
            ..Default::default()
        })
        .write_to_bytes()
        .unwrap();
        let runes = runes_by_height(&req_height).unwrap();
        assert!(runes.runes.len() == 2);

        // TODO: figure out what address to use for runesbyaddress
        let req_wallet: Vec<u8> = (WalletRequest {
            wallet: String::from("bc1pfs5dhzwk32xa53cjx8fx4dqy7hm4m6tys8zyvemqffz8ua4tytqs8vjdgr")
                .as_bytes()
                .to_vec(),
            ..Default::default()
        })
        .write_to_bytes()
        .unwrap();

        let runes_for_addr = runes_by_address(&req_wallet).unwrap();
        // assert!(runes_for_addr.balances > 0);
        std::println!("RUNES by addr: {:?}", runes_for_addr);

        let outpoint_res = rune_outpoint_to_outpoint_response(
            &WasmHost::default(),
            &(OutPoint {
                txid: block.txdata[298].compute_txid(),
                vout: 2,
            }),
        )
        .unwrap();
        let balances = protorune_support::proto::protorune::Balances::parse_from_bytes(&outpoint_res.balances).unwrap();
        let quorum_rune = balances.entries[0].clone();
        let balance = quorum_rune.balance.0.unwrap();
        let mut expected_balance = Uint128::new();
        expected_balance.lo = 21000000;
        assert!(*balance == expected_balance);
        // TODO: Assert rune
        std::println!(" with rune {:?}", quorum_rune.rune.0);

        // assert!(false);
    }
}
