use super::{
    extcall::*, read_arraybuffer, AlkanesExportsImpl, AlkanesHostFunctionsImpl,
    AlkanesRuntimeContext, AlkanesState, MEMORY_LIMIT,
};
use alkanes_support::response::ExtendedCallResponse;
use anyhow::{anyhow, Result};
use hex;
use std::sync::{Arc, Mutex};
use wasmi::*;

pub struct AlkanesInstance {
    pub(crate) instance: Instance,
    pub(crate) store: Store<AlkanesState>,
}

fn handle_extcall(v: Result<i32>, caller: Caller<'_, AlkanesState>) -> i32 {
    match v {
        Ok(v) => v,
        Err(_e) => {
            AlkanesHostFunctionsImpl::_abort(caller);
            -1
        }
    }
}

impl AlkanesInstance {
    pub fn consume_fuel(&mut self, fuel: u64) -> Result<()> {
        let fuel_remaining = self.store.get_fuel().unwrap();
        if fuel_remaining < fuel {
            Err(anyhow!(format!(
                "{} gas remaining but {} consumed by call",
                fuel_remaining, fuel
            )))
        } else {
            self.store.set_fuel(fuel_remaining - fuel).unwrap();
            Ok(())
        }
    }
    pub fn read_arraybuffer(&mut self, data_start: i32) -> anyhow::Result<Vec<u8>> {
        read_arraybuffer(self.get_memory()?.data(&self.store), data_start)
    }
    pub fn get_memory(&mut self) -> anyhow::Result<Memory> {
        self.instance
            .get_memory(&mut self.store, "memory")
            .ok_or("")
            .map_err(|_| anyhow!("memory segment not found"))
    }
    pub fn send_to_arraybuffer(&mut self, ptr: usize, v: &Vec<u8>) -> anyhow::Result<i32> {
        let mem = self.get_memory()?;
        mem.write(&mut self.store, ptr, &v.len().to_le_bytes())
            .map_err(|_| anyhow!("failed to write ArrayBuffer"))?;
        mem.write(&mut self.store, ptr + 4, v.as_slice())
            .map_err(|_| anyhow!("failed to write ArrayBuffer"))?;
        Ok((ptr + 4).try_into()?)
    }
    pub fn checkpoint(&mut self) {
        (&mut self.store.data_mut().context.lock().unwrap().message)
            .atomic
            .checkpoint();
    }
    pub fn commit(&mut self) {
        (&mut self.store.data_mut().context.lock().unwrap().message)
            .atomic
            .commit();
    }
    pub fn rollback(&mut self) {
        (&mut self.store.data_mut().context.lock().unwrap().message)
            .atomic
            .rollback();
    }
    pub fn from_alkane(
        context: Arc<Mutex<AlkanesRuntimeContext>>,
        binary: Arc<Vec<u8>>,
        start_fuel: u64,
    ) -> Result<Self> {
        /*
        let binary = context
            .message
            .atomic
            .keyword("/alkanes/")
            .select(&context.myself.clone().into())
            .get();
            */
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let mut store = Store::<AlkanesState>::new(
            &engine,
            AlkanesState {
                had_failure: false,
                limiter: StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT).build(),
                context: context.clone(),
            },
        );
        store.limiter(|state| &mut state.limiter);
        Store::<AlkanesState>::set_fuel(&mut store, start_fuel)?; // TODO: implement gas limits
        let module = Module::new(&engine, &mut &binary[..])?;
        let mut linker: Linker<AlkanesState> = Linker::<AlkanesState>::new(&engine);
        linker.func_wrap("env", "abort", AlkanesHostFunctionsImpl::abort)?;
        linker.func_wrap(
            "env",
            "__load_storage",
            |mut caller: Caller<'_, AlkanesState>, k: i32, v: i32| {
                match AlkanesHostFunctionsImpl::load_storage(&mut caller, k, v) {
                    Ok(v) => v,
                    Err(_e) => {
                        AlkanesHostFunctionsImpl::_abort(caller);
                        -1
                    }
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__request_storage",
            |mut caller: Caller<'_, AlkanesState>, k: i32| {
                match AlkanesHostFunctionsImpl::request_storage(&mut caller, k) {
                    Ok(v) => v,
                    Err(_e) => {
                        AlkanesHostFunctionsImpl::_abort(caller);
                        -1
                    }
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__log",
            |mut caller: Caller<'_, AlkanesState>, v: i32| {
                if let Err(_e) = AlkanesHostFunctionsImpl::log(&mut caller, v) {
                    AlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__balance",
            |mut caller: Caller<'_, AlkanesState>, who: i32, what: i32, output: i32| {
                if let Err(_e) = AlkanesHostFunctionsImpl::balance(&mut caller, who, what, output) {
                    AlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__request_context",
            |mut caller: Caller<'_, AlkanesState>| -> i32 {
                match AlkanesHostFunctionsImpl::request_context(&mut caller) {
                    Ok(v) => v,
                    Err(_e) => {
                        AlkanesHostFunctionsImpl::_abort(caller);
                        -1
                    }
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__load_context",
            |mut caller: Caller<'_, AlkanesState>, output: i32| {
                match AlkanesHostFunctionsImpl::load_context(&mut caller, output) {
                    Ok(v) => v,
                    Err(_e) => {
                        AlkanesHostFunctionsImpl::_abort(caller);
                        -1
                    }
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__sequence",
            |mut caller: Caller<'_, AlkanesState>, output: i32| {
                if let Err(_e) = AlkanesHostFunctionsImpl::sequence(&mut caller, output) {
                    AlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__fuel",
            |mut caller: Caller<'_, AlkanesState>, output: i32| {
                if let Err(_e) = AlkanesHostFunctionsImpl::fuel(&mut caller, output) {
                    AlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__height",
            |mut caller: Caller<'_, AlkanesState>, output: i32| {
                if let Err(_e) = AlkanesHostFunctionsImpl::height(&mut caller, output) {
                    AlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;

        linker.func_wrap(
            "env",
            "__returndatacopy",
            |mut caller: Caller<'_, AlkanesState>, output: i32| {
                if let Err(_e) = AlkanesHostFunctionsImpl::returndatacopy(&mut caller, output) {
                    AlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__request_transaction",
            |mut caller: Caller<'_, AlkanesState>| -> i32 {
                match AlkanesHostFunctionsImpl::request_transaction(&mut caller) {
                    Ok(v) => v,
                    Err(_e) => {
                        AlkanesHostFunctionsImpl::_abort(caller);
                        -1
                    }
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__load_transaction",
            |mut caller: Caller<'_, AlkanesState>, output: i32| {
                if let Err(_e) = AlkanesHostFunctionsImpl::load_transaction(&mut caller, output) {
                    AlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        /* removed below to prevent redundancy / requirement for archived chaindata */
        /*
        linker.func_wrap(
            "env",
            "__request_output",
            |mut caller: Caller<'_, AlkanesState>, outpoint: i32| -> i32 {
                match AlkanesHostFunctionsImpl::request_output(&mut caller, outpoint) {
                  Err(_e) => {
                    AlkanesHostFunctionsImpl::_abort(caller);
                    -1
                  }
                  Ok(v) => v
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__load_output",
            |mut caller: Caller<'_, AlkanesState>, outpoint: i32, output: i32| {
                if let Err(_e) = AlkanesHostFunctionsImpl::load_output(&mut caller, outpoint, output) {
                    AlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        */
        linker.func_wrap(
            "env",
            "__request_block",
            |mut caller: Caller<'_, AlkanesState>| match AlkanesHostFunctionsImpl::request_block(
                &mut caller,
            ) {
                Ok(v) => v,
                Err(_e) => {
                    AlkanesHostFunctionsImpl::_abort(caller);
                    -1
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__load_block",
            |mut caller: Caller<'_, AlkanesState>, output: i32| {
                if let Err(_e) = AlkanesHostFunctionsImpl::load_block(&mut caller, output) {
                    AlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__call",
            |mut caller: Caller<'_, AlkanesState>,
             cellpack_ptr: i32,
             incoming_alkanes_ptr: i32,
             checkpoint_ptr: i32,
             start_fuel: u64|
             -> i32 {
                handle_extcall(
                    AlkanesHostFunctionsImpl::extcall::<Call>(
                        &mut caller,
                        cellpack_ptr,
                        incoming_alkanes_ptr,
                        checkpoint_ptr,
                        start_fuel,
                    ),
                    caller,
                )
            },
        )?;
        linker.func_wrap(
            "env",
            "__delegatecall",
            |mut caller: Caller<'_, AlkanesState>,
             cellpack_ptr: i32,
             incoming_alkanes_ptr: i32,
             checkpoint_ptr: i32,
             start_fuel: u64|
             -> i32 {
                handle_extcall(
                    AlkanesHostFunctionsImpl::extcall::<Delegatecall>(
                        &mut caller,
                        cellpack_ptr,
                        incoming_alkanes_ptr,
                        checkpoint_ptr,
                        start_fuel,
                    ),
                    caller,
                )
            },
        )?;
        linker.func_wrap(
            "env",
            "__staticcall",
            |mut caller: Caller<'_, AlkanesState>,
             cellpack_ptr: i32,
             incoming_alkanes_ptr: i32,
             checkpoint_ptr: i32,
             start_fuel: u64|
             -> i32 {
                handle_extcall(
                    AlkanesHostFunctionsImpl::extcall::<Staticcall>(
                        &mut caller,
                        cellpack_ptr,
                        incoming_alkanes_ptr,
                        checkpoint_ptr,
                        start_fuel,
                    ),
                    caller,
                )
            },
        )?;
        Ok(AlkanesInstance {
            instance: linker
                .instantiate(&mut store, &module)?
                .ensure_no_start(&mut store)?,
            store,
        })
    }
    pub fn reset(&mut self) {
        self.store.data_mut().had_failure = false;
    }
    pub fn execute(&mut self) -> Result<ExtendedCallResponse> {
        self.checkpoint();
        let mut err: Option<anyhow::Error> = None;
        let (call_response, had_failure): (ExtendedCallResponse, bool) = {
            match AlkanesExportsImpl::execute(self) {
                Ok(v) => {
                    if self.store.data().had_failure {
                        (v, true)
                    } else {
                        (v, false)
                    }
                }
                Err(e) => {
                    err = Some(e);
                    (ExtendedCallResponse::default(), true)
                }
            }
        };
        self.reset();
        if had_failure {
            self.rollback();
            if call_response.data.len() >= 4
                && &call_response.data[0..4] == &[0x08, 0xc3, 0x79, 0xa0]
            {
                Err(anyhow!(format!(
                    "ALKANES: revert: {}",
                    String::from_utf8((&call_response.data[4..]).to_vec())
                        .unwrap_or_else(|_| hex::encode(&call_response.data[4..]))
                )))
            } else if let Some(e) = err {
                Err(anyhow!(format!("ALKANES: revert: {:?}", e)))
            } else {
                Err(anyhow!("ALKANES: revert"))
            }
        } else {
            self.commit();
            Ok(call_response)
        }
    }

    pub fn call_meta(&mut self) -> Result<Vec<u8>> {
        // Call the __meta function to get the ABI
        AlkanesExportsImpl::call_meta(self)
    }
}
