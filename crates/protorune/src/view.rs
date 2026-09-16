use crate::tables::RuneTable;
use crate::{
    balance_sheet::{load_sheet, load_sheet_chunked},
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

pub fn protorune_outpoint_to_outpoint_response(
    outpoint: &OutPoint,
    protocol_id: u128,
) -> Result<OutpointResponse> {
    //    println!("protocol_id: {}", protocol_id);
    let outpoint_bytes = outpoint_to_bytes(outpoint)?;
    // v3 chunked-outpoint read.
    let balance_sheet = load_sheet_chunked(
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
    // v3 chunked-outpoint read.
    let balance_sheet = load_sheet_chunked(&tables::RUNES.OUTPOINT_TO_RUNES.select(&outpoint_bytes));

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
    // v3 chunked-outpoint read.
    let balance_sheet = load_sheet_chunked(&tables::RUNES.OUTPOINT_TO_RUNES.select(&outpoint_bytes));
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

/// View entry-point for the `runesbyaddress` JSON-RPC method.
///
/// Gated behind the `address-indexing` Cargo feature. When OFF (the
/// default), returns an explicit "feature not enabled" error so callers
/// know the canonical v3 mainnet wasm does NOT serve this view —
/// address-keyed lookups are served from esplora's UTXO API via espo
/// middleware. Operators who need an indexer-served address-by-x
/// surface must rebuild the wasm with `--features mainnet,address-indexing`.
///
/// Determinism note: the iteration order matches the chunked
/// `AddressOutpoints` proto, which is sorted by `(txid_le, vout)`
/// ascending by `address_index::write_address_index`. View consumers
/// can rely on byte-equal responses across nodes.
///
/// Performance note: the per-outpoint balance-sheet reads in the loop
/// below are independent and side-effect-free — a future PR will
/// dispatch them concurrently via `metashrew_core::view::spawn` once
/// that wrapper ships in the host. This PR keeps the synchronous loop
/// to avoid a forward dependency on the not-yet-shipped wasm-side
/// wrapper.
#[cfg(feature = "address-indexing")]
pub fn runes_by_address(input: &Vec<u8>) -> Result<WalletResponse> {
    use crate::address_index;
    let mut result: WalletResponse = WalletResponse::default();
    if let Some(req) = proto::protorune::WalletRequest::decode(input.as_ref()).ok() {
        let chunk = match address_index::load_chunk(&req.wallet) {
            Some(c) => c,
            None => return Ok(result),
        };
        // Iteration order is the on-disk chunk order, which is the
        // canonical (txid_le, vout) ascending order. See
        // `address_index::write_address_index` for the sort invariant.
        result.outpoints = chunk
            .outpoints
            .into_iter()
            .map(|op| -> Result<OutpointResponse> {
                let outpoint = OutPoint {
                    txid: bitcoin::Txid::from_byte_array(
                        <Vec<u8> as AsRef<[u8]>>::as_ref(&op.txid).try_into()?,
                    ),
                    vout: op.vout,
                };
                outpoint_to_outpoint_response(&outpoint)
            })
            .collect::<Result<Vec<OutpointResponse>>>()?;
    }
    Ok(result)
}

/// Stub: `runesbyaddress` is gated behind the `address-indexing` Cargo
/// feature. See the gated arm above for the full description.
#[cfg(not(feature = "address-indexing"))]
pub fn runes_by_address(_input: &Vec<u8>) -> Result<WalletResponse> {
    Err(anyhow!(
        "runesbyaddress requires --features address-indexing — recompile the alkanes wasm with the flag"
    ))
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

/// View entry-point for the `protorunesbyaddress` JSON-RPC method.
///
/// Gated behind the `address-indexing` Cargo feature. See the
/// `runes_by_address` doc-comment for the full feature gating policy
/// and determinism contract. The wasm-side `protorunesbyaddress`
/// export in `src/lib.rs` is itself feature-gated, so when the feature
/// is OFF the JSON-RPC layer will get a "view function not found"
/// error rather than reaching this stub — but we still return a clear
/// error for the rlib path (tests, the alkanes-rpc-core dispatch
/// layer when present, etc.).
///
/// IMPORTANT: this view is NOT the optimal way to query address
/// state in production. The intended primary path is "esplora UTXO
/// API → list of outpoints → `protorunesbyoutpoint` on each", which
/// scales to whatever esplora can already serve and is parallel by
/// construction. This view is a convenience for operators who
/// explicitly opted in.
#[cfg(feature = "address-indexing")]
pub fn protorunes_by_address(input: &Vec<u8>) -> Result<WalletResponse> {
    use crate::address_index;
    let mut result: WalletResponse = WalletResponse::default();
    if let Some(req) = proto::protorune::ProtorunesWalletRequest::decode(input.as_ref()).ok() {
        let chunk = match address_index::load_chunk(&req.wallet) {
            Some(c) => c,
            None => return Ok(result),
        };
        let protocol_tag: u128 = req.clone().protocol_tag.unwrap().into();
        // Iteration order is the on-disk chunk order. See
        // `address_index::write_address_index` for the sort invariant.
        //
        // TODO(view::spawn): each protorune_outpoint_to_outpoint_response
        // call is an independent storage read. Dispatch via
        // `metashrew_core::view::spawn` once the wasm-side wrapper
        // ships. Synchronous loop today; one-line change later.
        result.outpoints = chunk
            .outpoints
            .into_iter()
            .map(|op| -> Result<OutpointResponse> {
                let outpoint = OutPoint {
                    txid: bitcoin::Txid::from_byte_array(
                        <Vec<u8> as AsRef<[u8]>>::as_ref(&op.txid).try_into()?,
                    ),
                    vout: op.vout,
                };
                protorune_outpoint_to_outpoint_response(&outpoint, protocol_tag)
            })
            .collect::<Result<Vec<OutpointResponse>>>()?;
    }
    Ok(result)
}

/// Stub: `protorunesbyaddress` is gated behind the `address-indexing`
/// Cargo feature.
#[cfg(not(feature = "address-indexing"))]
pub fn protorunes_by_address(_input: &Vec<u8>) -> Result<WalletResponse> {
    Err(anyhow!(
        "protorunesbyaddress requires --features address-indexing — recompile the alkanes wasm with the flag"
    ))
}

/// Legacy linked-list-backed protorunes-by-address variant. The
/// linked-list table (`OUTPOINT_SPENDABLE_BY_ADDRESS`) is populated by
/// `index_spendables_ll`, which is NOT on the v3 default dispatch
/// path. We keep the symbol available under `address-indexing` for
/// any caller that explicitly opts into the linked-list flow, but the
/// canonical opt-in path goes through `protorunes_by_address` above
/// against the chunked /v3/addr/* state.
#[cfg(feature = "address-indexing")]
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

#[cfg(not(feature = "address-indexing"))]
pub fn protorunes_by_address2(_input: &Vec<u8>) -> Result<WalletResponse> {
    Err(anyhow!(
        "protorunesbyaddress2 requires --features address-indexing — recompile the alkanes wasm with the flag"
    ))
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
