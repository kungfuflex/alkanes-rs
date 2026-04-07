//! Multi-indexer test runtime using direct wasmtime (fresh instance per block).
//!
//! This follows qubitcoin-indexer's pattern: compile once, create fresh
//! Store + Instance for each block. This avoids MetashrewRuntime's
//! force_initial_memory_commit which breaks HashMap initialization.

use anyhow::{anyhow, Context, Result};
use bitcoin::Block;
use metashrew_support::utils::consensus_encode;
use protorune_support::network::{set_network, NetworkParams};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wasmtime::*;

use crate::fixtures;

// ---------------------------------------------------------------------------
// Minimal metashrew-compatible host state
// ---------------------------------------------------------------------------

struct WasmState {
    input_data: Vec<u8>,
    storage: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    pending_flush: Vec<(Vec<u8>, Vec<u8>)>,
    write_cache: HashMap<Vec<u8>, Vec<u8>>,
    completed: bool,
    had_failure: bool,
    limits: StoreLimits,
}

impl WasmState {
    fn new(input_data: Vec<u8>, storage: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>) -> Self {
        Self {
            input_data,
            storage,
            pending_flush: Vec::new(),
            write_cache: HashMap::new(),
            completed: false,
            had_failure: false,
            limits: StoreLimitsBuilder::new()
                .memories(usize::MAX)
                .tables(usize::MAX)
                .instances(usize::MAX)
                .build(),
        }
    }
}

// ---------------------------------------------------------------------------
// ArrayBuffer helpers (metashrew ABI: [len_u32_le @ ptr-4][data @ ptr])
// ---------------------------------------------------------------------------

fn read_arraybuffer(data: &[u8], ptr: i32) -> Vec<u8> {
    let p = ptr as usize;
    if p < 4 || p > data.len() {
        return vec![];
    }
    let len = u32::from_le_bytes([data[p - 4], data[p - 3], data[p - 2], data[p - 1]]) as usize;
    if p + len > data.len() {
        return vec![];
    }
    data[p..p + len].to_vec()
}

fn read_arraybuffer_from_store(store: &Store<WasmState>, memory: &Memory, ptr: i32) -> Vec<u8> {
    read_arraybuffer(memory.data(store), ptr)
}

// ---------------------------------------------------------------------------
// Host function linking (matching metashrew/qubitcoin ABI)
// ---------------------------------------------------------------------------

fn link_host_functions(linker: &mut Linker<WasmState>) -> Result<()> {
    // __host_len() -> i32
    linker.func_wrap("env", "__host_len", |caller: Caller<'_, WasmState>| -> i32 {
        caller.data().input_data.len() as i32
    })?;

    // __load_input(ptr: i32)
    linker.func_wrap(
        "env",
        "__load_input",
        |mut caller: Caller<'_, WasmState>, ptr: i32| {
            let input = caller.data().input_data.clone();
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            mem.write(&mut caller, ptr as usize, &input).ok();
        },
    )?;

    // __get_len(key_ptr: i32) -> i32
    linker.func_wrap(
        "env",
        "__get_len",
        |mut caller: Caller<'_, WasmState>, key_ptr: i32| -> i32 {
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let key = read_arraybuffer(mem.data(&caller), key_ptr);
            // Check write_cache first, then persistent storage
            if let Some(v) = caller.data().write_cache.get(&key) {
                return v.len() as i32;
            }
            let storage = caller.data().storage.lock().unwrap();
            match storage.get(&key) {
                Some(v) => v.len() as i32,
                None => 0,
            }
        },
    )?;

    // __get(key_ptr: i32, value_ptr: i32)
    linker.func_wrap(
        "env",
        "__get",
        |mut caller: Caller<'_, WasmState>, key_ptr: i32, value_ptr: i32| {
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let key = read_arraybuffer(mem.data(&caller), key_ptr);
            let value = caller
                .data()
                .write_cache
                .get(&key)
                .cloned()
                .or_else(|| caller.data().storage.lock().unwrap().get(&key).cloned());
            if let Some(v) = value {
                mem.write(&mut caller, value_ptr as usize, &v).ok();
            }
        },
    )?;

    // __flush(data_ptr: i32)
    linker.func_wrap(
        "env",
        "__flush",
        |mut caller: Caller<'_, WasmState>, data_ptr: i32| {
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let data = read_arraybuffer(mem.data(&caller), data_ptr);

            // Decode protobuf KeyValueFlush: repeated bytes list
            // Format: field 1 (wire type 2 = length-delimited), repeated
            let mut pairs = Vec::new();
            let mut pos = 0;
            let mut items = Vec::new();
            while pos < data.len() {
                // Read protobuf tag
                let tag = data[pos];
                pos += 1;
                if tag == 0x0a {
                    // field 1, wire type 2 (length-delimited)
                    // Read varint length
                    let mut len: usize = 0;
                    let mut shift = 0;
                    loop {
                        if pos >= data.len() {
                            break;
                        }
                        let b = data[pos] as usize;
                        pos += 1;
                        len |= (b & 0x7f) << shift;
                        if b & 0x80 == 0 {
                            break;
                        }
                        shift += 7;
                    }
                    if pos + len <= data.len() {
                        items.push(data[pos..pos + len].to_vec());
                    }
                    pos += len;
                } else {
                    break;
                }
            }

            // Items are alternating [key, value, key, value, ...]
            let mut i = 0;
            while i + 1 < items.len() {
                let key = items[i].clone();
                let value = items[i + 1].clone();
                caller
                    .data_mut()
                    .write_cache
                    .insert(key.clone(), value.clone());
                pairs.push((key, value));
                i += 2;
            }

            caller.data_mut().pending_flush.extend(pairs);
            caller.data_mut().completed = true;
        },
    )?;

    // __log(ptr: i32)
    linker.func_wrap(
        "env",
        "__log",
        |mut caller: Caller<'_, WasmState>, ptr: i32| {
            let mem = caller.get_export("memory").unwrap().into_memory().unwrap();
            let bytes = read_arraybuffer(mem.data(&caller), ptr);
            if let Ok(text) = std::str::from_utf8(&bytes) {
                print!("{}", text);
            }
        },
    )?;

    // abort(msg: i32, file: i32, line: i32, col: i32)
    linker.func_wrap(
        "env",
        "abort",
        |mut caller: Caller<'_, WasmState>, _: i32, _: i32, _: i32, _: i32| {
            caller.data_mut().had_failure = true;
        },
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Single indexer instance
// ---------------------------------------------------------------------------

pub struct IndexerInstance {
    engine: Engine,
    module: Module,
    storage: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
    label: String,
}

impl IndexerInstance {
    fn create(wasm: &[u8], label: &str) -> Result<Self> {
        let mut config = Config::new();
        config.wasm_bulk_memory(true);
        config.wasm_multi_value(true);
        config.wasm_reference_types(true);
        config.wasm_simd(true);
        config.cranelift_nan_canonicalization(true);
        config.relaxed_simd_deterministic(true);
        config.memory_reservation(0x100000000);
        config.memory_guard_size(0x10000);
        config.memory_init_cow(true);
        config.async_support(true);

        let engine = Engine::new(&config)
            .with_context(|| format!("Failed to create engine for {}", label))?;
        let module = Module::new(&engine, wasm)
            .with_context(|| format!("Failed to compile WASM for {}", label))?;

        Ok(Self {
            engine,
            module,
            storage: Arc::new(Mutex::new(HashMap::new())),
            label: label.to_string(),
        })
    }

    /// Process a block: create fresh Store + Instance, call _start.
    pub fn run_block(&self, block_bytes: &[u8], height: u32) -> Result<()> {
        let mut input = Vec::with_capacity(4 + block_bytes.len());
        input.extend_from_slice(&height.to_le_bytes());
        input.extend_from_slice(block_bytes);

        let state = WasmState::new(input, Arc::clone(&self.storage));
        let mut store = Store::new(&self.engine, state);
        store.limiter(|s| &mut s.limits);

        let mut linker = Linker::new(&self.engine);
        link_host_functions(&mut linker)?;
        linker.define_unknown_imports_as_traps(&self.module)?;

        // Fresh instance per block (qubitcoin pattern).
        // Create a one-shot tokio runtime for async instantiation + execution.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let instance = rt.block_on(async {
            linker.instantiate_async(&mut store, &self.module).await
        })?;

        let start_fn = instance.get_typed_func::<(), ()>(&mut store, "_start")?;

        rt.block_on(async { start_fn.call_async(&mut store, ()).await })
            .with_context(|| {
                format!("{}: _start failed at height {}", self.label, height)
            })?;

        let state = store.into_data();
        if state.had_failure {
            return Err(anyhow!("{}: WASM module aborted at height {}", self.label, height));
        }

        // Commit flushed pairs to persistent storage
        {
            let mut storage = self.storage.lock().unwrap();
            let num_pairs = state.pending_flush.len();
            let mut alkanes_count = 0;
            for (k, v) in &state.pending_flush {
                let key_str = String::from_utf8_lossy(&k[..std::cmp::min(k.len(), 40)]);
                if key_str.contains("/alkanes/") {
                    alkanes_count += 1;
                    eprintln!("[{}:h{}] /alkanes/ key: {} bytes key, {} bytes val, key_hex={}",
                        self.label, height, k.len(), v.len(),
                        hex::encode(&k[..std::cmp::min(k.len(), 60)]));
                }
            }
            eprintln!("[{}:h{}] {} total pairs, {} /alkanes/ pairs",
                self.label, height, num_pairs, alkanes_count);
            for (k, v) in state.pending_flush {
                storage.insert(k, v);
            }
        }

        Ok(())
    }

    /// Call a view function.
    pub fn call_view(&self, fn_name: &str, input: &[u8], height: u32) -> Result<Vec<u8>> {
        let mut prefixed = Vec::with_capacity(4 + input.len());
        prefixed.extend_from_slice(&height.to_le_bytes());
        prefixed.extend_from_slice(input);

        let state = WasmState::new(prefixed, Arc::clone(&self.storage));
        let mut store = Store::new(&self.engine, state);
        store.limiter(|s| &mut s.limits);

        let mut linker = Linker::new(&self.engine);
        link_host_functions(&mut linker)?;
        linker.define_unknown_imports_as_traps(&self.module)?;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let instance = rt.block_on(async {
            linker.instantiate_async(&mut store, &self.module).await
        })?;

        let view_fn = instance
            .get_typed_func::<(), i32>(&mut store, fn_name)
            .with_context(|| format!("view function '{}' not found", fn_name))?;

        let result_ptr = rt.block_on(async { view_fn.call_async(&mut store, ()).await })?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow!("no memory export"))?;

        Ok(read_arraybuffer_from_store(&store, &memory, result_ptr))
    }
}

// ---------------------------------------------------------------------------
// Multi-indexer test harness
// ---------------------------------------------------------------------------

pub struct TestRuntime {
    pub alkanes: IndexerInstance,
    pub esplora: IndexerInstance,
    pub ord: Option<IndexerInstance>,
    height: std::sync::atomic::AtomicU32,
}

impl TestRuntime {
    /// Create harness with alkanes + esplora (ord is optional due to compatibility).
    pub fn new() -> Result<Self> {
        Self::configure_network();
        let ord = match IndexerInstance::create(fixtures::ORD_WASM, "ord") {
            Ok(inst) => Some(inst),
            Err(e) => {
                log::warn!("ord indexer not available: {}", e);
                None
            }
        };
        Ok(Self {
            alkanes: IndexerInstance::create(fixtures::ALKANES_WASM, "alkanes")?,
            esplora: IndexerInstance::create(fixtures::ESPLORA_WASM, "esplora")?,
            ord,
            height: std::sync::atomic::AtomicU32::new(0),
        })
    }

    fn configure_network() {
        set_network(NetworkParams {
            bech32_prefix: String::from("bcrt"),
            p2pkh_prefix: 0x64,
            p2sh_prefix: 0xc4,
        });
    }

    /// Index a block through all indexers.
    pub fn index_block(&self, block: &Block, height: u32) -> Result<()> {
        let block_bytes = consensus_encode(block)
            .with_context(|| format!("Failed to serialize block at height {}", height))?;

        self.alkanes.run_block(&block_bytes, height)?;
        self.esplora.run_block(&block_bytes, height)?;
        if let Some(ord) = &self.ord {
            if let Err(e) = ord.run_block(&block_bytes, height) {
                log::warn!("ord indexer failed at height {}: {}", height, e);
            }
        }

        self.height
            .store(height, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// Mine N empty blocks.
    pub fn mine_empty_blocks(&self, start_height: u32, count: u32) -> Result<()> {
        for h in start_height..(start_height + count) {
            let block = protorune::test_helpers::create_block_with_coinbase_tx(h);
            self.index_block(&block, h)?;
        }
        Ok(())
    }

    pub fn height(&self) -> u32 {
        self.height.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn alkanes_view(&self, symbol: &str, input: &[u8], height: u32) -> Result<Vec<u8>> {
        self.alkanes.call_view(symbol, input, height)
    }

    pub fn esplora_view(&self, symbol: &str, input: &[u8], height: u32) -> Result<Vec<u8>> {
        self.esplora.call_view(symbol, input, height)
    }

    pub fn ord_view(&self, symbol: &str, input: &[u8], height: u32) -> Result<Vec<u8>> {
        match &self.ord {
            Some(ord) => ord.call_view(symbol, input, height),
            None => Err(anyhow!("ord indexer not available")),
        }
    }
}
