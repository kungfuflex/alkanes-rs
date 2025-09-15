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

#[cfg(not(test))]
use crate::{
    into_proto::IntoProto,
    tables::{TRACES, TRACES_BY_HEIGHT},
    utils::alkane_id_to_outpoint,
    WasmHost,
};
#[cfg(test)]
use alkanes::{
    into_proto::IntoProto,
    tables::{TRACES, TRACES_BY_HEIGHT},
    utils::alkane_id_to_outpoint,
    WasmHost,
};
use alkanes_proto::alkanes::{
    AlkaneIdToOutpointRequest, AlkaneIdToOutpointResponse, AlkaneInventoryRequest,
    AlkaneInventoryResponse, AlkaneStorageRequest, AlkaneStorageResponse,
};
use alkanes_support::id::AlkaneId;
use alkanes_support::view::ViewHost;
use anyhow::{anyhow, Result};
use bitcoin::consensus::encode::serialize;
use bitcoin::hashes::Hash;
use bitcoin::{OutPoint};
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_encode;
use protobuf::{Message, MessageField};
use protorune_support::tables::RUNES;
use protorune_support::utils::consensus_decode;
use std::io::Cursor;

pub fn protorunes_by_outpoint(
    input: &Vec<u8>,
) -> Result<alkanes_proto::alkanes::OutpointResponse> {
    let request = alkanes_proto::alkanes::OutpointWithProtocol::parse_from_bytes(input)?;
    let outpoint = OutPoint {
        txid: bitcoin::Txid::from_slice(&request.txid)?,
        vout: request.vout,
    };
    let host = WasmHost::default();
    let result = alkanes_support::view::runes_by_outpoint(
        &host,
        &outpoint,
        ((request.protocol.clone().into_option().unwrap().hi as u128) << 64)
            | request.protocol.into_option().unwrap().lo as u128,
    )?;
    Ok(result.into_proto())
}

pub fn protorunes_by_address(
    input: &Vec<u8>,
) -> Result<alkanes_proto::alkanes::WalletResponse> {
    let request = alkanes_proto::alkanes::ProtorunesWalletRequest::parse_from_bytes(input)?;
    let host = WasmHost::default();
    let result = alkanes_support::view::runes_by_address(
        &host,
        &String::from_utf8(request.wallet)?,
        ((request.protocol_tag.clone().into_option().unwrap().hi as u128) << 64)
            | request.protocol_tag.into_option().unwrap().lo as u128,
    )?;
    Ok(result.into_proto())
}

pub fn protorunes_by_height(
    input: &Vec<u8>,
) -> Result<alkanes_proto::alkanes::RunesResponse> {
    let request = alkanes_proto::alkanes::ProtorunesByHeightRequest::parse_from_bytes(input)?;
    let host = WasmHost::default();
    let result = alkanes_support::view::runes_by_height(
        &host,
        request.height as u32,
        ((request.protocol_tag.clone().into_option().unwrap().hi as u128) << 64)
            | request.protocol_tag.into_option().unwrap().lo as u128,
    )?;
    let mut response = alkanes_proto::alkanes::RunesResponse::new();
    response.runes = result.into_proto();
    Ok(response)
}

pub fn alkanes_id_to_outpoint(input: &Vec<u8>) -> Result<AlkaneIdToOutpointResponse> {
    let request = AlkaneIdToOutpointRequest::parse_from_bytes(input)?;
    let mut response = AlkaneIdToOutpointResponse::new();
    let id = request.id.into_option().unwrap();
    let outpoint = alkane_id_to_outpoint(&AlkaneId {
        block: ((id.block.clone().into_option().unwrap().hi as u128) << 64)
            | id.block.into_option().unwrap().lo as u128,
        tx: ((id.tx.clone().into_option().unwrap().hi as u128) << 64)
            | id.tx.into_option().unwrap().lo as u128,
    })?;
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
    let id = req.id.clone().into_option().unwrap();
    let owner_id: AlkaneId = AlkaneId {
        block: ((id.block.clone().into_option().unwrap().hi as u128) << 64)
            | id.block.into_option().unwrap().lo as u128,
        tx: ((id.tx.clone().into_option().unwrap().hi as u128) << 64)
            | id.tx.into_option().unwrap().lo as u128,
    };
    let inventory_items = host.get_alkane_inventory(&owner_id)?;

    result.alkanes = inventory_items
        .into_iter()
        .map(|alkane_held_id| {
            let balance = host.get_balance(&owner_id, &alkane_held_id)?;
            Ok(alkanes_proto::alkanes::AlkaneTransfer {
                id: MessageField::some(alkane_held_id.into_proto()),
                value: MessageField::some(alkanes_proto::alkanes::Uint128 {
                    hi: (balance >> 64) as u64,
                    lo: balance as u64,
                    ..Default::default()
                }),
                ..Default::default()
            })
        })
        .collect::<Result<Vec<alkanes_proto::alkanes::AlkaneTransfer>>>()?;
    Ok(result)
}

pub fn getstorageat<H: ViewHost>(
    host: &H,
    req: &AlkaneStorageRequest,
) -> Result<AlkaneStorageResponse> {
    let mut result: AlkaneStorageResponse = AlkaneStorageResponse::new();
    let id = req.id.clone().into_option().unwrap();
    let alkane_id: AlkaneId = AlkaneId {
        block: ((id.block.clone().into_option().unwrap().hi as u128) << 64)
            | id.block.into_option().unwrap().lo as u128,
        tx: ((id.tx.clone().into_option().unwrap().hi as u128) << 64)
            | id.tx.into_option().unwrap().lo as u128,
    };
    result.value = host.get_alkane_storage_at(&alkane_id, &req.path)?;
    Ok(result)
}

pub fn traceblock(height: u32) -> Result<Vec<u8>> {
    let mut block_events: Vec<alkanes_proto::alkanes::AlkanesBlockEvent> = vec![];
    for outpoint in TRACES_BY_HEIGHT.select_value(height as u64).get_list() {
        let op = outpoint.clone().to_vec();
        let outpoint_decoded = consensus_decode::<OutPoint>(&mut Cursor::new(op))?;
        let txid = outpoint_decoded.txid.as_byte_array().to_vec();
        let txindex: u32 = RUNES.TXID_TO_TXINDEX.select(&txid).get_value();
        let trace = TRACES.select(outpoint.as_ref()).get();
        let trace = alkanes_proto::alkanes::AlkanesTrace::parse_from_bytes(trace.as_ref())?;
        let block_event = alkanes_proto::alkanes::AlkanesBlockEvent {
            txindex: txindex as u64,
            outpoint: MessageField::some(alkanes_proto::alkanes::Outpoint {
                txid,
                vout: outpoint_decoded.vout,
                ..Default::default()
            }),
            traces: MessageField::some(trace),
            ..Default::default()
        };
        block_events.push(block_event);
    }

    let result = alkanes_proto::alkanes::AlkanesBlockTraceEvent {
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

pub fn getbytecode<H: ViewHost>(host: &H, input: &Vec<u8>) -> Result<Vec<u8>> {
    let request = alkanes_proto::alkanes::BytecodeRequest::parse_from_bytes(input)?;
    let id = request.id.into_option().unwrap();
    let alkane_id: AlkaneId = AlkaneId {
        block: ((id.block.clone().into_option().unwrap().hi as u128) << 64)
            | id.block.into_option().unwrap().lo as u128,
        tx: ((id.tx.clone().into_option().unwrap().hi as u128) << 64)
            | id.tx.into_option().unwrap().lo as u128,
    };
    let bytecode = host.get_bytecode_by_alkane_id(&alkane_id)?;
    if bytecode.len() > 0 {
        Ok(alkanes_support::gz::decompress(bytecode)?)
    } else {
        Err(anyhow!("No bytecode found for the given AlkaneId"))
    }
}

pub fn getblock(input: &Vec<u8>) -> Result<Vec<u8>> {
    #[cfg(not(test))]
    use crate::etl;
    #[cfg(test)]
    use alkanes::etl;
    use alkanes_proto::alkanes::{BlockRequest, BlockResponse};
    use protobuf::Message;

    let request = BlockRequest::parse_from_bytes(input)?;
    let height = request.height;

    // Get the block from the etl module
    let block = etl::get_block(height)?;

    // Create a response with the block data
    let response = BlockResponse {
        block: serialize(&block),
        height: height,
        ..Default::default()
    };

    // Serialize the response
    response.write_to_bytes().map_err(|e| anyhow!("{:?}", e))
}

