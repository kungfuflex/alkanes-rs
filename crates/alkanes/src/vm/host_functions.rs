use super::fuel::compute_extcall_fuel;
use super::utils::{get_memory, read_arraybuffer, send_to_arraybuffer, sequence_pointer, Saveable, SaveableExtendedCallResponse};
use super::state::AlkanesState;
use super::extcall::Extcall;
use crate::utils::{balance_pointer, pipe_storagemap_to, transfer_from};
use crate::vm::utils::{run_after_special, run_special_cellpacks};

use alkanes_support::{
    cellpack::Cellpack,
    id::AlkaneId,
    parcel::AlkaneTransferParcel,
    response::CallResponse,
    storage::StorageMap,
    trace::{TraceContext, TraceEvent, TraceResponse},
    utils::overflow_error,
};
use metashrew_core::environment::RuntimeEnvironment;
use anyhow::{anyhow, Result};
use bitcoin::Transaction;
use metashrew_core::index_pointer::IndexPointer;

use metashrew_core::index_pointer::KeyValuePointer;
use num::traits::ToBytes;
use ordinals::Artifact;
use ordinals::Runestone;
use protorune_support::protostone::Protostone;

use crate::vm::fuel::{
    consume_fuel, fuel_extcall_deploy, Fuelable, FUEL_BALANCE,
    FUEL_FUEL, FUEL_HEIGHT, FUEL_LOAD_BLOCK, FUEL_LOAD_TRANSACTION, FUEL_PER_LOAD_BYTE,
    FUEL_PER_REQUEST_BYTE, FUEL_SEQUENCE,
};
use protorune_support::utils::{consensus_encode, decode_varint_list};
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
    fn preserve_context<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) {
        caller
            .data_mut()
            .context
            .lock()
            .unwrap()
            .message
            .atomic
            .checkpoint();
    }

    fn restore_context<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) {
        let state = caller.data_mut();
        let env = &mut state.env;
        state.context.lock().unwrap().message.atomic.commit(env);
    }

    // Get the current depth of the checkpoint stack
    fn get_checkpoint_depth<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) -> usize {
        caller
            .data_mut()
            .context
            .lock()
            .unwrap()
            .message
            .atomic
            .checkpoint_depth()
    }
    pub(super) fn _abort<'a, E: RuntimeEnvironment + Clone>(caller: Caller<'_, AlkanesState<E>>) {
        AlkanesHostFunctionsImpl::abort(caller, 0, 0, 0, 0);
    }
    pub(super) fn abort<'a, E: RuntimeEnvironment + Clone>(mut caller: Caller<'_, AlkanesState<E>>, _: i32, _: i32, _: i32, _: i32) {
        caller.data_mut().had_failure = true;
    }
    pub(super) fn request_storage<'a, E: RuntimeEnvironment + Clone>(
        caller: &mut Caller<'_, AlkanesState<E>>,
        k: i32,
    ) -> Result<i32> {
			let mem = get_memory(caller)?;
			let data = mem.data(&caller);
			read_arraybuffer(data, k)?
		};
		let state = caller.data_mut();
		let env = &mut state.env;
        let myself = state.context.lock().unwrap().myself.clone();
        let result: i32 = state
            .context
            .lock()
            .unwrap()
            .message
            .atomic
            .keyword("/alkanes/")
            .select(&myself.into())
            .keyword("/storage/")
            .select(&key)
            .get(env)
            .len()
            .try_into()?;
		let bytes_processed = (result as u64) + (key.len() as u64);

        let fuel_cost =
            overflow_error((bytes_processed as u64).checked_mul(FUEL_PER_REQUEST_BYTE))?;
        #[cfg(feature = "debug-log")]
        {
            MetashrewEnvironment::log(&format!("request_storage: key_size={} bytes, result_size={} bytes, fuel_cost={}",
                key.len(),
                result,
                fuel_cost
            ));
        }

        consume_fuel(caller, fuel_cost)?;
        Ok(result)
    }
    pub(super) fn load_storage<'a, E: RuntimeEnvironment + Clone>(
        caller: &mut Caller<'_, AlkanesState<E>>,
        k: i32,
        v: i32,
    ) -> Result<i32> {
			let mem = get_memory(caller)?;
			let data = mem.data(&caller);
			read_arraybuffer(data, k)?
		};
		let state = caller.data_mut();
		let env = &mut state.env;
		let value = {
			let myself = state.context.lock().unwrap().myself.clone();
			(&state.context.lock().unwrap().message)
				.atomic
				.keyword("/alkanes/")
				.select(&myself.into())
				.keyword("/storage/")
				.select(&key)
				.get(env)
		};
		let bytes_processed = key.len() + value.len();

        let fuel_cost = overflow_error((bytes_processed as u64).checked_mul(FUEL_PER_LOAD_BYTE))?;
        #[cfg(feature = "debug-log")]
        {
            MetashrewEnvironment::log(&format!("load_storage: key_size={} bytes, value_size={} bytes, total_size={} bytes, fuel_cost={}",
                key.len(), value.len(), bytes_processed, fuel_cost
            ));
        }

        consume_fuel(caller, fuel_cost)?;
        send_to_arraybuffer(caller, v.try_into()?, value.as_ref())
    }
    pub(super) fn request_context<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) -> Result<i32> {
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
            MetashrewEnvironment::log(&format!("request_context: context_size={} bytes, fuel_cost={}",
                result, fuel_cost
            ));
        }

        consume_fuel(caller, fuel_cost)?;
        Ok(result)
    }
    pub(super) fn load_context<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, v: i32) -> Result<i32> {
        let result: Vec<u8> = caller.data_mut().context.lock().unwrap().serialize();

        let fuel_cost = overflow_error((result.len() as u64).checked_mul(FUEL_PER_LOAD_BYTE))?;
        #[cfg(feature = "debug-log")]
        {
            MetashrewEnvironment::log(&format!("load_context: context_size={} bytes, fuel_cost={}",
                result.len(),
                fuel_cost
            ));
        }

        consume_fuel(caller, fuel_cost)?;

        send_to_arraybuffer(caller, v.try_into()?, &result)
    }
    pub(super) fn request_transaction<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) -> Result<i32> {
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
            MetashrewEnvironment::log(&format!("Requesting transaction size: {} bytes, fuel cost={} (fixed)",
                result, request_fuel
            ));
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
    pub(super) fn returndatacopy<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, output: i32) -> Result<()> {
        let returndata: Vec<u8> = caller.data_mut().context.lock().unwrap().returndata.clone();

        let fuel_cost = overflow_error((returndata.len() as u64).checked_mul(FUEL_PER_LOAD_BYTE))?;
        #[cfg(feature = "debug-log")]
        {
            MetashrewEnvironment::log(&format!("returndatacopy: data_size={} bytes, fuel_cost={}",
                returndata.len(),
                fuel_cost
            ));
        }

        consume_fuel(caller, fuel_cost)?;

        send_to_arraybuffer(caller, output.try_into()?, &returndata)?;
        Ok(())
    }
    pub(super) fn load_transaction<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, v: i32) -> Result<()> {
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
            MetashrewEnvironment::log(&format!("Loading transaction: size={} bytes, fuel cost={} (fixed)",
                transaction.len(),
                FUEL_LOAD_TRANSACTION
            ));
        }

        send_to_arraybuffer(caller, v.try_into()?, &transaction)?;
        Ok(())
    }
pub(super) fn request_block<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) -> Result<i32> {
            consensus_encode(&caller.data_mut().context.lock().unwrap().message.block)?;
        let len: i32 = block_data.len().try_into()?;

        // Use a small fixed cost for requesting block size
        // This is just getting the size, not loading the full block
        let request_fuel = std::cmp::min(100, FUEL_LOAD_BLOCK / 10);
        consume_fuel(caller, request_fuel)?;

        #[cfg(feature = "debug-log")]
        {
            MetashrewEnvironment::log(&format!("Requesting block size: {} bytes, fuel cost={} (fixed)",
                len, request_fuel
            ));
        }

        Ok(len)
    }
    pub(super) fn load_block<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, v: i32) -> Result<()> {
            consensus_encode(&caller.data_mut().context.lock().unwrap().message.block)?;

        // Use fixed fuel cost instead of scaling with block size
        consume_fuel(caller, FUEL_LOAD_BLOCK)?;

        #[cfg(feature = "debug-log")]
        {
            MetashrewEnvironment::log(&format!("Loading block: size={} bytes, fuel cost={} (fixed)",
                block.len(),
                FUEL_LOAD_BLOCK
            ));
        }
        send_to_arraybuffer(caller, v.try_into()?, &block)?;
        Ok(())
    }
    pub(super) fn sequence<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, output: i32) -> Result<()> {
            .get_value::<u128>(&mut caller.data_mut().env)
            .to_le_bytes())
            .to_vec();

        #[cfg(feature = "debug-log")]
        {
            MetashrewEnvironment::log(&format!("sequence: fuel_cost={}", FUEL_SEQUENCE));
        }

        consume_fuel(caller, FUEL_SEQUENCE)?;

        send_to_arraybuffer(caller, output.try_into()?, &buffer)?;
        Ok(())
    }
pub(super) fn fuel<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, output: i32) -> Result<()> {
        let buffer: Vec<u8> = (&remaining_fuel.to_le_bytes()).to_vec();

        #[cfg(feature = "debug-log")]
        {
            MetashrewEnvironment::log(&format!("fuel: remaining_fuel={}, fuel_cost={}",
                remaining_fuel, FUEL_FUEL
            ));
        }

        consume_fuel(caller, FUEL_FUEL)?;

        send_to_arraybuffer(caller, output.try_into()?, &buffer)?;
        Ok(())
    }
    pub(super) fn height<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, output: i32) -> Result<()> {
        let height_value = caller.data_mut().context.lock().unwrap().message.height;
        let height = (&height_value.to_le_bytes()).to_vec();

        #[cfg(feature = "debug-log")]
        {
            MetashrewEnvironment::log(&format!("height: block_height={}, fuel_cost={}",
                height_value, FUEL_HEIGHT
            ));
        }

        consume_fuel(caller, FUEL_HEIGHT)?;

        send_to_arraybuffer(caller, output.try_into()?, &height)?;
        Ok(())
    }
    pub(super) fn balance<'a, E: RuntimeEnvironment + Clone>(
        caller: &mut Caller<'a, AlkanesState<E>>,
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
        let balance = {
            let state = caller.data_mut();
            let env = &mut state.env;
            let atomic = &mut state.context.lock().unwrap().message.atomic;
            balance_pointer(atomic, &who.into(), &what.into(), env)
                .get(env)
                .as_ref()
                .clone()
        };

        #[cfg(feature = "debug-log")]
        {
            MetashrewEnvironment::log(&format!("balance: who=[{},{}], what=[{},{}], balance_size={} bytes, fuel_cost={}",
                who.block,
                who.tx,
                what.block,
                what.tx,
                balance.len(),
                FUEL_BALANCE
            ));
        }

        consume_fuel(caller, FUEL_BALANCE)?;

        send_to_arraybuffer(caller, output.try_into()?, &balance)?;
        Ok(())
    }
    fn _handle_extcall_abort<'a, E: RuntimeEnvironment + Clone, T: Extcall<E>>(
        caller: &mut Caller<'_, AlkanesState<E>>,
        e: anyhow::Error,
        should_rollback: bool,
    ) -> i32 {
        let state = caller.data_mut();
        let env = &mut state.env;
        env.log(&format!("[[handle_extcall]] Error during extcall: {:?}", e));
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
            let mut context_guard = state.context.lock().unwrap();
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
    fn _prepare_extcall_before_checkpoint<'a, E: RuntimeEnvironment + Clone, T: Extcall<E>>(
        caller: &mut Caller<'_, AlkanesState<E>>,
        cellpack_ptr: i32,
        incoming_alkanes_ptr: i32,
        checkpoint_ptr: i32,
    ) -> Result<(Cellpack, AlkaneTransferParcel, StorageMap, u64)> {
        let current_depth = AlkanesHostFunctionsImpl::get_checkpoint_depth(caller);
        if current_depth >= 75 {
            return Err(anyhow!(format!("Possible infinite recursion encountered: checkpoint depth too large({})",
                current_depth
            )));
        }
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
                env.log(&format!("extcall: deployment detected, additional fuel_cost={}",
                    fuel_extcall_deploy(height)
                ));
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
    pub(super) fn handle_extcall<'a, E: RuntimeEnvironment + Clone + 'static + std::default::Default, T: Extcall<E>>(
        caller: &mut Caller<'_, AlkanesState<E>>,
        cellpack_ptr: i32,
        incoming_alkanes_ptr: i32,
        checkpoint_ptr: i32,
        _start_fuel: u64, // this arg is not used, but cannot be removed due to backwards compat
    ) -> i32 {
        match Self::_prepare_extcall_before_checkpoint::<E, T>(
            caller,
            cellpack_ptr,
            incoming_alkanes_ptr,
            checkpoint_ptr,
        ) {
            Ok((cellpack, incoming_alkanes, storage_map, storage_map_len)) => {
                match Self::extcall::<E, T>(
                    caller,
                    cellpack,
                    incoming_alkanes,
                    storage_map,
                    storage_map_len,
                ) {
                    Ok(v) => v,
                    Err(e) => Self::_handle_extcall_abort::<E, T>(caller, e, true),
                }
            }
            Err(e) => Self::_handle_extcall_abort::<E, T>(caller, e, false),
        }
    }
    fn _get_block_header<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) -> Result<CallResponse> {
        let _env = &mut caller.data_mut().env;
        // Return the current block header
        #[cfg(feature = "debug-log")]
        {
            _env.log(&format!("Precompiled contract: returning current block header"));
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

    fn _get_coinbase_tx<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) -> Result<Transaction> {
        let context_guard = caller.data_mut().context.lock().unwrap();
        if context_guard.message.block.txdata.is_empty() {
            return Err(anyhow!("Block has no transactions"));
        }
        Ok(context_guard.message.block.txdata[0].clone())
    }

    fn _get_coinbase_tx_response<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) -> Result<CallResponse> {
        let _env = &mut caller.data_mut().env;
        // Return the coinbase transaction bytes
        #[cfg(feature = "debug-log")]
        {
            _env.log(&format!("Precompiled contract: returning coinbase transaction"));
        }

        // Get the coinbase transaction from the current block
        let coinbase_tx = Self::_get_coinbase_tx(caller)?;

        // Serialize the coinbase transaction
        let tx_bytes = consensus_encode(&coinbase_tx)?;
        let mut response = CallResponse::default();
        response.data = tx_bytes;
        Ok(response)
    }

    fn _get_total_miner_fee<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) -> Result<CallResponse> {
        let _env = &mut caller.data_mut().env;
        // Return the coinbase transaction bytes
        #[cfg(feature = "debug-log")]
        {
            _env.log(&format!("Precompiled contract: returning total miner fee"));
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

    fn _get_number_diesel_mints<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) -> Result<CallResponse> {
        let _env = &mut caller.data_mut().env;
        if let Some(cached_data) = DIESEL_MINTS_CACHE.read().unwrap().clone() {
            #[cfg(feature = "debug-log")]
            {
                _env.log(&format!("Precompiled contract: returning cached total number of diesel mints"));
            }
            let mut response = CallResponse::default();
            response.data = cached_data;
            return Ok(response);
        }
        #[cfg(feature = "debug-log")]
        {
            _env.log(&format!("Precompiled contract: calculating total number of diesel mints in this block"));
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
    fn _handle_special_extcall<E: RuntimeAdapter + Clone>(
        caller: &mut Caller<'_, AlkanesState<E>>,
        cellpack: Cellpack,
    ) -> Result<i32> {
        let _env = &mut caller.data_mut().env;
        #[cfg(feature = "debug-log")]
        {
            _env.log(&format!("extcall: precompiled contract detected at [{},{}]",
                cellpack.target.block, cellpack.target.tx
            ));
        }

        let response = match cellpack.target.tx {
            0 => Self::_get_block_header(caller),
            1 => Self::_get_coinbase_tx_response(caller),
            2 => Self::_get_number_diesel_mints(caller),
            3 => Self::_get_total_miner_fee(caller),
            _ => {
                return Err(anyhow!("Unknown precompiled contract: [{}, {}]",
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
pub(super) fn extcall<'a, E: RuntimeAdapter + Clone + 'static + Default, T: Extcall<E>>(
        caller: &mut Caller<'_, AlkanesState<E>>,
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

        let (subcontext, binary_rc, height) = {
            let state = caller.data_mut();
            let env = &mut state.env;
            let (myself, caller_id) = {
                let mut context_guard = state.context.lock().unwrap();
                context_guard.message.atomic.checkpoint();
                (context_guard.myself.clone(), context_guard.caller.clone())
            };
            pipe_storagemap_to(
                &storage_map,
                &mut state.context.lock().unwrap().message.atomic.derive(
                    &IndexPointer::from_keyword("/alkanes/").select(&myself.clone().into()),
                ),
                env,
            );

            let (_subcaller, submyself, binary) =
                run_special_cellpacks(state.context.clone(), &cellpack, env)?;

            //logging::record_alkane_creation(
            //     AlkaneCreation {
            //         alkane_id: submyself.clone(),
            //         wasm_size_kb: calculate_wasm_size_kb(&binary),
            //         creation_method: determine_creation_method(&cellpack.target, &submyself),
            //     }
            // );

            let context_guard = state.context.lock().unwrap();

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
                    env,
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
            (subbed, binary, context_guard.message.height as u32)
        };

        let total_fuel = compute_extcall_fuel(storage_map_len, height)?;

        #[cfg(feature = "debug-log")]
        {
            caller.data_mut().env.log(&format!("extcall: target=[{},{}], inputs={:?}, storage_size={} bytes, total_fuel={}, deployment={}",
                cellpack.target.block, cellpack.target.tx,
                cellpack.inputs, storage_map_len,
                total_fuel,
                cellpack.target.is_deployment()));
        }

        consume_fuel(caller, total_fuel)?;

        let mut trace_context: TraceContext = subcontext.flat().into();
        let start_fuel: u64 = caller.get_fuel()?;
        trace_context.fuel = start_fuel;
        let event: TraceEvent = T::event(trace_context);
        let subcontext_clone = subcontext.clone();
        subcontext.trace.clock(event);

        // Run the call in a new context
        let (response, gas_used) = {
            let state = caller.data_mut();
            let env = &mut state.env;
            run_after_special(
                Arc::new(Mutex::new(subcontext_clone)),
                binary_rc,
                start_fuel,
                env,
            )?
        };
        let serialized = CallResponse::from(response.clone().into()).serialize();
        caller.set_fuel(overflow_error(start_fuel.checked_sub(gas_used))?)?;
        let mut return_context: TraceResponse = response.clone().into();
        return_context.fuel_used = gas_used;

        // Update trace and context state
        let state = caller.data_mut();
        let env = &mut state.env;
        let mut context_guard = state.context.lock().unwrap();
        context_guard
            .trace
            .clock(TraceEvent::ReturnContext(return_context));
        let mut saveable: SaveableExtendedCallResponse<E> = response.clone().into();
        saveable.associate(&subcontext);
        saveable.save(
            &mut context_guard.message.atomic,
            T::isdelegate(),
            env,
        )?;
        context_guard.returndata = serialized.clone();
        T::handle_atomic(&mut context_guard.message.atomic, env);
        Ok(serialized.len() as i32)
    }
    pub(super) fn log<'a, E: RuntimeAdapter + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, v: i32) -> Result<()> {
        let message = {
            let data = mem.data(&caller);
            read_arraybuffer(data, v)?
        };
        caller.data_mut().env.log(&format!("{}", String::from_utf8(message)?));
        Ok(())
    }
}

// Implementation of the safe wrapper
impl SafeAlkanesHostFunctionsImpl {
    // Helper method to execute a function with proper context management and depth checking
    fn with_context_safety<E: RuntimeAdapter + Clone, F, R>(caller: &mut Caller<'_, AlkanesState<E>>, f: F) -> R
    where
        F: FnOnce(&mut Caller<'_, AlkanesState<E>>) -> R,
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
    pub(super) fn abort<'a, E: RuntimeEnvironment + Clone>(mut caller: Caller<'_, AlkanesState<E>>, _: i32, _: i32, _: i32, _: i32) {
        caller.data_mut().had_failure = true;
    }

    pub(super) fn request_storage<'a, E: RuntimeAdapter + Clone>(
        caller: &mut Caller<'_, AlkanesState<E>>,
        k: i32,
    ) -> Result<i32> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::request_storage(c, k))
    }

    pub(super) fn load_storage<'a, E: RuntimeAdapter + Clone>(
        caller: &mut Caller<'_, AlkanesState<E>>,
        k: i32,
        v: i32,
    ) -> Result<i32> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::load_storage(c, k, v))
    }

    pub(super) fn log<'a, E: RuntimeAdapter + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, v: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::log(c, v))
    }

    pub(super) fn balance<'a, E: RuntimeAdapter + Clone>(
        caller: &mut Caller<'a, AlkanesState<E>>,
        who: i32,
        what: i32,
        output: i32,
    ) -> Result<()> {
        Self::with_context_safety(caller, |c| {
            AlkanesHostFunctionsImpl::balance(c, who, what, output)
        })
    }

    pub(super) fn load_context<E: RuntimeEnvironment + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, v: i32) -> Result<i32> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::load_context(c, v))
    }

    pub(super) fn request_transaction<E: RuntimeAdapter + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) -> Result<i32> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::request_transaction(c))
    }

    pub(super) fn returndatacopy<E: RuntimeAdapter + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, output: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| {
            AlkanesHostFunctionsImpl::returndatacopy(c, output)
        })
    }

    pub(super) fn load_transaction<E: RuntimeAdapter + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, v: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::load_transaction(c, v))
    }

    pub(super) fn request_block<E: RuntimeAdapter + Clone>(caller: &mut Caller<'_, AlkanesState<E>>) -> Result<i32> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::request_block(c))
    }

    pub(super) fn load_block<E: RuntimeAdapter + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, v: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::load_block(c, v))
    }

    pub(super) fn sequence<E: RuntimeAdapter + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, output: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::sequence(c, output))
    }

    pub(super) fn fuel<E: RuntimeAdapter + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, output: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::fuel(c, output))
    }

    pub(super) fn height<E: RuntimeAdapter + Clone>(caller: &mut Caller<'_, AlkanesState<E>>, output: i32) -> Result<()> {
        Self::with_context_safety(caller, |c| AlkanesHostFunctionsImpl::height(c, output))
    }

    pub(super) fn handle_extcall<'a, E: RuntimeAdapter + Clone + 'static + std::default::Default, T: Extcall<E>>(
        caller: &mut Caller<'a, AlkanesState<E>>,
        cellpack_ptr: i32,
        incoming_alkanes_ptr: i32,
        checkpoint_ptr: i32,
        start_fuel: u64,
    ) -> i32 {
        Self::with_context_safety(caller, |c| {
            AlkanesHostFunctionsImpl::handle_extcall::<E, T>(
                c,
                cellpack_ptr,
                incoming_alkanes_ptr,
                checkpoint_ptr,
                start_fuel,
            )
        })
    }
}