// Copyright (c) 2023-2024 Deezel Inc. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

//
//
// Chadson's Documentation of ord.rs:
//
// Purpose:
// This file defines the data structures that represent the JSON responses from the `ord` server API.
// These structs are used to deserialize the JSON responses into strongly-typed Rust objects,
// which can then be used for processing, and pretty-printing in the `deezel` CLI.
//
// The structs defined here are based on the `api.rs` file from the `ord` reference implementation.
//
// Key Structs:
// - `Block`: Represents a Bitcoin block, including its hash, height, and associated inscriptions and runes.
// - `Inscription`: Represents an Ordinal inscription, including its ID, content type, and other metadata.
// - `Rune`: Represents a Rune, a fungible token on Bitcoin.
// - `Output`: Represents a transaction output (UTXO), including its value, script pubkey, and any associated inscriptions or runes.
// - `Sat`: Represents a single satoshi, including its rarity, charms, and associated inscriptions.
//
// Implementation Notes:
// - All structs derive `serde::{Deserialize, Serialize}` to allow for deserialization from JSON and serialization for the `--raw` flag.
// - Other common traits like `Debug`, `PartialEq`, and `Clone` are also derived for convenience.
// - This module uses types from the `ordinals` and `bitcoin` crates, which are dependencies of `deezel-common`.
// - Some types like `SpacedRune`, `Pile`, and `Charm` are defined locally as they are not available in the `ordinals` crate version used, or to avoid pulling in too many dependencies.
//
//

use bitcoin::{
    block::Header as BlockHeader, BlockHash, OutPoint,
    ScriptBuf, TxMerkleNode, Txid,
};
use alloc::{
    string::{String},
    vec::Vec,
    collections::BTreeMap,
};
use core::fmt::{self, Display, Formatter};
#[cfg(feature = "std")]
use crate::vendored_ord::{InscriptionId, SpacedRune};
#[cfg(not(feature = "std"))]
use crate::vendored_ord::{InscriptionId, SpacedRune};
use ordinals::{Rarity, Sat, SatPoint};
use crate::{
    address_parser::AddressParser,
    traits::{AddressResolver, Utxo, UtxoProvider},
    Result,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct OrdProvider<R: AddressResolver> {
    address_parser: AddressParser<R>,
}

impl<R: AddressResolver> OrdProvider<R> {
    pub fn new(address_resolver: R) -> Self {
        Self {
            address_parser: AddressParser::new(address_resolver),
        }
    }

    async fn get_address_info(&self, _address: &str) -> Result<AddressInfo> {
        // Placeholder for actual API call
        Ok(AddressInfo {
            outputs: vec![],
            inscriptions: None,
            sat_balance: 0,
            runes_balances: None,
        })
    }

    async fn get_output(&self, _outpoint: &OutPoint) -> Result<Output> {
        // Placeholder for actual API call
        unimplemented!()
    }
}

#[async_trait(?Send)]
impl<R: AddressResolver + Send + Sync> UtxoProvider for OrdProvider<R> {
    async fn get_utxos_by_spec(&self, spec: &[String]) -> Result<Vec<Utxo>> {
        let mut addresses = Vec::new();
        for s in spec {
            addresses.extend(self.address_parser.parse(s).await?);
        }

        let mut utxos = Vec::new();
        for address in addresses {
            let address_info = self.get_address_info(&address).await?;
            for outpoint in address_info.outputs {
                let output = self.get_output(&outpoint).await?;
                if !output.spent {
                    utxos.push(Utxo {
                        txid: outpoint.txid.to_string(),
                        vout: outpoint.vout,
                        amount: output.value,
                        address: address.clone(),
                    });
                }
            }
        }

        Ok(utxos)
    }
}


#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Block {
    pub hash: BlockHash,
    pub header: BlockHeader,
    pub info: Option<BlockInfo>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct BlockInfo {
    pub hash: BlockHash,
    pub confirmations: i32,
    pub height: i32,
    pub version: i32,
    #[serde(rename = "versionHex")]
    pub version_hex: String,
    #[serde(rename = "merkleroot")]
    pub merkle_root: TxMerkleNode,
    pub time: u32,
    #[serde(rename = "mediantime")]
    pub median_time: u32,
    pub nonce: u32,
    pub bits: String,
    pub difficulty: f64,
    #[serde(rename = "chainwork")]
    pub chain_work: String,
    #[serde(rename = "nTx")]
    pub n_tx: u32,
    #[serde(rename = "previousblockhash")]
    pub previous_block_hash: Option<BlockHash>,
    #[serde(rename = "nextblockhash")]
    pub next_block_hash: Option<BlockHash>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Blocks {
    pub last: u64,
    pub blocks: Vec<BlockHash>,
    pub featured_blocks: BTreeMap<BlockHash, Vec<InscriptionId>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Children {
    pub ids: Vec<InscriptionId>,
    pub more: bool,
    pub page: usize,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ChildInscriptions {
    pub children: Vec<RelativeInscriptionRecursive>,
    pub more: bool,
    pub page: usize,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ParentInscriptions {
    pub parents: Vec<RelativeInscriptionRecursive>,
    pub more: bool,
    pub page: usize,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Inscription {
    pub address: Option<String>,
    pub charms: Vec<Charm>,
    pub child_count: u64,
    pub children: Vec<InscriptionId>,
    pub content_length: Option<usize>,
    pub content_type: Option<String>,
    pub effective_content_type: Option<String>,
    pub fee: u64,
    pub height: u32,
    pub id: InscriptionId,
    pub next: Option<InscriptionId>,
    pub number: i32,
    pub parents: Vec<InscriptionId>,
    pub previous: Option<InscriptionId>,
    pub rune: Option<SpacedRune>,
    pub sat: Option<Sat>,
    pub satpoint: SatPoint,
    pub timestamp: i64,
    pub value: Option<u64>,
    pub metaprotocol: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct InscriptionRecursive {
    pub charms: Vec<Charm>,
    pub content_type: Option<String>,
    pub content_length: Option<usize>,
    pub delegate: Option<InscriptionId>,
    pub fee: u64,
    pub height: u32,
    pub id: InscriptionId,
    pub number: i32,
    pub output: OutPoint,
    pub sat: Option<Sat>,
    pub satpoint: SatPoint,
    pub timestamp: i64,
    pub value: Option<u64>,
    pub address: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct RelativeInscriptionRecursive {
    pub charms: Vec<Charm>,
    pub fee: u64,
    pub height: u32,
    pub id: InscriptionId,
    pub number: i32,
    pub output: OutPoint,
    pub sat: Option<Sat>,
    pub satpoint: SatPoint,
    pub timestamp: i64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Inscriptions {
    pub ids: Vec<InscriptionId>,
    pub more: bool,
    pub page_index: u32,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct UtxoRecursive {
    pub inscriptions: Option<Vec<InscriptionId>>,
    pub runes: Option<BTreeMap<SpacedRune, Pile>>,
    pub sat_ranges: Option<Vec<(u64, u64)>>,
    pub value: u64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Output {
    pub address: Option<String>,
    pub confirmations: u32,
    pub indexed: bool,
    pub inscriptions: Option<Vec<InscriptionId>>,
    pub outpoint: OutPoint,
    pub runes: Option<BTreeMap<SpacedRune, Pile>>,
    pub sat_ranges: Option<Vec<(u64, u64)>>,
    pub script_pubkey: ScriptBuf,
    pub spent: bool,
    pub transaction: Txid,
    pub value: u64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SatResponse {
    pub address: Option<String>,
    pub block: u32,
    pub charms: Vec<Charm>,
    pub cycle: u32,
    pub decimal: String,
    pub degree: String,
    pub epoch: u32,
    pub inscriptions: Vec<InscriptionId>,
    pub name: String,
    pub number: u64,
    pub offset: u64,
    pub percentile: String,
    pub period: u32,
    pub rarity: Rarity,
    pub satpoint: Option<SatPoint>,
    pub timestamp: i64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SatInscription {
    pub id: Option<InscriptionId>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SatInscriptions {
    pub ids: Vec<InscriptionId>,
    pub more: bool,
    pub page: u64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AddressInfo {
    pub outputs: Vec<OutPoint>,
    pub inscriptions: Option<Vec<InscriptionId>>,
    pub sat_balance: u64,
    pub runes_balances: Option<Vec<(SpacedRune, String, Option<char>)>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct RuneInfo {
    pub burned: String,
    pub divisibility: u8,
    pub etching: Txid,
    pub height: u32,
    pub id: String,
    pub index: u64,
    pub mints: String,
    pub number: u64,
    pub rune: SpacedRune,
    pub supply: String,
    pub symbol: Option<char>,
    pub timestamp: i64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Runes {
    pub runes: BTreeMap<String, RuneInfo>,
    pub next_page_number: Option<u32>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct TxInfo {
    pub chain: String,
    pub etching: Option<SpacedRune>,
    pub inscriptions: Vec<InscriptionId>,
    pub transaction: bitcoin::Transaction,
    pub txid: Txid,
}

impl Display for Charm {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Charm {
    Coin,
    Cursed,
    Epic,
    Legendary,
    Lost,
    Nineball,
    Rare,
    Reinscription,
    Unbound,
    Uncommon,
    Vindicated,
    Mythic,
    Burned,
    Palindrome,
}


#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, Copy)]
pub struct Pile {
    pub amount: u128,
    pub divisibility: u8,
    pub symbol: Option<char>,
}

/// JSON-RPC method names for Ord endpoints
pub struct OrdJsonRpcMethods;

impl OrdJsonRpcMethods {
    pub const ADDRESS: &'static str = "ord_address";
    pub const BLOCK: &'static str = "ord_block";
    pub const BLOCK_COUNT: &'static str = "ord_blockcount";
    pub const BLOCKS: &'static str = "ord_blocks";
    pub const CHILDREN: &'static str = "ord_children";
    pub const CONTENT: &'static str = "ord_content";
    pub const INSCRIPTION: &'static str = "ord_inscription";
    pub const INSCRIPTIONS: &'static str = "ord_inscriptions";
    pub const INSCRIPTIONS_IN_BLOCK: &'static str = "ord_inscriptionsinblock";
    pub const OUTPUT: &'static str = "ord_output";
    pub const PARENTS: &'static str = "ord_parents";
    pub const RUNE: &'static str = "ord_rune";
    pub const RUNES: &'static str = "ord_runes";
    pub const SAT: &'static str = "ord_sat";
    pub const TX: &'static str = "ord_tx";
}


