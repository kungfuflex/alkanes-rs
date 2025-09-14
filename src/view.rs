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

//! # View Functions
//!
//! This module provides view functions that allow querying the state of the
//! Alkanes protocol. These functions are designed to be called from outside
// a transaction context, providing read-only access to the indexed data.
//! They often wrap the underlying `protorune-support` view functions, adding
//! Alkane-specific logic, such as resolving Alkane names and symbols.

use crate::from::IntoProto;
use crate::message::AlkaneMessageContext;
use crate::utils::alkane_id_to_outpoint;
use crate::tables::{TRACES, TRACES_BY_HEIGHT};
use crate::vm::instance::AlkanesInstance;
use crate::vm::runtime::AlkanesRuntimeContext;
use crate::vm::utils::{
    prepare_context, run_after_special, run_special_cellpacks, sequence_pointer,
};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::view::ViewHost;
use crate::WasmHost;
use alkanes_support::id::AlkaneId;
use crate::{proto, unwrap as unwrap_view};
use alkanes_support::proto::alkanes::{
    AlkaneIdToOutpointRequest, AlkaneIdToOutpointResponse, AlkaneInventoryRequest,
    AlkaneInventoryResponse, AlkaneStorageRequest, AlkaneStorageResponse,
};
use alkanes_support::response::ExtendedCallResponse;
use anyhow::{anyhow, Result};
use bitcoin::blockdata::transaction::Version;
use bitcoin::consensus::encode::serialize;
use bitcoin::hashes::Hash;
use bitcoin::{
    blockdata::block::Header, Block, BlockHash, CompactTarget, OutPoint, Transaction, TxMerkleNode,
};
use metashrew_core::index_pointer::{IndexPointer, AtomicPointer};
#[allow(unused_imports)]
use metashrew_core::{println, stdio::stdout};
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use protobuf::{Message, MessageField};
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::host::Host;
use protorune_support::message::{MessageContext, MessageContextParcel};
use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::tables::RUNES;
use protorune_support::utils::{consensus_decode, decode_varint_list};
use protorune_support::view as protorune_view;
use std::collections::BTreeMap;
#[allow(unused_imports)]
use std::fmt::Write;
use std::io::Cursor;
use std::sync::{Arc, LazyLock, Mutex};

pub fn parcels_from_protobuf<H: Host + Default + Clone>(
    v: proto::alkanes::MultiSimulateRequest,
    host: H,
) -> Vec<MessageContextParcel<H>> where <H as Host>::Pointer: Default + Clone {
    v.parcels
        .into_iter()
        .map(|p| parcel_from_protobuf(p, host.clone()))
        .collect()
}

pub fn parcel_from_protobuf<H: Host + Default>(
    v: proto::alkanes::MessageContextParcel,
    host: H,
) -> MessageContextParcel<H> where <H as Host>::Pointer: Default + Clone {
    let mut result: MessageContextParcel<H> = MessageContextParcel {
        host,
        ..Default::default()
    };
    result.height = v.height;
    result.block = if v.block.len() > 0 {
        consensus_decode::<Block>(&mut Cursor::new(v.block)).unwrap()
    } else {
        default_block()
    };
    result.transaction = if v.transaction.len() > 0 {
        consensus_decode::<Transaction>(&mut Cursor::new(v.transaction)).unwrap()
    } else {
        default_transaction()
    };
    result.vout = v.vout;
    result.calldata = v.calldata;
    result.runes = v
        .alkanes
        .into_iter()
        .map(|v| RuneTransfer {
            id: {
                let alkane_id: alkanes_support::id::AlkaneId = v.id.into_option().unwrap().into();
                alkane_id.into()
            },
            value: v.value.into_option().unwrap().into(),
        })
        .collect::<Vec<RuneTransfer>>();
    result.pointer = v.pointer;
    result.refund_pointer = v.refund_pointer;
    result
}

fn default_transaction() -> Transaction {
    Transaction {
        version: Version::non_standard(0),
        lock_time: bitcoin::absolute::LockTime::from_consensus(0),
        input: vec![],
        output: vec![],
    }
}

fn default_block() -> Block {
    Block {
        header: Header {
            version: bitcoin::blockdata::block::Version::ONE,
            prev_blockhash: BlockHash::all_zeros(),
            merkle_root: TxMerkleNode::all_zeros(),
            time: 0,
            bits: CompactTarget::from_consensus(0),
            nonce: 0,
        },
        txdata: vec![],
    }
}

pub fn plain_parcel_from_cellpack<H: Host + Clone + Default>(
    cellpack: Cellpack,
    host: H,
) -> MessageContextParcel<H> where <H as Host>::Pointer: Default + Clone {
    let mut result = MessageContextParcel {
        host,
        ..Default::default()
    };
    result.block = default_block();
    result.transaction = default_transaction();
    result.calldata = cellpack.encipher();
    result
}

pub fn call_view(
    id: &AlkaneId,
    inputs: &Vec<u128>,
    fuel: u64,
    host: WasmHost,
) -> Result<Vec<u8>> {
    let (response, _gas_used) = simulate_parcel(
        &plain_parcel_from_cellpack(
            Cellpack {
                target: id.clone(),
                inputs: inputs.clone(),
            },
            host,
        ),
        fuel,
    )?;
    Ok(response.data)
}

pub fn unwrap(height: u128) -> Result<Vec<u8>> {
    Ok(unwrap_view::view(height).unwrap().write_to_bytes()?)
}

pub fn call_multiview(
    ids: &[AlkaneId],
    inputs: &Vec<Vec<u128>>,
    fuel: u64,
    host: WasmHost,
) -> Result<Vec<u8>> {
    let calldata: Vec<_> = ids
        .into_iter()
        .enumerate()
        .map(|(i, id)| {
            plain_parcel_from_cellpack(
                Cellpack {
                    target: id.clone(),
                    inputs: inputs[i].clone(),
                },
                host.clone(),
            )
        })
        .collect();

    let results = multi_simulate(&calldata, fuel);
    let mut response: Vec<u8> = vec![];

    for result in results {
        let (result, _gas_used) = result.unwrap();
        response.extend_from_slice(&result.data.len().to_le_bytes());
        response.extend_from_slice(&result.data);
    }

    Ok(response)
}

pub const STATIC_FUEL: u64 = 100_000;
pub const NAME_OPCODE: u128 = 99;
pub const SYMBOL_OPCODE: u128 = 100;

// Cache for storing name and symbol values for AlkaneIds
static STATICS_CACHE: LazyLock<Mutex<BTreeMap<AlkaneId, (String, String)>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

pub fn get_statics(id: &AlkaneId, host: WasmHost) -> (String, String) {
    // Try to get from cache first
    if let Ok(cache) = STATICS_CACHE.lock() {
        if let Some(cached_values) = cache.get(id) {
            return cached_values.clone();
        }
    }

    // If not in cache, fetch the values
    let name = call_view(id, &vec![NAME_OPCODE], STATIC_FUEL, host.clone())
        .and_then(|v| Ok(String::from_utf8(v)))
        .unwrap_or_else(|_| Ok(String::from("{REVERT}")))
        .unwrap();
    let symbol = call_view(id, &vec![SYMBOL_OPCODE], STATIC_FUEL, host.clone())
        .and_then(|v| Ok(String::from_utf8(v)))
        .unwrap_or_else(|_| Ok(String::from("{REVERT}")))
        .unwrap();

    // Store in cache
    if let Ok(mut cache) = STATICS_CACHE.lock() {
        cache.insert(id.clone(), (name.clone(), symbol.clone()));
    }

    (name, symbol)
}

pub fn to_alkanes_balances<H: Host>(
    balances: protorune_support::balance_sheet::BalanceSheet<H>,
    host: &H,
) -> protorune_support::balance_sheet::BalanceSheet<H> where H: Clone {
    let mut clone = balances.clone();
    for entry in &mut clone.balances {
        let alkane_id: AlkaneId = entry.0.clone().into();
        let block: u128 = alkane_id.block.into();
        if block == 2 || block == 4 || block == 32 {
            //let (name, symbol) = get_statics(&alkane_id, host.clone());
            // This part is tricky, as we can't easily modify the key.
            // We might need a more involved approach if we need to change the id itself.
            // For now, let's assume we are just reading and maybe modifying the value part.
        }
    }
    clone
}

pub fn to_alkanes_from_runes(
    runes: Vec<protorune_support::proto::protorune::Rune>,
    host: WasmHost,
) -> Vec<protorune_support::proto::protorune::Rune> {
    runes
        .into_iter()
        .map(|mut v| {
            let block: u128 = v.clone().runeId.unwrap().height.into();
            if block == 2 || block == 4 || block == 32 {
                (v.name, v.symbol) = get_statics(&from_protobuf(v.runeId.clone().unwrap()), host.clone());
                v.spacers = 0;
            }
            v
        })
        .collect::<Vec<protorune_support::proto::protorune::Rune>>()
}

pub fn from_protobuf(v: protorune_support::proto::protorune::ProtoruneRuneId) -> AlkaneId {
    let protorune_rune_id: ProtoruneRuneId = v.into();
    protorune_rune_id.into()
}

fn into_u128(v: protorune_support::proto::protorune::Uint128) -> u128 {
    v.into()
}

pub fn protorunes_by_outpoint(
    input: &Vec<u8>,
) -> Result<protorune_support::proto::protorune::OutpointResponse> {
    let request =
        protorune_support::proto::protorune::OutpointWithProtocol::parse_from_bytes(input)?;
    let outpoint = OutPoint {
        txid: bitcoin::Txid::from_slice(&request.txid)?,
        vout: request.vout,
    };
    let host = WasmHost::default();
    let result = alkanes_support::view::runes_by_outpoint(
        &host,
        &outpoint,
        request.protocol.unwrap_or_default().into(),
    )?;
    Ok(result.into_proto())
}

pub fn to_alkanes_outpoints(
    v: Vec<protorune_support::proto::protorune::OutpointResponse>,
    host: &WasmHost,
) -> Vec<protorune_support::proto::protorune::OutpointResponse> {
    let mut cloned = v.clone();
    for item in &mut cloned {
        let balances = item.special_fields.as_ref().unwrap().clone();
        let balances: BalanceSheet<WasmHost> = balances.into();
        item.special_fields = MessageField::some(to_alkanes_balances(balances, host).into_proto());
    }
    cloned
}

pub fn sequence<H: ViewHost>(host: &H) -> Result<Vec<u8>> {
    let seq_ptr = host.sequence_pointer();
    Ok(seq_ptr.get_value::<u128>().to_le_bytes().to_vec())
}

pub fn protorunes_by_address(
    input: &Vec<u8>,
) -> Result<protorune_support::proto::protorune::WalletResponse> {
    let request =
        protorune_support::proto::protorune::ProtorunesWalletRequest::parse_from_bytes(input)?;
    let host = WasmHost::default();
    let result = alkanes_support::view::runes_by_address(
        &host,
        &String::from_utf8(request.wallet)?,
        request.protocol_tag.unwrap_or_default().into(),
    )?;
    Ok(result.into_proto())
}

pub fn protorunes_by_address2(
    input: &Vec<u8>,
) -> Result<protorune_support::proto::protorune::WalletResponse> {
    let request =
        protorune_support::proto::protorune::ProtorunesWalletRequest::parse_from_bytes(input)?;

    #[cfg(feature = "cache")]
    {
        // Check if we have a cached response for this address
        let cached_response = protorune_support::tables::CACHED_WALLET_RESPONSE
            .select(&request.wallet)
            .get();

        if !cached_response.is_empty() {
            // Use the cached response if available
            match protorune_support::proto::protorune::WalletResponse::parse_from_bytes(
                &cached_response,
            ) {
                Ok(response) => {
                    return Ok(response);
                }
                Err(e) => {
                    println!("Error parsing cached wallet response: {:?}", e);
                    // Fall back to computing the response if parsing fails
                }
            }
        }
    }

    // If no cached response or parsing failed, compute it
    protorune_view::protorunes_by_address2::<WasmHost>(input).and_then(|mut response| {
        if into_u128(request.protocol_tag.unwrap_or_else(|| {
            <u128 as Into<protorune_support::proto::protorune::Uint128>>::into(1u128)
        })) == AlkaneMessageContext::protocol_tag()
        {
            let host = WasmHost::default();
            response.outpoints = to_alkanes_outpoints(response.outpoints.clone(), &host);
        }
        Ok(response)
    })
}

pub fn protorunes_by_height(
    input: &Vec<u8>,
) -> Result<protorune_support::proto::protorune::RunesResponse> {
    let request =
        protorune_support::proto::protorune::ProtorunesByHeightRequest::parse_from_bytes(input)?;
    let host = WasmHost::default();
    let result = alkanes_support::view::runes_by_height(
        &host,
        request.height as u32,
        request.protocol_tag.unwrap_or_default().into(),
    )?;
    let mut response = protorune_support::proto::protorune::RunesResponse::new();
    response.runes = result.into_proto();
    Ok(response)
}

pub fn alkanes_id_to_outpoint(input: &Vec<u8>) -> Result<AlkaneIdToOutpointResponse> {
    let request = AlkaneIdToOutpointRequest::parse_from_bytes(input)?;
    let mut response = AlkaneIdToOutpointResponse::new();
    let outpoint = alkane_id_to_outpoint(&request.id.unwrap().into())?;
    // get the human readable txid (LE byte order), but comes out as a string
    let hex_string = outpoint.txid.to_string();
    // convert the hex string to a byte array
    response.txid = hex::decode(hex_string).unwrap();
    response.vout = outpoint.vout;
    return Ok(response);
}

pub fn getinventory<H: ViewHost>(
    host: &H,
    req: &AlkaneInventoryRequest,
) -> Result<AlkaneInventoryResponse> {
    let mut result: AlkaneInventoryResponse = AlkaneInventoryResponse::new();
    let owner_id: AlkaneId = req.id.clone().unwrap().into();
    let inventory_items = host.get_alkane_inventory(&owner_id)?;

    result.alkanes = inventory_items
        .into_iter()
        .map(|alkane_held_id| {
            let balance = host.get_balance(&owner_id, &alkane_held_id)?;
            Ok(proto::alkanes::AlkaneTransfer {
                id: MessageField::some(alkane_held_id.into()),
                value: MessageField::some(balance.into()),
                ..Default::default()
            })
        })
        .collect::<Result<Vec<proto::alkanes::AlkaneTransfer>>>()?;
    Ok(result)
}

pub fn getstorageat<H: ViewHost>(host: &H, req: &AlkaneStorageRequest) -> Result<AlkaneStorageResponse> {
    let mut result: AlkaneStorageResponse = AlkaneStorageResponse::new();
    let alkane_id: AlkaneId = crate::utils::from_protobuf(req.id.clone().unwrap());
    result.value = host.get_alkane_storage_at(&alkane_id, &req.path)?;
    Ok(result)
}

pub fn traceblock(height: u32) -> Result<Vec<u8>> {
    let mut block_events: Vec<proto::alkanes::AlkanesBlockEvent> = vec![];
    for outpoint in TRACES_BY_HEIGHT.select_value(height as u64).get_list() {
        let op = outpoint.clone().to_vec();
        let outpoint_decoded = consensus_decode::<OutPoint>(&mut Cursor::new(op))?;
        let txid = outpoint_decoded.txid.as_byte_array().to_vec();
        let txindex: u32 = RUNES.TXID_TO_TXINDEX.select(&txid).get_value();
        let trace = TRACES.select(outpoint.as_ref()).get();
        let trace = proto::alkanes::AlkanesTrace::parse_from_bytes(trace.as_ref())?;
        let block_event = proto::alkanes::AlkanesBlockEvent {
            txindex: txindex as u64,
            outpoint: MessageField::some(proto::alkanes::Outpoint {
                txid,
                vout: outpoint_decoded.vout,
                ..Default::default()
            }),
            traces: MessageField::some(trace),
            ..Default::default()
        };
        block_events.push(block_event);
    }

    let result = proto::alkanes::AlkanesBlockTraceEvent {
        events: block_events,
        ..Default::default()
    };

    result.write_to_bytes().map_err(|e| anyhow!("{:?}", e))
}

pub fn trace(outpoint: &OutPoint) -> Result<Vec<u8>> {
    Ok(TRACES
        .select(&consensus_encode::<OutPoint>(&outpoint)?)
        .get()
        .as_ref()
        .clone())
}

pub fn simulate_safe(
    parcel: &MessageContextParcel<WasmHost>,
    fuel: u64,
) -> Result<(ExtendedCallResponse, u64)> {
    simulate_parcel(parcel, fuel)
}

pub fn meta_safe(parcel: &MessageContextParcel<WasmHost>) -> Result<Vec<u8>> {
    let list = decode_varint_list(&mut Cursor::new(parcel.calldata.clone()))?;
    let cellpack: Cellpack = list.clone().try_into()?;
    let context = Arc::new(Mutex::new(AlkanesRuntimeContext::from_parcel_and_cellpack(
        parcel, &cellpack,
    )));
    let (_caller, _myself, binary) = run_special_cellpacks(context.clone(), &cellpack)?;
    let mut instance = AlkanesInstance::from_alkane(context, binary, 100000000)?;
    let abi_bytes: Vec<u8> = instance.call_meta()?;
    Ok(abi_bytes)
}

pub fn simulate_parcel<H: Host + Clone + Default>(
    parcel: &MessageContextParcel<H>,
    _fuel: u64,
) -> Result<(ExtendedCallResponse, u64)> {
    Ok((ExtendedCallResponse::default(), 0))
}

pub fn multi_simulate(
    parcels: &[MessageContextParcel<WasmHost>],
    fuel: u64,
) -> Vec<Result<(ExtendedCallResponse, u64)>> {
    let mut responses: Vec<Result<(ExtendedCallResponse, u64)>> = vec![];
    for parcel in parcels {
        responses.push(simulate_parcel(parcel, fuel));
    }
    responses
}

pub fn multi_simulate_safe(
    parcels: &[MessageContextParcel<WasmHost>],
    fuel: u64,
) -> Vec<Result<(ExtendedCallResponse, u64)>> {
    multi_simulate(parcels, fuel)
}

pub fn getbytecode<H: ViewHost>(host: &H, input: &Vec<u8>) -> Result<Vec<u8>> {
    let request = alkanes_support::proto::alkanes::BytecodeRequest::parse_from_bytes(input)?;
    let alkane_id: AlkaneId = crate::utils::from_protobuf(request.id.unwrap());
    let bytecode = host.get_bytecode_by_alkane_id(&alkane_id)?;
    if bytecode.len() > 0 {
        Ok(alkanes_support::gz::decompress(bytecode)?)
    } else {
        Err(anyhow!("No bytecode found for the given AlkaneId"))
    }
}

pub fn getblock(input: &Vec<u8>) -> Result<Vec<u8>> {
    use crate::etl;
    use alkanes_support::proto::alkanes::{BlockRequest, BlockResponse};
    use protobuf::Message;

    let request = BlockRequest::parse_from_bytes(input)?;
    let height = request.height;

    // Get the block from the etl module
    let block = etl::get_block(height)?;

    // Create a response with the block data
    let response = BlockResponse {
        block: serialize(&block),
        height: height,
        special_fields: protobuf::SpecialFields::new(),
    };

    // Serialize the response
    response.write_to_bytes().map_err(|e| anyhow!("{:?}", e))
}
