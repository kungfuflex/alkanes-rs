use super::{
    extcall::*, read_arraybuffer, AlkanesExportsImpl, AlkanesRuntimeContext, AlkanesState,
    SafeAlkanesHostFunctionsImpl, MEMORY_LIMIT,
};
use alkanes_support::{
    response::{CallResponse, ExtendedCallResponse},
    trace::{TraceEvent, TraceResponse},
};
use anyhow::{anyhow, Result};
use hex;
use std::sync::{Arc, Mutex};
use wasmi::*;

#[allow(unused_imports)]
use metashrew_core::{
    print, println,
    stdio::{stdout, Write},
};

pub struct AlkanesInstance {
    pub(crate) instance: Instance,
    pub(crate) store: Store<AlkanesState>,
}

/// Every host function an alkane can import. Built once per [`VmCache`]
/// engine; a `Linker` only holds these definitions, never per-call state.
fn build_linker(engine: &Engine) -> Result<Linker<AlkanesState>> {
    let mut linker: Linker<AlkanesState> = Linker::<AlkanesState>::new(engine);
    linker.func_wrap("env", "abort", SafeAlkanesHostFunctionsImpl::abort)?;
    linker.func_wrap(
        "env",
        "__load_storage",
        |mut caller: Caller<'_, AlkanesState>, k: i32, v: i32| {
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
        |mut caller: Caller<'_, AlkanesState>, k: i32| {
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
        |mut caller: Caller<'_, AlkanesState>, v: i32| {
            if let Err(_e) = SafeAlkanesHostFunctionsImpl::log(&mut caller, v) {
                SafeAlkanesHostFunctionsImpl::_abort(caller);
            }
        },
    )?;
    linker.func_wrap(
        "env",
        "__balance",
        |mut caller: Caller<'_, AlkanesState>, who: i32, what: i32, output: i32| {
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
        |mut caller: Caller<'_, AlkanesState>| -> i32 {
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
        |mut caller: Caller<'_, AlkanesState>, output: i32| {
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
        |mut caller: Caller<'_, AlkanesState>, output: i32| {
            if let Err(_e) = SafeAlkanesHostFunctionsImpl::sequence(&mut caller, output) {
                SafeAlkanesHostFunctionsImpl::_abort(caller);
            }
        },
    )?;
    linker.func_wrap(
        "env",
        "__fuel",
        |mut caller: Caller<'_, AlkanesState>, output: i32| {
            if let Err(_e) = SafeAlkanesHostFunctionsImpl::fuel(&mut caller, output) {
                SafeAlkanesHostFunctionsImpl::_abort(caller);
            }
        },
    )?;
    linker.func_wrap(
        "env",
        "__height",
        |mut caller: Caller<'_, AlkanesState>, output: i32| {
            if let Err(_e) = SafeAlkanesHostFunctionsImpl::height(&mut caller, output) {
                SafeAlkanesHostFunctionsImpl::_abort(caller);
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "__returndatacopy",
        |mut caller: Caller<'_, AlkanesState>, output: i32| {
            if let Err(_e) = SafeAlkanesHostFunctionsImpl::returndatacopy(&mut caller, output) {
                SafeAlkanesHostFunctionsImpl::_abort(caller);
            }
        },
    )?;
    linker.func_wrap(
        "env",
        "__request_transaction",
        |mut caller: Caller<'_, AlkanesState>| -> i32 {
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
        |mut caller: Caller<'_, AlkanesState>, output: i32| {
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
        |mut caller: Caller<'_, AlkanesState>, outpoint: i32| -> i32 {
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
        |mut caller: Caller<'_, AlkanesState>, outpoint: i32, output: i32| {
            if let Err(_e) = SafeAlkanesHostFunctionsImpl::load_output(&mut caller, outpoint, output) {
                SafeAlkanesHostFunctionsImpl::_abort(caller);
            }
        },
    )?;
    */
    linker.func_wrap(
        "env",
        "__request_block",
        |mut caller: Caller<'_, AlkanesState>| match SafeAlkanesHostFunctionsImpl::request_block(
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
        |mut caller: Caller<'_, AlkanesState>, output: i32| {
            if let Err(_e) = SafeAlkanesHostFunctionsImpl::load_block(&mut caller, output) {
                SafeAlkanesHostFunctionsImpl::_abort(caller);
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
            SafeAlkanesHostFunctionsImpl::handle_extcall::<Call>(
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
        |mut caller: Caller<'_, AlkanesState>,
         cellpack_ptr: i32,
         incoming_alkanes_ptr: i32,
         checkpoint_ptr: i32,
         start_fuel: u64|
         -> i32 {
            SafeAlkanesHostFunctionsImpl::handle_extcall::<Delegatecall>(
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
        |mut caller: Caller<'_, AlkanesState>,
         cellpack_ptr: i32,
         incoming_alkanes_ptr: i32,
         checkpoint_ptr: i32,
         start_fuel: u64|
         -> i32 {
            SafeAlkanesHostFunctionsImpl::handle_extcall::<Staticcall>(
                &mut caller,
                cellpack_ptr,
                incoming_alkanes_ptr,
                checkpoint_ptr,
                start_fuel,
            )
        },
    )?;
    Ok(linker)
}

/// Per-block wasmi state shared by every alkane call in the block.
///
/// Before this, every call (top-level message and every extcall) built a new
/// `Engine`, re-parsed/validated/translated the whole module and rebuilt the
/// `Linker`: ~36% of indexing time on a DIESEL-mint-heavy block, three times
/// per v3 mint (DIESEL, gate, treasury). Now the engine and linker live for
/// the block and each distinct bytecode is compiled once.
///
/// Consensus-neutral: wasmi compiles eagerly (`CompilationMode::Eager`, the
/// default), so translation happens in `Module::new` with no fuel charged;
/// fuel is only consumed by execution in a fresh per-call `Store`. Each call
/// still gets its own `Store`, `Instance` and linear memory.
///
/// Modules are keyed by the identity of the bytecode `Arc`, which is held in
/// the entry so its address cannot be reused while cached. Callers get stable
/// `Arc`s from the content-keyed bytecode cache in `vm::utils`
/// (`cached_decompress` / precompiled-by-height), so the same code maps to
/// the same entry; a fresh `Arc` (e.g. a CREATE in this tx) simply compiles
/// once more. Reset at the start and end of every block (`VmCacheGuard`);
/// bounded by module count and compiled bytes, so long-lived view instances
/// (which never reset) stay bounded too.
pub(crate) struct VmCache {
    engine: Engine,
    linker: Linker<AlkanesState>,
    modules: std::collections::HashMap<usize, (Arc<Vec<u8>>, Module)>,
    /// Bytecode bytes compiled into `engine` (a proxy for its code size).
    compiled_bytes: usize,
}

impl VmCache {
    fn new() -> Result<Self> {
        let engine = Engine::new(&engine_config());
        let linker = build_linker(&engine)?;
        Ok(VmCache {
            engine,
            linker,
            modules: std::collections::HashMap::new(),
            compiled_bytes: 0,
        })
    }
}

/// Bounds on one engine's compiled code. Hitting either replaces the engine
/// (see `from_alkane`). Real contracts are ~0.1-0.5 MiB; these only matter
/// against blocks built to blow up the cache.
const VM_CACHE_MAX_MODULES: usize = 512;
const VM_CACHE_MAX_COMPILED_BYTES: usize = 128 * 1024 * 1024;
/// Larger binaries are compiled per call (the old behaviour), never cached.
pub(crate) const VM_CACHE_MAX_BINARY_BYTES: usize = 4 * 1024 * 1024;

thread_local! {
    static VM_CACHE: std::cell::RefCell<Option<VmCache>> = std::cell::RefCell::new(None);
}

/// Drop the engine, linker and every compiled module. Called at the start of
/// each block so no compiled code outlives the block that compiled it.
pub fn reset_vm_cache() {
    VM_CACHE.with(|c| *c.borrow_mut() = None);
    super::utils::reset_binary_cache();
}

/// Resets the VM and bytecode caches when dropped: held for the whole of
/// `index_block` so nothing compiled for a block outlives it, error paths
/// included.
pub struct VmCacheGuard(());

impl VmCacheGuard {
    pub fn new() -> Self {
        reset_vm_cache();
        VmCacheGuard(())
    }
}

impl Drop for VmCacheGuard {
    fn drop(&mut self) {
        reset_vm_cache();
    }
}

fn engine_config() -> Config {
    let mut config = Config::default();
    config.consume_fuel(true);
    config
}

impl AlkanesInstance {
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
        VM_CACHE.with(|cell| -> Result<Self> {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                *slot = Some(VmCache::new()?);
            }
            let key = Arc::as_ptr(&binary) as usize;
            let hit = slot.as_ref().unwrap().modules.get(&key).map(|(_, m)| m.clone());
            let module = match hit {
                Some(module) => module,
                None => {
                    let cacheable = binary.len() <= VM_CACHE_MAX_BINARY_BYTES;
                    {
                        let cache = slot.as_ref().unwrap();
                        let over = cache.modules.len() >= VM_CACHE_MAX_MODULES
                            || cache.compiled_bytes + binary.len() > VM_CACHE_MAX_COMPILED_BYTES;
                        if cacheable && over {
                            // wasmi never frees compiled code inside an Engine, so a
                            // full cache drops the whole engine (running calls keep
                            // theirs alive through their Store) and the bytecode
                            // cache with it, so the two never drift apart.
                            *slot = Some(VmCache::new()?);
                            super::utils::reset_binary_cache();
                        }
                    }
                    let cache = slot.as_mut().unwrap();
                    let module = Module::new(&cache.engine, &mut &binary[..])?;
                    if cacheable {
                        cache.compiled_bytes += binary.len();
                        cache.modules.insert(key, (binary.clone(), module.clone()));
                    }
                    module
                }
            };
            let cache = slot.as_ref().unwrap();
            let mut store = Store::<AlkanesState>::new(
                &cache.engine,
                AlkanesState {
                    had_failure: false,
                    limiter: StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT).build(),
                    context: context.clone(),
                },
            );
            store.limiter(|state| &mut state.limiter);
            Store::<AlkanesState>::set_fuel(&mut store, start_fuel)?; // TODO: implement gas limits
            let instance = cache
                .linker
                .instantiate(&mut store, &module)?
                .ensure_no_start(&mut store)?;
            drop(slot);
            Self::finish_instance(instance, store)
        })
    }

    fn finish_instance(instance: Instance, store: Store<AlkanesState>) -> Result<Self> {
        let mut alkanes_instance = AlkanesInstance { instance, store };
        let memory = alkanes_instance.get_memory()?;

        let current_pages = memory.size(&alkanes_instance.store);

        // The module's declared memory maximum is attacker-controlled; a
        // maximum below 512 pages makes this grow fail. That must be a
        // per-message revert, never a panic that halts block indexing.
        if current_pages < 512 {
            memory
                .grow(&mut alkanes_instance.store, 512 - current_pages)
                .map_err(|e| anyhow!("failed to grow memory to 512 pages: {:?}", e))?;
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
