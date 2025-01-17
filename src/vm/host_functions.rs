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
use anyhow::Result;
use metashrew::index_pointer::IndexPointer;
#[allow(unused_imports)]
use metashrew::{
    print, println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;

use crate::vm::fuel::{
    consume_fuel, Fuelable, FUEL_BALANCE, FUEL_EXTCALL, FUEL_EXTCALL_DEPLOY, FUEL_FUEL,
    FUEL_HEIGHT, FUEL_PER_LOAD_BYTE, FUEL_PER_REQUEST_BYTE, FUEL_PER_STORE_BYTE, FUEL_SEQUENCE,
};
use protorune_support::utils::consensus_encode;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use wasmi::*;

pub struct AlkanesHostFunctionsImpl(());

impl AlkanesHostFunctionsImpl {
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
        consume_fuel(
            caller,
            overflow_error((bytes_processed as u64).checked_mul(FUEL_PER_REQUEST_BYTE))?,
        )?;
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
        consume_fuel(
            caller,
            overflow_error((bytes_processed as u64).checked_mul(FUEL_PER_LOAD_BYTE))?,
        )?;
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
        consume_fuel(
            caller,
            overflow_error((result as u64).checked_mul(FUEL_PER_REQUEST_BYTE))?,
        )?;
        Ok(result)
    }
    pub(super) fn load_context(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<i32> {
        let result: Vec<u8> = caller.data_mut().context.lock().unwrap().serialize();
        consume_fuel(
            caller,
            overflow_error((result.len() as u64).checked_mul(FUEL_PER_LOAD_BYTE))?,
        )?;
        send_to_arraybuffer(caller, v.try_into()?, &result)
    }
    pub(super) fn request_transaction(caller: &mut Caller<'_, AlkanesState>) -> Result<i32> {
        let result: i32 = consensus_encode(
            &caller
                .data_mut()
                .context
                .lock()
                .unwrap()
                .message
                .transaction,
        )?
        .len()
        .try_into()?;
        consume_fuel(
            caller,
            overflow_error((result as u64).checked_mul(FUEL_PER_REQUEST_BYTE))?,
        )?;
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
        consume_fuel(
            caller,
            overflow_error((returndata.len() as u64).checked_mul(FUEL_PER_LOAD_BYTE))?,
        )?;
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
        consume_fuel(
            caller,
            overflow_error((transaction.len() as u64).checked_mul(FUEL_PER_LOAD_BYTE))?,
        )?;
        send_to_arraybuffer(caller, v.try_into()?, &transaction)?;
        Ok(())
    }
    pub(super) fn request_block(caller: &mut Caller<'_, AlkanesState>) -> Result<i32> {
        let len: i32 = consensus_encode(&caller.data_mut().context.lock().unwrap().message.block)?
            .len()
            .try_into()?;
        consume_fuel(
            caller,
            overflow_error((len as u64).checked_mul(FUEL_PER_REQUEST_BYTE))?,
        )?;
        Ok(len)
    }
    pub(super) fn load_block(caller: &mut Caller<'_, AlkanesState>, v: i32) -> Result<()> {
        let block: Vec<u8> =
            consensus_encode(&caller.data_mut().context.lock().unwrap().message.block)?;
        consume_fuel(
            caller,
            overflow_error((block.len() as u64).checked_mul(FUEL_PER_LOAD_BYTE))?,
        )?;
        send_to_arraybuffer(caller, v.try_into()?, &block)?;
        Ok(())
    }
    pub(super) fn sequence(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        let buffer: Vec<u8> =
            (&sequence_pointer(&caller.data_mut().context.lock().unwrap().message.atomic)
                .get_value::<u128>()
                .to_le_bytes())
                .to_vec();
        consume_fuel(caller, FUEL_SEQUENCE)?;
        send_to_arraybuffer(caller, output.try_into()?, &buffer)?;
        Ok(())
    }
    pub(super) fn fuel(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        let buffer: Vec<u8> = (&caller.get_fuel().unwrap().to_le_bytes()).to_vec();
        consume_fuel(caller, FUEL_FUEL)?;
        send_to_arraybuffer(caller, output.try_into()?, &buffer)?;
        Ok(())
    }
    pub(super) fn height(caller: &mut Caller<'_, AlkanesState>, output: i32) -> Result<()> {
        let height = (&caller
            .data_mut()
            .context
            .lock()
            .unwrap()
            .message
            .height
            .to_le_bytes())
            .to_vec();
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
        let (subcontext, binary_rc) = {
            if cellpack.target.is_deployment() {
                caller.consume_fuel(FUEL_EXTCALL_DEPLOY)?;
            }
            let context = caller.data_mut().context.clone();
            context.lock().unwrap().message.atomic.checkpoint();
            let myself = context.lock().unwrap().myself.clone();
            let (_subcaller, submyself, binary) =
                run_special_cellpacks(context.clone(), &cellpack)?;
            pipe_storagemap_to(
                &storage_map,
                &mut context.lock().unwrap().message.atomic.derive(
                    &IndexPointer::from_keyword("/alkanes/").select(&myself.clone().into()),
                ),
            );
            if let Err(_) = transfer_from(
                &incoming_alkanes,
                &mut context
                    .lock()
                    .unwrap()
                    .message
                    .atomic
                    .derive(&IndexPointer::default()),
                &myself,
                &submyself,
            ) {
                context.lock().unwrap().message.atomic.rollback();
                context.lock().unwrap().returndata = Vec::<u8>::new();
                return Ok(0);
            }
            let mut subbed = context.lock().unwrap().clone();
            subbed.message.atomic = context
                .lock()
                .unwrap()
                .message
                .atomic
                .derive(&IndexPointer::default());
            let caller = context.lock().unwrap().caller.clone();
            (subbed.caller, subbed.myself) =
                T::change_context(submyself.clone(), caller, myself.clone());
            subbed.returndata = vec![];
            subbed.incoming_alkanes = incoming_alkanes.clone();
            subbed.inputs = cellpack.inputs.clone();
            (subbed, binary)
        };
        consume_fuel(
            caller,
            overflow_error(FUEL_EXTCALL.checked_add(overflow_error(
                FUEL_PER_STORE_BYTE.checked_mul(storage_map_len),
            )?))?,
        )?;
        let mut trace_context: TraceContext = subcontext.to_context().into();
        trace_context.fuel = start_fuel;
        let event: TraceEvent = T::event(trace_context);
        subcontext.trace.clock(event);
        run_after_special(
            Arc::new(Mutex::new(subcontext.clone())),
            binary_rc,
            start_fuel,
        )
        .and_then(|(response, gas_used)| {
            caller.set_fuel(overflow_error(start_fuel.checked_sub(gas_used))?)?;
            let mut return_context: TraceResponse = response.clone().into();
            return_context.fuel_used = gas_used;
            subcontext
                .trace
                .clock(TraceEvent::ReturnContext(return_context));
            let mut context = caller.data().context.lock().unwrap();
            let mut saveable: SaveableExtendedCallResponse = response.clone().into();
            saveable.associate(&subcontext);
            saveable.save(&mut context.message.atomic)?;
            T::handle_atomic(&mut context.message.atomic);
            let plain_response: CallResponse = response.clone().into();
            let serialized = plain_response.serialize();
            context.returndata = serialized;
            Ok(context.returndata.len().try_into()?)
        })
        .and_then(|len| {
            let mut context = caller.data_mut().context.lock().unwrap();
            T::handle_atomic(&mut context.message.atomic);
            Ok(len)
        })
        .or_else(|e| {
            let mut context = caller.data_mut().context.lock().unwrap();
            let mut revert_context: TraceResponse = TraceResponse::default();
            revert_context.inner.data = vec![0x08, 0xc3, 0x79, 0xa0];
            revert_context.inner.data.extend(e.to_string().as_bytes());
            context
                .trace
                .clock(TraceEvent::RevertContext(revert_context));
            context.message.atomic.rollback();
            context.returndata = vec![];
            Ok(0)
        })
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
