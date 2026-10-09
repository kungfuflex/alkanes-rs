use crate::tables::RuneTable;
use crate::{
    balance_sheet::{load_sheet, load_sheet_page, MAX_VIEW_SHEET_ENTRIES},
    tables,
};
use anyhow::{anyhow, Result};
use bitcoin;
use protorune_support::balance_sheet::{BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::proto;
use protorune_support::proto::protorune::{
    Outpoint,
    OutpointCursorResponse,
    OutpointResponse,
    Output,
    Rune,
    //RunesByHeightRequest,
    RunesResponse,
    WalletCursorResponse,
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

pub fn protorunes_by_address(input: &Vec<u8>) -> Result<WalletResponse> {
    let mut result: WalletResponse = WalletResponse::default();
    if let Some(req) = proto::protorune::ProtorunesWalletRequest::decode(input.as_ref()).ok() {
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
                    Some(protorune_outpoint_to_outpoint_response(
                        &v,
                        req.clone().protocol_tag.unwrap().into(),
                    ))
                } else {
                    None
                }
            })
            .collect::<Result<Vec<OutpointResponse>>>()?;
    }
    Ok(result)
}

pub fn protorunes_by_address2(input: &Vec<u8>) -> Result<WalletResponse> {
    let mut result: WalletResponse = WalletResponse::default();
    if let Some(req) = proto::protorune::ProtorunesWalletRequest::decode(input.as_ref()).ok() {
        result.outpoints = tables::OUTPOINT_SPENDABLE_BY_ADDRESS
            .select(&req.wallet)
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
            .collect::<Result<Vec<OutpointResponse>>>()?
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

// ---- Cursor views: bounded, resumable reads of balance sheets ------------
//
// `protorunes_by_outpoint` / `protorunes_by_address` above load whole balance
// sheets in one call, which on an outpoint holding e.g. 100k alkanes runs long
// enough to time out (and holds a view permit the whole time). The cursor
// views below return at most `MAX_VIEW_SHEET_ENTRIES` balance-sheet entries
// per call plus a cursor to resume from, so a client can stream any sheet with
// serial requests. The legacy views and their messages are unchanged.

/// Most address-list positions one `protorunes_by_address_cursor` call scans
/// (spent outpoints are skipped but still scanned), so a long run of spent
/// outpoints can't make one page unbounded either.
pub const MAX_CURSOR_SCAN_OUTPOINTS: u32 = 10_000;

fn cursor_page_limit(limit: u32) -> u32 {
    if limit == 0 {
        MAX_VIEW_SHEET_ENTRIES
    } else {
        limit.min(MAX_VIEW_SHEET_ENTRIES)
    }
}

/// The outpoint's own height and position in its block, plus its output.
/// Unlike the legacy views, height/txindex are never replaced by the first
/// balance's rune id: a page's first entry varies, so that would make them
/// differ from page to page.
fn outpoint_cursor_metadata(outpoint: &OutPoint) -> Result<(u32, u32, Output)> {
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    let height: u64 = tables::RUNES
        .OUTPOINT_TO_HEIGHT
        .select(&outpoint_bytes)
        .get_value::<u64>();
    let txindex = tables::RUNES
        .HEIGHT_TO_TRANSACTION_IDS
        .select_value::<u64>(height)
        .get_list()
        .into_iter()
        .position(|v| v.as_ref().to_vec() == outpoint.txid.as_byte_array().to_vec())
        .ok_or_else(|| anyhow!("txid not indexed in table"))?;
    let output = Output::decode(
        tables::OUTPOINT_TO_OUTPUT
            .select(&outpoint_bytes)
            .get()
            .as_ref()
            .as_slice(),
    )?;
    Ok((height as u32, txindex as u32, output))
}

/// One page of `outpoint`'s protocol-`protocol_id` balance sheet, starting at
/// stored entry `cursor`, as an `OutpointResponse` plus the sheet's total size
/// and the next cursor (0 = end). Also returns how many entries were read.
fn outpoint_cursor_page(
    outpoint: &OutPoint,
    protocol_id: u128,
    cursor: u32,
    limit: u32,
) -> Result<(OutpointResponse, u32, u32, u32)> {
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    let page = load_sheet_page(
        &tables::RuneTable::for_protocol(protocol_id)
            .OUTPOINT_TO_RUNES
            .select(&outpoint_bytes),
        cursor,
        cursor_page_limit(limit),
    );
    let start = cursor.min(page.total_entries);
    let end = if page.next_offset == 0 {
        page.total_entries
    } else {
        page.next_offset
    };
    // A metadata miss must not fail the page (a cursor walk would stall on
    // it): report zeros/an empty output, as for an unknown outpoint.
    let (height, txindex, output) =
        outpoint_cursor_metadata(outpoint).unwrap_or_else(|_| (0, 0, Output::default()));
    Ok((
        OutpointResponse {
            balances: Some(page.sheet.into()),
            outpoint: Some(core_outpoint_to_proto(outpoint)),
            output: Some(output),
            height,
            txindex,
        },
        page.total_entries,
        page.next_offset,
        end - start,
    ))
}

pub fn protorunes_by_outpoint_cursor(input: &Vec<u8>) -> Result<OutpointCursorResponse> {
    let req = proto::protorune::OutpointCursorRequest::decode(input.as_ref())
        .map_err(|_| anyhow!("malformed request"))?;
    let protocol_tag: u128 = req
        .protocol
        .ok_or_else(|| anyhow!("missing protocol"))?
        .into();
    let outpoint = OutPoint {
        txid: bitcoin::blockdata::transaction::Txid::from_byte_array(
            <Vec<u8> as AsRef<[u8]>>::as_ref(&req.txid).try_into()?,
        ),
        vout: req.vout,
    };
    let (response, total_entries, next_cursor, _) =
        outpoint_cursor_page(&outpoint, protocol_tag, req.cursor, req.limit)?;
    Ok(OutpointCursorResponse {
        outpoint: Some(response),
        total_entries,
        next_cursor,
    })
}

pub fn protorunes_by_address_cursor(input: &Vec<u8>) -> Result<WalletCursorResponse> {
    let req = proto::protorune::ProtorunesWalletCursorRequest::decode(input.as_ref())
        .map_err(|_| anyhow!("malformed request"))?;
    let protocol_tag: u128 = req
        .protocol_tag
        .ok_or_else(|| anyhow!("missing protocol_tag"))?
        .into();
    // Read the address's outpoint list by index rather than `get_list()`, so a
    // page only touches the positions it scans.
    let list = tables::OUTPOINTS_FOR_ADDRESS.select(&req.wallet);
    let total_outpoints = list.length();
    let mut budget = cursor_page_limit(req.limit);
    let mut position = req.outpoint_cursor;
    let mut entry_cursor = req.entry_cursor;
    let mut scanned: u32 = 0;
    let mut result = WalletCursorResponse {
        total_outpoints,
        ..Default::default()
    };
    while position < total_outpoints && scanned < MAX_CURSOR_SCAN_OUTPOINTS && budget > 0 {
        scanned += 1;
        let mut cursor = Cursor::new(list.select_index(position).get().as_ref().clone());
        let outpoint = consensus_decode::<bitcoin::blockdata::transaction::OutPoint>(&mut cursor)?;
        // Same "still spendable by this address" test as the legacy view.
        let spendable_by = tables::OUTPOINT_SPENDABLE_BY
            .select(&outpoint_to_bytes(&outpoint)?)
            .get();
        // Skip spent outpoints and, cheaply (one length read, before the
        // metadata lookup), outpoints with no protocol balances at all.
        let sheet_len = tables::RuneTable::for_protocol(protocol_tag)
            .OUTPOINT_TO_RUNES
            .select(&outpoint_to_bytes(&outpoint)?)
            .keyword("/runes")
            .length();
        if spendable_by.len() != req.wallet.len() || sheet_len == 0 {
            position += 1;
            entry_cursor = 0;
            continue;
        }
        let (response, _, next_entry, read) =
            outpoint_cursor_page(&outpoint, protocol_tag, entry_cursor, budget)?;
        budget = budget.saturating_sub(read);
        // Like the legacy export, outpoints with no balances are left out.
        if response
            .balances
            .as_ref()
            .map_or(false, |b| !b.entries.is_empty())
        {
            result.outpoints.push(response);
        }
        if next_entry != 0 {
            // This outpoint's sheet continues on the next page.
            entry_cursor = next_entry;
            break;
        }
        position += 1;
        entry_cursor = 0;
    }
    result.done = position >= total_outpoints;
    if !result.done {
        result.next_outpoint_cursor = position;
        result.next_entry_cursor = entry_cursor;
    }
    Ok(result)
}
