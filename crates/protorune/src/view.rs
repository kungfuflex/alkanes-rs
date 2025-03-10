use crate::tables::RuneTable;
use crate::{balance_sheet::load_sheet, tables};
use anyhow::{anyhow, Result};
use bitcoin;
use protorune_support::balance_sheet::{BalanceSheet, ProtoruneRuneId};
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
//use metashrew::utils::{ consume_exact, consume_sized_int };
use metashrew::{println, stdio::stdout};
use metashrew_support::index_pointer::KeyValuePointer;
use protobuf::{Message, MessageField, SpecialFields};
use std::fmt::Write;
use std::io::Cursor;

pub fn outpoint_to_bytes(outpoint: &OutPoint) -> Result<Vec<u8>> {
    Ok(outpoint_encode(outpoint)?)
}

pub fn core_outpoint_to_proto(outpoint: &OutPoint) -> Outpoint {
    Outpoint {
        txid: outpoint.txid.as_byte_array().to_vec().clone(),
        vout: outpoint.vout,
        special_fields: SpecialFields::new(),
    }
}

pub fn protorune_outpoint_to_outpoint_response(
    outpoint: &OutPoint,
    protocol_id: u128,
) -> Result<OutpointResponse> {
    println!("protocol_id: {}", protocol_id);
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    let balance_sheet: BalanceSheet = load_sheet(
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

    if let Some((rune_id, _)) = balance_sheet.clone().balances.iter().next() {
        height = rune_id.block.into();
        txindex = rune_id.tx.into();
    }
    let decoded_output: Output = Output::parse_from_bytes(
        &tables::OUTPOINT_TO_OUTPUT
            .select(&outpoint_bytes)
            .get()
            .as_ref(),
    )?;
    Ok(OutpointResponse {
        balances: MessageField::some(balance_sheet.into()),
        outpoint: MessageField::some(core_outpoint_to_proto(&outpoint)),
        output: MessageField::some(decoded_output),
        height: height as u32,
        txindex: txindex as u32,
        special_fields: SpecialFields::new(),
    })
}

pub fn rune_outpoint_to_outpoint_response(outpoint: &OutPoint) -> Result<OutpointResponse> {
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    let balance_sheet: BalanceSheet =
        load_sheet(&tables::RUNES.OUTPOINT_TO_RUNES.select(&outpoint_bytes));

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

    if let Some((rune_id, _)) = balance_sheet.clone().balances.iter().next() {
        height = rune_id.block.into();
        txindex = rune_id.tx.into();
    }
    let decoded_output: Output = Output::parse_from_bytes(
        &tables::OUTPOINT_TO_OUTPUT
            .select(&outpoint_bytes)
            .get()
            .as_ref(),
    )?;
    Ok(OutpointResponse {
        balances: MessageField::some(balance_sheet.into()),
        outpoint: MessageField::some(core_outpoint_to_proto(&outpoint)),
        output: MessageField::some(decoded_output),
        height: height as u32,
        txindex: txindex as u32,
        special_fields: SpecialFields::new(),
    })
}

pub fn outpoint_to_outpoint_response(outpoint: &OutPoint) -> Result<OutpointResponse> {
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    let balance_sheet: BalanceSheet =
        load_sheet(&tables::RUNES.OUTPOINT_TO_RUNES.select(&outpoint_bytes));
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

    if let Some((rune_id, _)) = balance_sheet.clone().balances.iter().next() {
        height = rune_id.block;
        txindex = rune_id.tx;
    }
    let decoded_output: Output = Output::parse_from_bytes(
        &tables::OUTPOINT_TO_OUTPUT
            .select(&outpoint_bytes)
            .get()
            .as_ref(),
    )?;
    Ok(OutpointResponse {
        balances: MessageField::some(balance_sheet.into()),
        outpoint: MessageField::some(core_outpoint_to_proto(&outpoint)),
        output: MessageField::some(decoded_output),
        height: height as u32,
        txindex: txindex as u32,
        special_fields: SpecialFields::new(),
    })
}

pub fn runes_by_address(input: &Vec<u8>) -> Result<WalletResponse> {
    let mut result: WalletResponse = WalletResponse::new();
    if let Some(req) = proto::protorune::WalletRequest::parse_from_bytes(input).ok() {
        result.outpoints = tables::OUTPOINTS_FOR_ADDRESS
            .select(&req.wallet)
            .get_list()
            .into_iter()
            .map(|v| -> Result<OutPoint> {
                let mut cursor = Cursor::new(v.as_ref().clone());
                Ok(consensus_decode::<bitcoin::blockdata::transaction::OutPoint>(&mut cursor)?)
            })
            .collect::<Result<Vec<OutPoint>>>()?
            .into_iter()
            .filter_map(|v| -> Option<Result<OutpointResponse>> {
                let outpoint_bytes = match outpoint_to_bytes(&v) {
                    Ok(v) => v,
                    Err(e) => {
                        return Some(Err(e));
                    }
                };
                let _address = tables::OUTPOINT_SPENDABLE_BY.select(&outpoint_bytes).get();
                if req.wallet.len() == _address.len() {
                    Some(outpoint_to_outpoint_response(&v))
                } else {
                    None
                }
            })
            .collect::<Result<Vec<OutpointResponse>>>()?;
    }
    Ok(result)
}

pub fn protorunes_by_outpoint(input: &Vec<u8>) -> Result<OutpointResponse> {
    match proto::protorune::OutpointWithProtocol::parse_from_bytes(input).ok() {
        Some(req) => {
            let protocol_tag: u128 = req.protocol.into_option().unwrap().into();

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
    match proto::protorune::Outpoint::parse_from_bytes(input).ok() {
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

pub fn protorunes_by_address(input: &Vec<u8>) -> Result<WalletResponse> {
    let mut result: WalletResponse = WalletResponse::new();
    if let Some(req) = proto::protorune::ProtorunesWalletRequest::parse_from_bytes(input).ok() {
        result.outpoints = tables::OUTPOINT_SPENDABLE_BY_ADDRESS
            .select(&req.wallet)
            .map_ll(|ptr, _| -> Result<OutPoint> {
                let mut cursor = Cursor::new(ptr.get().as_ref().clone());
                Ok(consensus_decode::<bitcoin::blockdata::transaction::OutPoint>(&mut cursor)?)
            })
            .into_iter()
            .collect::<Result<Vec<OutPoint>>>()?
            .into_iter()
            .filter_map(|v| -> Option<Result<OutpointResponse>> {
                let outpoint_bytes = match outpoint_to_bytes(&v) {
                    Ok(v) => v,
                    Err(e) => {
                        return Some(Err(e));
                    }
                };
                let _address = tables::OUTPOINT_SPENDABLE_BY.select(&outpoint_bytes).get();
                if req.wallet.len() == _address.len() {
                    Some(protorune_outpoint_to_outpoint_response(
                        &v,
                        req.clone().protocol_tag.into_option().unwrap().into(),
                    ))
                } else {
                    None
                }
            })
            .collect::<Result<Vec<OutpointResponse>>>()?;
    }
    Ok(result)
}

pub fn runes_by_height(input: &Vec<u8>) -> Result<RunesResponse> {
    let mut result: RunesResponse = RunesResponse::new();
    if let Some(req) = proto::protorune::RunesByHeightRequest::parse_from_bytes(input).ok() {
        for rune in tables::HEIGHT_TO_RUNES
            .select_value(req.height)
            .get_list()
            .into_iter()
        {
            let mut _rune: Rune = Rune::new();
            _rune.name = String::from_utf8(rune.as_ref().clone())?;
            _rune.runeId = MessageField::from_option(
                proto::protorune::ProtoruneRuneId::parse_from_bytes(
                    &tables::RUNES.ETCHING_TO_RUNE_ID.select(&rune).get(),
                )
                .ok(),
            );
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
    let mut result: RunesResponse = RunesResponse::new();
    if let Some(req) = proto::protorune::ProtorunesByHeightRequest::parse_from_bytes(input).ok() {
        let table =
            RuneTable::for_protocol(req.protocol_tag.unwrap_or_else(|| (0u128).into()).into());
        for rune in table
            .HEIGHT_TO_RUNE_ID
            .select_value(req.height)
            .get_list()
            .into_iter()
        {
            let mut _rune: Rune = Rune::new();
            _rune.name = String::from("");
            _rune.symbol = String::from("");
            _rune.runeId = MessageField::some(
                <Vec<u8> as TryInto<ProtoruneRuneId>>::try_into(rune.as_ref().clone())?.into(),
            );
            _rune.spacers = 0;

            _rune.divisibility = 0;
            result.runes.push(_rune);
        }
    }
    Ok(result)
}
