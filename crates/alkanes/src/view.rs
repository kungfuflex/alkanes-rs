use crate::message::AlkaneMessageContext;
use crate::network::set_view_mode;
use crate::tables::{TRACES, TRACES_BY_HEIGHT};
use crate::{unwrap as unwrap_view};
use crate::utils::{
    alkane_id_to_outpoint, alkane_inventory_pointer, balance_pointer, credit_balances,
    debit_balances, pipe_storagemap_to,
};
use crate::vm::instance::AlkanesInstance;
use crate::vm::runtime::AlkanesRuntimeContext;
use crate::vm::utils::{
    prepare_context, run_after_special, run_special_cellpacks, sequence_pointer,
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
use metashrew_support::environment::RuntimeEnvironment;
use bitcoin::consensus::encode::serialize;
use bitcoin::hashes::Hash;
use bitcoin::{
    blockdata::block::Header, Block, BlockHash, CompactTarget, OutPoint, Transaction, TxMerkleNode,
};
use metashrew_support::index_pointer::{AtomicPointer, IndexPointer};

use metashrew_support::{index_pointer::KeyValuePointer, utils::consensus_encode};
use protobuf::{Message, MessageField};
use protorune::balance_sheet::MintableDebit;
use protorune::message::{MessageContext, MessageContextParcel};
use protorune::tables::RuneTable;

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
use std::marker::PhantomData;

pub fn parcels_from_protobuf<E: RuntimeEnvironment + Clone + Default>(v: proto::alkanes::MultiSimulateRequest) -> Vec<MessageContextParcel<AlkaneMessageContext<E>>> {
    v.parcels.into_iter().map(parcel_from_protobuf).collect()
}

pub fn parcel_from_protobuf<E: RuntimeEnvironment + Clone + Default>(v: proto::alkanes::MessageContextParcel) -> MessageContextParcel<AlkaneMessageContext<E>> {
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
            id: v.id.into_option().unwrap().clone().into(),
            value: v.value.into_option().unwrap().into(),
        })
        .collect::<Vec<RuneTransfer>>();
    result.pointer = v.pointer;
    result.refund_pointer = v.refund_pointer;
    result._phantom = PhantomData;
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

pub fn plain_parcel_from_cellpack<E: RuntimeEnvironment + Clone + Default>(cellpack: Cellpack) -> MessageContextParcel<AlkaneMessageContext<E>> {
    let mut result = MessageContextParcel::default();
    result.block = default_block();
    result.transaction = default_transaction();
    result.calldata = cellpack.encipher();
    result._phantom = PhantomData;
    result
}

pub fn call_view<E: RuntimeEnvironment + Clone + Default + 'static>(id: &AlkaneId, inputs: &Vec<u128>, fuel: u64) -> Result<Vec<u8>> {
    let (response, _gas_used) = simulate_parcel(
        &plain_parcel_from_cellpack::<E>(Cellpack {
            target: id.clone(),
            inputs: inputs.clone(),
        }),
        fuel,
    )?;
    Ok(response.data)
}

pub fn unwrap<E: RuntimeEnvironment + Clone + Default>(height: u128) -> Result<Vec<u8>> {
    Ok(unwrap_view::view::<E>(height).unwrap().write_to_bytes()?)
}

pub fn call_multiview<E: RuntimeEnvironment + Clone + Default + 'static>(ids: &[AlkaneId], inputs: &Vec<Vec<u128>>, fuel: u64) -> Result<Vec<u8>> {
    let calldata: Vec<_> = ids
        .into_iter()
        .enumerate()
        .map(|(i, id)| {
            plain_parcel_from_cellpack::<E>(Cellpack {
                target: id.clone(),
                inputs: inputs[i].clone(),
            })
        })
        .collect();

    let results = multi_simulate::<E>(&calldata, fuel);
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

pub fn get_statics<E: RuntimeEnvironment + Clone + Default + 'static>(id: &AlkaneId) -> (String, String) {
    // Try to get from cache first
    if let Ok(cache) = STATICS_CACHE.lock() {
        if let Some(cached_values) = cache.get(id) {
            return cached_values.clone();
        }
    }

    // If not in cache, fetch the values
    let name = call_view::<E>(id, &vec![NAME_OPCODE], STATIC_FUEL)
        .and_then(|v| Ok(String::from_utf8(v)))
        .unwrap_or_else(|_| Ok(String::from("{REVERT}")))
        .unwrap();
    let symbol = call_view::<E>(id, &vec![SYMBOL_OPCODE], STATIC_FUEL)
        .and_then(|v| Ok(String::from_utf8(v)))
        .unwrap_or_else(|_| Ok(String::from("{REVERT}")))
        .unwrap();

    // Store in cache
    if let Ok(mut cache) = STATICS_CACHE.lock() {
        cache.insert(id.clone(), (name.clone(), symbol.clone()));
    }

    (name, symbol)
}

pub fn to_alkanes_balances<E: RuntimeEnvironment + Clone + Default + 'static>(
    balances: protorune_support::proto::protorune::BalanceSheet,
) -> protorune_support::proto::protorune::BalanceSheet {
    let mut clone = balances.clone();
    for entry in &mut clone.entries {
        let block: u128 = entry
            .rune
            .clone()
            .unwrap()
            .runeId
            .height
            .clone()
            .unwrap()
            .into();
        if block == 2 || block == 4 || block == 32 {
            (
                entry.rune.as_mut().unwrap().name,
                entry.rune.as_mut().unwrap().symbol,
            ) = get_statics::<E>(&from_protobuf(entry.rune.runeId.clone().unwrap()));
            entry.rune.as_mut().unwrap().spacers = 0;
        }
    }
    clone
}

pub fn to_alkanes_from_runes<E: RuntimeEnvironment + Clone + Default + 'static>(
    runes: Vec<protorune_support::proto::protorune::Rune>,
) -> Vec<protorune_support::proto::protorune::Rune> {
    runes
        .into_iter()
        .map(|mut v| {
            let block: u128 = v.clone().runeId.height.clone().unwrap().into();
            if block == 2 || block == 4 || block == 32 {
                (v.name, v.symbol) = get_statics::<E>(&from_protobuf(v.runeId.clone().unwrap()));
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

pub fn protorunes_by_outpoint<'a, E: RuntimeEnvironment + Clone + Default + 'a + 'static>(
    input: &'a Vec<u8>,
) -> Result<protorune_support::proto::protorune::OutpointResponse> {
    let request =
        protorune_support::proto::protorune::OutpointWithProtocol::parse_from_bytes(input)?;
    view::protorunes_by_outpoint::<AlkaneMessageContext<E>>(input).and_then(|mut response| {
        if into_u128(request.protocol.unwrap_or_else(|| {
            <u128 as Into<protorune_support::proto::protorune::Uint128>>::into(1u128)
        })) == (AlkaneMessageContext::<E>::protocol_tag())
        {
            response.balances =
                MessageField::some(
                    to_alkanes_balances::<E>(response.balances.unwrap_or_else(|| {
                        protorune_support::proto::protorune::BalanceSheet::new()
                    }))
                    .clone(),
                );
        }
        Ok(response)
    })
}

pub fn to_alkanes_outpoints<E: RuntimeEnvironment + Clone + Default + 'static>(
    v: Vec<protorune_support::proto::protorune::OutpointResponse>,
) -> Vec<protorune_support::proto::protorune::OutpointResponse> {
    let mut cloned = v.clone();
    for item in &mut cloned {
        item.balances = MessageField::some(
            to_alkanes_balances::<E>(
                item.balances
                    .clone()
                    .unwrap_or_else(|| protorune_support::proto::protorune::BalanceSheet::new()),
            )
            .clone(),
        );
    }
    cloned
}

pub fn sequence<E: RuntimeEnvironment + Clone + Default>() -> Result<Vec<u8>> {
    Ok(sequence_pointer(&AtomicPointer::<AlkaneMessageContext<E>>::default())
        .get_value::<u128>()
        .to_le_bytes()
        .to_vec())
}

pub fn protorunes_by_address<'a, E: RuntimeEnvironment + Clone + Default + 'a + 'static>(
    input: &'a Vec<u8>,
) -> Result<protorune_support::proto::protorune::WalletResponse> {
    let request =
        protorune_support::proto::protorune::ProtorunesWalletRequest::parse_from_bytes(input)?;
    view::protorunes_by_address::<AlkaneMessageContext<E>>(input).and_then(|mut response| {
        if into_u128(request.protocol_tag.unwrap_or_else(|| {
            <u128 as Into<protorune_support::proto::protorune::Uint128>>::into(1u128)
        })) == (AlkaneMessageContext::<E>::protocol_tag())        {
            response.outpoints = to_alkanes_outpoints::<E>(response.outpoints.clone());
        }
        Ok(response)
    })
}

pub fn protorunes_by_address2<'a, E: RuntimeEnvironment + Clone + Default + 'a + 'static>(
    input: &'a Vec<u8>,
) -> Result<protorune_support::proto::protorune::WalletResponse> {
    let request =
        protorune_support::proto::protorune::ProtorunesWalletRequest::parse_from_bytes(input)?;

    #[cfg(feature = "cache")]
    {
        // Check if we have a cached response for this address
        let cached_response = protorune::tables::CACHED_WALLET_RESPONSE
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
                    E::log(&format!("Error parsing cached wallet response: {:?}", e));
                    // Fall back to computing the response if parsing fails
                }
            }
        }
    }

    // If no cached response or parsing failed, compute it
    view::protorunes_by_address2::<AlkaneMessageContext<E>>(input).and_then(|mut response| {
        if into_u128(request.protocol_tag.unwrap_or_else(|| {
            <u128 as Into<protorune_support::proto::protorune::Uint128>>::into(1u128)
        })) == (AlkaneMessageContext::<E>::protocol_tag())        {
            response.outpoints = to_alkanes_outpoints::<E>(response.outpoints.clone());
        }
        Ok(response)
    })
}

pub fn protorunes_by_height<'a, E: RuntimeEnvironment + Clone + Default + 'a + 'static>(
    input: &'a Vec<u8>,
) -> Result<protorune_support::proto::protorune::RunesResponse> {
    let request =
        protorune_support::proto::protorune::ProtorunesByHeightRequest::parse_from_bytes(input)?;
    view::protorunes_by_height::<AlkaneMessageContext<E>>(input).and_then(|mut response| {
        if into_u128(request.protocol_tag.unwrap_or_else(|| {
            <u128 as Into<protorune_support::proto::protorune::Uint128>>::into(1u128)
        })) == (AlkaneMessageContext::<E>::protocol_tag())        {
            response.runes = to_alkanes_from_runes::<E>(response.runes.clone());
        }
        Ok(response)
    })
}

pub fn alkanes_id_to_outpoint<E: RuntimeEnvironment + Clone + Default>(input: &Vec<u8>) -> Result<AlkaneIdToOutpointResponse> {
    let request = AlkaneIdToOutpointRequest::parse_from_bytes(input)?;
    let mut response = AlkaneIdToOutpointResponse::new();
    let outpoint = alkane_id_to_outpoint::<E>(&request.id.unwrap().into())?;
    // get the human readable txid (LE byte order), but comes out as a string
    let hex_string = outpoint.txid.to_string();
    // convert the hex string to a byte array
    response.txid = hex::decode(hex_string).unwrap();
    response.vout = outpoint.vout;
    return Ok(response);
}

pub fn getinventory<E: RuntimeEnvironment + Clone + Default>(req: &AlkaneInventoryRequest) -> Result<AlkaneInventoryResponse> {
    let mut result: AlkaneInventoryResponse = AlkaneInventoryResponse::new();
    let alkane_inventory = alkane_inventory_pointer::<E>(&req.id.clone().unwrap().into());
    result.alkanes = alkane_inventory
        .get_list()
        .into_iter()
        .map(|alkane_held| -> proto::alkanes::AlkaneTransfer {
            let id = alkanes_support::id::AlkaneId::parse(&mut Cursor::new(
                alkane_held.as_ref().clone(),
            ))
            .unwrap();
            let balance_pointer = balance_pointer(
                &mut AtomicPointer::<AlkaneMessageContext<E>>::default(),
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

pub fn getstorageat<E: RuntimeEnvironment + Clone + Default>(req: &AlkaneStorageRequest) -> Result<AlkaneStorageResponse> {
    let mut result: AlkaneStorageResponse = AlkaneStorageResponse::new();
    let alkane_storage_pointer = IndexPointer::<AlkaneMessageContext<E>>::from_keyword("/alkanes/")
        .select(&crate::utils::from_protobuf(req.id.clone().unwrap()).into())
        .keyword("/storage/")
        .select(&req.path);
    result.value = alkane_storage_pointer.get().to_vec();
    Ok(result)
}

pub fn traceblock<E: RuntimeEnvironment + Clone + Default>(height: u32) -> Result<Vec<u8>> {
    let mut block_events: Vec<proto::alkanes::AlkanesBlockEvent> = vec![];
    for outpoint in TRACES_BY_HEIGHT.select_value(height as u64).get_list() {
        let op = outpoint.clone().to_vec();
        let outpoint_decoded = consensus_decode::<OutPoint>(&mut Cursor::new(op))?;
        let txid = outpoint_decoded.txid.as_byte_array().to_vec();
        let txindex: u32 = RuneTable::<AlkaneMessageContext<E>>::new().TXID_TO_TXINDEX.select(&txid).get_value();
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

pub fn simulate_safe<E: RuntimeEnvironment + Clone + Default + 'static>(
    parcel: &MessageContextParcel<AlkaneMessageContext<E>>,
    fuel: u64,
) -> Result<(ExtendedCallResponse, u64)> {
    set_view_mode();
    simulate_parcel(parcel, fuel)
}

pub fn meta_safe<E: RuntimeEnvironment + Clone + Default + 'static>(parcel: &MessageContextParcel<AlkaneMessageContext<E>>) -> Result<Vec<u8>> {
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

pub fn simulate_parcel<E: RuntimeEnvironment + Clone + Default + 'static>(
    parcel: &MessageContextParcel<AlkaneMessageContext<E>>,
    fuel: u64,
) -> Result<(ExtendedCallResponse, u64)> {
    let list = decode_varint_list(&mut Cursor::new(parcel.calldata.clone()))?;
    let cellpack: Cellpack = list.clone().try_into()?;
    E::log(&format!("{:?}, {:?}", list, cellpack));
    let context = Arc::new(Mutex::new(AlkanesRuntimeContext::from_parcel_and_cellpack(
        parcel, &cellpack,
    )));
    let mut atomic = parcel.atomic.derive(&IndexPointer::<AlkaneMessageContext<E>>::default());
    let (caller, myself, binary) = run_special_cellpacks(context.clone(), &cellpack)?;
    credit_balances(&mut atomic, &myself, &parcel.runes)?;
    prepare_context(context.clone(), &caller, &myself, false);
    let (response, gas_used) = run_after_special(context.clone(), binary, fuel)?;
    pipe_storagemap_to(
        &response.storage,
        &mut atomic.derive(&IndexPointer::<AlkaneMessageContext<E>>::from_keyword("/alkanes/").select(&myself.clone().into())),
    );
    let mut combined = parcel.runtime_balances.as_ref().clone();
    <BalanceSheet<AtomicPointer<AlkaneMessageContext<E>>> as TryFrom<Vec<RuneTransfer>>>::try_from(parcel.runes.clone())?
        .pipe(&mut combined)?;
    let sheet = <BalanceSheet<AtomicPointer<AlkaneMessageContext<E>>> as TryFrom<Vec<RuneTransfer>>>::try_from(
        response.alkanes.clone().into(),
    )?;
    combined.debit_mintable(&sheet, &mut atomic)?;
    debit_balances(&mut atomic, &myself, &response.alkanes)?;
    Ok((response, gas_used))
}

pub fn multi_simulate<E: RuntimeEnvironment + Clone + Default + 'static>(
    parcels: &[MessageContextParcel<AlkaneMessageContext<E>>],
    fuel: u64,
) -> Vec<Result<(ExtendedCallResponse, u64)>> {
    let mut responses: Vec<Result<(ExtendedCallResponse, u64)>> = vec![];
    for parcel in parcels {
        responses.push(simulate_parcel(parcel, fuel));
    }
    responses
}

pub fn multi_simulate_safe<E: RuntimeEnvironment + Clone + Default + 'static>(
    parcels: &[MessageContextParcel<AlkaneMessageContext<E>>],
    fuel: u64,
) -> Vec<Result<(ExtendedCallResponse, u64)>> {
    set_view_mode();
    multi_simulate(parcels, fuel)
}

pub fn getbytecode<E: RuntimeEnvironment + Clone + Default>(input: &Vec<u8>) -> Result<Vec<u8>> {
    let request = alkanes_support::proto::alkanes::BytecodeRequest::parse_from_bytes(input)?;
    let alkane_id = request.id.unwrap();
    let alkane_id = crate::utils::from_protobuf(alkane_id);

    // Get the bytecode from the storage
    let bytecode = metashrew_support::index_pointer::IndexPointer::<AlkaneMessageContext<E>>::from_keyword("/alkanes/")
        .select(&alkane_id.into())
        .get();

    // Return the uncompressed bytecode. Note that gzip bomb is not possible since these bytecodes are upper bound by the size of the Witness
    if bytecode.len() > 0 {
        Ok(alkanes_support::gz::decompress(bytecode.to_vec())?)
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
