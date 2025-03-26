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
use anyhow::{anyhow, Result};
use metashrew::index_pointer::IndexPointer;
#[allow(unused_imports)]
use metashrew::{
    print, println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;

use crate::vm::fuel::{
    consume_fuel, Fuelable, FUEL_BALANCE, FUEL_EXTCALL, FUEL_EXTCALL_DEPLOY, FUEL_FUEL,
    FUEL_HEIGHT, FUEL_LOAD_BLOCK, FUEL_LOAD_TRANSACTION, FUEL_PER_LOAD_BYTE, FUEL_PER_REQUEST_BYTE,
    FUEL_PER_STORE_BYTE, FUEL_SEQUENCE,
};
use protorune_support::utils::consensus_encode;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use wasmi::*;

pub struct AlkanesHostFunctionsImpl(());

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
        println!(
            "request_storage: key_size={} bytes, result_size={} bytes, fuel_cost={}",
            bytes_processed - (result as u64),
            result,
            fuel_cost
        );

        consume_fuel(caller, fuel_cost)?;
        Ok(result)
    }
    pub(super) fn load_storage<'a>(
        caller: &mut Caller<'_, AlkanesState>,
        k: i32,
        v: i32,
    ) -> Result<i32> {
        Self::preserve_context(caller);

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
        println!("load_storage: key_size={} bytes, value_size={} bytes, total_size={} bytes, fuel_cost={}",
            bytes_processed - value.len(), value.len(), bytes_processed, fuel_cost);

        consume_fuel(caller, fuel_cost)?;

        Self::restore_context(caller);
        send_to_arraybuffer(caller, v.try_into()?, value.as_ref())
    }
    pub(super) fn request_context(caller: &mut Caller<'_, AlkanesState>) -> Result<i32> {
        Self::preserve_context(caller);

        let result: i32 = caller
            .data_mut()
            .context
            .lock()
            .unwrap()
            .serialize()
            .len()
            .try_into()?;

        let fuel_cost = overflow_error((result as u64).checked_mul(FUEL_PER_REQUEST_BYTE))?;
        println!(
            "request_context: context_size={} bytes, fuel_cost={}",
            result, fuel_cost
        );

        consume_fuel(caller, fuel_cost)?;

        Self::restore_context(caller);
        Ok(result)
    }
    pub(super) fn load_context(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<i32> {
        Self::preserve_context(caller);

        let result: Vec<u8> = caller.data_mut().context.lock().unwrap().serialize();

        let fuel_cost = overflow_error((result.len() as u64).checked_mul(FUEL_PER_LOAD_BYTE))?;
        println!(
            "load_context: context_size={} bytes, fuel_cost={}",
            result.len(),
            fuel_cost
        );

        consume_fuel(caller, fuel_cost)?;

        Self::restore_context(caller);
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

        println!(
            "Requesting transaction size: {} bytes, fuel cost={} (fixed)",
            result, request_fuel
        );

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
        Self::preserve_context(caller);

        let returndata: Vec<u8> = caller.data_mut().context.lock().unwrap().returndata.clone();

        let fuel_cost = overflow_error((returndata.len() as u64).checked_mul(FUEL_PER_LOAD_BYTE))?;
        println!(
            "returndatacopy: data_size={} bytes, fuel_cost={}",
            returndata.len(),
            fuel_cost
        );

        consume_fuel(caller, fuel_cost)?;

        Self::restore_context(caller);
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

        // Log transaction size and fuel cost
        println!(
            "Loading transaction: size={} bytes, fuel cost={} (fixed)",
            transaction.len(),
            FUEL_LOAD_TRANSACTION
        );

        send_to_arraybuffer(caller, v.try_into()?, &transaction)?;
        Ok(())
    }
    pub(super) fn request_block(caller: &mut Caller<'_, AlkanesState>) -> Result<i32> {
        Self::preserve_context(caller);

        let block_data =
            consensus_encode(&caller.data_mut().context.lock().unwrap().message.block)?;
        let len: i32 = block_data.len().try_into()?;

        // Use a small fixed cost for requesting block size
        // This is just getting the size, not loading the full block
        let request_fuel = std::cmp::min(100, FUEL_LOAD_BLOCK / 10);
        consume_fuel(caller, request_fuel)?;

        println!(
            "Requesting block size: {} bytes, fuel cost={} (fixed)",
            len, request_fuel
        );

        Self::restore_context(caller);
        Ok(len)
    }
    pub(super) fn load_block(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<()> {
        Self::preserve_context(caller);

        let block: Vec<u8> =
            consensus_encode(&caller.data_mut().context.lock().unwrap().message.block)?;

        // Use fixed fuel cost instead of scaling with block size
        consume_fuel(caller, FUEL_LOAD_BLOCK)?;

        // Log block size and fuel cost
        println!(
            "Loading block: size={} bytes, fuel cost={} (fixed)",
            block.len(),
            FUEL_LOAD_BLOCK
        );

        Self::restore_context(caller);
        send_to_arraybuffer(caller, v.try_into()?, &block)?;
        Ok(())
    }
    pub(super) fn sequence(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        let buffer: Vec<u8> =
            (&sequence_pointer(&caller.data_mut().context.lock().unwrap().message.atomic)
                .get_value::<u128>()
                .to_le_bytes())
                .to_vec();

        println!("sequence: fuel_cost={}", FUEL_SEQUENCE);
        consume_fuel(caller, FUEL_SEQUENCE)?;

        send_to_arraybuffer(caller, output.try_into()?, &buffer)?;
        Ok(())
    }
    pub(super) fn fuel(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        let remaining_fuel = caller.get_fuel().unwrap();
        let buffer: Vec<u8> = (&remaining_fuel.to_le_bytes()).to_vec();

        println!(
            "fuel: remaining_fuel={}, fuel_cost={}",
            remaining_fuel, FUEL_FUEL
        );
        consume_fuel(caller, FUEL_FUEL)?;

        send_to_arraybuffer(caller, output.try_into()?, &buffer)?;
        Ok(())
    }
    pub(super) fn height(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        let height_value = caller.data_mut().context.lock().unwrap().message.height;
        let height = (&height_value.to_le_bytes()).to_vec();

        println!(
            "height: block_height={}, fuel_cost={}",
            height_value, FUEL_HEIGHT
        );
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

        println!(
            "balance: who=[{},{}], what=[{},{}], balance_size={} bytes, fuel_cost={}",
            who.block,
            who.tx,
            what.block,
            what.tx,
            balance.len(),
            FUEL_BALANCE
        );
        consume_fuel(caller, FUEL_BALANCE)?;

        send_to_arraybuffer(caller, output.try_into()?, &balance)?;
        Ok(())
    }
    pub(super) fn extcall<'a, T: Extcall>(
        caller: &mut Caller<'_, AlkanesState>,
        cellpack_ptr: i32,
        incoming_alkanes_ptr: i32,
        checkpoint_ptr: i32,
        start_fuel: u64,
    ) -> Result<i32> {
        Self::preserve_context(caller);

        // Read all input data first
        let (cellpack, incoming_alkanes, storage_map, storage_map_len) = {
            let mem = get_memory(caller)?;
            let data = mem.data(&caller);
            let buffer = read_arraybuffer(data, cellpack_ptr)?;
            let cellpack = Cellpack::parse(&mut Cursor::new(buffer))?;
            let buf = read_arraybuffer(data, incoming_alkanes_ptr)?;
            let incoming_alkanes = AlkaneTransferParcel::parse(&mut Cursor::new(buf))?;
            let storage_map_buffer = read_arraybuffer(data, checkpoint_ptr)?;
            let storage_map_len = storage_map_buffer.len();
            let storage_map = StorageMap::parse(&mut Cursor::new(storage_map_buffer))?;
            (
                cellpack,
                incoming_alkanes,
                storage_map,
                storage_map_len as u64,
            )
        };

        // Handle deployment fuel first
        if cellpack.target.is_deployment() {
            println!(
                "extcall: deployment detected, additional fuel_cost={}",
                FUEL_EXTCALL_DEPLOY
            );
            caller.consume_fuel(FUEL_EXTCALL_DEPLOY)?;
        }

        // Prepare subcontext data
        let (subcontext, binary_rc) = {
            let mut context_guard = caller.data_mut().context.lock().unwrap();
            context_guard.message.atomic.checkpoint();
            let myself = context_guard.myself.clone();
            let caller_id = context_guard.caller.clone();
            std::mem::drop(context_guard); // Release lock before calling run_special_cellpacks

            let (_subcaller, submyself, binary) =
                run_special_cellpacks(caller.data_mut().context.clone(), &cellpack)?;

            // Re-acquire lock for state updates
            let transfer_error = {
                let mut context_guard = caller.data_mut().context.lock().unwrap();
                pipe_storagemap_to(
                    &storage_map,
                    &mut context_guard.message.atomic.derive(
                        &IndexPointer::from_keyword("/alkanes/").select(&myself.clone().into()),
                    ),
                );

                let _transfer_error = transfer_from(
                    &incoming_alkanes,
                    &mut context_guard
                        .message
                        .atomic
                        .derive(&IndexPointer::default()),
                    &myself,
                    &submyself,
                )
                .is_err();
                if _transfer_error {
                    context_guard.message.atomic.rollback();
                    context_guard.returndata = Vec::<u8>::new();
                }
                _transfer_error
            };
            if transfer_error {
                Self::restore_context(caller);
                return Ok(0);
            }
            let mut context_guard = caller.data_mut().context.lock().unwrap();

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

        let base_fuel = FUEL_EXTCALL;
        let storage_fuel = overflow_error(FUEL_PER_STORE_BYTE.checked_mul(storage_map_len))?;
        let total_fuel = overflow_error(base_fuel.checked_add(storage_fuel))?;

        println!("extcall: target=[{},{}], inputs={}, storage_size={} bytes, base_fuel={}, storage_fuel={}, total_fuel={}, deployment={}",
            cellpack.target.block, cellpack.target.tx,
            cellpack.inputs.len(), storage_map_len,
            base_fuel, storage_fuel, total_fuel,
            cellpack.target.is_deployment());

        consume_fuel(caller, total_fuel)?;

        let mut trace_context: TraceContext = subcontext.flat().into();
        trace_context.fuel = start_fuel;
        let event: TraceEvent = T::event(trace_context);
        subcontext.trace.clock(event);

        // Run the call in a new context
        let result = match run_after_special(
            Arc::new(Mutex::new(subcontext.clone())),
            binary_rc,
            start_fuel,
        ) {
            Ok((response, gas_used)) => {
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
                saveable.save(&mut context_guard.message.atomic)?;
                let serialized = CallResponse::from(response.into()).serialize();
                context_guard.returndata = serialized.clone();
                T::handle_atomic(&mut context_guard.message.atomic);
                serialized.len() as i32
            }
            Err(e) => {
                let mut data: Vec<u8> = vec![0x08, 0xc3, 0x79, 0xa0];
                data.extend(e.to_string().as_bytes());

                let mut revert_context: TraceResponse = TraceResponse::default();
                revert_context.inner.data = data.clone();

                let mut response = CallResponse::default();
                response.data = data.clone();
                let serialized = response.serialize();

                // Handle revert state
                let mut context_guard = caller.data_mut().context.lock().unwrap();
                context_guard
                    .trace
                    .clock(TraceEvent::RevertContext(revert_context));
                context_guard.message.atomic.rollback();
                context_guard.returndata = serialized.clone();
                (serialized.len() as i32).checked_neg().unwrap_or(-1)
            }
        };

        Self::restore_context(caller);
        Ok(result)
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
