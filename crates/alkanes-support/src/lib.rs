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

//! # Alkanes Support
//!
//! This crate provides the environment-agnostic core logic for the Alkanes
//! protocol. It is designed to be used by a host environment, such as the
//! `alkanes` crate, which provides the concrete implementations for the
//! `AlkanesHost` and `protorune_support::host::Host` traits.

pub mod alkanes_log;
pub mod cellpack;
pub mod constants;
pub mod context;
pub mod continuation;
pub mod gz;
pub mod host;
pub mod envelope;
pub mod id;
pub mod message;
pub mod network;
pub mod parcel;
pub mod proto;
pub use proto::alkanes;
pub mod response;
pub mod rune_result;
pub mod rune_transfer;
pub mod storage;
pub mod trace;
pub mod transaction;
pub mod utils;
pub mod witness;
pub mod view;

use anyhow::Result;
use bitcoin::blockdata::block::Block;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
#[allow(unused_imports)]
use metashrew_support::index_pointer::KeyValuePointer;
use crate::network::Network;
use protorune_support::network::set_network;
use protorune_support::Protorune;
use std::collections::BTreeSet;

/*
 * Chadson's Journal:
 *
 * I'm updating this file to align with the recent refactoring.
 * - The `configure_network` function is updated to use the new `to_bitcoin_network`
 *   method, which correctly converts the internal `Network` enum to the
 *   `bitcoin::Network` enum expected by `protorune_support`.
 * - The call to `Protorune::new()` has been removed, as the `Protorune` struct
 *   is now a zero-sized type and doesn't require instantiation. The `index_block`
 *   method is now called directly as a static method on the `Protorune` type.
 * - I've also removed an unused import of `AtomicPointer`.
 */
pub fn configure_network(network: Network) {
    set_network(network.to_bitcoin_network());
}

use crate::host::AlkanesHost;

pub fn index_block<
    H: AlkanesHost + Clone + Default,
    T: protorune_support::message::MessageContext<H>,
>(
    host: &H,
    block: &Block,
    height: u32,
    network: Network,
) -> Result<BTreeSet<Vec<u8>>> {
    configure_network(network);
    let updated_addresses =
        Protorune::<H>::index_block::<T>(host, block.clone(), height.into())?;
    Ok(updated_addresses)
}

