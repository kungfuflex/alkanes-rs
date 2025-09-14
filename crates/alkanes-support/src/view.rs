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

//! # Alkanes Support View Functions
//!
//! This module provides generic, environment-agnostic view functions for the
//! Alkanes protocol. These functions are designed to be used by a host
//! environment, which provides the concrete implementations for the `ViewHost`
//! trait.

use crate::host::AlkanesHost;
use anyhow::Result;
use crate::id::AlkaneId;
use bitcoin::{OutPoint, TxOut};
use protorune_support::ProtoruneRuneId;

/// A trait that defines the necessary functionality for a host environment to
/// support the Alkanes view functions.
pub trait ViewHost: AlkanesHost {
    fn get_outpoints_by_address(&self, address: &str) -> Result<Vec<OutPoint>>;
    fn get_balances_by_outpoint(
        &self,
        outpoint: &OutPoint,
        protocol_tag: u128,
    ) -> Result<Vec<Balance>>;
    fn get_output(&self, outpoint: &OutPoint) -> Result<TxOut>;
    fn get_height(&self, outpoint: &OutPoint) -> Result<u32>;
    fn get_txindex(&self, outpoint: &OutPoint) -> Result<u32>;
    fn get_runes_by_height(&self, height: u32, protocol_tag: u128) -> Result<Vec<Rune>>;
    fn sequence_pointer(&self) -> Self::Pointer;
    fn get_alkane_inventory(&self, owner_id: &AlkaneId) -> Result<Vec<AlkaneId>>;
    fn get_balance(&self, owner_id: &AlkaneId, alkane_id: &AlkaneId) -> Result<u128>;
    fn get_alkane_storage_at(&self, alkane_id: &AlkaneId, path: &[u8]) -> Result<Vec<u8>>;
    fn get_bytecode_by_alkane_id(&self, alkane_id: &AlkaneId) -> Result<Vec<u8>>;
}

/// Represents a wallet, containing a list of outpoints and their associated
/// balances.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Wallet {
    pub outpoints: Vec<Outpoint>,
}

/// Represents an outpoint, containing the transaction output and its associated
/// balances.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Outpoint {
    pub outpoint: OutPoint,
    pub output: TxOut,
    pub balances: Vec<Balance>,
    pub height: u32,
    pub txindex: u32,
}

/// Represents a balance of a single rune.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Balance {
    pub rune_id: ProtoruneRuneId,
    pub amount: u128,
}

pub struct Rune {
    pub rune_id: ProtoruneRuneId,
    pub name: String,
    pub symbol: String,
    pub spacers: u32,
    pub divisibility: u32,
}

pub fn runes_by_height<H: ViewHost>(
    host: &H,
    height: u32,
    protocol_tag: u128,
) -> Result<Vec<Rune>> {
    host.get_runes_by_height(height, protocol_tag)
}

pub fn runes_by_address<H: ViewHost>(
    host: &H,
    address: &str,
    protocol_tag: u128,
) -> Result<Wallet> {
    let mut wallet = Wallet { outpoints: vec![] };
    let outpoints = host.get_outpoints_by_address(address)?;
    for outpoint in outpoints {
        let outpoint_balances = host.get_balances_by_outpoint(&outpoint, protocol_tag)?;
        let output = host.get_output(&outpoint)?;
        let height = host.get_height(&outpoint)?;
        let txindex = host.get_txindex(&outpoint)?;
        wallet.outpoints.push(Outpoint {
            outpoint,
            output,
            balances: outpoint_balances,
            height,
            txindex,
        });
    }
    Ok(wallet)
}

pub fn runes_by_outpoint<H: ViewHost>(
    host: &H,
    outpoint: &OutPoint,
    protocol_tag: u128,
) -> Result<Outpoint> {
    let balances = host.get_balances_by_outpoint(outpoint, protocol_tag)?;
    let output = host.get_output(outpoint)?;
    let height = host.get_height(outpoint)?;
    let txindex = host.get_txindex(outpoint)?;
    Ok(Outpoint {
        outpoint: outpoint.clone(),
        output,
        balances,
        height,
        txindex,
    })
}
