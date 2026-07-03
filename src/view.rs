use crate::message::AlkaneMessageContext;
use crate::network::set_view_mode;
use crate::tables::{TRACES, TRACES_BY_HEIGHT};
use crate::unwrap as unwrap_view;
use crate::utils::{
    alkane_id_to_outpoint, alkane_inventory_pointer, balance_pointer, credit_balances,
    debit_balances, disable_touched_storage_collector, drain_touched_storage,
    enable_touched_storage_collector, pipe_storagemap_to,
};
use crate::vm::instance::AlkanesInstance;
use crate::vm::runtime::AlkanesRuntimeContext;
use crate::vm::utils::{
    get_alkane_binary, prepare_context, run_after_special, run_special_cellpacks, sequence_pointer,
};
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::parcel::AlkaneTransfer;
use alkanes_support::proto;
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
    blockdata::block::Header, Amount, Block, BlockHash, CompactTarget, OutPoint, ScriptBuf,
    Sequence, Transaction, TxIn, TxMerkleNode, TxOut, Witness,
};
use metashrew_core::index_pointer::{AtomicPointer, IndexPointer};
#[allow(unused_imports)]
use metashrew_core::{println, stdio::stdout};
use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use prost::Message;
use protorune::balance_sheet::MintableDebit;
use protorune::message::{MessageContext, MessageContextParcel};
use protorune::tables::RUNES;
use protorune::view;
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations};
use protorune_support::rune_transfer::RuneTransfer;
use protorune_support::utils::{consensus_decode, decode_varint_list};
use std::collections::BTreeMap;
#[allow(unused_imports)]
use std::fmt::Write;
use std::io::Cursor;
use std::sync::{Arc, LazyLock, Mutex};

pub fn parcels_from_protobuf(v: proto::alkanes::MultiSimulateRequest) -> Vec<MessageContextParcel> {
    v.parcels.into_iter().map(parcel_from_protobuf).collect()
}

pub fn parcel_from_protobuf(v: proto::alkanes::MessageContextParcel) -> MessageContextParcel {
    let mut result = MessageContextParcel::default();
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
            id: v.id.clone().unwrap().into(),
            value: v.value.clone().unwrap().into(),
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

pub fn plain_parcel_from_cellpack(cellpack: Cellpack) -> MessageContextParcel {
    let mut result = MessageContextParcel::default();
    result.block = default_block();
    result.transaction = default_transaction();
    result.calldata = cellpack.encipher();
    result
}

pub fn call_view(id: &AlkaneId, inputs: &Vec<u128>, fuel: u64) -> Result<Vec<u8>> {
    let (response, _gas_used) = simulate_parcel(
        &plain_parcel_from_cellpack(Cellpack {
            target: id.clone(),
            inputs: inputs.clone(),
        }),
        fuel,
    )?;
    Ok(response.data)
}

pub fn unwrap(height: u128) -> Result<Vec<u8>> {
    Ok(unwrap_view::view(height).unwrap().encode_to_vec())
}

pub fn call_multiview(ids: &[AlkaneId], inputs: &Vec<Vec<u128>>, fuel: u64) -> Result<Vec<u8>> {
    let calldata: Vec<_> = ids
        .into_iter()
        .enumerate()
        .map(|(i, id)| {
            plain_parcel_from_cellpack(Cellpack {
                target: id.clone(),
                inputs: inputs[i].clone(),
            })
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

pub fn get_statics(id: &AlkaneId) -> (String, String) {
    // Try to get from cache first
    if let Ok(cache) = STATICS_CACHE.lock() {
        if let Some(cached_values) = cache.get(id) {
            return cached_values.clone();
        }
    }

    // If not in cache, fetch the values
    let name = call_view(id, &vec![NAME_OPCODE], STATIC_FUEL)
        .and_then(|v| Ok(String::from_utf8(v)))
        .unwrap_or_else(|_| Ok(String::from("{REVERT}")))
        .unwrap();
    let symbol = call_view(id, &vec![SYMBOL_OPCODE], STATIC_FUEL)
        .and_then(|v| Ok(String::from_utf8(v)))
        .unwrap_or_else(|_| Ok(String::from("{REVERT}")))
        .unwrap();

    // Store in cache
    if let Ok(mut cache) = STATICS_CACHE.lock() {
        cache.insert(id.clone(), (name.clone(), symbol.clone()));
    }

    (name, symbol)
}

pub fn to_alkanes_balances(
    balances: protorune_support::proto::protorune::BalanceSheet,
) -> protorune_support::proto::protorune::BalanceSheet {
    const MAX_SIMULATE_CALLS: usize = 300;
    let mut clone = balances.clone();
    let mut simulate_calls = 0;
    for entry in &mut clone.entries {
        let block: u128 = entry
            .rune
            .clone()
            .unwrap()
            .rune_id
            .clone()
            .unwrap()
            .height
            .unwrap()
            .into();
        if (block == 2 || block == 4 || block == 32) {
            if simulate_calls < MAX_SIMULATE_CALLS {
                (
                    entry.rune.as_mut().unwrap().name,
                    entry.rune.as_mut().unwrap().symbol,
                ) = get_statics(&from_protobuf(
                    entry.rune.as_ref().unwrap().rune_id.clone().unwrap(),
                ));
                entry.rune.as_mut().unwrap().spacers = 0;
                simulate_calls += 1;
            } else {
                let id = entry.rune.as_ref().unwrap().rune_id.as_ref().unwrap();
                let name_from_id = format!(
                    "{:?}:{:?}",
                    id.height.as_ref().unwrap(),
                    id.txindex.as_ref().unwrap()
                );
                entry.rune.as_mut().unwrap().name = name_from_id.clone();
                entry.rune.as_mut().unwrap().symbol = name_from_id.clone();
            }
        }
    }
    clone
}

pub fn to_alkanes_from_runes(
    runes: Vec<protorune_support::proto::protorune::Rune>,
) -> Vec<protorune_support::proto::protorune::Rune> {
    runes
        .into_iter()
        .map(|mut v| {
            let block: u128 = v.clone().rune_id.clone().unwrap().height.unwrap().into();
            if block == 2 || block == 4 || block == 32 {
                (v.name, v.symbol) = get_statics(&from_protobuf(v.rune_id.clone().unwrap()));
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
    let request = protorune_support::proto::protorune::OutpointWithProtocol::decode(&**input)?;
    view::protorunes_by_outpoint(input).and_then(|mut response| {
        if into_u128(request.protocol.unwrap_or_else(|| {
            <u128 as Into<protorune_support::proto::protorune::Uint128>>::into(1u128)
        })) == AlkaneMessageContext::protocol_tag()
        {
            response.balances = Some(
                to_alkanes_balances(response.balances.unwrap_or_else(|| {
                    protorune_support::proto::protorune::BalanceSheet::default()
                }))
                .clone(),
            );
        }
        Ok(response)
    })
}

pub fn to_alkanes_outpoints(
    v: Vec<protorune_support::proto::protorune::OutpointResponse>,
) -> Vec<protorune_support::proto::protorune::OutpointResponse> {
    let mut cloned = v.clone();
    for item in &mut cloned {
        item.balances =
            Some(
                to_alkanes_balances(item.balances.clone().unwrap_or_else(|| {
                    protorune_support::proto::protorune::BalanceSheet::default()
                }))
                .clone(),
            );
    }
    cloned
}

pub fn sequence() -> Result<Vec<u8>> {
    Ok(sequence_pointer(&AtomicPointer::default())
        .get_value::<u128>()
        .to_le_bytes()
        .to_vec())
}

pub fn protorunes_by_address(
    input: &Vec<u8>,
) -> Result<protorune_support::proto::protorune::WalletResponse> {
    let request = protorune_support::proto::protorune::ProtorunesWalletRequest::decode(&**input)?;
    view::protorunes_by_address(input).and_then(|mut response| {
        if into_u128(request.protocol_tag.unwrap_or_else(|| {
            <u128 as Into<protorune_support::proto::protorune::Uint128>>::into(1u128)
        })) == AlkaneMessageContext::protocol_tag()
        {
            response.outpoints = to_alkanes_outpoints(response.outpoints.clone());
        }
        Ok(response)
    })
}

pub fn protorunes_by_address2(
    input: &Vec<u8>,
) -> Result<protorune_support::proto::protorune::WalletResponse> {
    let request = protorune_support::proto::protorune::ProtorunesWalletRequest::decode(&**input)?;

    #[cfg(feature = "cache")]
    {
        // Check if we have a cached response for this address
        let cached_response = protorune::tables::CACHED_WALLET_RESPONSE
            .select(&request.wallet)
            .get();

        if !cached_response.is_empty() {
            // Use the cached response if available
            match protorune_support::proto::protorune::WalletResponse::decode(&*cached_response) {
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
    view::protorunes_by_address2(input).and_then(|mut response| {
        if into_u128(request.protocol_tag.unwrap_or_else(|| {
            <u128 as Into<protorune_support::proto::protorune::Uint128>>::into(1u128)
        })) == AlkaneMessageContext::protocol_tag()
        {
            response.outpoints = to_alkanes_outpoints(response.outpoints.clone());
        }
        Ok(response)
    })
}

pub fn protorunes_by_height(
    input: &Vec<u8>,
) -> Result<protorune_support::proto::protorune::RunesResponse> {
    let request = protorune_support::proto::protorune::ProtorunesByHeightRequest::decode(&**input)?;
    view::protorunes_by_height(input).and_then(|mut response| {
        if into_u128(request.protocol_tag.unwrap_or_else(|| {
            <u128 as Into<protorune_support::proto::protorune::Uint128>>::into(1u128)
        })) == AlkaneMessageContext::protocol_tag()
        {
            response.runes = to_alkanes_from_runes(response.runes.clone());
        }
        Ok(response)
    })
}

pub fn alkanes_id_to_outpoint(input: &Vec<u8>) -> Result<AlkaneIdToOutpointResponse> {
    let request = AlkaneIdToOutpointRequest::decode(&**input)?;
    let mut response = AlkaneIdToOutpointResponse::default();
    let outpoint = alkane_id_to_outpoint(&request.id.clone().unwrap().into())?;
    // get the human readable txid (LE byte order), but comes out as a string
    let hex_string = outpoint.txid.to_string();
    // convert the hex string to a byte array
    response.txid = hex::decode(hex_string).unwrap();
    response.vout = outpoint.vout;
    return Ok(response);
}

pub fn getinventory(req: &AlkaneInventoryRequest) -> Result<AlkaneInventoryResponse> {
    let mut result: AlkaneInventoryResponse = AlkaneInventoryResponse::default();
    let alkane_inventory = alkane_inventory_pointer(&req.id.clone().unwrap().into());
    result.alkanes = alkane_inventory
        .get_list()
        .into_iter()
        .map(|alkane_held| -> proto::alkanes::AlkaneTransfer {
            let id = alkanes_support::id::AlkaneId::parse(&mut Cursor::new(
                alkane_held.as_ref().clone(),
            ))
            .unwrap();
            let balance_pointer = balance_pointer(
                &mut AtomicPointer::default(),
                &req.id.clone().unwrap().into(),
                &id,
            );
            let balance = balance_pointer.get_value::<u128>();
            (AlkaneTransfer {
                id: id,
                value: balance,
            })
            .into()
        })
        .collect::<Vec<proto::alkanes::AlkaneTransfer>>();
    Ok(result)
}

pub fn getstorageat(req: &AlkaneStorageRequest) -> Result<AlkaneStorageResponse> {
    let mut result: AlkaneStorageResponse = AlkaneStorageResponse::default();
    let alkane_storage_pointer = IndexPointer::from_keyword("/alkanes/")
        .select(&crate::utils::from_protobuf(req.id.clone().unwrap()).into())
        .keyword("/storage/")
        .select(&req.path);
    result.value = alkane_storage_pointer.get().to_vec();
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
        let trace = proto::alkanes::AlkanesTrace::decode(trace.as_slice())?;
        let block_event = proto::alkanes::AlkanesBlockEvent {
            txindex: txindex as u64,
            outpoint: Some(proto::alkanes::Outpoint {
                txid,
                vout: outpoint_decoded.vout,
            }),
            traces: Some(trace),
        };
        block_events.push(block_event);
    }

    let result = proto::alkanes::AlkanesBlockTraceEvent {
        events: block_events,
    };

    Ok(result.encode_to_vec())
}

pub fn trace(outpoint: &OutPoint) -> Result<Vec<u8>> {
    Ok(TRACES
        .select(&consensus_encode::<OutPoint>(&outpoint)?)
        .get()
        .as_ref()
        .clone())
}

pub fn simulate_safe(
    parcel: &MessageContextParcel,
    fuel: u64,
) -> Result<(ExtendedCallResponse, u64)> {
    set_view_mode();
    simulate_parcel(parcel, fuel)
}

pub fn meta_safe(parcel: &MessageContextParcel) -> Result<Vec<u8>> {
    set_view_mode();
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

pub fn simulate_parcel(
    parcel: &MessageContextParcel,
    fuel: u64,
) -> Result<(ExtendedCallResponse, u64)> {
    let list = decode_varint_list(&mut Cursor::new(parcel.calldata.clone()))?;
    let cellpack: Cellpack = list.clone().try_into()?;
    println!("{:?}, {:?}", list, cellpack);
    let context = Arc::new(Mutex::new(AlkanesRuntimeContext::from_parcel_and_cellpack(
        parcel, &cellpack,
    )));
    let mut atomic = parcel.atomic.derive(&IndexPointer::default());
    let (caller, myself, binary) = run_special_cellpacks(context.clone(), &cellpack)?;
    credit_balances(&mut atomic, &myself, &parcel.runes)?;
    prepare_context(context.clone(), &caller, &myself, false);
    let (response, gas_used) = run_after_special(context.clone(), binary, fuel)?;
    pipe_storagemap_to(
        &response.storage,
        &mut atomic.derive(&IndexPointer::from_keyword("/alkanes/").select(&myself.clone().into())),
    );
    let mut combined = parcel.runtime_balances.as_ref().clone();
    <BalanceSheet<AtomicPointer> as TryFrom<Vec<RuneTransfer>>>::try_from(parcel.runes.clone())?
        .pipe(&mut combined)?;
    let sheet = <BalanceSheet<AtomicPointer> as TryFrom<Vec<RuneTransfer>>>::try_from(
        response.alkanes.clone().into(),
    )?;
    combined.debit_mintable(&sheet, &mut atomic)?;
    debit_balances(&mut atomic, &myself, &response.alkanes)?;
    Ok((response, gas_used))
}

pub fn multi_simulate(
    parcels: &[MessageContextParcel],
    fuel: u64,
) -> Vec<Result<(ExtendedCallResponse, u64)>> {
    let mut responses: Vec<Result<(ExtendedCallResponse, u64)>> = vec![];
    for parcel in parcels {
        responses.push(simulate_parcel(parcel, fuel));
    }
    responses
}

pub fn multi_simulate_safe(
    parcels: &[MessageContextParcel],
    fuel: u64,
) -> Vec<Result<(ExtendedCallResponse, u64)>> {
    set_view_mode();
    multi_simulate(parcels, fuel)
}

// ---------------------------------------------------------------------------
// simulate_protostones / simulate_transaction — full per-tx sandbox replay
// ---------------------------------------------------------------------------
//
// Two-tier design:
//
//   * `simulate_protostones(input)` — lower-level. Caller supplies:
//       - alkane inputs (what flows in to the first protostone),
//       - a list of protostones (already enciphered, same bytes a
//         runestone's protocol field would carry),
//       - optional transaction bytes (for `self.transaction()` host call;
//         when omitted, a synthetic tx is constructed),
//       - optional block bytes (for `self.block()` host call; when
//         omitted, a minimal single-tx wrapper is built),
//       - optional storage overrides (pre-execution writes to specific
//         alkanes' storage slots, applied through the sandbox atomic).
//
//   * `simulate_transaction(input)` — wrapper. Decodes a PSBT or raw tx,
//     derives `alkane_inputs` by loading each spent outpoint's
//     OUTPOINT_TO_RUNES through the sandbox atomic (so they reflect live
//     chain state at `height`), and auto-synthesizes a realistic-looking
//     faux block chained off the last indexed header (vanity nonce with
//     CRC32-salted low bytes — see `synthesize_faux_block_for`). Then
//     delegates to `simulate_protostones` and returns the same response
//     shape.
//
// Both reuse the exact code path the indexer uses
// (`Protorune::index_protostones::<AlkaneMessageContext>`) — same fuel
// accounting, trace shape, edict processing, message dispatch. The view-
// mode toggles ensure zero on-disk side effects:
//
//   1. Sandbox `AtomicPointer` — never committed; writes discarded on drop.
//   2. `skip_protostone_persistence` — gates the terminal `save_balances`
//      + `clear_balances` block (those go through non-atomic IndexPointers
//      so the sandbox atomic alone wouldn't roll them back).
//   3. View-mode trace collector — `save_trace` pushes to a thread-local
//      buffer instead of TRACES + TRACES_BY_HEIGHT.
//   4. Final-balances sink — captures `proto_balances_by_output` at the
//      end of `index_protostones` for the caller.
//   5. Touched-storage collector — per-protostone, per-alkane storage
//      writes (from both `handle_message` and the extcall `Saveable::save`
//      path), bucketed using the per-iteration protostone index set by
//      protorune.

/// Native Rust response shape. The proto encoding in `simulate_protostones_proto`
/// / `simulate_transaction_proto` mirrors this. Mirrors the user's spec:
/// per-vout final balance sheets, per-protostone fuel + trace +
/// touched-storage map, total fuel, the actually-used block/tx bytes
/// (provided or synthesized).
#[derive(Debug, Clone)]
pub struct SimulateTransactionResponseNative {
    pub txid: String,
    pub height: u64,
    pub protostones: Vec<ProtostoneExecution>,
    pub final_balances_by_vout: Vec<VoutBalances>,
    pub total_fuel_used: u64,
    /// Consensus-encoded bytes of the tx that was actually run. For
    /// `simulate_transaction` this is whatever the caller supplied (the
    /// final-form Transaction extracted from a PSBT). For
    /// `simulate_protostones` this is either the caller-supplied tx or
    /// the synthesized one carrying their protostones in OP_RETURN.
    pub used_transaction_bytes: Vec<u8>,
    /// Consensus-encoded bytes of the block that was actually run. For
    /// `simulate_transaction` this is the auto-synthesized faux block
    /// (header chained off the previous indexed block, vanity nonce).
    /// For `simulate_protostones` this is the caller-supplied block or
    /// a minimal single-tx wrapper around the (provided or synthesized) tx.
    pub used_block_bytes: Vec<u8>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProtostoneExecution {
    pub index: usize,
    /// The synthetic outpoint the trace was keyed on
    /// (txid + shadow vout = num_real_outputs + 1 + index).
    pub outpoint: OutPoint,
    pub trace: alkanes_support::trace::Trace,
    pub fuel_used: u64,
    /// Final values of every storage slot touched by this protostone,
    /// keyed by the alkane that owns the slot (an alkane can extcall
    /// and mutate another alkane's storage during execution, so multiple
    /// entries per protostone are normal). Captures the LAST write per
    /// (alkane_id, key) pair across all `handle_message` + extcall
    /// returns that fired inside this protostone's processing.
    pub touched_storage: Vec<(AlkaneId, Vec<(Vec<u8>, Vec<u8>)>)>,
}

#[derive(Debug, Clone)]
pub struct VoutBalances {
    pub vout: u32,
    pub runes: Vec<(ProtoruneRuneId, u128)>,
}

/// Structured input for `simulate_protostones`. Mirrors the
/// SimulateProtostonesRequest proto. All fields except `height` are
/// optional from the caller's perspective: an empty `alkane_inputs`
/// means the first protostone sees no incoming alkanes; an empty
/// `protostones` means there's nothing to execute and the response
/// will be empty with `error = "no protostones"`.
#[derive(Debug, Clone)]
pub struct SimulateProtostonesInput {
    pub height: u64,
    pub alkane_inputs: Vec<alkanes_support::parcel::AlkaneTransfer>,
    /// Enciphered protostones — the same bytes a runestone's protocol
    /// field carries (i.e. `encode_varint_list(&protostones.encipher()?)`
    /// in Rust client code).
    pub protostones_bytes: Vec<u8>,
    pub transaction_bytes: Option<Vec<u8>>,
    pub block_bytes: Option<Vec<u8>>,
    pub storage_overrides: Vec<(AlkaneId, Vec<(Vec<u8>, Vec<u8>)>)>,
}

/// Decode either a PSBT-hex or raw-tx-hex string into a `bitcoin::Transaction`.
/// PSBT first (because a PSBT envelope is the more common mobile-app input);
/// fall back to raw-tx if PSBT deserialize fails.
pub fn decode_tx_or_psbt(input_hex: &str) -> Result<Transaction> {
    let bytes = hex::decode(input_hex.trim_start_matches("0x"))
        .map_err(|e| anyhow!("hex decode failed: {}", e))?;
    decode_tx_or_psbt_bytes(&bytes)
}

pub fn decode_tx_or_psbt_bytes(bytes: &[u8]) -> Result<Transaction> {
    if let std::result::Result::Ok(psbt) = bitcoin::Psbt::deserialize(bytes) {
        return Ok(psbt.extract_tx_unchecked_fee_rate());
    }
    bitcoin::consensus::deserialize::<Transaction>(bytes)
        .map_err(|e| anyhow!("input is neither valid PSBT nor raw tx: {}", e))
}

/// Minimal coinbase tx for a synthesized block. Real coinbase txs encode
/// the height in BIP34; we follow the same shape (script_sig = BIP34
/// little-endian height bytes) so a contract introspecting block.txdata[0]
/// sees a structurally-valid coinbase. Single zero-value output to a
/// nobody address.
fn synth_coinbase(height: u64) -> Transaction {
    let mut height_script = Vec::<u8>::new();
    // BIP34: push the height as a little-endian minimal byte string.
    let mut h = height;
    let mut hbytes = Vec::<u8>::new();
    if h == 0 {
        hbytes.push(0);
    } else {
        while h > 0 {
            hbytes.push((h & 0xff) as u8);
            h >>= 8;
        }
    }
    height_script.push(hbytes.len() as u8);
    height_script.extend_from_slice(&hbytes);
    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::from_bytes(height_script),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            script_pubkey: ScriptBuf::new(),
            value: Amount::from_sat(0),
        }],
    }
}

/// Compute a vanity nonce that's structurally a u32 but unambiguously
/// non-PoW. Layout: high 16 bits = 0xDEAD (visible marker that this is a
/// simulator-fabricated nonce), low 16 bits = CRC32(txid + height) & 0xFFFF
/// (salt so independent simulations don't collide on the marker alone).
/// A real miner would never produce 0xDEAD???? except by 1-in-65,536 luck;
/// any developer inspecting the block can see this is fake immediately.
fn vanity_nonce(seed_bytes: &[u8]) -> u32 {
    let crc = crc32(seed_bytes);
    0xDEAD_0000u32 | (crc & 0xFFFF)
}

/// Tiny CRC32 (poly 0xEDB88320) so we don't pull in a crate just for this.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Synthesize a faux block that wraps `tx` at the given `height`. Chains
/// off the previous indexed block's header if available (so prev_blockhash,
/// time, and bits look continuous with the chain); otherwise falls back
/// to zeroed defaults. The nonce is always the vanity sentinel — see
/// `vanity_nonce`. The merkle root is computed honestly over [coinbase,
/// tx] so a contract that recomputes it from block.txdata sees a
/// consistent block. Used by `simulate_transaction`'s auto-build path.
pub fn synthesize_faux_block_for(tx: Transaction, height: u64) -> Block {
    let coinbase = synth_coinbase(height);
    let txdata = vec![coinbase, tx];

    // Previous-block lookup: best-effort. If `etl::get_block(height-1)`
    // succeeds, chain off it. Otherwise default to zeros.
    let (prev_blockhash, prev_time, prev_bits) = if height > 0 {
        match crate::etl::get_block((height - 1) as u32) {
            std::result::Result::Ok(prev) => (
                prev.header.block_hash(),
                prev.header.time,
                prev.header.bits,
            ),
            std::result::Result::Err(_) => (
                BlockHash::all_zeros(),
                0,
                CompactTarget::from_consensus(0),
            ),
        }
    } else {
        (
            BlockHash::all_zeros(),
            0,
            CompactTarget::from_consensus(0),
        )
    };

    // Nonce seed: bind to the user tx's txid + height so two distinct
    // simulations don't end up with the same vanity value.
    let mut seed = Vec::<u8>::new();
    seed.extend_from_slice(&txdata[1].compute_txid()[..]);
    seed.extend_from_slice(&height.to_le_bytes());
    let nonce = vanity_nonce(&seed);

    let mut block = Block {
        header: Header {
            version: bitcoin::blockdata::block::Version::ONE,
            prev_blockhash,
            merkle_root: TxMerkleNode::all_zeros(),
            time: prev_time.saturating_add(600), // ~10 min — typical block interval
            bits: prev_bits,
            nonce,
        },
        txdata,
    };
    // Compute the honest merkle root now that txdata is fixed.
    block.header.merkle_root = block
        .compute_merkle_root()
        .unwrap_or_else(TxMerkleNode::all_zeros);
    block
}

/// Build a minimal single-tx Block wrapper (no faux-block heuristics).
/// Used by `simulate_protostones` when the caller doesn't supply a block
/// — the message-dispatch path doesn't introspect header fields beyond
/// `block.txdata`, so dummies are safe at this layer. Callers that need
/// realistic-looking blocks should pass `block_bytes` explicitly.
fn minimal_block_wrapper(tx: Transaction) -> Block {
    Block {
        header: Header {
            version: bitcoin::blockdata::block::Version::ONE,
            prev_blockhash: BlockHash::all_zeros(),
            merkle_root: TxMerkleNode::all_zeros(),
            time: 0,
            bits: CompactTarget::from_consensus(0),
            nonce: 0,
        },
        txdata: vec![tx],
    }
}

/// Build a synthetic tx that carries the given protostones in its
/// OP_RETURN. Used by `simulate_protostones` when the caller doesn't
/// provide their own transaction. Constructed shape:
///   * 1 input pointing at `synth_input_outpoint` (the caller will
///     pre-populate that outpoint's OUTPOINT_TO_RUNES with the
///     `alkane_inputs` via the sandbox atomic before
///     `index_protostones` runs).
///   * `num_dust_outputs` zero-value outputs (script_pubkey = empty) so
///     any protostone.pointer values referencing vouts 0..N have somewhere
///     real to land.
///   * 1 final OP_RETURN carrying the runestone with the protostones'
///     protocol field set to the decoded `protostones_values`.
fn synth_tx_carrying_protostones(
    synth_input_outpoint: OutPoint,
    num_dust_outputs: usize,
    protostones_values: Vec<u128>,
) -> Transaction {
    use ordinals::Runestone;
    let runestone_script: ScriptBuf = (Runestone {
        etching: None,
        mint: None,
        pointer: None,
        edicts: vec![],
        protocol: Some(protostones_values),
    })
    .encipher();

    let mut output: Vec<TxOut> = (0..num_dust_outputs)
        .map(|_| TxOut {
            script_pubkey: ScriptBuf::new(),
            value: Amount::from_sat(0),
        })
        .collect();
    output.push(TxOut {
        script_pubkey: runestone_script,
        value: Amount::from_sat(0),
    });

    Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: synth_input_outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output,
    }
}

/// Walk the storage overrides and write each entry into the sandbox
/// atomic at `/alkanes/<id>/storage/<key>`. The next read through the
/// host's `load_storage` will see these values (atomic reads walk the
/// pending writes top-down before falling through to the underlying
/// store).
fn apply_storage_overrides(
    atomic: &mut AtomicPointer,
    overrides: &[(AlkaneId, Vec<(Vec<u8>, Vec<u8>)>)],
) {
    for (alkane, entries) in overrides {
        for (k, v) in entries {
            let mut ptr = atomic.derive(
                &IndexPointer::from_keyword("/alkanes/")
                    .select(&alkane.clone().into())
                    .keyword("/storage/")
                    .select(k),
            );
            ptr.set(Arc::new(v.clone()));
        }
    }
}

fn txid_is_all_zeros(txid: &bitcoin::Txid) -> bool {
    txid[..].iter().all(|b| *b == 0)
}

/// Pre-populate the sandbox atomic with the alkane_inputs at the synth
/// input outpoint. Mirrors what the indexer would see if the spent
/// outpoint actually held these alkanes on-chain.
fn seed_input_balances(
    atomic: &mut AtomicPointer,
    table: &protorune::tables::RuneTable,
    outpoint: &OutPoint,
    inputs: &[alkanes_support::parcel::AlkaneTransfer],
) -> Result<()> {
    use protorune::balance_sheet::PersistentRecord;
    let runes: Vec<RuneTransfer> = inputs
        .iter()
        .map(|t| RuneTransfer {
            id: t.id.clone().into(),
            value: t.value,
        })
        .collect();
    let sheet =
        <BalanceSheet<AtomicPointer> as TryFrom<Vec<RuneTransfer>>>::try_from(runes)?;
    let key = consensus_encode(outpoint)?;
    sheet.save(&mut atomic.derive(&table.OUTPOINT_TO_RUNES.select(&key)), false);
    Ok(())
}

/// Decode the enciphered protostones bytes — the same bytes the
/// runestone's protocol field would carry — back into a `Vec<u128>`
/// suitable for `Runestone { protocol: Some(values) }`. Empty input
/// is allowed and produces an empty Vec.
fn decode_protostones_bytes(bytes: &[u8]) -> Result<Vec<u128>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    decode_varint_list(&mut Cursor::new(bytes.to_vec()))
        .map_err(|e| anyhow!("failed to decode enciphered protostones bytes: {}", e))
}

/// Convert the touched-storage buckets returned by `drain_touched_storage`
/// into the response shape (per-protostone ordered Vec, sorted by
/// AlkaneId so the bytes are deterministic for caching/diffing).
fn touched_storage_for_protostone(
    bucket: &std::collections::BTreeMap<AlkaneId, std::collections::BTreeMap<Vec<u8>, Vec<u8>>>,
) -> Vec<(AlkaneId, Vec<(Vec<u8>, Vec<u8>)>)> {
    bucket
        .iter()
        .map(|(id, entries)| {
            (
                id.clone(),
                entries
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            )
        })
        .collect()
}

/// RAII guard for the four process-global view-mode collectors. Disabling on
/// `Drop` guarantees they are turned off on EVERY exit from
/// `simulate_protostones` — the happy path, an early `?` return (e.g. a failing
/// `seed_input_balances`), or a panic unwind inside `index_protostones`. Without
/// this, a leaked `SKIP_PROTOSTONE_PERSISTENCE = true` would make the next real
/// `index_block` skip `save_balances`/`clear_balances` and silently corrupt the
/// index. Disable is idempotent, so it composes with the explicit drains below.
struct ViewCollectorGuard;
impl Drop for ViewCollectorGuard {
    fn drop(&mut self) {
        crate::trace::disable_view_trace_collector();
        protorune::disable_skip_protostone_persistence();
        protorune::disable_final_balances_sink();
        disable_touched_storage_collector();
    }
}

/// Lower-level entry point. Caller supplies the alkane inputs +
/// protostones directly. See the module doc above for the full design.
pub fn simulate_protostones(
    input: SimulateProtostonesInput,
) -> Result<SimulateTransactionResponseNative> {
    set_view_mode();

    let SimulateProtostonesInput {
        height,
        alkane_inputs,
        protostones_bytes,
        transaction_bytes,
        block_bytes,
        storage_overrides,
    } = input;

    let protostones_values = decode_protostones_bytes(&protostones_bytes)?;

    // Decode or synthesize the transaction. Synth shape: 1 input
    // pointing at a deterministic-but-fake outpoint (so the sandbox can
    // pre-populate its OUTPOINT_TO_RUNES) + enough dust outputs to give
    // protostone.pointer references a real vout to land on + the runestone
    // OP_RETURN carrying these protostones.
    let synth_input_outpoint = OutPoint {
        txid: bitcoin::Txid::all_zeros(),
        vout: 0,
    };
    let tx = if let Some(bytes) = transaction_bytes {
        decode_tx_or_psbt_bytes(&bytes)?
    } else {
        // Figure out how many dust outputs the protostones reference via
        // their pointer fields. We need (max_pointer + 1) real outputs
        // so the runestone OP_RETURN doesn't collide with a referenced
        // vout. Build a temporary Vec<Protostone> from the decoded
        // values to inspect pointers.
        use protorune_support::protostone::Protostone;
        let parsed_for_pointers = Protostone::decipher(&protostones_values).unwrap_or_default();
        let max_pointer: u32 = parsed_for_pointers
            .iter()
            .filter_map(|p| p.pointer)
            .max()
            .unwrap_or(0);
        let num_dust_outputs = (max_pointer as usize).saturating_add(1);
        synth_tx_carrying_protostones(
            synth_input_outpoint,
            num_dust_outputs,
            protostones_values.clone(),
        )
    };
    let txid = tx.compute_txid().to_string();
    let used_transaction_bytes = serialize(&tx);

    // Build a Runestone with the user-supplied protocol field, so
    // index_protostones sees exactly the protostones the caller asked
    // for (independent of what the tx's actual OP_RETURN says).
    use ordinals::Runestone;
    let runestone = Runestone {
        etching: None,
        mint: None,
        pointer: None,
        edicts: vec![],
        protocol: if protostones_values.is_empty() {
            None
        } else {
            Some(protostones_values.clone())
        },
    };
    let runestone_output_index = protorune::Protorune::get_runestone_output_index(&tx)
        .unwrap_or((tx.output.len() as u32).saturating_sub(1));

    // Decode or build the block.
    let block = if let Some(bytes) = block_bytes {
        bitcoin::consensus::deserialize::<Block>(&bytes)
            .map_err(|e| anyhow!("block bytes did not decode: {}", e))?
    } else {
        minimal_block_wrapper(tx.clone())
    };
    let used_block_bytes = serialize(&block);

    // Empty-protocol short-circuit. Saves a fuel-tank init + sandbox spin-up.
    if protostones_values.is_empty() {
        return Ok(SimulateTransactionResponseNative {
            txid,
            height,
            protostones: vec![],
            final_balances_by_vout: vec![],
            total_fuel_used: 0,
            used_transaction_bytes,
            used_block_bytes,
            error: Some("no protostones to simulate".to_string()),
        });
    }

    use crate::vm::fuel::FuelTank;
    FuelTank::initialize(&block, height as u32);

    // Activate the view-mode collectors. The guard disables them on ANY exit
    // (early `?` return / panic / normal), so they can never leak into the
    // indexer. The explicit drains + disables below still run on the normal
    // path to capture data; the guard is the safety net for the other paths.
    crate::trace::enable_view_trace_collector();
    protorune::enable_skip_protostone_persistence();
    protorune::enable_final_balances_sink();
    enable_touched_storage_collector();
    let _view_guard = ViewCollectorGuard;

    let mut sandbox_atomic = AtomicPointer::default();

    // Pre-execution seeds: storage overrides + input outpoint balances.
    apply_storage_overrides(&mut sandbox_atomic, &storage_overrides);
    let table = protorune::tables::RuneTable::for_protocol(
        <AlkaneMessageContext as MessageContext>::protocol_tag(),
    );
    // Only seed input balances when the synth-tx path is in effect:
    // there's no real chain state behind `synth_input_outpoint`, so
    // `index_protostones`'s load_sheet would see nothing. When the caller
    // supplies a real transaction, its inputs point at real outpoints
    // that already hold the right OUTPOINT_TO_RUNES state — seeding on
    // top of that would write duplicate entries (BalanceSheet::save
    // appends, not overwrites) and inflate the alkane balance going in.
    let used_synth_tx = txid_is_all_zeros(&tx.input[0].previous_output.txid);
    if used_synth_tx && !alkane_inputs.is_empty() {
        seed_input_balances(
            &mut sandbox_atomic,
            &table,
            &synth_input_outpoint,
            &alkane_inputs,
        )?;
    }

    let mut balances_by_output: BTreeMap<u32, BalanceSheet<AtomicPointer>> = BTreeMap::new();

    let outcome = protorune::Protorune::index_protostones::<AlkaneMessageContext>(
        &mut sandbox_atomic,
        &tx,
        0,
        &block,
        height,
        &runestone,
        runestone_output_index,
        &mut balances_by_output,
        protorune::default_output(&tx),
    );

    // Drain everything before returning so the next call starts clean.
    let collected_traces = crate::trace::drain_view_traces();
    let collected_balances = protorune::drain_final_balances();
    let touched_buckets = drain_touched_storage();
    crate::trace::disable_view_trace_collector();
    protorune::disable_skip_protostone_persistence();
    protorune::disable_final_balances_sink();
    disable_touched_storage_collector();

    if let Err(e) = outcome {
        return Ok(SimulateTransactionResponseNative {
            txid,
            height,
            protostones: vec![],
            final_balances_by_vout: vec![],
            total_fuel_used: 0,
            used_transaction_bytes,
            used_block_bytes,
            error: Some(format!("index_protostones failed: {}", e)),
        });
    }

    // Sum fuel from each trace's ReturnContext + RevertContext events.
    // Per-protostone fuel = sum of all return/revert frames in that
    // protostone's trace (top-level + extcalls).
    use alkanes_support::trace::TraceEvent;
    let sum_fuel = |tr: &alkanes_support::trace::Trace| -> u64 {
        let mut acc: u64 = 0;
        for ev in tr.0.lock().unwrap().iter() {
            match ev {
                TraceEvent::ReturnContext(r) | TraceEvent::RevertContext(r) => {
                    if r.fuel_used != u64::MAX {
                        acc = acc.saturating_add(r.fuel_used);
                    }
                }
                _ => {}
            }
        }
        acc
    };

    let mut total_fuel_used: u64 = 0;
    let protostones: Vec<ProtostoneExecution> = collected_traces
        .into_iter()
        .enumerate()
        .map(|(i, (op, tr))| {
            let fuel = sum_fuel(&tr);
            total_fuel_used = total_fuel_used.saturating_add(fuel);
            let touched = touched_buckets
                .get(i)
                .map(touched_storage_for_protostone)
                .unwrap_or_default();
            ProtostoneExecution {
                index: i,
                outpoint: op,
                trace: tr,
                fuel_used: fuel,
                touched_storage: touched,
            }
        })
        .collect();

    let final_balances_by_vout: Vec<VoutBalances> = collected_balances
        .into_iter()
        .map(|(vout, sheet)| VoutBalances {
            vout,
            runes: sheet
                .balances()
                .iter()
                .map(|(id, amt)| (id.clone(), *amt))
                .collect(),
        })
        .collect();

    Ok(SimulateTransactionResponseNative {
        txid,
        height,
        protostones,
        final_balances_by_vout,
        total_fuel_used,
        used_transaction_bytes,
        used_block_bytes,
        error: None,
    })
}

/// Higher-level entry point. Wraps `simulate_protostones`: decodes the
/// PSBT/raw-tx input, derives `alkane_inputs` from the spent outpoints'
/// live OUTPOINT_TO_RUNES state, auto-synthesizes a faux block, and
/// delegates. Returns the same response shape.
pub fn simulate_transaction(
    input_hex: &str,
    height: u64,
) -> Result<SimulateTransactionResponseNative> {
    simulate_transaction_with_overrides(input_hex, height, Vec::new())
}

pub fn simulate_transaction_with_overrides(
    input_hex: &str,
    height: u64,
    storage_overrides: Vec<(AlkaneId, Vec<(Vec<u8>, Vec<u8>)>)>,
) -> Result<SimulateTransactionResponseNative> {
    set_view_mode();

    let tx = decode_tx_or_psbt(input_hex)?;
    let tx_bytes = serialize(&tx);
    let txid = tx.compute_txid().to_string();

    // No runestone → nothing to simulate. Return an empty but well-formed response.
    use ordinals::{Artifact, Runestone};
    let runestone = match Runestone::decipher(&tx) {
        Some(Artifact::Runestone(r)) => r,
        _ => {
            let faux_block = synthesize_faux_block_for(tx.clone(), height);
            return Ok(SimulateTransactionResponseNative {
                txid,
                height,
                protostones: vec![],
                final_balances_by_vout: vec![],
                total_fuel_used: 0,
                used_transaction_bytes: tx_bytes,
                used_block_bytes: serialize(&faux_block),
                error: Some("no runestone in transaction".to_string()),
            });
        }
    };

    // Re-encode the runestone's protocol field as enciphered bytes for
    // the lower-level call. The Runestone we just deciphered already
    // carries `protocol: Option<Vec<u128>>` directly — no need to round-trip
    // through a Vec<Protostone>.
    let protocol_values = runestone.protocol.clone().unwrap_or_default();
    use protorune_support::utils::encode_varint_list;
    let protostones_bytes = if protocol_values.is_empty() {
        Vec::new()
    } else {
        encode_varint_list(&protocol_values)
    };

    // Derive alkane_inputs from spent outpoints' live OUTPOINT_TO_RUNES.
    // We load through a sandbox atomic over the real table so we don't
    // perturb on-disk state (load_sheet only reads; even atomically derived).
    let table = protorune::tables::RuneTable::for_protocol(
        <AlkaneMessageContext as MessageContext>::protocol_tag(),
    );
    let probe_atomic = AtomicPointer::default();
    let mut combined: Vec<alkanes_support::parcel::AlkaneTransfer> = Vec::new();
    for input in &tx.input {
        use protorune::balance_sheet::load_sheet;
        let sheet = load_sheet(&mut probe_atomic.derive(
            &table
                .OUTPOINT_TO_RUNES
                .select(&consensus_encode(&input.previous_output)?),
        ));
        for (rune_id, balance) in sheet.balances().iter() {
            if *balance == 0 {
                continue;
            }
            combined.push(alkanes_support::parcel::AlkaneTransfer {
                id: AlkaneId::from(rune_id.clone()),
                value: *balance,
            });
        }
    }

    // Auto-synthesize the faux block.
    let faux_block = synthesize_faux_block_for(tx.clone(), height);
    let faux_block_bytes = serialize(&faux_block);

    let mut response = simulate_protostones(SimulateProtostonesInput {
        height,
        alkane_inputs: combined,
        protostones_bytes,
        transaction_bytes: Some(tx_bytes.clone()),
        block_bytes: Some(faux_block_bytes.clone()),
        storage_overrides,
    })?;

    // Overwrite txid/used_bytes (just to be defensive — they should
    // already match since we passed them through).
    response.txid = txid;
    response.used_transaction_bytes = tx_bytes;
    response.used_block_bytes = faux_block_bytes;
    Ok(response)
}

// ---------------------------------------------------------------------------
// simulate_block — full block replay in a shared sandbox.
// ---------------------------------------------------------------------------
//
// Drives every tx in `block` through the same per-tx code path
// simulatetransaction uses, with a SINGLE shared sandbox AtomicPointer
// that carries writes across tx boundaries — so tx[1] sees the
// OUTPOINT_TO_RUNES writes that tx[0]'s `save_balances` made (and the
// `clear_balances` clearings on tx[0]'s spent inputs). Matches the
// intra-block atomicity the live indexer enforces, but with zero
// on-disk side effects: the shared sandbox is never committed past
// the outer sandbox layer we install at the start.
//
// Checkpoint discipline:
//   * Construct AtomicPointer::default()  // depth=1 (the auto-flush layer)
//   * sandbox.checkpoint()                // depth=2 (our sandbox layer)
//   * For each tx with a runestone:
//       sandbox.checkpoint()              // depth=3 (tx layer)
//       run index_protostones
//       on Ok → sandbox.commit()          // depth=2, merge tx into sandbox
//       on Err → sandbox.rollback()       // depth=2, discard tx writes
//   * Drop sandbox — depth=2 contents discarded; auto-flush layer (depth=1)
//     never had any writes piped to it, so nothing reaches the global store.
//
// `should_skip_protostone_persistence` is OFF for the simulateblock path
// (we WANT save_balances + clear_balances to fire so the next tx in the
// block sees the right OUTPOINT_TO_RUNES state). Both now route through
// the atomic — see the corresponding `clear_balances` change in
// `crates/protorune/src/lib.rs`.

#[derive(Debug, Clone)]
pub struct SimulateBlockResponseNative {
    pub block_hash: String,
    pub height: u64,
    /// One entry per tx in `block.txdata`, in block order. Coinbase /
    /// no-runestone txs surface as empty-shape entries with `error =
    /// Some("no_runestone")` so `txs[i]` aligns with `block.txdata[i]`.
    pub txs: Vec<SimulateTransactionResponseNative>,
    pub total_fuel_used: u64,
    pub used_block_bytes: Vec<u8>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SimulateBlockInput {
    pub height: u64,
    pub block_bytes: Vec<u8>,
    pub storage_overrides: Vec<(AlkaneId, Vec<(Vec<u8>, Vec<u8>)>)>,
}

/// Build an empty-shape per-tx response for txs we skip (coinbase /
/// no-runestone). Keeps `txs[i]` aligned with `block.txdata[i]` so
/// callers can graph asset flow against the block's tx order.
fn empty_tx_response(tx: &Transaction, height: u64, reason: &str) -> SimulateTransactionResponseNative {
    SimulateTransactionResponseNative {
        txid: tx.compute_txid().to_string(),
        height,
        protostones: vec![],
        final_balances_by_vout: vec![],
        total_fuel_used: 0,
        used_transaction_bytes: serialize(tx),
        used_block_bytes: Vec::new(),
        error: Some(reason.to_string()),
    }
}

pub fn simulate_block(input: SimulateBlockInput) -> Result<SimulateBlockResponseNative> {
    set_view_mode();

    let block: Block = bitcoin::consensus::deserialize::<Block>(&input.block_bytes)
        .map_err(|e| anyhow!("simulateblock: block bytes did not decode: {}", e))?;
    let used_block_bytes = input.block_bytes;
    let block_hash = block.block_hash().to_string();

    use crate::vm::fuel::FuelTank;
    FuelTank::initialize(&block, input.height as u32);

    // Construct the shared sandbox atomic + push the sandbox layer
    // checkpoint. Per-tx checkpoints nest INSIDE this one, so per-tx
    // commits only ever merge into the sandbox layer — never into the
    // auto-flush root.
    let mut sandbox_atomic = AtomicPointer::default();
    sandbox_atomic.checkpoint(); // depth=2: sandbox layer

    // Apply pre-execution storage overrides into the sandbox layer.
    apply_storage_overrides(&mut sandbox_atomic, &input.storage_overrides);

    let table = protorune::tables::RuneTable::for_protocol(
        <AlkaneMessageContext as MessageContext>::protocol_tag(),
    );

    let mut txs: Vec<SimulateTransactionResponseNative> = Vec::with_capacity(block.txdata.len());
    let mut total_fuel_used: u64 = 0;

    use ordinals::{Artifact, Runestone};
    for (txindex, tx) in block.txdata.iter().enumerate() {
        // Skip coinbase outright — it never carries a runestone and the
        // alkanes code path doesn't run for it.
        if txindex == 0 && tx.is_coinbase() {
            txs.push(empty_tx_response(tx, input.height, "coinbase"));
            continue;
        }

        let runestone = match Runestone::decipher(tx) {
            Some(Artifact::Runestone(r)) => r,
            _ => {
                txs.push(empty_tx_response(tx, input.height, "no_runestone"));
                continue;
            }
        };
        let runestone_output_index = match protorune::Protorune::get_runestone_output_index(tx) {
            std::result::Result::Ok(i) => i,
            std::result::Result::Err(_) => {
                txs.push(empty_tx_response(tx, input.height, "runestone_output_index_unknown"));
                continue;
            }
        };

        // Per-tx checkpoint on the shared sandbox: any writes during this
        // tx's index_protostones land in a temporary layer that gets
        // merged into the sandbox layer on success (visible to the next
        // tx) or discarded on failure.
        sandbox_atomic.checkpoint(); // depth=3: tx layer

        // Activate the per-protostone view-mode collectors fresh for
        // this tx. drain afterwards so they don't bleed across txs.
        crate::trace::enable_view_trace_collector();
        protorune::enable_final_balances_sink();
        enable_touched_storage_collector();
        // Persistence flag is OFF for simulateblock so save_balances +
        // clear_balances fire (through the atomic) and the next tx in
        // the block sees the right outpoint state.
        protorune::disable_skip_protostone_persistence();

        let mut balances_by_output: BTreeMap<u32, BalanceSheet<AtomicPointer>> = BTreeMap::new();
        let outcome = protorune::Protorune::index_protostones::<AlkaneMessageContext>(
            &mut sandbox_atomic,
            tx,
            txindex as u32,
            &block,
            input.height,
            &runestone,
            runestone_output_index,
            &mut balances_by_output,
            protorune::default_output(tx),
        );

        let collected_traces = crate::trace::drain_view_traces();
        let collected_balances = protorune::drain_final_balances();
        let touched_buckets = drain_touched_storage();
        crate::trace::disable_view_trace_collector();
        protorune::disable_final_balances_sink();
        disable_touched_storage_collector();

        match outcome {
            std::result::Result::Ok(_) => {
                // Merge this tx's writes into the sandbox layer so the
                // next tx in the block sees them.
                sandbox_atomic.commit(); // depth=2
            }
            std::result::Result::Err(e) => {
                // Discard this tx's partial writes; the next tx sees
                // pre-failure state.
                sandbox_atomic.rollback(); // depth=2
                txs.push(SimulateTransactionResponseNative {
                    txid: tx.compute_txid().to_string(),
                    height: input.height,
                    protostones: vec![],
                    final_balances_by_vout: vec![],
                    total_fuel_used: 0,
                    used_transaction_bytes: serialize(tx),
                    used_block_bytes: Vec::new(),
                    error: Some(format!("index_protostones failed: {}", e)),
                });
                continue;
            }
        }

        // Build the per-tx response. Same shape simulate_transaction
        // returns.
        use alkanes_support::trace::TraceEvent;
        let sum_fuel = |tr: &alkanes_support::trace::Trace| -> u64 {
            let mut acc: u64 = 0;
            for ev in tr.0.lock().unwrap().iter() {
                match ev {
                    TraceEvent::ReturnContext(r) | TraceEvent::RevertContext(r) => {
                        if r.fuel_used != u64::MAX {
                            acc = acc.saturating_add(r.fuel_used);
                        }
                    }
                    _ => {}
                }
            }
            acc
        };

        let mut tx_fuel: u64 = 0;
        let protostones: Vec<ProtostoneExecution> = collected_traces
            .into_iter()
            .enumerate()
            .map(|(i, (op, tr))| {
                let fuel = sum_fuel(&tr);
                tx_fuel = tx_fuel.saturating_add(fuel);
                let touched = touched_buckets
                    .get(i)
                    .map(touched_storage_for_protostone)
                    .unwrap_or_default();
                ProtostoneExecution {
                    index: i,
                    outpoint: op,
                    trace: tr,
                    fuel_used: fuel,
                    touched_storage: touched,
                }
            })
            .collect();

        let final_balances_by_vout: Vec<VoutBalances> = collected_balances
            .into_iter()
            .map(|(vout, sheet)| VoutBalances {
                vout,
                runes: sheet
                    .balances()
                    .iter()
                    .map(|(id, amt)| (id.clone(), *amt))
                    .collect(),
            })
            .collect();

        total_fuel_used = total_fuel_used.saturating_add(tx_fuel);
        txs.push(SimulateTransactionResponseNative {
            txid: tx.compute_txid().to_string(),
            height: input.height,
            protostones,
            final_balances_by_vout,
            total_fuel_used: tx_fuel,
            used_transaction_bytes: serialize(tx),
            used_block_bytes: Vec::new(),
            error: None,
        });
    }

    // Reference `table` to silence any unused-binding lint if no tx with
    // a runestone existed — table is wired through so the read path uses
    // the right RuneTable shape per AlkaneMessageContext::protocol_tag.
    let _ = table;

    // sandbox_atomic drops here. depth=2 holds all committed-tx writes
    // and depth=1 is the original auto-flush root (empty). Neither
    // reaches `set()` because no outer commit is called.
    Ok(SimulateBlockResponseNative {
        block_hash,
        height: input.height,
        txs,
        total_fuel_used,
        used_block_bytes,
        error: None,
    })
}

// ---------------------------------------------------------------------------
// proto-encoded entry points — what `lib.rs` calls from the no_mangle
// wasm exports `simulateprotostones()` and `simulatetransaction()`.
// ---------------------------------------------------------------------------

fn alkane_id_to_proto(id: &AlkaneId) -> proto::alkanes::AlkaneId {
    proto::alkanes::AlkaneId {
        block: Some(proto::alkanes::Uint128 {
            lo: id.block as u64,
            hi: (id.block >> 64) as u64,
        }),
        tx: Some(proto::alkanes::Uint128 {
            lo: id.tx as u64,
            hi: (id.tx >> 64) as u64,
        }),
    }
}

fn alkane_id_from_proto(v: &proto::alkanes::AlkaneId) -> AlkaneId {
    AlkaneId {
        block: v
            .block
            .as_ref()
            .map(|u| ((u.hi as u128) << 64) | (u.lo as u128))
            .unwrap_or(0),
        tx: v
            .tx
            .as_ref()
            .map(|u| ((u.hi as u128) << 64) | (u.lo as u128))
            .unwrap_or(0),
    }
}

fn alkane_transfer_from_proto(
    v: &proto::alkanes::AlkaneTransfer,
) -> alkanes_support::parcel::AlkaneTransfer {
    alkanes_support::parcel::AlkaneTransfer {
        id: alkane_id_from_proto(v.id.as_ref().unwrap()),
        value: v
            .value
            .as_ref()
            .map(|u| ((u.hi as u128) << 64) | (u.lo as u128))
            .unwrap_or(0),
    }
}

fn overrides_from_proto(
    v: &[proto::alkanes::StorageOverride],
) -> Vec<(AlkaneId, Vec<(Vec<u8>, Vec<u8>)>)> {
    v.iter()
        .map(|o| {
            let id = o
                .alkane
                .as_ref()
                .map(alkane_id_from_proto)
                .unwrap_or(AlkaneId { block: 0, tx: 0 });
            let entries: Vec<(Vec<u8>, Vec<u8>)> = o
                .entries
                .iter()
                .map(|kv| (kv.key.clone(), kv.value.clone()))
                .collect();
            (id, entries)
        })
        .collect()
}

fn response_to_proto(r: SimulateTransactionResponseNative) -> proto::alkanes::SimulateTransactionResponse {
    proto::alkanes::SimulateTransactionResponse {
        txid: r.txid,
        height: r.height,
        protostones: r
            .protostones
            .into_iter()
            .map(|p| proto::alkanes::ProtostoneExecution {
                index: p.index as u32,
                outpoint: Some(proto::alkanes::Outpoint {
                    txid: p.outpoint.txid[..].to_vec(),
                    vout: p.outpoint.vout,
                }),
                trace: Some(p.trace.into()),
                fuel_used: p.fuel_used,
                touched_storage: p
                    .touched_storage
                    .into_iter()
                    .map(|(id, entries)| proto::alkanes::TouchedStorage {
                        alkane: Some(alkane_id_to_proto(&id)),
                        entries: entries
                            .into_iter()
                            .map(|(k, v)| proto::alkanes::KeyValuePair { key: k, value: v })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
        final_balances_by_vout: r
            .final_balances_by_vout
            .into_iter()
            .map(|vb| proto::alkanes::VoutBalances {
                vout: vb.vout,
                balances: vb
                    .runes
                    .into_iter()
                    .map(|(id, amt)| {
                        let alkane = AlkaneId::from(id);
                        proto::alkanes::AlkaneTransfer {
                            id: Some(alkane_id_to_proto(&alkane)),
                            value: Some(proto::alkanes::Uint128 {
                                lo: amt as u64,
                                hi: (amt >> 64) as u64,
                            }),
                        }
                    })
                    .collect(),
            })
            .collect(),
        total_fuel_used: r.total_fuel_used,
        used_transaction: r.used_transaction_bytes,
        used_block: r.used_block_bytes,
        error: r.error.unwrap_or_default(),
    }
}

/// Entry point for the `simulateprotostones()` wasm export.
pub fn simulate_protostones_proto(input: &[u8]) -> Result<Vec<u8>> {
    let req = proto::alkanes::SimulateProtostonesRequest::decode(input)
        .map_err(|e| anyhow!("decode SimulateProtostonesRequest: {}", e))?;
    let native = simulate_protostones(SimulateProtostonesInput {
        height: req.height,
        alkane_inputs: req
            .alkane_inputs
            .iter()
            .map(alkane_transfer_from_proto)
            .collect(),
        protostones_bytes: req.protostones,
        transaction_bytes: if req.transaction.is_empty() {
            None
        } else {
            Some(req.transaction)
        },
        block_bytes: if req.block.is_empty() {
            None
        } else {
            Some(req.block)
        },
        storage_overrides: overrides_from_proto(&req.storage_overrides),
    })?;
    Ok(response_to_proto(native).encode_to_vec())
}

/// Entry point for the `simulatetransaction()` wasm export.
pub fn simulate_transaction_proto(input: &[u8]) -> Result<Vec<u8>> {
    let req = proto::alkanes::SimulateTransactionRequest::decode(input)
        .map_err(|e| anyhow!("decode SimulateTransactionRequest: {}", e))?;
    // Accept either raw tx bytes or PSBT — decode_tx_or_psbt_bytes
    // handles both. We re-hex-encode here just to satisfy the existing
    // `simulate_transaction(&str, u64)` signature without adding a
    // bytes-flavored sibling. Cheap.
    let hex_input = hex::encode(&req.transaction);
    let native = simulate_transaction_with_overrides(
        &hex_input,
        req.height,
        overrides_from_proto(&req.storage_overrides),
    )?;
    Ok(response_to_proto(native).encode_to_vec())
}

/// Entry point for the `simulateblock()` wasm export.
pub fn simulate_block_proto(input: &[u8]) -> Result<Vec<u8>> {
    let req = proto::alkanes::SimulateBlockRequest::decode(input)
        .map_err(|e| anyhow!("decode SimulateBlockRequest: {}", e))?;
    let native = simulate_block(SimulateBlockInput {
        height: req.height,
        block_bytes: req.block,
        storage_overrides: overrides_from_proto(&req.storage_overrides),
    })?;
    let resp = proto::alkanes::SimulateBlockResponse {
        block_hash: native.block_hash,
        height: native.height,
        txs: native.txs.into_iter().map(response_to_proto).collect(),
        total_fuel_used: native.total_fuel_used,
        used_block: native.used_block_bytes,
        error: native.error.unwrap_or_default(),
    };
    Ok(resp.encode_to_vec())
}

pub fn getbytecode(input: &Vec<u8>) -> Result<Vec<u8>> {
    let request = alkanes_support::proto::alkanes::BytecodeRequest::decode(&**input)?;
    let alkane_id = request.id.clone().unwrap();
    let alkane_id = crate::utils::from_protobuf(alkane_id);

    // Get the bytecode from the storage
    let bytecode = get_alkane_binary(
        metashrew_core::index_pointer::IndexPointer::from_keyword("/alkanes/"),
        &alkane_id,
    )?;

    // Return the uncompressed bytecode. Note that gzip bomb is not possible since these bytecodes are upper bound by the size of the Witness
    if bytecode.len() > 0 {
        Ok(bytecode.to_vec())
    } else {
        Err(anyhow!("No bytecode found for the given AlkaneId"))
    }
}

pub fn getblock(input: &Vec<u8>) -> Result<Vec<u8>> {
    use crate::etl;
    use alkanes_support::proto::alkanes::{BlockRequest, BlockResponse};
    use prost::Message;

    let request = BlockRequest::decode(&**input)?;
    let height = request.height;

    // Get the block from the etl module
    let block = etl::get_block(height)?;

    // Create a response with the block data
    let response = BlockResponse {
        block: serialize(&block),
        height,
    };

    // Serialize the response
    Ok(response.encode_to_vec())
}
