use super::fuel::compute_extcall_fuel;
use super::{
    get_memory, read_arraybuffer, send_to_arraybuffer, sequence_pointer, AlkanesState, Extcall,
    Saveable, SaveableExtendedCallResponse,
};
use crate::utils::{balance_pointer, pipe_storagemap_to, transfer_from};
use crate::vm::{run_after_special, run_special_cellpacks};
use alkanes_support::{
    cellpack::Cellpack,
    id::AlkaneId,
    parcel::AlkaneTransferParcel,
    response::CallResponse,
    storage::StorageMap,
    trace::{TraceContext, TraceEvent, TraceResponse},
    utils::overflow_error,
};
#[allow(unused_imports)]
use anyhow::{anyhow, Result};
use bitcoin::{BlockHash, Transaction};
use metashrew_core::index_pointer::IndexPointer;
#[allow(unused_imports)]
use metashrew_core::{
    print, println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;
use num::traits::ToBytes;
use ordinals::Artifact;
use ordinals::Runestone;
use protorune_support::protostone::Protostone;

use crate::vm::fuel::{
    consume_fuel, fuel_extcall_deploy, fuel_per_store_byte, Fuelable, FUEL_BALANCE, FUEL_EXTCALL,
    FUEL_FUEL, FUEL_HEIGHT, FUEL_LOAD_BLOCK, FUEL_LOAD_TRANSACTION, FUEL_PER_LOAD_BYTE,
    FUEL_PER_REQUEST_BYTE, FUEL_SEQUENCE,
};
use protorune_support::utils::{consensus_encode, decode_varint_list};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use wasmi::*;

static DIESEL_MINTS_CACHE: LazyLock<Arc<RwLock<Option<Vec<u8>>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(None)));

pub fn clear_diesel_mints_cache() {
    if let Ok(mut cache) = DIESEL_MINTS_CACHE.try_write() {
        *cache = None;
    }
}

pub struct AlkanesHostFunctionsImpl(());

// New wrapper struct that ensures proper context management
pub struct SafeAlkanesHostFunctionsImpl(());

impl AlkanesHostFunctionsImpl {
    fn preserve_context(caller: &mut Caller<'_, AlkanesState>) {
        caller
            .data_mut()
            .context
            .lock()
            .unwrap()
            .message
            .atomic
            .checkpoint();
    }

    fn restore_context(caller: &mut Caller<'_, AlkanesState>) {
        caller
            .data_mut()
            .context
            .lock()
            .unwrap()
            .message
            .atomic
            .commit();
    }

    // Get the current depth of the checkpoint stack
    fn get_checkpoint_depth(caller: &mut Caller<'_, AlkanesState>) -> usize {
        caller
            .data_mut()
            .context
            .lock()
            .unwrap()
            .message
            .atomic
            .checkpoint_depth()
    }
    pub(super) fn _abort<'a>(caller: Caller<'_, AlkanesState>) {
        AlkanesHostFunctionsImpl::abort(caller, 0, 0, 0, 0);
    }
    pub(super) fn abort<'a>(mut caller: Caller<'_, AlkanesState>, _: i32, _: i32, _: i32, _: i32) {
        caller.data_mut().had_failure = true;
    }
    pub(super) fn request_storage<'a>(
        caller: &mut Caller<'_, AlkanesState>,
        k: i32,
    ) -> Result<i32> {
        let (bytes_processed, result) = {
            let mem = get_memory(caller)?;
            let key = {
                let data = mem.data(&caller);
                read_arraybuffer(data, k)?
            };
            let myself = caller.data_mut().context.lock().unwrap().myself.clone();
            let result: i32 = caller
                .data_mut()
                .context
                .lock()
                .unwrap()
                .message
                .atomic
                .keyword("/alkanes/")
                .select(&myself.into())
                .keyword("/storage/")
                .select(&key)
                .get()
                .len()
                .try_into()?;
            ((result as u64) + (key.len() as u64), result)
        };

        let fuel_cost =
            overflow_error((bytes_processed as u64).checked_mul(FUEL_PER_REQUEST_BYTE))?;
        #[cfg(feature = "debug-log")]
        {
            println!(
                "request_storage: key_size={} bytes, result_size={} bytes, fuel_cost={}",
                bytes_processed - (result as u64),
                result,
                fuel_cost
            );
        }

        consume_fuel(caller, fuel_cost)?;
        Ok(result)
    }
    pub(super) fn load_storage<'a>(
        caller: &mut Caller<'_, AlkanesState>,
        k: i32,
        v: i32,
    ) -> Result<i32> {
        let (bytes_processed, value) = {
            let mem = get_memory(caller)?;
            let key = {
                let data = mem.data(&caller);
                read_arraybuffer(data, k)?
            };
            let value = {
                let myself = caller.data_mut().context.lock().unwrap().myself.clone();
                (&caller.data_mut().context.lock().unwrap().message)
                    .atomic
                    .keyword("/alkanes/")
                    .select(&myself.into())
                    .keyword("/storage/")
                    .select(&key)
                    .get()
            };
            (key.len() + value.len(), value)
        };

        let fuel_cost = overflow_error((bytes_processed as u64).checked_mul(FUEL_PER_LOAD_BYTE))?;
        #[cfg(feature = "debug-log")]
        {
            println!(
                "load_storage: key_size={} bytes, value_size={} bytes, total_size={} bytes, fuel_cost={}",
                bytes_processed - value.len(), value.len(), bytes_processed, fuel_cost
            );
        }

        consume_fuel(caller, fuel_cost)?;
        send_to_arraybuffer(caller, v.try_into()?, value.as_ref())
    }
    pub(super) fn request_context(caller: &mut Caller<'_, AlkanesState>) -> Result<i32> {
        let result: i32 = caller
            .data_mut()
            .context
            .lock()
            .unwrap()
            .serialize()
            .len()
            .try_into()?;

        let fuel_cost = overflow_error((result as u64).checked_mul(FUEL_PER_REQUEST_BYTE))?;
        #[cfg(feature = "debug-log")]
        {
            println!(
                "request_context: context_size={} bytes, fuel_cost={}",
                result, fuel_cost
            );
        }

        consume_fuel(caller, fuel_cost)?;
        Ok(result)
    }
    pub(super) fn load_context(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<i32> {
        let result: Vec<u8> = caller.data_mut().context.lock().unwrap().serialize();

        let fuel_cost = overflow_error((result.len() as u64).checked_mul(FUEL_PER_LOAD_BYTE))?;
        #[cfg(feature = "debug-log")]
        {
            println!(
                "load_context: context_size={} bytes, fuel_cost={}",
                result.len(),
                fuel_cost
            );
        }

        consume_fuel(caller, fuel_cost)?;

        send_to_arraybuffer(caller, v.try_into()?, &result)
    }
    pub(super) fn request_transaction(caller: &mut Caller<'_, AlkanesState>) -> Result<i32> {
        let tx_data = consensus_encode(
            &caller
                .data_mut()
                .context
                .lock()
                .unwrap()
                .message
                .transaction,
        )?;
        let result: i32 = tx_data.len().try_into()?;

        // Use a small fixed cost for requesting transaction size
        // This is just getting the size, not loading the full transaction
        let request_fuel = std::cmp::min(50, FUEL_LOAD_TRANSACTION / 10);
        consume_fuel(caller, request_fuel)?;

        #[cfg(feature = "debug-log")]
        {
            println!(
                "Requesting transaction size: {} bytes, fuel cost={} (fixed)",
                result, request_fuel
            );
        }

        Ok(result)
    }
    /*
    pub(super) fn request_output(caller: &mut Caller<'_, AlkanesState>, outpoint: i32) -> Result<i32> {
        let mem = get_memory(caller)?;
        let key = {
          let data = mem.data(&caller);
          read_arraybuffer(data, outpoint)?
        };
        Ok(caller
                .data_mut()
                .context
                .lock()
                .unwrap()
                .message
                .atomic
                .derive(&*protorune::tables::OUTPOINT_TO_OUTPUT)
                .select(&key).get().as_ref().len() as i32)
    }
    pub(super) fn load_output(caller: &mut Caller<'_, AlkanesState>, outpoint: i32, output: i32) -> Result<i32> {
        let mem = get_memory(caller)?;
        let key = {
          let data = mem.data(&caller);
          read_arraybuffer(data, outpoint)?
        };
        let value = caller.data_mut()
                .context
                .lock()
                .unwrap()
                .message
                .atomic
                .derive(&*protorune::tables::OUTPOINT_TO_OUTPUT)
                .select(&key).get().as_ref().clone();
        Ok(send_to_arraybuffer(caller, output.try_into()?, &value)?)
    }
    */
    pub(super) fn returndatacopy(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        let returndata: Vec<u8> = caller.data_mut().context.lock().unwrap().returndata.clone();

        let fuel_cost = overflow_error((returndata.len() as u64).checked_mul(FUEL_PER_LOAD_BYTE))?;
        #[cfg(feature = "debug-log")]
        {
            println!(
                "returndatacopy: data_size={} bytes, fuel_cost={}",
                returndata.len(),
                fuel_cost
            );
        }

        consume_fuel(caller, fuel_cost)?;

        send_to_arraybuffer(caller, output.try_into()?, &returndata)?;
        Ok(())
    }
    pub(super) fn load_transaction(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<()> {
        let transaction: Vec<u8> = consensus_encode(
            &caller
                .data_mut()
                .context
                .lock()
                .unwrap()
                .message
                .transaction,
        )?;

        // Use fixed fuel cost instead of scaling with transaction size
        consume_fuel(caller, FUEL_LOAD_TRANSACTION)?;

        #[cfg(feature = "debug-log")]
        {
            println!(
                "Loading transaction: size={} bytes, fuel cost={} (fixed)",
                transaction.len(),
                FUEL_LOAD_TRANSACTION
            );
        }

        send_to_arraybuffer(caller, v.try_into()?, &transaction)?;
        Ok(())
    }
    pub(super) fn request_block(caller: &mut Caller<'_, AlkanesState>) -> Result<i32> {
        let block_data =
            consensus_encode(&caller.data_mut().context.lock().unwrap().message.block)?;
        let len: i32 = block_data.len().try_into()?;

        // Use a small fixed cost for requesting block size
        // This is just getting the size, not loading the full block
        let request_fuel = std::cmp::min(100, FUEL_LOAD_BLOCK / 10);
        consume_fuel(caller, request_fuel)?;

        #[cfg(feature = "debug-log")]
        {
            println!(
                "Requesting block size: {} bytes, fuel cost={} (fixed)",
                len, request_fuel
            );
        }

        Ok(len)
    }
    pub(super) fn load_block(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<()> {
        let block: Vec<u8> =
            consensus_encode(&caller.data_mut().context.lock().unwrap().message.block)?;

        // Use fixed fuel cost instead of scaling with block size
        consume_fuel(caller, FUEL_LOAD_BLOCK)?;

        #[cfg(feature = "debug-log")]
        {
            println!(
                "Loading block: size={} bytes, fuel cost={} (fixed)",
                block.len(),
                FUEL_LOAD_BLOCK
            );
        }
        send_to_arraybuffer(caller, v.try_into()?, &block)?;
        Ok(())
    }
    pub(super) fn sequence(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        let buffer: Vec<u8> =
            (&sequence_pointer(&caller.data_mut().context.lock().unwrap().message.atomic)
                .get_value::<u128>()
                .to_le_bytes())
                .to_vec();

        #[cfg(feature = "debug-log")]
        {
            println!("sequence: fuel_cost={}", FUEL_SEQUENCE);
        }

        consume_fuel(caller, FUEL_SEQUENCE)?;

        send_to_arraybuffer(caller, output.try_into()?, &buffer)?;
        Ok(())
    }
    pub(super) fn fuel(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        let remaining_fuel = caller.get_fuel()?;
        let buffer: Vec<u8> = (&remaining_fuel.to_le_bytes()).to_vec();

        #[cfg(feature = "debug-log")]
        {
            println!(
                "fuel: remaining_fuel={}, fuel_cost={}",
                remaining_fuel, FUEL_FUEL
            );
        }

        consume_fuel(caller, FUEL_FUEL)?;

        send_to_arraybuffer(caller, output.try_into()?, &buffer)?;
        Ok(())
    }
    pub(super) fn height(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        let height_value = caller.data_mut().context.lock().unwrap().message.height;
        let height = (&height_value.to_le_bytes()).to_vec();

        #[cfg(feature = "debug-log")]
        {
            println!(
                "height: block_height={}, fuel_cost={}",
                height_value, FUEL_HEIGHT
            );
        }

        consume_fuel(caller, FUEL_HEIGHT)?;

        send_to_arraybuffer(caller, output.try_into()?, &height)?;
        Ok(())
    }
    pub(super) fn balance<'a>(
        caller: &mut Caller<'a, AlkanesState>,
        who_ptr: i32,
        what_ptr: i32,
        output: i32,
    ) -> Result<()> {
        let (who, what) = {
            let mem = get_memory(caller)?;
            let data = mem.data(&caller);
            (
                AlkaneId::parse(&mut Cursor::new(read_arraybuffer(data, who_ptr)?))?,
                AlkaneId::parse(&mut Cursor::new(read_arraybuffer(data, what_ptr)?))?,
            )
        };
        let balance = balance_pointer(
            &mut caller.data_mut().context.lock().unwrap().message.atomic,
            &who.into(),
            &what.into(),
        )
        .get()
        .as_ref()
        .clone();

        #[cfg(feature = "debug-log")]
        {
            println!(
                "balance: who=[{},{}], what=[{},{}], balance_size={} bytes, fuel_cost={}",
                who.block,
                who.tx,
                what.block,
                what.tx,
                balance.len(),
                FUEL_BALANCE
            );
        }

        consume_fuel(caller, FUEL_BALANCE)?;

        send_to_arraybuffer(caller, output.try_into()?, &balance)?;
        Ok(())
    }
    fn _handle_extcall_abort<'a, T: Extcall>(
        caller: &mut Caller<'_, AlkanesState>,
        e: anyhow::Error,
        should_rollback: bool,
    ) -> i32 {
        println!("[[handle_extcall]] Error during extcall: {:?}", e);
        let mut data: Vec<u8> = vec![0x08, 0xc3, 0x79, 0xa0];
        data.extend(e.to_string().as_bytes());

        let mut revert_context: TraceResponse = TraceResponse::default();
        revert_context.inner.data = data.clone();

        let mut response = CallResponse::default();
        response.data = data.clone();
        let serialized = response.serialize();

        // Store the serialized length before we drop context_guard
        let result = (serialized.len() as i32).checked_neg().unwrap_or(-1);

        // Handle revert state in a separate scope so context_guard is dropped
        {
            let mut context_guard = caller.data_mut().context.lock().unwrap();
            context_guard
                .trace
                .clock(TraceEvent::RevertContext(revert_context));
            if should_rollback {
                context_guard.message.atomic.rollback();
            }
            context_guard.returndata = serialized;
            // context_guard is dropped here when the scope ends
        }

        // Now we can use caller again
        Self::_abort(caller.into());
        result
    }
    fn _prepare_extcall_before_checkpoint<'a, T: Extcall>(
        caller: &mut Caller<'_, AlkanesState>,
        cellpack_ptr: i32,
        incoming_alkanes_ptr: i32,
        checkpoint_ptr: i32,
    ) -> Result<(Cellpack, AlkaneTransferParcel, StorageMap, u64)> {
        let current_depth = AlkanesHostFunctionsImpl::get_checkpoint_depth(caller);
        if current_depth >= 75 {
            return Err(anyhow!(format!(
                "Possible infinite recursion encountered: checkpoint depth too large({})",
                current_depth
            )));
        }
        // Read all input data first
        let mem = get_memory(caller)?;
        let data = mem.data(&caller);
        let buffer = read_arraybuffer(data, cellpack_ptr)?;
        let cellpack = Cellpack::parse(&mut Cursor::new(buffer))?;
        let buf = read_arraybuffer(data, incoming_alkanes_ptr)?;
        let incoming_alkanes = AlkaneTransferParcel::parse(&mut Cursor::new(buf))?;
        let storage_map_buffer = read_arraybuffer(data, checkpoint_ptr)?;
        let storage_map_len = storage_map_buffer.len();
        let storage_map = StorageMap::parse(&mut Cursor::new(storage_map_buffer))?;
        // Handle deployment fuel first
        if cellpack.target.is_deployment() {
            // Extract height into a local variable to avoid multiple mutable borrows
            let height = caller.data_mut().context.lock().unwrap().message.height as u32;

            #[cfg(feature = "debug-log")]
            {
                println!(
                    "extcall: deployment detected, additional fuel_cost={}",
                    fuel_extcall_deploy(height)
                );
            }
            caller.consume_fuel(fuel_extcall_deploy(height))?;
        }
        Ok((
            cellpack,
            incoming_alkanes,
            storage_map,
            storage_map_len as u64,
        ))
    }
    pub(super) fn handle_extcall<'a, T: Extcall>(
        caller: &mut Caller<'_, AlkanesState>,
        cellpack_ptr: i32,
        incoming_alkanes_ptr: i32,
        checkpoint_ptr: i32,
        _start_fuel: u64, // this arg is not used, but cannot be removed due to backwards compat
    ) -> i32 {
        match Self::_prepare_extcall_before_checkpoint::<T>(
            caller,
            cellpack_ptr,
            incoming_alkanes_ptr,
            checkpoint_ptr,
        ) {
            Ok((cellpack, incoming_alkanes, storage_map, storage_map_len)) => {
                match Self::extcall::<T>(
                    caller,
                    cellpack,
                    incoming_alkanes,
                    storage_map,
                    storage_map_len,
                ) {
                    Ok(v) => v,
                    Err(e) => Self::_handle_extcall_abort::<T>(caller, e, true),
                }
            }
            Err(e) => Self::_handle_extcall_abort::<T>(caller, e, false),
        }
    }
    fn _get_block_header(caller: &mut Caller<'_, AlkanesState>) -> Result<CallResponse> {
        // Return the current block header
        #[cfg(feature = "debug-log")]
        {
            println!("Precompiled contract: returning current block header");
        }

        // Get the block header from the current context
        let block = {
            let context_guard = caller.data_mut().context.lock().unwrap();
            context_guard.message.block.clone()
        };

        // Serialize just the header (not the full block with transactions)
        let header_bytes = consensus_encode(&block.header)?;
        let mut response = CallResponse::default();
        response.data = header_bytes;
        Ok(response)
    }

    fn _get_coinbase_tx(caller: &mut Caller<'_, AlkanesState>) -> Result<Transaction> {
        let context_guard = caller.data_mut().context.lock().unwrap();
        if context_guard.message.block.txdata.is_empty() {
            return Err(anyhow!("Block has no transactions"));
        }
        Ok(context_guard.message.block.txdata[0].clone())
    }

    fn _get_coinbase_tx_response(caller: &mut Caller<'_, AlkanesState>) -> Result<CallResponse> {
        // Return the coinbase transaction bytes
        #[cfg(feature = "debug-log")]
        {
            println!("Precompiled contract: returning coinbase transaction");
        }

        // Get the coinbase transaction from the current block
        let coinbase_tx = Self::_get_coinbase_tx(caller)?;

        // Serialize the coinbase transaction
        let tx_bytes = consensus_encode(&coinbase_tx)?;
        let mut response = CallResponse::default();
        response.data = tx_bytes;
        Ok(response)
    }

    fn _get_total_miner_fee(caller: &mut Caller<'_, AlkanesState>) -> Result<CallResponse> {
        // Return the coinbase transaction bytes
        #[cfg(feature = "debug-log")]
        {
            println!("Precompiled contract: returning total miner fee");
        }

        // Get the coinbase transaction from the current block
        let coinbase_tx = Self::_get_coinbase_tx(caller)?;
        let total_fees: u128 = coinbase_tx
            .output
            .iter()
            .map(|out| out.value.to_sat() as u128)
            .sum();

        let mut response = CallResponse::default();
        response.data = total_fees.to_le_bytes().to_vec();
        Ok(response)
    }

    fn _get_number_diesel_mints(caller: &mut Caller<'_, AlkanesState>) -> Result<CallResponse> {
        if let Some(cached_data) = DIESEL_MINTS_CACHE.read().unwrap().clone() {
            #[cfg(feature = "debug-log")]
            {
                println!("Precompiled contract: returning cached total number of diesel mints");
            }
            let mut response = CallResponse::default();
            response.data = cached_data;
            return Ok(response);
        }
        #[cfg(feature = "debug-log")]
        {
            println!("Precompiled contract: calculating total number of diesel mints in this block");
        }

        // Get the block header from the current context
        let block = {
            let context_guard = caller.data_mut().context.lock().unwrap();
            context_guard.message.block.clone()
        };
        let mut counter: u128 = 0;
        for tx in &block.txdata {
            if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(tx) {
                let protostones = Protostone::from_runestone(runestone)?;
                for protostone in protostones {
                    if protostone.protocol_tag != 1 {
                        continue;
                    }
                    let calldata: Vec<u8> = protostone
                        .message
                        .iter()
                        .flat_map(|v| v.to_be_bytes())
                        .collect();
                    if calldata.is_empty() {
                        continue;
                    }
                    let varint_list = decode_varint_list(&mut Cursor::new(calldata))?;
                    if varint_list.len() < 2 {
                        continue;
                    }
                    if let Ok(cellpack) = TryInto::<Cellpack>::try_into(varint_list) {
                        if cellpack.target == AlkaneId::new(2, 0)
                            && !cellpack.inputs.is_empty()
                            && cellpack.inputs[0] == 77
                        {
                            counter += 1;
                            break;
                        }
                    }
                }
            }
        }
        let mut response = CallResponse::default();
        response.data = counter.to_le_bytes().to_vec();
        *DIESEL_MINTS_CACHE.write().unwrap() = Some(response.data.clone());
        Ok(response)
    }
    fn _handle_special_extcall(
        caller: &mut Caller<'_, AlkanesState>,
        cellpack: Cellpack,
    ) -> Result<i32> {
        #[cfg(feature = "debug-log")]
        {
            println!(
                "extcall: precompiled contract detected at [{},{}]",
                cellpack.target.block, cellpack.target.tx
            );
        }

        let response = match cellpack.target.tx {
            0 => Self::_get_block_header(caller),
            1 => Self::_get_coinbase_tx_response(caller),
            2 => Self::_get_number_diesel_mints(caller),
            3 => Self::_get_total_miner_fee(caller),
            _ => {
                return Err(anyhow!(
                    "Unknown precompiled contract: [{}, {}]",
                    cellpack.target.block,
                    cellpack.target.tx
                ));
            }
        }?;

        // Serialize the response and return
        let serialized = response.serialize();
        {
            let mut context_guard = caller.data_mut().context.lock().unwrap();
            context_guard.returndata = serialized.clone();

            // Create a trace response
            let mut return_context = TraceResponse::default();
            return_context.inner = response.clone().into();
            return_context.fuel_used = 0; // Precompiled contracts don't use fuel
            context_guard
                .trace
                .clock(TraceEvent::ReturnContext(return_context));
        }

        Ok(serialized.len() as i32)
    }
    pub(super) fn extcall<'a, T: Extcall>(
        caller: &mut Caller<'_, AlkanesState>,
        cellpack: Cellpack,
        incoming_alkanes: AlkaneTransferParcel,
        storage_map: StorageMap,
        storage_map_len: u64,
    ) -> Result<i32> {
        // Check for precompiled contract addresses
        if cellpack.target.block == 800000000 {
            // 8e8
            return Self::_handle_special_extcall(caller, cellpack);
        }

        // Regular contract execution
        // Prepare subcontext data
        let (subcontext, binary_rc) = {
            let mut context_guard = caller.data_mut().context.lock().unwrap();
            context_guard.message.atomic.checkpoint();
            let myself = context_guard.myself.clone();
            let caller_id = context_guard.caller.clone();
            pipe_storagemap_to(
                &storage_map,
                &mut context_guard.message.atomic.derive(
                    &IndexPointer::from_keyword("/alkanes/").select(&myself.clone().into()),
                ),
            );
            std::mem::drop(context_guard); // Release lock before calling run_special_cellpacks

            let (_subcaller, submyself, binary) =
                run_special_cellpacks(caller.data_mut().context.clone(), &cellpack)?;

            let context_guard = caller.data_mut().context.lock().unwrap();

            if !T::isdelegate() {
                // delegate call retains caller and myself, so no alkanes are transferred to the subcontext
                transfer_from(
                    &incoming_alkanes,
                    &mut context_guard
                        .message
                        .atomic
                        .derive(&IndexPointer::default()),
                    &myself,
                    &submyself,
                )?;
            }
            // Create subcontext
            let mut subbed = context_guard.clone();
            subbed.message.atomic = context_guard
                .message
                .atomic
                .derive(&IndexPointer::default());
            (subbed.caller, subbed.myself) =
                T::change_context(submyself.clone(), caller_id, myself.clone());
            subbed.returndata = vec![];
            subbed.incoming_alkanes = incoming_alkanes.clone();
            subbed.inputs = cellpack.inputs.clone();
            (subbed, binary)
        };

        let height = caller.data_mut().context.lock().unwrap().message.height as u32;

        let total_fuel = compute_extcall_fuel(storage_map_len, height)?;

        #[cfg(feature = "debug-log")]
        {
            println!("extcall: target=[{},{}], inputs={:?}, storage_size={} bytes, total_fuel={}, deployment={}",
                cellpack.target.block, cellpack.target.tx,
                cellpack.inputs, storage_map_len,
                total_fuel,
                cellpack.target.is_deployment());
        }

        consume_fuel(caller, total_fuel)?;

        let mut trace_context: TraceContext = subcontext.flat().into();
        let start_fuel: u64 = caller.get_fuel()?;
        trace_context.fuel = start_fuel;
        let event: TraceEvent = T::event(trace_context);
        subcontext.trace.clock(event);

        // Run the call in a new context
        let (response, gas_used) = run_after_special(
            Arc::new(Mutex::new(subcontext.clone())),
            binary_rc,
            start_fuel,
        )?;
        let serialized = CallResponse::from(response.clone().into()).serialize();
        {
            caller.set_fuel(overflow_error(start_fuel.checked_sub(gas_used))?)?;
            let mut return_context: TraceResponse = response.clone().into();
            return_context.fuel_used = gas_used;

            // Update trace and context state
            let mut context_guard = caller.data_mut().context.lock().unwrap();
            context_guard
                .trace
                .clock(TraceEvent::ReturnContext(return_context));
            let mut saveable: SaveableExtendedCallResponse = response.clone().into();
            saveable.associate(&subcontext);
            saveable.save(&mut context_guard.message.atomic, T::isdelegate())?;
            context_guard.returndata = serialized.clone();
            T::handle_atomic(&mut context_guard.message.atomic);
        }
        Ok(serialized.len() as i32)
    }
    pub(super) fn log<'a>(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<()> {
        let mem = get_memory(caller)?;
        let message = {
            let data = mem.data(&caller);
            read_arraybuffer(data, v)?
        };
        print!("{}", String::from_utf8(message)?);
        Ok(())
    }
}

// Implementation of the safe wrapper
impl SafeAlkanesHostFunctionsImpl {
    // Helper method to execute a function with proper context management and depth checking
    fn with_context_safety<F, R>(caller: &mut Caller<'_, AlkanesState>, f: F) -> R
    where
        F: FnOnce(&mut Caller<'_, AlkanesState>) -> R,
    {
        // Get initial checkpoint depth
        let initial_depth = AlkanesHostFunctionsImpl::get_checkpoint_depth(caller);

        // Preserve context
        AlkanesHostFunctionsImpl::preserve_context(caller);

        // Execute the function
        let result = f(caller);

        // Restore context
        AlkanesHostFunctionsImpl::restore_context(caller);

        // Check that the checkpoint depth is the same as before
        let final_depth = AlkanesHostFunctionsImpl::get_checkpoint_depth(caller);
        assert_eq!(
            initial_depth, final_depth,
            "IndexCheckpointStack depth changed: {} -> {}",
            initial_depth, final_depth
        );

        result
    }
    pub(super) fn _abort<'a>(caller: Caller<'_, AlkanesState>) {
        SafeAlkanesHostFunctionsImpl::abort(caller, 0, 0, 0, 0);
    }
    pub(super) fn abort<'a>(mut caller: Caller<'_, AlkanesState>, _: i32, _: i32, _: i32, _: i32) {
        caller.data_mut().had_failure = true;
    }

    pub(super) fn request_storage<'a>(
        caller: &mut Caller<'_, AlkanesState>,
        k: i32,
    ) -> Result<i32> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::request_storage(c, k))
    }

    pub(super) fn load_storage<'a>(
        caller: &mut Caller<'_, AlkanesState>,
        k: i32,
        v: i32,
    ) -> Result<i32> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::load_storage(c, k, v))
    }

    pub(super) fn log<'a>(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::log(c, v))
    }

    pub(super) fn balance<'a>(
        caller: &mut Caller<'a, AlkanesState>,
        who: i32,
        what: i32,
        output: i32,
    ) -> Result<()> {
        Self::with_context_safety(caller, |c| {
            AlkanesHostFunctionsImpl::balance(c, who, what, output)
        })
    }

    pub(super) fn request_context(caller: &mut Caller<'_, AlkanesState>) -> Result<i32> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::request_context(c))
    }

    pub(super) fn load_context(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<i32> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::load_context(c, v))
    }

    pub(super) fn request_transaction(caller: &mut Caller<'_, AlkanesState>) -> Result<i32> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::request_transaction(c))
    }

    pub(super) fn returndatacopy(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| {
            AlkanesHostFunctionsImpl::returndatacopy(c, output)
        })
    }

    pub(super) fn load_transaction(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::load_transaction(c, v))
    }

    pub(super) fn request_block(caller: &mut Caller<'_, AlkanesState>) -> Result<i32> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::request_block(c))
    }

    pub(super) fn load_block(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::load_block(c, v))
    }

    pub(super) fn sequence(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::sequence(c, output))
    }

    pub(super) fn fuel(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::fuel(c, output))
    }

    pub(super) fn height(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::height(c, output))
    }

    pub(super) fn handle_extcall<'a, T: Extcall>(
        caller: &mut Caller<'a, AlkanesState>,
        cellpack_ptr: i32,
        incoming_alkanes_ptr: i32,
        checkpoint_ptr: i32,
        start_fuel: u64,
    ) -> i32 {
        Self::with_context_safety(caller, |c| {
            AlkanesHostFunctionsImpl::handle_extcall::<T>(
                c,
                cellpack_ptr,
                incoming_alkanes_ptr,
                checkpoint_ptr,
                start_fuel,
            )
        })
    }
}
