//! WASM indexer runtime with read-through upstream host functions.
//!
//! Parallel structure to the slim `WasmIndexerRuntime` in
//! `vendor/qubitcoin-indexer/src/runtime.rs`. Key differences:
//!
//!   * `__get`/`__get_len` fall through to a [`ForkUpstream`] when the
//!     in-memory storage doesn't have a key. The fetched value is
//!     written back into the overlay so subsequent reads short-circuit.
//!   * Storage is a [`MemStorage`] (HashMap-backed) instead of RocksDB.
//!
//! The sync host fns use `tokio::task::block_in_place` +
//! `Handle::current().block_on(...)` to reach the async upstream from
//! inside the synchronous wasmtime `Caller`. The async host fns
//! `.await` directly. Runtime instantiation creates a one-shot
//! multi-threaded tokio runtime per `run_block` call so
//! `block_in_place` works even from worker threads.

use prost::Message;
use std::sync::Arc;
use tokio::runtime::Handle;
use wasmtime::*;

use crate::state::ForkWasmState;
use crate::storage::MemStorage;
use crate::upstream::ForkUpstream;

/// Protobuf message reuse: the wire format matches the slim runtime,
/// so we can lean on `qubitcoin_indexer_core::proto::KeyValueFlush`.
use qubitcoin_indexer_core::proto;

// ---------------------------------------------------------------------------
// Engine config (matches the slim runtime)
// ---------------------------------------------------------------------------

fn base_config() -> Config {
    let mut config = Config::new();
    config.wasm_bulk_memory(true);
    config.wasm_multi_value(true);
    config.wasm_reference_types(true);
    config.wasm_simd(true);
    config.cranelift_nan_canonicalization(true);
    config.relaxed_simd_deterministic(true);
    config.memory_reservation(0x100000000); // 4GB
    config.memory_guard_size(0x10000); // 64KB
    config.memory_init_cow(true);
    config
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

/// A compiled WASM indexer runtime backed by [`MemStorage`] with
/// optional read-through upstream.
pub struct ForkRuntime {
    /// Sync engine for block processing (paired with async _start).
    engine: Engine,
    module: Module,
    /// Async engine with fuel for view functions.
    async_engine: Engine,
    async_module: Module,
}

impl ForkRuntime {
    /// Compile a WASM module from bytes.
    pub fn new(wasm_bytes: &[u8]) -> Result<Self, String> {
        let mut config = base_config();
        config.async_support(true);
        let engine = Engine::new(&config).map_err(|e| format!("wasmtime engine: {}", e))?;
        let module =
            Module::new(&engine, wasm_bytes).map_err(|e| format!("wasmtime compile: {}", e))?;

        let mut async_config = base_config();
        async_config.async_support(true);
        async_config.consume_fuel(true);
        let async_engine = Engine::new(&async_config)
            .map_err(|e| format!("async wasmtime engine: {}", e))?;
        let async_module = Module::new(&async_engine, wasm_bytes)
            .map_err(|e| format!("async wasmtime compile: {}", e))?;

        Ok(Self {
            engine,
            module,
            async_engine,
            async_module,
        })
    }

    /// Run `_start()` for block processing using async execution.
    ///
    /// Creates a one-shot multi-threaded tokio runtime so the sync
    /// host fns can use `block_in_place` to reach the async upstream.
    pub fn run_block(
        &self,
        input_data: Vec<u8>,
        storage: MemStorage,
        label: &str,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>, String> {
        let state = ForkWasmState::new(input_data, storage, label, false);
        let mut store = Store::new(&self.engine, state);
        store.limiter(|s| &mut s.limits);

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| format!("tokio runtime: {}", e))?;

        let result: Result<(), String> = rt.block_on(async {
            let instance = self.instantiate_async_block(&mut store).await?;
            prepare_memory(&instance, &mut store);

            let start_fn = instance
                .get_typed_func::<(), ()>(&mut store, "_start")
                .map_err(|e| format!("missing _start: {}", e))?;

            start_fn
                .call_async(&mut store, ())
                .await
                .map_err(|e| format!("_start failed: {}", e))?;
            Ok(())
        });
        result?;

        let state = store.into_data();
        if state.had_failure {
            return Err("WASM module aborted".into());
        }
        if !state.completed {
            return Err("WASM module did not call __flush".into());
        }
        Ok(state.pending_flush.unwrap_or_default())
    }

    /// Call a view function synchronously (caller threads through the
    /// metashrew height-prefixed input).
    ///
    /// Internally drives a multi-threaded tokio runtime so the sync
    /// host fns can use `block_in_place` to reach the async upstream.
    pub fn call_view(
        &self,
        fn_name: &str,
        input_data: Vec<u8>,
        storage: MemStorage,
        label: &str,
    ) -> Result<Vec<u8>, String> {
        let tip_height = {
            use qubitcoin_indexer_core::traits::IndexerStorageReader;
            storage.tip_height()
        };
        let mut prefixed_input = Vec::with_capacity(4 + input_data.len());
        prefixed_input.extend_from_slice(&tip_height.to_le_bytes());
        prefixed_input.extend_from_slice(&input_data);

        let state = ForkWasmState::new(prefixed_input, storage, label, true);
        let mut store = Store::new(&self.engine, state);
        store.limiter(|s| &mut s.limits);

        // Multi-threaded tokio runtime so block_in_place works inside
        // the sync host fns when they reach upstream.
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| format!("tokio runtime: {}", e))?;

        let bytes: Result<Vec<u8>, String> = rt.block_on(async {
            // Use the sync linker (sync host fns) but instantiate via
            // instantiate_async since the engine has async_support.
            let mut linker = Linker::new(&self.engine);
            link_host_functions_sync(&mut linker)?;
            linker
                .define_unknown_imports_as_traps(&self.module)
                .map_err(|e| format!("define unknown imports: {}", e))?;
            let instance = linker
                .instantiate_async(&mut store, &self.module)
                .await
                .map_err(|e| format!("wasmtime instantiate_async: {}", e))?;
            prepare_memory(&instance, &mut store);

            let view_fn = instance
                .get_typed_func::<(), i32>(&mut store, fn_name)
                .map_err(|e| format!("missing view fn '{}': {}", fn_name, e))?;

            // Drive the sync host fns from a blocking task so
            // block_in_place inside them is allowed.
            let result_ptr = tokio::task::block_in_place(|| {
                view_fn
                    .call(&mut store, ())
                    .map_err(|e| format!("view fn '{}' failed: {}", fn_name, e))
            })?;

            let memory = instance
                .get_memory(&mut store, "memory")
                .ok_or("no memory export")?;

            read_arraybuffer(&store, &memory, result_ptr)
        });
        bytes
    }

    /// Async view call with fuel-based cooperative yielding.
    pub async fn call_view_async(
        &self,
        fn_name: &str,
        input_data: Vec<u8>,
        storage: MemStorage,
        label: &str,
    ) -> Result<Vec<u8>, String> {
        let tip_height = {
            use qubitcoin_indexer_core::traits::IndexerStorageReader;
            storage.tip_height()
        };
        let mut prefixed_input = Vec::with_capacity(4 + input_data.len());
        prefixed_input.extend_from_slice(&tip_height.to_le_bytes());
        prefixed_input.extend_from_slice(&input_data);

        let state = ForkWasmState::new(prefixed_input, storage, label, true);
        let mut store = Store::new(&self.async_engine, state);
        store.limiter(|s| &mut s.limits);

        store
            .set_fuel(u64::MAX)
            .map_err(|e| format!("set fuel: {}", e))?;
        store
            .fuel_async_yield_interval(Some(10000))
            .map_err(|e| format!("fuel yield interval: {}", e))?;

        let instance = self.instantiate_async(&mut store).await?;
        prepare_memory(&instance, &mut store);

        let view_fn = instance
            .get_typed_func::<(), i32>(&mut store, fn_name)
            .map_err(|e| format!("missing view fn '{}': {}", fn_name, e))?;

        let result_ptr = view_fn
            .call_async(&mut store, ())
            .await
            .map_err(|e| format!("view fn '{}' failed: {}", fn_name, e))?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or("no memory export")?;

        read_arraybuffer(&store, &memory, result_ptr)
    }

    /// Instantiate for block processing using the async linker.
    async fn instantiate_async_block(
        &self,
        store: &mut Store<ForkWasmState>,
    ) -> Result<Instance, String> {
        let mut linker = Linker::new(&self.engine);
        link_host_functions_async(&mut linker)?;
        linker
            .define_unknown_imports_as_traps(&self.module)
            .map_err(|e| format!("define unknown imports: {}", e))?;
        linker
            .instantiate_async(&mut *store, &self.module)
            .await
            .map_err(|e| format!("wasmtime instantiate_async: {}", e))
    }

    /// Async-engine instantiate for view functions.
    async fn instantiate_async(
        &self,
        store: &mut Store<ForkWasmState>,
    ) -> Result<Instance, String> {
        let mut linker = Linker::new(&self.async_engine);
        link_host_functions_async(&mut linker)?;
        linker
            .define_unknown_imports_as_traps(&self.async_module)
            .map_err(|e| format!("define unknown imports: {}", e))?;
        linker
            .instantiate_async(&mut *store, &self.async_module)
            .await
            .map_err(|e| format!("wasmtime instantiate_async: {}", e))
    }
}

fn prepare_memory(_instance: &Instance, _store: &mut Store<ForkWasmState>) {
    // Same as slim runtime: memory grows on demand via StoreLimits.
}

// ---------------------------------------------------------------------------
// Storage read with upstream fall-through
// ---------------------------------------------------------------------------
//
// These two functions encapsulate the read-through behavior. Both flavors
// (sync linker + async linker) call them with the appropriate awaiter.

fn read_storage_local(
    write_cache: &std::collections::HashMap<Vec<u8>, Vec<u8>>,
    storage: &MemStorage,
    key: &[u8],
) -> Option<Vec<u8>> {
    if let Some(v) = write_cache.get(key) {
        return Some(v.clone());
    }
    use qubitcoin_indexer_core::traits::IndexerStorageReader;
    storage
        .get_latest(key)
        .or_else(|| storage.get(key))
}

/// Sync upstream call from inside a tokio runtime. Uses `block_in_place`
/// to bridge the sync wasmtime caller to the async upstream future. Must
/// be invoked from a multi-threaded runtime worker; the runtime created
/// in `call_view` and `run_block` satisfies this.
fn upstream_fetch_sync(
    upstream: &Arc<dyn ForkUpstream>,
    key: &[u8],
) -> Result<Option<Vec<u8>>, String> {
    let key = key.to_vec();
    let upstream = upstream.clone();
    tokio::task::block_in_place(move || {
        Handle::current().block_on(async move { upstream.fetch(&key).await })
    })
}

// ---------------------------------------------------------------------------
// Sync host function linker (used for view calls; same engine as block)
// ---------------------------------------------------------------------------

fn link_host_functions_sync(linker: &mut Linker<ForkWasmState>) -> Result<(), String> {
    linker
        .func_wrap(
            "env",
            "__host_len",
            |caller: Caller<'_, ForkWasmState>| -> i32 { caller.data().input_data.len() as i32 },
        )
        .map_err(|e| format!("link __host_len: {}", e))?;

    linker
        .func_wrap(
            "env",
            "__load_input",
            |mut caller: Caller<'_, ForkWasmState>, ptr: i32| {
                let data = caller.data().input_data.clone();
                let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
                memory.write(&mut caller, ptr as usize, &data).ok();
            },
        )
        .map_err(|e| format!("link __load_input: {}", e))?;

    linker
        .func_wrap(
            "env",
            "__get_len",
            |mut caller: Caller<'_, ForkWasmState>, key_ptr: i32| -> i32 {
                let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
                let key = match read_arraybuffer(&caller, &memory, key_ptr) {
                    Ok(k) => k,
                    Err(_) => return 0,
                };
                let local = read_storage_local(
                    &caller.data().write_cache,
                    &caller.data().storage,
                    &key,
                );
                if let Some(v) = local {
                    return v.len() as i32;
                }
                if caller.data().storage.is_negative_cached(&key) {
                    return 0;
                }
                let upstream = caller.data().storage.upstream();
                if let Some(up) = upstream {
                    match upstream_fetch_sync(&up, &key) {
                        Ok(Some(v)) => {
                            let len = v.len() as i32;
                            caller.data_mut().storage.cache_upstream_hit(&key, &v);
                            len
                        }
                        Ok(None) => {
                            caller.data_mut().storage.cache_upstream_miss(&key);
                            0
                        }
                        Err(e) => {
                            tracing::warn!(
                                indexer = %caller.data().label,
                                error = %e,
                                "upstream __get_len failed"
                            );
                            0
                        }
                    }
                } else {
                    0
                }
            },
        )
        .map_err(|e| format!("link __get_len: {}", e))?;

    linker
        .func_wrap(
            "env",
            "__get",
            |mut caller: Caller<'_, ForkWasmState>, key_ptr: i32, value_ptr: i32| {
                let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
                let key = match read_arraybuffer(&caller, &memory, key_ptr) {
                    Ok(k) => k,
                    Err(_) => return,
                };
                let local = read_storage_local(
                    &caller.data().write_cache,
                    &caller.data().storage,
                    &key,
                );
                let value = match local {
                    Some(v) => Some(v),
                    None => {
                        if caller.data().storage.is_negative_cached(&key) {
                            None
                        } else if let Some(up) = caller.data().storage.upstream() {
                            match upstream_fetch_sync(&up, &key) {
                                Ok(Some(v)) => {
                                    caller.data_mut().storage.cache_upstream_hit(&key, &v);
                                    Some(v)
                                }
                                Ok(None) => {
                                    caller.data_mut().storage.cache_upstream_miss(&key);
                                    None
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        indexer = %caller.data().label,
                                        error = %e,
                                        "upstream __get failed"
                                    );
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    }
                };
                if let Some(value) = value {
                    memory.write(&mut caller, value_ptr as usize, &value).ok();
                }
            },
        )
        .map_err(|e| format!("link __get: {}", e))?;

    linker
        .func_wrap(
            "env",
            "__flush",
            |mut caller: Caller<'_, ForkWasmState>, data_ptr: i32| {
                flush_handler(&mut caller, data_ptr);
            },
        )
        .map_err(|e| format!("link __flush: {}", e))?;

    linker
        .func_wrap(
            "env",
            "__log",
            |mut caller: Caller<'_, ForkWasmState>, ptr: i32| {
                log_handler(&mut caller, ptr);
            },
        )
        .map_err(|e| format!("link __log: {}", e))?;

    linker
        .func_wrap(
            "env",
            "abort",
            |mut caller: Caller<'_, ForkWasmState>,
             msg_ptr: i32,
             _file: i32,
             line: i32,
             col: i32| {
                abort_handler(&mut caller, msg_ptr, line, col);
            },
        )
        .map_err(|e| format!("link abort: {}", e))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Async host function linker (used for block processing + view fns)
// ---------------------------------------------------------------------------

fn link_host_functions_async(linker: &mut Linker<ForkWasmState>) -> Result<(), String> {
    linker
        .func_wrap_async(
            "env",
            "__host_len",
            |caller: Caller<'_, ForkWasmState>, ()| {
                Box::new(async move { caller.data().input_data.len() as i32 })
            },
        )
        .map_err(|e| format!("link async __host_len: {}", e))?;

    linker
        .func_wrap_async(
            "env",
            "__load_input",
            |mut caller: Caller<'_, ForkWasmState>, (ptr,): (i32,)| {
                Box::new(async move {
                    let data = caller.data().input_data.clone();
                    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
                    memory
                        .write(&mut caller, ptr as usize, &data)
                        .expect("FATAL: __load_input memory write failed");
                })
            },
        )
        .map_err(|e| format!("link async __load_input: {}", e))?;

    linker
        .func_wrap_async(
            "env",
            "__get_len",
            |mut caller: Caller<'_, ForkWasmState>, (key_ptr,): (i32,)| {
                Box::new(async move {
                    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
                    let key = match read_arraybuffer(&caller, &memory, key_ptr) {
                        Ok(k) => k,
                        Err(_) => return 0i32,
                    };
                    let local = read_storage_local(
                        &caller.data().write_cache,
                        &caller.data().storage,
                        &key,
                    );
                    if let Some(v) = local {
                        return v.len() as i32;
                    }
                    if caller.data().storage.is_negative_cached(&key) {
                        return 0;
                    }
                    let upstream = caller.data().storage.upstream();
                    if let Some(up) = upstream {
                        match up.fetch(&key).await {
                            Ok(Some(v)) => {
                                let len = v.len() as i32;
                                caller.data_mut().storage.cache_upstream_hit(&key, &v);
                                len
                            }
                            Ok(None) => {
                                caller.data_mut().storage.cache_upstream_miss(&key);
                                0
                            }
                            Err(e) => {
                                tracing::warn!(
                                    indexer = %caller.data().label,
                                    error = %e,
                                    "upstream __get_len failed (async)"
                                );
                                0
                            }
                        }
                    } else {
                        0
                    }
                })
            },
        )
        .map_err(|e| format!("link async __get_len: {}", e))?;

    linker
        .func_wrap_async(
            "env",
            "__get",
            |mut caller: Caller<'_, ForkWasmState>, (key_ptr, value_ptr): (i32, i32)| {
                Box::new(async move {
                    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
                    let key = match read_arraybuffer(&caller, &memory, key_ptr) {
                        Ok(k) => k,
                        Err(_) => return,
                    };
                    let local = read_storage_local(
                        &caller.data().write_cache,
                        &caller.data().storage,
                        &key,
                    );
                    let value = match local {
                        Some(v) => Some(v),
                        None => {
                            if caller.data().storage.is_negative_cached(&key) {
                                None
                            } else if let Some(up) = caller.data().storage.upstream() {
                                match up.fetch(&key).await {
                                    Ok(Some(v)) => {
                                        caller.data_mut().storage.cache_upstream_hit(&key, &v);
                                        Some(v)
                                    }
                                    Ok(None) => {
                                        caller.data_mut().storage.cache_upstream_miss(&key);
                                        None
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            indexer = %caller.data().label,
                                            error = %e,
                                            "upstream __get failed (async)"
                                        );
                                        None
                                    }
                                }
                            } else {
                                None
                            }
                        }
                    };
                    if let Some(value) = value {
                        memory
                            .write(&mut caller, value_ptr as usize, &value)
                            .expect("FATAL: __get memory write failed");
                    }
                })
            },
        )
        .map_err(|e| format!("link async __get: {}", e))?;

    linker
        .func_wrap_async(
            "env",
            "__flush",
            |mut caller: Caller<'_, ForkWasmState>, (data_ptr,): (i32,)| {
                Box::new(async move {
                    flush_handler(&mut caller, data_ptr);
                })
            },
        )
        .map_err(|e| format!("link async __flush: {}", e))?;

    linker
        .func_wrap_async(
            "env",
            "__log",
            |mut caller: Caller<'_, ForkWasmState>, (ptr,): (i32,)| {
                Box::new(async move {
                    log_handler(&mut caller, ptr);
                })
            },
        )
        .map_err(|e| format!("link async __log: {}", e))?;

    linker
        .func_wrap_async(
            "env",
            "abort",
            |mut caller: Caller<'_, ForkWasmState>,
             (msg_ptr, _file, line, col): (i32, i32, i32, i32)| {
                Box::new(async move {
                    abort_handler(&mut caller, msg_ptr, line, col);
                })
            },
        )
        .map_err(|e| format!("link async abort: {}", e))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Shared host function bodies (sync — flush/log/abort)
// ---------------------------------------------------------------------------

fn flush_handler(caller: &mut Caller<'_, ForkWasmState>, data_ptr: i32) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let data = match read_arraybuffer(&*caller, &memory, data_ptr) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(error = %e, "failed to read __flush data");
            caller.data_mut().had_failure = true;
            return;
        }
    };

    let flush_msg = match proto::KeyValueFlush::decode(data.as_slice()) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(error = %e, "failed to decode KeyValueFlush");
            caller.data_mut().had_failure = true;
            return;
        }
    };

    let mut pairs = Vec::new();
    let list = &flush_msg.list;
    let mut i = 0;
    while i + 1 < list.len() {
        pairs.push((list[i].to_vec(), list[i + 1].to_vec()));
        i += 2;
    }

    for (k, v) in &pairs {
        caller.data_mut().write_cache.insert(k.clone(), v.clone());
    }

    match caller.data_mut().pending_flush.as_mut() {
        Some(existing) => existing.extend(pairs),
        None => caller.data_mut().pending_flush = Some(pairs),
    }
    caller.data_mut().completed = true;
}

fn log_handler(caller: &mut Caller<'_, ForkWasmState>, ptr: i32) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    if let Ok(msg_bytes) = read_arraybuffer(&*caller, &memory, ptr) {
        let msg = String::from_utf8_lossy(&msg_bytes);
        let label = &caller.data().label;
        tracing::info!(indexer = %label, "{}", msg);
    }
}

fn abort_handler(caller: &mut Caller<'_, ForkWasmState>, msg_ptr: i32, line: i32, col: i32) {
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let msg = read_arraybuffer(&*caller, &memory, msg_ptr)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .unwrap_or_else(|| "<unreadable>".into());
    let label = caller.data().label.clone();
    tracing::error!(indexer = %label, line = line, col = col, "WASM abort: {}", msg);
    caller.data_mut().had_failure = true;
}

// ---------------------------------------------------------------------------
// ArrayBuffer helper (matches slim runtime)
// ---------------------------------------------------------------------------

fn read_arraybuffer(
    store: impl AsContext,
    memory: &Memory,
    ptr: i32,
) -> Result<Vec<u8>, String> {
    if ptr < 4 {
        return Err("invalid arraybuffer pointer".into());
    }
    let mem_data = memory.data(store.as_context());
    let len_offset = (ptr - 4) as usize;
    if len_offset + 4 > mem_data.len() {
        return Err("arraybuffer length out of bounds".into());
    }
    let len = u32::from_le_bytes([
        mem_data[len_offset],
        mem_data[len_offset + 1],
        mem_data[len_offset + 2],
        mem_data[len_offset + 3],
    ]) as usize;
    let data_offset = ptr as usize;
    if data_offset + len > mem_data.len() {
        return Err("arraybuffer data out of bounds".into());
    }
    Ok(mem_data[data_offset..data_offset + len].to_vec())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upstream::testing::StubUpstream;
    use std::collections::HashMap as Map;
    use std::sync::Arc;

    fn build_minimal_wasm() -> Vec<u8> {
        wat::parse_str(
            r#"
            (module
                (import "env" "__host_len" (func $host_len (result i32)))
                (import "env" "__load_input" (func $load_input (param i32)))
                (import "env" "__flush" (func $flush (param i32)))
                (import "env" "__log" (func $log (param i32)))
                (import "env" "__get" (func $get (param i32 i32)))
                (import "env" "__get_len" (func $get_len (param i32) (result i32)))
                (import "env" "abort" (func $abort (param i32 i32 i32 i32)))
                (memory (export "memory") 1)

                (func (export "_start")
                    (drop (call $host_len))
                    (i32.store (i32.const 96) (i32.const 0))
                    (call $flush (i32.const 100))
                )
            )
            "#,
        )
        .expect("failed to parse WAT")
    }

    /// View module that probes `__get_len` against a key embedded in
    /// memory at offset 8 ("hello", 5 bytes), with an ArrayBuffer
    /// length header at offset 4.
    ///
    ///   bytes [4..8]  = length 5 (LE)
    ///   bytes [8..13] = "hello"
    /// view fn returns an ArrayBuffer at offset 200 holding the
    /// 4-byte LE length of __get_len("hello").
    fn build_probe_get_len_wasm() -> Vec<u8> {
        wat::parse_str(
            r#"
            (module
                (import "env" "__host_len" (func $host_len (result i32)))
                (import "env" "__load_input" (func $load_input (param i32)))
                (import "env" "__flush" (func $flush (param i32)))
                (import "env" "__log" (func $log (param i32)))
                (import "env" "__get" (func $get (param i32 i32)))
                (import "env" "__get_len" (func $get_len (param i32) (result i32)))
                (import "env" "abort" (func $abort (param i32 i32 i32 i32)))
                (memory (export "memory") 1)
                (data (i32.const 4) "\05\00\00\00hello")

                (func (export "probe") (result i32)
                    ;; Result ArrayBuffer at offset 200 (length header at 196).
                    (i32.store (i32.const 196) (i32.const 4))
                    (i32.store (i32.const 200) (call $get_len (i32.const 8)))
                    (i32.const 200)
                )
            )
            "#,
        )
        .expect("failed to parse WAT for probe module")
    }

    /// View module that uses `__get` to copy the value of a key into
    /// memory at offset 300 (with an ArrayBuffer length header at 296),
    /// then returns the buffer pointer. Reuses the `hello` key at [8..13].
    fn build_probe_get_wasm() -> Vec<u8> {
        wat::parse_str(
            r#"
            (module
                (import "env" "__host_len" (func $host_len (result i32)))
                (import "env" "__load_input" (func $load_input (param i32)))
                (import "env" "__flush" (func $flush (param i32)))
                (import "env" "__log" (func $log (param i32)))
                (import "env" "__get" (func $get (param i32 i32)))
                (import "env" "__get_len" (func $get_len (param i32) (result i32)))
                (import "env" "abort" (func $abort (param i32 i32 i32 i32)))
                (memory (export "memory") 1)
                (data (i32.const 4) "\05\00\00\00hello")

                (func (export "probe_get") (result i32)
                    (local $len i32)
                    ;; len = __get_len("hello")
                    (local.set $len (call $get_len (i32.const 8)))
                    ;; Write the length header at offset 296.
                    (i32.store (i32.const 296) (local.get $len))
                    ;; Copy value bytes into memory starting at 300.
                    (call $get (i32.const 8) (i32.const 300))
                    (i32.const 300)
                )
            )
            "#,
        )
        .expect("failed to parse WAT for probe_get module")
    }

    fn upstream_with(pairs: &[(&[u8], &[u8])]) -> Arc<StubUpstream> {
        let map: Map<Vec<u8>, Vec<u8>> = pairs
            .iter()
            .map(|(k, v)| (k.to_vec(), v.to_vec()))
            .collect();
        Arc::new(StubUpstream::new(map))
    }

    #[test]
    fn test_compile_wasm() {
        let wasm = build_minimal_wasm();
        assert!(ForkRuntime::new(&wasm).is_ok());
    }

    #[test]
    fn test_compile_invalid_wasm() {
        assert!(ForkRuntime::new(b"not wasm").is_err());
    }

    #[test]
    fn test_run_block_minimal() {
        let wasm = build_minimal_wasm();
        let runtime = ForkRuntime::new(&wasm).unwrap();
        let storage = MemStorage::new(upstream_with(&[]), 100);

        let mut input = Vec::new();
        input.extend_from_slice(&100u32.to_le_bytes());
        input.extend_from_slice(b"fake_block_data");

        let result = runtime.run_block(input, storage, "test");
        assert!(result.is_ok(), "run_block failed: {:?}", result.err());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_call_view_get_len_falls_through_to_upstream() {
        let wasm = build_probe_get_len_wasm();
        let runtime = ForkRuntime::new(&wasm).unwrap();
        let stub = upstream_with(&[(b"hello", b"world!" /* 6 bytes */)]);
        let counter = stub.fetch_counter();
        let storage = MemStorage::new(stub, 1);

        let result = runtime
            .call_view("probe", vec![], storage.clone(), "fork-test")
            .unwrap();
        assert_eq!(result.len(), 4);
        let len = u32::from_le_bytes([result[0], result[1], result[2], result[3]]);
        assert_eq!(len, 6);
        assert_eq!(
            counter.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "upstream should have been called exactly once"
        );
        // Write-back cache: the returned value should now live in the overlay.
        assert_eq!(storage.local_get(b"hello"), Some(b"world!".to_vec()));
    }

    #[test]
    fn test_call_view_get_len_zero_for_missing_upstream_key() {
        let wasm = build_probe_get_len_wasm();
        let runtime = ForkRuntime::new(&wasm).unwrap();
        let storage = MemStorage::new(upstream_with(&[]), 1);

        let result = runtime
            .call_view("probe", vec![], storage.clone(), "fork-test")
            .unwrap();
        let len = u32::from_le_bytes([result[0], result[1], result[2], result[3]]);
        assert_eq!(len, 0);
        assert!(storage.is_negative_cached(b"hello"));
    }

    #[test]
    fn test_call_view_get_returns_upstream_bytes() {
        let wasm = build_probe_get_wasm();
        let runtime = ForkRuntime::new(&wasm).unwrap();
        let stub = upstream_with(&[(b"hello", b"abcde")]);
        let storage = MemStorage::new(stub, 1);

        let result = runtime
            .call_view("probe_get", vec![], storage, "fork-test")
            .unwrap();
        assert_eq!(result, b"abcde");
    }

    #[test]
    fn test_overlay_shadows_upstream() {
        let wasm = build_probe_get_wasm();
        let runtime = ForkRuntime::new(&wasm).unwrap();
        let stub = upstream_with(&[(b"hello", b"upstream")]);
        let counter = stub.fetch_counter();
        let storage = MemStorage::new(stub, 1);
        // Pre-seed the overlay with a different value.
        storage.put_helper(b"hello", b"shadow");

        let result = runtime
            .call_view("probe_get", vec![], storage, "fork-test")
            .unwrap();
        assert_eq!(result, b"shadow");
        // Upstream must NOT have been hit (overlay short-circuits).
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn test_repeated_reads_only_hit_upstream_once() {
        let wasm = build_probe_get_len_wasm();
        let runtime = ForkRuntime::new(&wasm).unwrap();
        let stub = upstream_with(&[(b"hello", b"x")]);
        let counter = stub.fetch_counter();
        let storage = MemStorage::new(stub, 1);

        // First call: upstream fetched.
        let _ = runtime
            .call_view("probe", vec![], storage.clone(), "test")
            .unwrap();
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);

        // Second call on the same storage: should now hit overlay,
        // upstream count stays at 1.
        let _ = runtime
            .call_view("probe", vec![], storage, "test")
            .unwrap();
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn test_call_view_missing_fn() {
        let wasm = build_minimal_wasm();
        let runtime = ForkRuntime::new(&wasm).unwrap();
        let storage = MemStorage::new(upstream_with(&[]), 1);

        let result = runtime.call_view("nonexistent", vec![], storage, "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing view fn"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_call_view_async_falls_through_to_upstream() {
        let wasm = build_probe_get_wasm();
        let runtime = ForkRuntime::new(&wasm).unwrap();
        let stub = upstream_with(&[(b"hello", b"async")]);
        let storage = MemStorage::new(stub, 1);

        let result = runtime
            .call_view_async("probe_get", vec![], storage, "fork-test")
            .await
            .unwrap();
        assert_eq!(result, b"async");
    }

    #[test]
    fn test_no_upstream_returns_zero_len() {
        let wasm = build_probe_get_len_wasm();
        let runtime = ForkRuntime::new(&wasm).unwrap();
        // Storage with no upstream: misses just return 0 / nothing.
        let storage = MemStorage::new_local(1);

        let result = runtime
            .call_view("probe", vec![], storage, "fork-test")
            .unwrap();
        let len = u32::from_le_bytes([result[0], result[1], result[2], result[3]]);
        assert_eq!(len, 0);
    }
}

// Helper trait for tests: lets us call `storage.put_helper(...)` from
// inside the runtime tests without leaking the trait imports.
#[cfg(test)]
trait TestPut {
    fn put_helper(&self, key: &[u8], value: &[u8]);
}
#[cfg(test)]
impl TestPut for MemStorage {
    fn put_helper(&self, key: &[u8], value: &[u8]) {
        use qubitcoin_indexer_core::traits::IndexerStorageWriter;
        self.put(key, value).unwrap();
    }
}
