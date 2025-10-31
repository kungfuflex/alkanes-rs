use super::{
    extcall::*, AlkanesExportsImpl, AlkanesRuntimeContext, state::AlkanesState,
    SafeAlkanesHostFunctionsImpl, MEMORY_LIMIT, utils::read_arraybuffer,
};
use alkanes_support::{
    response::ExtendedCallResponse,
};
use anyhow::{anyhow, Result};
use hex;
use std::sync::{Arc, Mutex};
use wasmi::*;



pub struct AlkanesInstance<'a, E: RuntimeEnvironment + Clone> {
    pub(crate) instance: Instance,
    pub(crate) store: Store<AlkanesState<'a, E>>,
}

impl<'a, E: RuntimeAdapter + 'static + Clone + Default> AlkanesInstance<'a, E> {
    pub fn consume_fuel(&mut self, fuel: u64) -> Result<()> {
        let fuel_remaining = self.store.get_fuel()?;
        if fuel_remaining < fuel {
            Err(anyhow!(format!(
                "{} gas remaining but {} consumed by call",
                fuel_remaining, fuel
            )))
        } else {
            self.store.set_fuel(fuel_remaining - fuel)?;
            Ok(())
        }
    }
    pub fn read_from_memory(&mut self, data_start: i32) -> Result<Vec<u8>> {
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
        let mut atomic = {
            self.store
                .data_mut()
                .context
                .lock()
                .unwrap()
                .message
                .atomic
                .clone()
        };
        let env = &mut self.store.data_mut().env;
        atomic.commit();
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
        let mut store = Store::<AlkanesState<'a, E>>::new(
            &engine,
            AlkanesState {
                had_failure: false,
                limiter: StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT).build(),
                context: context.clone(),
                env,
            },
        );
        store.limiter(|state| &mut state.limiter);
        Store::<AlkanesState<'a, E>>::set_fuel(&mut store, start_fuel)?; // TODO: implement gas limits
        let module = Module::new(&engine, &mut &binary[..])?;
        let mut linker: Linker<AlkanesState<'a, E>> = Linker::<AlkanesState<'a, E>>::new(&engine);
        linker.func_wrap("env", "abort", SafeAlkanesHostFunctionsImpl::abort)?;
        linker.func_wrap(
            "env",
            "__load_storage",
            |mut caller: Caller<'_, AlkanesState<'a, E>>, k: i32, v: i32| {
                match SafeAlkanesHostFunctionsImpl::load_storage(&mut caller, k, v) {
                    Ok(v) => v,
                    Err(_e) => {
                        SafeAlkanesHostFunctionsImpl::_abort(caller);
                        -1
                    }
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__request_storage",
            |mut caller: Caller<'_, AlkanesState<'a, E>>, k: i32| {
                match SafeAlkanesHostFunctionsImpl::request_storage(&mut caller, k) {
                    Ok(v) => v,
                    Err(_e) => {
                        SafeAlkanesHostFunctionsImpl::_abort(caller);
                        -1
                    }
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__log",
            |mut caller: Caller<'_, AlkanesState<'a, E>>, v: i32| {
                if let Err(_e) = SafeAlkanesHostFunctionsImpl::log(&mut caller, v) {
                    SafeAlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__balance",
            |mut caller: Caller<'_, AlkanesState<'a, E>>, who: i32, what: i32, output: i32| {
                if let Err(_e) =
                    SafeAlkanesHostFunctionsImpl::balance(&mut caller, who, what, output)
                {
                    SafeAlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__request_context",
            |mut caller: Caller<'_, AlkanesState<'a, E>>| -> i32 {
                match SafeAlkanesHostFunctionsImpl::request_context(&mut caller) {
                    Ok(v) => v,
                    Err(_e) => {
                        SafeAlkanesHostFunctionsImpl::_abort(caller);
                        -1
                    }
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__load_context",
            |mut caller: Caller<'_, AlkanesState<'a, E>>, output: i32| {
                match SafeAlkanesHostFunctionsImpl::load_context(&mut caller, output) {
                    Ok(v) => v,
                    Err(_e) => {
                        SafeAlkanesHostFunctionsImpl::_abort(caller);
                        -1
                    }
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__sequence",
            |mut caller: Caller<'_, AlkanesState<'a, E>>, output: i32| {
                if let Err(_e) = SafeAlkanesHostFunctionsImpl::sequence(&mut caller, output) {
                    SafeAlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__fuel",
            |mut caller: Caller<'_, AlkanesState<'a, E>>, output: i32| {
                if let Err(_e) = SafeAlkanesHostFunctionsImpl::fuel(&mut caller, output) {
                    SafeAlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__height",
            |mut caller: Caller<'_, AlkanesState<'a, E>>, output: i32| {
                if let Err(_e) = SafeAlkanesHostFunctionsImpl::height(&mut caller, output) {
                    SafeAlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;

        linker.func_wrap(
            "env",
            "__returndatacopy",
            |mut caller: Caller<'_, AlkanesState<'a, E>>, output: i32| {
                if let Err(_e) = SafeAlkanesHostFunctionsImpl::returndatacopy(&mut caller, output) {
                    SafeAlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__request_transaction",
            |mut caller: Caller<'_, AlkanesState<'a, E>>| -> i32 {
                match SafeAlkanesHostFunctionsImpl::request_transaction(&mut caller) {
                    Ok(v) => v,
                    Err(_e) => {
                        SafeAlkanesHostFunctionsImpl::_abort(caller);
                        -1
                    }
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__load_transaction",
            |mut caller: Caller<'_, AlkanesState<'a, E>>, output: i32| {
                if let Err(_e) = SafeAlkanesHostFunctionsImpl::load_transaction(&mut caller, output)
                {
                    SafeAlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        /* removed below to prevent redundancy / requirement for archived chaindata */
        /*
        linker.func_wrap(
            "env",
            "__request_output",
            |mut caller: Caller<'_, AlkanesState<E>>, outpoint: i32| -> i32 {
                match SafeAlkanesHostFunctionsImpl::request_output(&mut caller, outpoint) {
                  Err(_e) => {
                    SafeAlkanesHostFunctionsImpl::_abort(caller);
                    -1
                  }
                  Ok(v) => v
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__load_output",
            |mut caller: Caller<'_, AlkanesState<E>>, outpoint: i32, output: i32| {
                if let Err(_e) = SafeAlkanesHostFunctionsImpl::load_output(&mut caller, outpoint, output) {
                    SafeAlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        */
        linker.func_wrap(
            "env",
            "__request_block",
            |mut caller: Caller<'_, AlkanesState<'a, E>>| match SafeAlkanesHostFunctionsImpl::request_block(
                &mut caller,
            ) {
                Ok(v) => v,
                Err(_e) => {
                    SafeAlkanesHostFunctionsImpl::_abort(caller);
                    -1
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__load_block",
            |mut caller: Caller<'_, AlkanesState<'a, E>>, output: i32| {
                if let Err(_e) = SafeAlkanesHostFunctionsImpl::load_block(&mut caller, output) {
                    SafeAlkanesHostFunctionsImpl::_abort(caller);
                }
            },
        )?;
        linker.func_wrap(
            "env",
            "__call",
            |mut caller: Caller<'_, AlkanesState<'a, E>>,
             cellpack_ptr: i32,
             incoming_alkanes_ptr: i32,
             checkpoint_ptr: i32,
             start_fuel: u64|
             -> i32 {
                SafeAlkanesHostFunctionsImpl::handle_extcall::<E, Call>(
                    &mut caller,
                    cellpack_ptr,
                    incoming_alkanes_ptr,
                    checkpoint_ptr,
                    start_fuel,
                )
            },
        )?;
        linker.func_wrap(
            "env",
            "__delegatecall",
            |mut caller: Caller<'_, AlkanesState<'a, E>>,
             cellpack_ptr: i32,
             incoming_alkanes_ptr: i32,
             checkpoint_ptr: i32,
             start_fuel: u64|
             -> i32 {
                SafeAlkanesHostFunctionsImpl::handle_extcall::<E, Delegatecall>(
                    &mut caller,
                    cellpack_ptr,
                    incoming_alkanes_ptr,
                    checkpoint_ptr,
                    start_fuel,
                )
            },
        )?;
        linker.func_wrap(
            "env",
            "__staticcall",
            |mut caller: Caller<'_, AlkanesState<'a, E>>,
             cellpack_ptr: i32,
             incoming_alkanes_ptr: i32,
             checkpoint_ptr: i32,
             start_fuel: u64|
             -> i32 {
                SafeAlkanesHostFunctionsImpl::handle_extcall::<E, Staticcall>(
                    &mut caller,
                    cellpack_ptr,
                    incoming_alkanes_ptr,
                    checkpoint_ptr,
                    start_fuel,
                )
            },
        )?;
        let mut alkanes_instance = AlkanesInstance {
            instance: linker
                .instantiate(&mut store, &module)?
                .ensure_no_start(&mut store)?,
            store,
        };

        let memory = alkanes_instance.get_memory()?;

        let current_pages = memory.size(&alkanes_instance.store);

        if current_pages < 512 {
            memory
                .grow(&mut alkanes_instance.store, 512 - current_pages)
                .expect("Failed to grow memory");
        }

        Ok(alkanes_instance)
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
