use crate::tables::RuneTable;
use crate::{balance_sheet::load_sheet, tables};
use anyhow::{anyhow, Result};
use bitcoin;
use protorune_support::balance_sheet::{BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::proto;
use protorune_support::proto::protorune::{
    Outpoint,
    OutpointResponse,
    Output,
    Rune,
    //RunesByHeightRequest,
    RunesResponse,
    WalletResponse,
};
use protorune_support::utils::{consensus_decode, outpoint_encode};
//use bitcoin::consensus::Decodable;
use bitcoin::hashes::Hash;
use bitcoin::OutPoint;
//use metashrew_core::utils::{ consume_exact, consume_sized_int };
#[allow(unused_imports)]
use metashrew_core::{println, stdio::stdout};
use metashrew_support::index_pointer::KeyValuePointer;
use prost::Message;
#[allow(unused_imports)]
use std::fmt::Write;
use std::io::Cursor;

use bitcoin::address::{Address, NetworkUnchecked};
use metashrew_support::address::Payload;

pub fn script_pubkey_from_address(address_str: &str) -> Result<Vec<u8>> {
    let address: Address<NetworkUnchecked> = address_str.parse()?;
    let address = address.require_network(bitcoin::Network::Bitcoin)?;
    let script_pubkey = address.script_pubkey();
    Ok(script_pubkey.to_bytes())
}

pub fn outpoint_to_bytes(outpoint: &OutPoint) -> Result<Vec<u8>> {
    Ok(outpoint_encode(outpoint)?)
}

pub fn core_outpoint_to_proto(outpoint: &OutPoint) -> Outpoint {
    Outpoint {
        txid: outpoint.txid.as_byte_array().to_vec().clone(),
        vout: outpoint.vout,
    }
}

pub fn protorune_outpoint_to_outpoint_response(
    outpoint: &OutPoint,
    protocol_id: u128,
) -> Result<OutpointResponse> {
    //    println!("protocol_id: {}", protocol_id);
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    let balance_sheet = load_sheet(
        &tables::RuneTable::for_protocol(protocol_id)
            .OUTPOINT_TO_RUNES
            .select(&outpoint_bytes),
    );

    let mut height: u128 = tables::RUNES
        .OUTPOINT_TO_HEIGHT
        .select(&outpoint_bytes)
        .get_value::<u64>()
        .into();
    let mut txindex: u128 = tables::RUNES
        .HEIGHT_TO_TRANSACTION_IDS
        .select_value::<u64>(height as u64)
        .get_list()
        .into_iter()
        .position(|v| v.as_ref().to_vec() == outpoint.txid.as_byte_array().to_vec())
        .ok_or("")
        .map_err(|_| anyhow!("txid not indexed in table"))? as u128;

    if let Some((rune_id, _)) = balance_sheet.balances().iter().next() {
        height = rune_id.block.into();
        txindex = rune_id.tx.into();
    }
    let decoded_output: Output = Output::decode(
        tables::OUTPOINT_TO_OUTPUT
            .select(&outpoint_bytes)
            .get()
            .as_ref()
            .as_slice(),
    )?;
    Ok(OutpointResponse {
        balances: Some(balance_sheet.into()),
        outpoint: Some(core_outpoint_to_proto(&outpoint)),
        output: Some(decoded_output),
        height: height as u32,
        txindex: txindex as u32,
    })
}

pub fn rune_outpoint_to_outpoint_response(outpoint: &OutPoint) -> Result<OutpointResponse> {
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    let balance_sheet = load_sheet(&tables::RUNES.OUTPOINT_TO_RUNES.select(&outpoint_bytes));

    let mut height: u128 = tables::RUNES
        .OUTPOINT_TO_HEIGHT
        .select(&outpoint_bytes)
        .get_value::<u64>()
        .into();
    let mut txindex: u128 = tables::RUNES
        .HEIGHT_TO_TRANSACTION_IDS
        .select_value::<u64>(height as u64)
        .get_list()
        .into_iter()
        .position(|v| v.as_ref().to_vec() == outpoint.txid.as_byte_array().to_vec())
        .ok_or("")
        .map_err(|_| anyhow!("txid not indexed in table"))? as u128;

    if let Some((rune_id, _)) = balance_sheet.balances().iter().next() {
        height = rune_id.block.into();
        txindex = rune_id.tx.into();
    }
    let decoded_output: Output = Output::decode(
        tables::OUTPOINT_TO_OUTPUT
            .select(&outpoint_bytes)
            .get()
            .as_ref()
            .as_slice(),
    )?;
    Ok(OutpointResponse {
        balances: Some(balance_sheet.into()),
        outpoint: Some(core_outpoint_to_proto(&outpoint)),
        output: Some(decoded_output),
        height: height as u32,
        txindex: txindex as u32,
    })
}

pub fn outpoint_to_outpoint_response(outpoint: &OutPoint) -> Result<OutpointResponse> {
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    let balance_sheet = load_sheet(&tables::RUNES.OUTPOINT_TO_RUNES.select(&outpoint_bytes));
    let mut height: u128 = tables::RUNES
        .OUTPOINT_TO_HEIGHT
        .select(&outpoint_bytes)
        .get_value::<u64>()
        .into();
    let mut txindex: u128 = tables::RUNES
        .HEIGHT_TO_TRANSACTION_IDS
        .select_value::<u64>(height as u64)
        .get_list()
        .into_iter()
        .position(|v| v.as_ref().to_vec() == outpoint.txid.as_byte_array().to_vec())
        .ok_or("")
        .map_err(|_| anyhow!("txid not indexed in table"))? as u128;

    if let Some((rune_id, _)) = balance_sheet.balances().iter().next() {
        height = rune_id.block;
        txindex = rune_id.tx;
    }
    let decoded_output: Output = Output::decode(
        tables::OUTPOINT_TO_OUTPUT
            .select(&outpoint_bytes)
            .get()
            .as_ref()
            .as_slice(),
    )?;
    Ok(OutpointResponse {
        balances: Some(balance_sheet.into()),
        outpoint: Some(core_outpoint_to_proto(&outpoint)),
        output: Some(decoded_output),
        height: height as u32,
        txindex: txindex as u32,
    })
}

pub fn runes_by_address(input: &Vec<u8>) -> Result<WalletResponse> {
    let mut result: WalletResponse = WalletResponse::default();
    if let Some(req) = proto::protorune::WalletRequest::decode(input.as_ref()).ok() {
        let script_pubkey = script_pubkey_from_address(&String::from_utf8(req.wallet.clone())?)?;
        result.outpoints = tables::OUTPOINTS_FOR_ADDRESS
            .select(&script_pubkey)
            .get_list()
            .into_iter()
            .map(|v| -> Result<OutPoint> {
                let mut cursor = Cursor::new(v.as_ref().clone());
                Ok(consensus_decode::<bitcoin::blockdata::transaction::OutPoint>(&mut cursor)?)
            })
            .collect::<Result<Vec<OutPoint>>>()?
            .into_iter()
            .filter_map(|v| -> Option<Result<OutpointResponse>> {
                Some(outpoint_to_outpoint_response(&v))
            })
            .collect::<Result<Vec<OutpointResponse>>>()?;
    }
    Ok(result)
}

pub fn protorunes_by_outpoint(input: &Vec<u8>) -> Result<OutpointResponse> {
    match proto::protorune::OutpointWithProtocol::decode(input.as_ref()).ok() {
        Some(req) => {
            let protocol_tag: u128 = req.protocol.unwrap().into();

            let outpoint = OutPoint {
                txid: bitcoin::blockdata::transaction::Txid::from_byte_array(
                    <Vec<u8> as AsRef<[u8]>>::as_ref(&req.txid).try_into()?,
                ),
                vout: req.vout,
            };
            protorune_outpoint_to_outpoint_response(&outpoint, protocol_tag)
        }
        None => Err(anyhow!("malformed request")),
    }
}

pub fn runes_by_outpoint(input: &Vec<u8>) -> Result<OutpointResponse> {
    match proto::protorune::Outpoint::decode(input.as_ref()).ok() {
        Some(req) => {
            let outpoint = OutPoint {
                txid: bitcoin::blockdata::transaction::Txid::from_byte_array(
                    <Vec<u8> as AsRef<[u8]>>::as_ref(&req.txid).try_into()?,
                ),
                vout: req.vout,
            };
            rune_outpoint_to_outpoint_response(&outpoint)
        }
        None => Err(anyhow!("malformed request")),
    }
}



pub fn protorunes_by_address2(input: &Vec<u8>) -> Result<WalletResponse> {
    let mut result: WalletResponse = WalletResponse::default();
    if let Some(req) = proto::protorune::ProtorunesWalletRequest::decode(input.as_ref()).ok() {
        let script_pubkey = script_pubkey_from_address(&String::from_utf8(req.wallet.clone())?)?;
        result.outpoints = tables::OUTPOINT_SPENDABLE_BY_ADDRESS
            .select(&script_pubkey)
            .map_ll(|ptr, _| -> Result<OutpointResponse> {
                let mut cursor = Cursor::new(ptr.get().as_ref().clone());
                let outpoint =
                    consensus_decode::<bitcoin::blockdata::transaction::OutPoint>(&mut cursor)?;
                protorune_outpoint_to_outpoint_response(
                    &outpoint,
                    req.clone().protocol_tag.unwrap().into(),
                )
            })
            .into_iter()
            .collect::<Result<Vec<OutpointResponse>>>()?;
    }
    Ok(result)
}

pub fn runes_by_height(input: &Vec<u8>) -> Result<RunesResponse> {
    let mut result: RunesResponse = RunesResponse::default();
    if let Some(req) = proto::protorune::RunesByHeightRequest::decode(input.as_ref()).ok() {
        for rune in tables::HEIGHT_TO_RUNES
            .select_value(req.height)
            .get_list()
            .into_iter()
        {
            let tmp: ProtoruneRuneId = tables::RUNES.ETCHING_TO_RUNE_ID.select(&rune).get().into();
            let mut _rune: Rune = Rune::default();
            _rune.name = String::from_utf8(rune.as_ref().clone())?;
            _rune.rune_id = Some(tmp.into());
            _rune.spacers = tables::RUNES.SPACERS.select(&rune).get_value::<u32>();

            let symbol_bytes = tables::RUNES.SYMBOL.select(&rune).get().as_ref().clone();
            if symbol_bytes.len() != 4 {
                return Err(anyhow!("INDEXER HAS STORED THE SYMBOL INCORRECTLY!"));
            }

            let symbol_unicode = u32::from_ne_bytes([
                symbol_bytes[0],
                symbol_bytes[1],
                symbol_bytes[2],
                symbol_bytes[3],
            ]);

            _rune.symbol = char::from_u32(symbol_unicode).unwrap().to_string();
            _rune.divisibility = tables::RUNES.DIVISIBILITY.select(&rune).get_value::<u8>() as u32;
            result.runes.push(_rune);
        }
    }
    Ok(result)
}

pub fn protorunes_by_height(input: &Vec<u8>) -> Result<RunesResponse> {
    let mut result: RunesResponse = RunesResponse::default();
    if let Some(req) = proto::protorune::ProtorunesByHeightRequest::decode(input.as_ref()).ok() {
        let table =
            RuneTable::for_protocol(req.protocol_tag.unwrap_or_else(|| (0u128).into()).into());
        for rune in table
            .HEIGHT_TO_RUNE_ID
            .select_value(req.height)
            .get_list()
            .into_iter()
        {
            let mut _rune: Rune = Rune::default();
            _rune.name = String::from("");
            _rune.symbol = String::from("");
            _rune.rune_id = Some(
                <Vec<u8> as TryInto<ProtoruneRuneId>>::try_into(rune.as_ref().clone())?.into(),
            );
            _rune.spacers = 0;

            _rune.divisibility = 0;
            result.runes.push(_rune);
        }
    }
    Ok(result)
}
