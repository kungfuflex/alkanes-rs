use crate::tables::RuneTable;
use crate::{
    balance_sheet::{load_sheet_page, MAX_VIEW_SHEET_ENTRIES},
    tables,
};
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

pub fn outpoint_to_bytes(outpoint: &OutPoint) -> Result<Vec<u8>> {
    Ok(outpoint_encode(outpoint)?)
}

pub fn core_outpoint_to_proto(outpoint: &OutPoint) -> Outpoint {
    Outpoint {
        txid: outpoint.txid.as_byte_array().to_vec().clone(),
        vout: outpoint.vout,
    }
}

/// Position of `txid` in the block at `height`, as recorded in
/// `HEIGHT_TO_TRANSACTION_IDS`. Tries the `TXID_TO_TXINDEX` index first and
/// confirms it with a single list read. If that misses, falls back to scanning
/// the block's whole txid list, as this used to do for every outpoint. The
/// genesis outpoint, for example, is in the list but not the index. Either way
/// the result is identical to the scan; the index only saves reads.
fn txindex_in_block(txid: &bitcoin::Txid, height: u64) -> Result<u128> {
    let txid_bytes = txid.as_byte_array().to_vec();
    let block_txids = tables::RUNES
        .HEIGHT_TO_TRANSACTION_IDS
        .select_value::<u64>(height);
    // Raw bytes rather than `get_value`, which panics on a value that is not
    // 4 bytes; anything unexpected just takes the fallback.
    let raw = tables::RUNES.TXID_TO_TXINDEX.select(&txid_bytes).get();
    if let Ok(hint) = <[u8; 4]>::try_from(raw.as_slice()) {
        let hint = u32::from_le_bytes(hint);
        if block_txids.select_index(hint).get().as_ref() == &txid_bytes {
            return Ok(hint as u128);
        }
    }
    Ok(block_txids
        .get_list()
        .into_iter()
        .position(|v| v.as_ref().to_vec() == txid_bytes)
        .ok_or("")
        .map_err(|_| anyhow!("txid not indexed in table"))? as u128)
}

/// Build an `OutpointResponse` from one page of the balance sheet at
/// `sheet_ptr`. Reads at most `MAX_VIEW_SHEET_ENTRIES` entries; see
/// `load_sheet_page` for the `offset`/`limit` semantics.
fn sheet_to_outpoint_response<T: KeyValuePointer + Clone>(
    outpoint: &OutPoint,
    sheet_ptr: &T,
    offset: u32,
    limit: u32,
) -> Result<OutpointResponse> {
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    let page = load_sheet_page(sheet_ptr, offset, limit);
    let balance_sheet = page.sheet;

    let mut height: u128 = tables::RUNES
        .OUTPOINT_TO_HEIGHT
        .select(&outpoint_bytes)
        .get_value::<u64>()
        .into();
    let mut txindex: u128 = txindex_in_block(&outpoint.txid, height as u64)?;

    // Derived from the lowest rune id in the returned page, so on a sheet
    // spanning several pages each page can report a different height/txindex.
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
        total_entries: page.total_entries,
        next_offset: page.next_offset,
    })
}

/// First page of the outpoint's protorune balance sheet (at most
/// `MAX_VIEW_SHEET_ENTRIES` entries). Wallet views use this per outpoint.
pub fn protorune_outpoint_to_outpoint_response(
    outpoint: &OutPoint,
    protocol_id: u128,
) -> Result<OutpointResponse> {
    protorune_outpoint_to_outpoint_response_paged(outpoint, protocol_id, 0, 0)
}

pub fn protorune_outpoint_to_outpoint_response_paged(
    outpoint: &OutPoint,
    protocol_id: u128,
    offset: u32,
    limit: u32,
) -> Result<OutpointResponse> {
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    sheet_to_outpoint_response(
        outpoint,
        &tables::RuneTable::for_protocol(protocol_id)
            .OUTPOINT_TO_RUNES
            .select(&outpoint_bytes),
        offset,
        limit,
    )
}

pub fn rune_outpoint_to_outpoint_response(outpoint: &OutPoint) -> Result<OutpointResponse> {
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    sheet_to_outpoint_response(
        outpoint,
        &tables::RUNES.OUTPOINT_TO_RUNES.select(&outpoint_bytes),
        0,
        0,
    )
}

pub fn outpoint_to_outpoint_response(outpoint: &OutPoint) -> Result<OutpointResponse> {
    rune_outpoint_to_outpoint_response(outpoint)
}

/// Most stored outpoint-list positions one wallet view call scans.
///
/// `OUTPOINTS_FOR_ADDRESS` is append-only: every output ever paid to the
/// address stays in it, spent or not. Without a bound, a view over a busy
/// address walks its entire history while holding a metashrew view permit.
pub const MAX_VIEW_WALLET_OUTPOINTS: u32 = 10_000;

/// Most balance-sheet entries one wallet view call loads, summed over the
/// outpoints it returns. Each outpoint is still capped at
/// `MAX_VIEW_SHEET_ENTRIES` by `load_sheet_page`.
pub const MAX_VIEW_WALLET_ENTRIES: u32 = 10_000;

/// One bounded page of an address's spendable outpoints. `sheet_for` picks
/// the balance-sheet table (runes, or a protorune protocol) for an outpoint.
///
/// Scans stored positions `[offset, offset + n)`, where `n` is `limit`
/// clamped to `1..=MAX_VIEW_WALLET_OUTPOINTS` (0 = the cap). Spent outpoints
/// are skipped but still count as scanned. The page ends early, before the
/// outpoint that would overrun `MAX_VIEW_WALLET_ENTRIES`. The first outpoint
/// returned on a page is always included, so every page makes progress. An
/// outpoint whose own sheet exceeds the per-outpoint cap comes back as its
/// first page, with `total_entries`/`next_offset` set, so the caller can
/// fetch the rest from the by-outpoint view.
fn wallet_page<F>(wallet: &Vec<u8>, offset: u32, limit: u32, sheet_for: F) -> Result<WalletResponse>
where
    F: Fn(&Vec<u8>) -> metashrew_core::index_pointer::IndexPointer,
{
    let list = tables::OUTPOINTS_FOR_ADDRESS.select(wallet);
    let total = list.length();
    let limit = if limit == 0 {
        MAX_VIEW_WALLET_OUTPOINTS
    } else {
        limit.min(MAX_VIEW_WALLET_OUTPOINTS)
    };
    let end = offset.min(total).saturating_add(limit).min(total);
    let mut result = WalletResponse::default();
    let mut entries_left = MAX_VIEW_WALLET_ENTRIES;
    let mut i = offset.min(total);
    while i < end {
        let mut cursor = Cursor::new(list.select_index(i).get().as_ref().clone());
        let outpoint = consensus_decode::<bitcoin::blockdata::transaction::OutPoint>(&mut cursor)?;
        let outpoint_bytes = outpoint_to_bytes(&outpoint)?;
        let spendable_by = tables::OUTPOINT_SPENDABLE_BY.select(&outpoint_bytes).get();
        if wallet.len() == spendable_by.len() {
            let sheet_ptr = sheet_for(&outpoint_bytes);
            let cost = sheet_ptr
                .keyword("/runes")
                .length()
                .min(MAX_VIEW_SHEET_ENTRIES);
            if cost > entries_left && !result.outpoints.is_empty() {
                break;
            }
            result
                .outpoints
                .push(sheet_to_outpoint_response(&outpoint, &sheet_ptr, 0, 0)?);
            entries_left = entries_left.saturating_sub(cost);
        }
        i += 1;
    }
    result.total_outpoints = total;
    result.next_offset = if i < total { i } else { 0 };
    Ok(result)
}

pub fn runes_by_address(input: &Vec<u8>) -> Result<WalletResponse> {
    match proto::protorune::WalletRequest::decode(input.as_ref()).ok() {
        Some(req) => wallet_page(&req.wallet, req.offset, req.limit, |outpoint_bytes| {
            tables::RUNES.OUTPOINT_TO_RUNES.select(outpoint_bytes)
        }),
        None => Ok(WalletResponse::default()),
    }
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
            protorune_outpoint_to_outpoint_response_paged(&outpoint, protocol_tag, req.offset, req.limit)
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
    match proto::protorune::ProtorunesWalletRequest::decode(input.as_ref()).ok() {
        Some(req) => {
            let protocol_tag: u128 = req.protocol_tag.unwrap().into();
            let table = tables::RuneTable::for_protocol(protocol_tag);
            wallet_page(&req.wallet, req.offset, req.limit, |outpoint_bytes| {
                table.OUTPOINT_TO_RUNES.select(outpoint_bytes)
            })
        }
        None => Ok(WalletResponse::default()),
    }
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
