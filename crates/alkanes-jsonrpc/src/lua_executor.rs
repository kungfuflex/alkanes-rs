use alkanes_rpc_core::types::{JsonRpcRequest, JsonRpcResponse};
use crate::handler::ProdDispatcher;
use crate::proxy::ProxyClient;
use anyhow::{anyhow, Result};
use mlua::prelude::*;
use moka::sync::Cache;
use serde_json::Value;
use std::path::PathBuf;
use alkanes_rpc_core::dispatch::CallBudget;
use std::collections::HashSet;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

/// Default max size of the LRU cache in bytes (128 MB)
const DEFAULT_CACHE_MAX_SIZE: u64 = 128 * 1024 * 1024;

/// Script storage for saved Lua scripts with LRU cache and optional disk persistence.
/// Scripts are loaded lazily from disk on demand, not eagerly on startup.
#[derive(Clone)]
pub struct ScriptStorage {
    cache: Cache<String, String>,
    disk_path: Option<PathBuf>,
}

impl ScriptStorage {
    fn build_cache() -> Cache<String, String> {
        Cache::builder()
            .weigher(|key: &String, value: &String| -> u32 {
                // Weight is the size in bytes of key + value
                (key.len() + value.len()).try_into().unwrap_or(u32::MAX)
            })
            .max_capacity(DEFAULT_CACHE_MAX_SIZE)
            .build()
    }

    pub fn new() -> Self {
        Self {
            cache: Self::build_cache(),
            disk_path: None,
        }
    }

    pub fn with_disk_path(path: PathBuf) -> Self {
        // Ensure directory exists, but don't load scripts eagerly
        if !path.exists() {
            if let Err(e) = std::fs::create_dir_all(&path) {
                log::warn!("Failed to create Lua script directory {:?}: {}", path, e);
            } else {
                log::info!("Created Lua script directory at {:?}", path);
            }
        } else {
            log::info!("Lua script directory configured at {:?} (lazy loading enabled)", path);
        }

        Self {
            cache: Self::build_cache(),
            disk_path: Some(path),
        }
    }

    /// Compute hash of a script
    fn compute_hash(script: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(script.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Save script to disk if disk_path is configured
    fn save_to_disk(&self, hash: &str, script: &str) {
        if let Some(ref path) = self.disk_path {
            let file_path = path.join(format!("{}.lua", hash));
            if !file_path.exists() {
                if let Err(e) = std::fs::write(&file_path, script) {
                    log::warn!("Failed to persist Lua script to disk: {}", e);
                } else {
                    log::debug!("Persisted Lua script to disk: {}", hash);
                }
            }
        }
    }

    /// Load script from disk if available
    fn load_from_disk(&self, hash: &str) -> Option<String> {
        if let Some(ref path) = self.disk_path {
            let file_path = path.join(format!("{}.lua", hash));
            if file_path.exists() {
                match std::fs::read_to_string(&file_path) {
                    Ok(content) => {
                        log::debug!("Loaded Lua script from disk: {}", hash);
                        return Some(content);
                    }
                    Err(e) => {
                        log::warn!("Failed to read Lua script from disk: {}", e);
                    }
                }
            }
        }
        None
    }

    pub async fn save(&self, script: String) -> String {
        let hash = Self::compute_hash(&script);

        // Insert into cache if not present
        if self.cache.get(&hash).is_none() {
            self.cache.insert(hash.clone(), script.clone());
            // Persist to disk
            self.save_to_disk(&hash, &script);
        }
        hash
    }

    pub async fn get(&self, hash: &str) -> Option<String> {
        // Check LRU cache first
        if let Some(script) = self.cache.get(hash) {
            return Some(script);
        }

        // Fallback to disk (lazy loading)
        if let Some(script) = self.load_from_disk(hash) {
            // Insert into LRU cache for future access
            self.cache.insert(hash.to_string(), script.clone());
            return Some(script);
        }

        None
    }
}

impl Default for ScriptStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of Lua script execution
#[derive(Debug, serde::Serialize)]
pub struct LuaExecutionResult {
    pub calls: usize,
    pub returns: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<LuaError>,
    pub runtime: u64, // milliseconds
}

#[derive(Debug, serde::Serialize)]
pub struct LuaError {
    pub code: i32,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Script sandbox (Halborn: "Unauthenticated Lua Evaluation Executes OS
// Commands", "Unauthenticated Lua Scripts Can Exhaust All JSON-RPC Workers")
//
// `lua_evalscript` runs caller-supplied Lua on an UNAUTHENTICATED endpoint.
// It used to run it in `Lua::new()`: the full standard library (`os.execute`,
// `io.open`, `package.loadlib`, `os.getenv`, binary-chunk `load`), no
// instruction budget, no memory ceiling, no deadline and no concurrency bound.
//
// The bounds below are independent on purpose:
//   * memory limit           - an allocating loop fails the request, not the process;
//   * instruction budget     - a non-yielding spin loop is cut off;
//   * wall-clock deadline    - checked by the hook (CPU loops) AND enforced by
//                              `tokio::time::timeout` around the whole run
//                              (scripts parked on slow backend calls);
//   * concurrency semaphore  - at most N scripts run at once, so even scripts
//                              that use their full budget cannot occupy every
//                              actix worker.
//
// Escape hatches closed so the budget cannot be dodged:
//   * `coroutine` is not reachable: mlua's instruction hook applies only to the
//     thread it is installed on, so a script-created coroutine would run
//     unmetered. (mlua's own async glue caches `coroutine.yield` when the
//     `_RPC` functions are created; the global is removed afterwards.)
//   * `pcall`/`xpcall` re-raise once the budget is exhausted, so a budget error
//     cannot be caught and the loop resumed.
//   * `setmetatable` refuses `__gc`: finalizers can run on the main state
//     (e.g. at `lua_close`) where no hook is installed.
//   * Lua pattern matching runs in C, invisible to the instruction hook; the
//     pattern functions are guarded by a backtracking-cost estimate.
//   * result conversion uses raw table access, so no metamethod (Lua code)
//     runs outside the hooked thread.
// ---------------------------------------------------------------------------

/// Heap ceiling for one script, in bytes.
const MAX_LUA_MEMORY_BYTES: usize = 64 * 1024 * 1024;

/// VM instructions one script may execute. The shipped scripts are I/O-bound
/// and spend tens of thousands; a spin loop does ~1e8-1e9/s, so this bounds
/// `while true do end` to about a second of one worker.
const MAX_LUA_INSTRUCTIONS: u64 = 500_000_000;

/// How often the hook runs. Charged in whole intervals against the budget.
const LUA_HOOK_INTERVAL: u32 = 100_000;

/// Wall-clock deadline for one script, including time spent awaiting backends.
const MAX_LUA_WALL_CLOCK: Duration = Duration::from_secs(60);

/// `_RPC.*` calls one script may make. Each is one backend round-trip; the
/// heaviest shipped script issues one `esplora_tx` per UTXO.
const MAX_LUA_RPC_CALLS: usize = 4096;

/// Sub-calls all `sandshrew_multicall`s issued by ONE script may fan out to in
/// total. Shared across every `_RPC.sandshrew_multicall` the script makes, so
/// a loop of multicalls cannot multiply the per-request multicall cap.
const MAX_LUA_MULTICALL_SUBCALLS: u32 = 256;

/// Default number of scripts executing concurrently (env
/// `LUA_MAX_CONCURRENT_SCRIPTS` overrides).
const DEFAULT_LUA_MAX_CONCURRENT: usize = 4;

/// How long a request waits for an execution slot before being refused.
const LUA_QUEUE_TIMEOUT: Duration = Duration::from_secs(10);

/// Pattern-matching cost ceiling: `len ^ (quantifiers + 1)` must stay below
/// this (a rough upper bound on Lua's backtracking matcher, which runs in C
/// where the instruction hook cannot see it).
const MAX_PATTERN_COST: f64 = 1e9;

/// Lua -> JSON conversion bounds (cyclic / exponentially shared results).
const MAX_JSON_DEPTH: usize = 128;
const MAX_JSON_NODES: usize = 1_000_000;

/// Runtime limits for one script; `Default` is the production configuration.
#[derive(Clone, Debug)]
pub struct LuaLimits {
    pub memory_bytes: usize,
    pub instructions: u64,
    pub wall_clock: Duration,
}

impl Default for LuaLimits {
    fn default() -> Self {
        Self {
            memory_bytes: MAX_LUA_MEMORY_BYTES,
            instructions: MAX_LUA_INSTRUCTIONS,
            wall_clock: MAX_LUA_WALL_CLOCK,
        }
    }
}

/// Process-wide bound on concurrently executing scripts.
fn lua_semaphore() -> &'static Arc<Semaphore> {
    static SEM: OnceLock<Arc<Semaphore>> = OnceLock::new();
    SEM.get_or_init(|| {
        let n = std::env::var("LUA_MAX_CONCURRENT_SCRIPTS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_LUA_MAX_CONCURRENT);
        log::info!("Lua: at most {} concurrent scripts", n);
        Arc::new(Semaphore::new(n))
    })
}

/// Standard libraries a script may use. Deliberately absent: `os`, `io`,
/// `package`, `debug`. `coroutine` is loaded only so mlua's async glue can
/// cache `coroutine.yield`; the global is removed in `harden_globals`.
fn script_stdlib() -> LuaStdLib {
    LuaStdLib::STRING | LuaStdLib::TABLE | LuaStdLib::MATH | LuaStdLib::UTF8 | LuaStdLib::COROUTINE
}

/// Base-library globals removed outright. `dofile`/`loadfile` reach the
/// filesystem; `print`/`warn` write to the gateway's stdout/stderr;
/// `collectgarbage` could stop the collector.
const BANNED_GLOBALS: &[&str] = &["dofile", "loadfile", "print", "warn", "collectgarbage", "coroutine", "require"];

/// Number of backtracking quantifiers in a Lua pattern (conservative).
fn pattern_quantifiers(pat: &[u8]) -> i32 {
    let mut q = 0;
    let mut i = 0;
    while i < pat.len() {
        match pat[i] {
            b'%' => {
                // `%b` is a balance match (scan); other escapes are one class.
                if pat.get(i + 1) == Some(&b'b') {
                    q += 1;
                }
                i += 2;
                continue;
            }
            b'*' | b'+' | b'-' | b'?' => q += 1,
            _ => {}
        }
        i += 1;
    }
    q
}

fn pattern_cost_ok(subject_len: usize, pat: &[u8]) -> bool {
    let q = pattern_quantifiers(pat);
    let n = (subject_len.max(1)) as f64;
    n.powi(q + 1) <= MAX_PATTERN_COST || q == 0
}

/// Lua prelude, run once on a fresh VM before the user script. Receives the
/// budget re-raise check and the pattern guard as chunk arguments, so the raw
/// functions it wraps live only in upvalues the script cannot reach (no
/// `debug` library).
const PRELUDE: &str = r#"
local check, pat_guard = ...
local raw_pcall, raw_xpcall, raw_load = pcall, xpcall, load
local raw_setmt, raw_rawget, raw_select, raw_type, raw_error = setmetatable, rawget, select, type, error

pcall = function(...) return check(raw_pcall(...)) end
xpcall = function(...) return check(raw_xpcall(...)) end

-- Text chunks only: binary bytecode can be crafted to corrupt the VM.
load = function(chunk, name, mode, ...)
  if raw_select('#', ...) > 0 then return raw_load(chunk, name, "t", ...) end
  return raw_load(chunk, name, "t")
end

setmetatable = function(t, mt)
  if raw_type(mt) == "table" and raw_rawget(mt, "__gc") ~= nil then
    raw_error("__gc metamethods are not permitted", 2)
  end
  return raw_setmt(t, mt)
end

local s = string
local find, match, gmatch, gsub = s.find, s.match, s.gmatch, s.gsub
s.find = function(str, pat, init, plain)
  if not plain then pat_guard(str, pat) end
  return find(str, pat, init, plain)
end
s.match = function(str, pat, init) pat_guard(str, pat) return match(str, pat, init) end
s.gmatch = function(str, pat, ...) pat_guard(str, pat) return gmatch(str, pat, ...) end
s.gsub = function(str, pat, repl, n) pat_guard(str, pat) return gsub(str, pat, repl, n) end
s.dump = nil
"#;

/// Shared state between the hook, the re-raise check and the runner.
#[derive(Clone)]
struct Budget {
    exhausted: Arc<AtomicBool>,
    reason: Arc<std::sync::Mutex<Option<String>>>,
}

impl Budget {
    fn new() -> Self {
        Self { exhausted: Arc::new(AtomicBool::new(false)), reason: Arc::new(std::sync::Mutex::new(None)) }
    }
    fn trip(&self, why: String) -> mlua::Error {
        if let Ok(mut r) = self.reason.lock() {
            r.get_or_insert(why.clone());
        }
        self.exhausted.store(true, Ordering::SeqCst);
        mlua::Error::RuntimeError(why)
    }
    fn tripped(&self) -> Option<String> {
        if self.exhausted.load(Ordering::SeqCst) {
            Some(self.reason.lock().ok().and_then(|r| r.clone()).unwrap_or_else(|| "script budget exhausted".into()))
        } else {
            None
        }
    }
}

/// Build a fresh, sandboxed VM. `register` runs while `coroutine` is still
/// present (mlua's async functions need it at creation time); afterwards the
/// globals are hardened.
fn new_sandboxed_lua<F>(limits: &LuaLimits, budget: &Budget, register: F) -> Result<Lua>
where
    F: FnOnce(&Lua) -> LuaResult<()>,
{
    let lua = Lua::new_with(script_stdlib(), LuaOptions::default())?;
    // Fail closed: if the allocator cannot be bounded, do not run at all.
    lua.set_memory_limit(limits.memory_bytes)?;

    register(&lua)?;

    let globals = lua.globals();
    for name in BANNED_GLOBALS {
        globals.raw_set(*name, LuaValue::Nil)?;
    }

    let b = budget.clone();
    let check = lua.create_function(move |_, rets: LuaMultiValue| {
        if let Some(why) = b.tripped() {
            return Err(mlua::Error::RuntimeError(why));
        }
        Ok(rets)
    })?;
    let pat_guard = lua.create_function(|_, (subject, pat): (LuaValue, LuaValue)| {
        let len = match &subject {
            LuaValue::String(s) => s.as_bytes().len(),
            _ => 64, // numbers coerce to short strings; others error in the raw fn
        };
        if let LuaValue::String(p) = &pat {
            if !pattern_cost_ok(len, p.as_bytes()) {
                return Err(mlua::Error::RuntimeError(format!(
                    "pattern too expensive for a {}-byte subject",
                    len
                )));
            }
        }
        Ok(())
    })?;
    lua.load(PRELUDE)
        .set_name("=prelude")
        .set_mode(mlua::ChunkMode::Text)
        .call::<_, ()>((check, pat_guard))?;
    drop(globals);
    Ok(lua)
}

/// Run `script` on `lua` under the instruction budget and wall-clock deadline
/// and convert its result to JSON.
async fn run_sandboxed(lua: &Lua, script: &str, limits: &LuaLimits, budget: &Budget) -> Result<Value> {
    let start = Instant::now();

    // Text mode: a binary (precompiled bytecode) chunk is refused.
    let func = lua
        .load(script)
        .set_name("=script")
        .set_mode(mlua::ChunkMode::Text)
        .into_function()?;

    // Deliberately NOT `eval_async()`: that runs the chunk on a coroutine mlua
    // creates internally, and mlua's hook dispatcher ignores a hook installed
    // for a different thread. Creating the thread here makes the hooked
    // thread the one the script runs on.
    let thread = lua.create_thread(func)?;
    let used = Arc::new(AtomicU64::new(0));
    {
        let used = used.clone();
        let budget = budget.clone();
        let max_instr = limits.instructions;
        let deadline = limits.wall_clock;
        thread.set_hook(
            LuaHookTriggers::new().every_nth_instruction(LUA_HOOK_INTERVAL),
            move |_lua, _debug| {
                let n = used.fetch_add(LUA_HOOK_INTERVAL as u64, Ordering::Relaxed) + LUA_HOOK_INTERVAL as u64;
                if n > max_instr {
                    return Err(budget.trip(format!("script exceeded its budget of {} VM instructions", max_instr)));
                }
                if start.elapsed() > deadline {
                    return Err(budget.trip(format!("script exceeded its {}s deadline", deadline.as_secs())));
                }
                if budget.exhausted.load(Ordering::Relaxed) {
                    return Err(mlua::Error::RuntimeError("script budget exhausted".into()));
                }
                Ok(())
            },
        );
    }

    let outcome = tokio::time::timeout(limits.wall_clock, thread.into_async::<_, LuaValue>(())).await;
    let value = match outcome {
        Err(_) => {
            budget.trip(format!("script exceeded its {}s deadline", limits.wall_clock.as_secs()));
            return Err(anyhow!("script exceeded its {}s deadline", limits.wall_clock.as_secs()));
        }
        Ok(Err(e)) => {
            // Report the budget reason rather than whatever error the
            // unwinding produced.
            if let Some(why) = budget.tripped() {
                return Err(anyhow!(why));
            }
            return Err(e.into());
        }
        Ok(Ok(v)) => v,
    };
    lua_to_json(&value).map_err(Into::into)
}

/// Context for RPC calls made from Lua
#[derive(Clone)]
struct RpcContext {
    dispatcher: Arc<ProdDispatcher>,
    call_count: Arc<AtomicUsize>,
    /// Multicall allowance shared by every dispatch this script makes.
    multicall_budget: CallBudget,
}

impl RpcContext {
    fn new(dispatcher: Arc<ProdDispatcher>) -> Self {
        Self {
            dispatcher,
            call_count: Arc::new(AtomicUsize::new(0)),
            multicall_budget: CallBudget::with_limit(MAX_LUA_MULTICALL_SUBCALLS),
        }
    }

    fn get_call_count(&self) -> usize {
        self.call_count.load(Ordering::Relaxed)
    }

    async fn call_rpc(&self, method: &str, params: Vec<Value>) -> Result<Value> {
        let n = self.call_count.fetch_add(1, Ordering::Relaxed) + 1;
        if n > MAX_LUA_RPC_CALLS {
            return Err(anyhow!("script exceeded its budget of {} RPC calls", MAX_LUA_RPC_CALLS));
        }

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: serde_json::Value::Number(1.into()),
        };

        // Straight to the core dispatcher (every `_RPC` method is a core
        // method) with the script-wide multicall budget, so the bitcoind
        // allow-list and the multicall caps apply exactly as they do to a
        // top-level request - and are not reset per call.
        let response = self
            .dispatcher
            .dispatch_with_budget(&request, self.multicall_budget.clone())
            .await?;

        match response {
            JsonRpcResponse::Success { result, .. } => Ok(result),
            JsonRpcResponse::Error { error, .. } => {
                Err(anyhow!("RPC error {}: {}", error.code, error.message))
            }
        }
    }
}

/// Execute a Lua script with RPC access
pub async fn execute_lua_script(
    script: &str,
    args: Vec<Value>,
    dispatcher: &Arc<ProdDispatcher>,
    _proxy: &ProxyClient,
) -> Result<LuaExecutionResult> {
    // Bounded concurrency: wait (briefly) for a slot instead of piling on.
    let _permit = match tokio::time::timeout(LUA_QUEUE_TIMEOUT, lua_semaphore().clone().acquire_owned()).await {
        Ok(Ok(p)) => p,
        _ => return Err(anyhow!("Lua executor busy, try again later")),
    };

    let start = Instant::now();
    let limits = LuaLimits::default();
    let budget = Budget::new();
    let rpc_context = RpcContext::new(dispatcher.clone());

    let ctx = rpc_context.clone();
    let lua = new_sandboxed_lua(&limits, &budget, |lua| {
        // Flat _RPC table with all methods (created while `coroutine` exists).
        let rpc_table = lua.create_table()?;
        add_all_rpc_methods(lua, &rpc_table, ctx)?;
        lua.globals().set("_RPC", rpc_table)?;
        Ok(())
    })?;

    // Set args as global table
    let args_table = lua.create_table()?;
    for (i, arg) in args.iter().enumerate() {
        let lua_value = json_to_lua(&lua, arg)?;
        args_table.set(i + 1, lua_value)?;
    }
    lua.globals().set("args", args_table)?;

    let result = run_sandboxed(&lua, script, &limits, &budget).await?;

    let runtime = start.elapsed().as_millis() as u64;
    let calls = rpc_context.get_call_count();

    Ok(LuaExecutionResult {
        calls,
        returns: result,
        error: None,
        runtime,
    })
}

/// Create a Lua function that calls an RPC method using async callbacks
fn create_rpc_function<'lua>(
    _lua: &'lua Lua,
    method: &str,
    rpc_context: RpcContext,
) -> LuaResult<LuaFunction<'lua>>
{
    let method = method.to_string();
    // Use create_async_function to handle async RPC calls properly
    _lua.create_async_function(move |lua, args: LuaMultiValue| {
        let method = method.clone();
        let rpc_context = rpc_context.clone();
        async move {
            // Convert Lua args to JSON values
            let mut json_params = Vec::new();
            for arg in args {
                let json_val = lua_to_json(&arg)?;
                json_params.push(json_val);
            }

            // Make the RPC call directly using the async context
            let result = rpc_context.call_rpc(&method, json_params).await
                .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

            // Convert result back to Lua
            json_to_lua(lua, &result)
        }
    })
}

/// Add all RPC methods to a flat _RPC table
fn add_all_rpc_methods<'lua>(
    lua: &'lua Lua,
    rpc_table: &LuaTable<'lua>,
    rpc_context: RpcContext,
) -> LuaResult<()>
{
    // Esplora methods
    rpc_table.set("esplora_addressutxo", create_rpc_function(lua, "esplora_address::utxo", rpc_context.clone())?)?;
    rpc_table.set("esplora_addresstxs", create_rpc_function(lua, "esplora_address::txs", rpc_context.clone())?)?;
    rpc_table.set("esplora_addresstxschain", create_rpc_function(lua, "esplora_address::txs:chain", rpc_context.clone())?)?;
    rpc_table.set("esplora_addresstxsmempool", create_rpc_function(lua, "esplora_address::txs:mempool", rpc_context.clone())?)?;
    rpc_table.set("esplora_address", create_rpc_function(lua, "esplora_address", rpc_context.clone())?)?;
    rpc_table.set("esplora_tx", create_rpc_function(lua, "esplora_tx", rpc_context.clone())?)?;
    rpc_table.set("esplora_txstatus", create_rpc_function(lua, "esplora_tx::status", rpc_context.clone())?)?;
    rpc_table.set("esplora_txhex", create_rpc_function(lua, "esplora_tx::hex", rpc_context.clone())?)?;
    rpc_table.set("esplora_txraw", create_rpc_function(lua, "esplora_tx::raw", rpc_context.clone())?)?;
    rpc_table.set("esplora_txoutspends", create_rpc_function(lua, "esplora_tx::outspends", rpc_context.clone())?)?;
    rpc_table.set("esplora_block", create_rpc_function(lua, "esplora_block", rpc_context.clone())?)?;
    rpc_table.set("esplora_blockstatus", create_rpc_function(lua, "esplora_block::status", rpc_context.clone())?)?;
    rpc_table.set("esplora_blocktxs", create_rpc_function(lua, "esplora_block::txs", rpc_context.clone())?)?;
    rpc_table.set("esplora_blocktxids", create_rpc_function(lua, "esplora_block::txids", rpc_context.clone())?)?;
    rpc_table.set("esplora_blockheight", create_rpc_function(lua, "esplora_block-height", rpc_context.clone())?)?;
    rpc_table.set("esplora_mempool", create_rpc_function(lua, "esplora_mempool", rpc_context.clone())?)?;
    rpc_table.set("esplora_mempooltxids", create_rpc_function(lua, "esplora_mempool:txids", rpc_context.clone())?)?;
    rpc_table.set("esplora_mempoolrecent", create_rpc_function(lua, "esplora_mempool:recent", rpc_context.clone())?)?;
    rpc_table.set("esplora_feeestimates", create_rpc_function(lua, "esplora_fee-estimates", rpc_context.clone())?)?;

    // Ord methods
    rpc_table.set("ord_content", create_rpc_function(lua, "ord_content", rpc_context.clone())?)?;
    rpc_table.set("ord_blockheight", create_rpc_function(lua, "ord_blockheight", rpc_context.clone())?)?;
    rpc_table.set("ord_blockcount", create_rpc_function(lua, "ord_blockcount", rpc_context.clone())?)?;
    rpc_table.set("ord_blockhash", create_rpc_function(lua, "ord_blockhash", rpc_context.clone())?)?;
    rpc_table.set("ord_blocktime", create_rpc_function(lua, "ord_blocktime", rpc_context.clone())?)?;
    rpc_table.set("ord_blocks", create_rpc_function(lua, "ord_blocks", rpc_context.clone())?)?;
    rpc_table.set("ord_outputs", create_rpc_function(lua, "ord_outputs", rpc_context.clone())?)?;
    rpc_table.set("ord_inscription", create_rpc_function(lua, "ord_inscription", rpc_context.clone())?)?;
    rpc_table.set("ord_inscriptions", create_rpc_function(lua, "ord_inscriptions", rpc_context.clone())?)?;
    rpc_table.set("ord_block", create_rpc_function(lua, "ord_block", rpc_context.clone())?)?;
    rpc_table.set("ord_output", create_rpc_function(lua, "ord_output", rpc_context.clone())?)?;
    rpc_table.set("ord_rune", create_rpc_function(lua, "ord_rune", rpc_context.clone())?)?;
    rpc_table.set("ord_runes", create_rpc_function(lua, "ord_runes", rpc_context.clone())?)?;
    rpc_table.set("ord_sat", create_rpc_function(lua, "ord_sat", rpc_context.clone())?)?;
    rpc_table.set("ord_children", create_rpc_function(lua, "ord_children", rpc_context.clone())?)?;
    rpc_table.set("ord_parents", create_rpc_function(lua, "ord_parents", rpc_context.clone())?)?;
    rpc_table.set("ord_collections", create_rpc_function(lua, "ord_collections", rpc_context.clone())?)?;
    rpc_table.set("ord_decode", create_rpc_function(lua, "ord_decode", rpc_context.clone())?)?;

    // Bitcoin Core methods
    rpc_table.set("btc_getbestblockhash", create_rpc_function(lua, "btc_getbestblockhash", rpc_context.clone())?)?;
    rpc_table.set("btc_getblock", create_rpc_function(lua, "btc_getblock", rpc_context.clone())?)?;
    rpc_table.set("btc_getblockcount", create_rpc_function(lua, "btc_getblockcount", rpc_context.clone())?)?;
    rpc_table.set("btc_getblockhash", create_rpc_function(lua, "btc_getblockhash", rpc_context.clone())?)?;
    rpc_table.set("btc_getblockheader", create_rpc_function(lua, "btc_getblockheader", rpc_context.clone())?)?;
    rpc_table.set("btc_getblockstats", create_rpc_function(lua, "btc_getblockstats", rpc_context.clone())?)?;
    rpc_table.set("btc_getchaintips", create_rpc_function(lua, "btc_getchaintips", rpc_context.clone())?)?;
    rpc_table.set("btc_getchaintxstats", create_rpc_function(lua, "btc_getchaintxstats", rpc_context.clone())?)?;
    rpc_table.set("btc_getdifficulty", create_rpc_function(lua, "btc_getdifficulty", rpc_context.clone())?)?;
    rpc_table.set("btc_getmempoolancestors", create_rpc_function(lua, "btc_getmempoolancestors", rpc_context.clone())?)?;
    rpc_table.set("btc_getmempooldescendants", create_rpc_function(lua, "btc_getmempooldescendants", rpc_context.clone())?)?;
    rpc_table.set("btc_getmininginfo", create_rpc_function(lua, "btc_getmininginfo", rpc_context.clone())?)?;
    rpc_table.set("btc_getnetworkhashps", create_rpc_function(lua, "btc_getnetworkhashps", rpc_context.clone())?)?;
    rpc_table.set("btc_ping", create_rpc_function(lua, "btc_ping", rpc_context.clone())?)?;
    rpc_table.set("btc_getblockchaininfo", create_rpc_function(lua, "btc_getblockchaininfo", rpc_context.clone())?)?;
    rpc_table.set("btc_getrawtransaction", create_rpc_function(lua, "btc_getrawtransaction", rpc_context.clone())?)?;
    rpc_table.set("btc_sendrawtransaction", create_rpc_function(lua, "btc_sendrawtransaction", rpc_context.clone())?)?;
    rpc_table.set("btc_getmempoolinfo", create_rpc_function(lua, "btc_getmempoolinfo", rpc_context.clone())?)?;
    rpc_table.set("btc_getrawmempool", create_rpc_function(lua, "btc_getrawmempool", rpc_context.clone())?)?;
    rpc_table.set("btc_getmempoolentry", create_rpc_function(lua, "btc_getmempoolentry", rpc_context.clone())?)?;
    rpc_table.set("btc_getnetworkinfo", create_rpc_function(lua, "btc_getnetworkinfo", rpc_context.clone())?)?;
    rpc_table.set("btc_gettxout", create_rpc_function(lua, "btc_gettxout", rpc_context.clone())?)?;
    rpc_table.set("btc_decoderawtransaction", create_rpc_function(lua, "btc_decoderawtransaction", rpc_context.clone())?)?;

    // Alkanes methods
    rpc_table.set("alkanes_getbytecode", create_rpc_function(lua, "alkanes_getbytecode", rpc_context.clone())?)?;
    rpc_table.set("alkanes_protorunesbyaddress", create_rpc_function(lua, "alkanes_protorunesbyaddress", rpc_context.clone())?)?;
    rpc_table.set("alkanes_protorunesbyoutpoint", create_rpc_function(lua, "alkanes_protorunesbyoutpoint", rpc_context.clone())?)?;
    // Alias for Lua script compatibility (protorunes_by_outpoint is used in batch_utxo_balances.lua)
    rpc_table.set("protorunes_by_outpoint", create_rpc_function(lua, "alkanes_protorunesbyoutpoint", rpc_context.clone())?)?;

    // Metashrew methods
    rpc_table.set("metashrew_view", create_rpc_function(lua, "metashrew_view", rpc_context.clone())?)?;
    rpc_table.set("metashrew_height", create_rpc_function(lua, "metashrew_height", rpc_context.clone())?)?;

    // Sandshrew methods
    rpc_table.set("sandshrew_multicall", create_rpc_function(lua, "sandshrew_multicall", rpc_context.clone())?)?;
    rpc_table.set("sandshrew_balances", create_rpc_function(lua, "sandshrew_balances", rpc_context.clone())?)?;

    Ok(())
}

/// Convert JSON Value to Lua Value
fn json_to_lua<'lua>(lua: &'lua Lua, value: &Value) -> LuaResult<LuaValue<'lua>> {
    match value {
        Value::Null => Ok(LuaValue::Nil),
        Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(LuaValue::Number(f))
            } else {
                Ok(LuaValue::Nil)
            }
        }
        Value::String(s) => Ok(LuaValue::String(lua.create_string(s)?)),
        Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                table.set(i + 1, json_to_lua(lua, v)?)?;
            }
            Ok(LuaValue::Table(table))
        }
        Value::Object(obj) => {
            let table = lua.create_table()?;
            for (k, v) in obj.iter() {
                table.set(k.as_str(), json_to_lua(lua, v)?)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}

/// Convert Lua Value to JSON Value.
///
/// Halborn: "A Cyclic Lua Return Crashes the JSON-RPC Gateway". The old
/// converter recursed without bound, so `local t = {}; t.self = t; return t`
/// overflowed the native stack and aborted the whole gateway process. Now:
///   * tables on the current path are tracked, a cycle is an error;
///   * depth is capped (`MAX_JSON_DEPTH`);
///   * total nodes are capped (`MAX_JSON_NODES`), so a DAG that shares one
///     subtable exponentially often cannot blow up the output either;
///   * tables are read with RAW access, so no `__index`/`__len` metamethod
///     (Lua code) runs here, outside the hooked thread.
fn lua_to_json(value: &LuaValue) -> LuaResult<Value> {
    let mut state = JsonConv { path: HashSet::new(), nodes: 0 };
    state.convert(value, 0)
}

struct JsonConv {
    path: HashSet<*const c_void>,
    nodes: usize,
}

impl JsonConv {
    fn convert(&mut self, value: &LuaValue, depth: usize) -> LuaResult<Value> {
        self.nodes += 1;
        if self.nodes > MAX_JSON_NODES {
            return Err(mlua::Error::RuntimeError(format!(
                "result too large (more than {} values)",
                MAX_JSON_NODES
            )));
        }
        match value {
            LuaValue::Nil => Ok(Value::Null),
            LuaValue::Boolean(b) => Ok(Value::Bool(*b)),
            LuaValue::Integer(i) => Ok(Value::Number((*i).into())),
            LuaValue::Number(n) => serde_json::Number::from_f64(*n)
                .map(Value::Number)
                .ok_or_else(|| mlua::Error::RuntimeError("Invalid number conversion".to_string())),
            LuaValue::String(s) => Ok(Value::String(s.to_str()?.to_string())),
            LuaValue::Table(table) => {
                if depth >= MAX_JSON_DEPTH {
                    return Err(mlua::Error::RuntimeError(format!(
                        "result nested too deeply (limit {})",
                        MAX_JSON_DEPTH
                    )));
                }
                let ptr = table.to_pointer();
                if !self.path.insert(ptr) {
                    return Err(mlua::Error::RuntimeError(
                        "result contains a cyclic table".to_string(),
                    ));
                }
                let out = self.convert_table(table, depth);
                self.path.remove(&ptr);
                out
            }
            _ => Ok(Value::Null),
        }
    }

    fn convert_table(&mut self, table: &LuaTable, depth: usize) -> LuaResult<Value> {
        // Array if it has a raw sequence part starting at 1.
        let len = table.raw_len();
        if len > 0 {
            let mut arr = Vec::with_capacity(len.min(4096));
            for i in 1..=len {
                let val: LuaValue = table.raw_get(i)?;
                arr.push(self.convert(&val, depth + 1)?);
            }
            return Ok(Value::Array(arr));
        }

        let mut entries: Vec<(String, LuaValue)> = Vec::new();
        table.for_each(|k: LuaValue, v: LuaValue| {
            let key = match k {
                LuaValue::String(s) => s.to_str()?.to_string(),
                LuaValue::Integer(i) => i.to_string(),
                LuaValue::Number(n) => n.to_string(),
                _ => return Ok(()),
            };
            entries.push((key, v));
            Ok(())
        })?;
        let mut obj = serde_json::Map::new();
        for (k, v) in entries {
            obj.insert(k, self.convert(&v, depth + 1)?);
        }
        Ok(Value::Object(obj))
    }
}

#[cfg(test)]
mod sandbox_tests {
    use super::*;

    fn small_limits() -> LuaLimits {
        LuaLimits {
            memory_bytes: 16 * 1024 * 1024,
            instructions: 20_000_000,
            wall_clock: Duration::from_secs(20),
        }
    }

    async fn run(script: &str) -> Result<Value> {
        let limits = small_limits();
        let budget = Budget::new();
        let lua = new_sandboxed_lua(&limits, &budget, |_| Ok(()))?;
        run_sandboxed(&lua, script, &limits, &budget).await
    }

    fn err_of(r: Result<Value>) -> String {
        match r {
            Ok(v) => panic!("expected an error, got {v}"),
            Err(e) => format!("{e:#}"),
        }
    }

    #[tokio::test]
    async fn plain_scripts_still_work() {
        let v = run(r#"
            local t = {}
            for i = 1, 10 do table.insert(t, i * 2) end
            local ok, r = pcall(function() return string.format("%d", #t) end)
            return { n = #t, ok = ok, r = r, s = ("a,b"):find(",", 1, true), m = math.max(1, 2) }
        "#).await.unwrap();
        assert_eq!(v["n"], 10);
        assert_eq!(v["ok"], true);
        assert_eq!(v["r"], "10");
        assert_eq!(v["s"], 2);
        assert_eq!(v["m"], 2);
    }

    #[tokio::test]
    async fn os_io_package_debug_absent() {
        let v = run(r#"
            return {
              os = type(os), io = type(io), package = type(package), debug = type(debug),
              coroutine = type(coroutine), dofile = type(dofile), loadfile = type(loadfile),
              require = type(require), dump = type(string.dump),
            }
        "#).await.unwrap();
        for k in ["os", "io", "package", "debug", "coroutine", "dofile", "loadfile", "require", "dump"] {
            assert_eq!(v[k], "nil", "{k} must not be reachable: {v}");
        }
        // The literal Halborn PoC.
        let e = err_of(run(r#"return os.execute("id")"#).await);
        assert!(e.contains("os"), "{e}");
    }

    #[tokio::test]
    async fn binary_chunk_rejected() {
        // Lua 5.4 binary chunks start with "\27Lua".
        let bin = "\x1bLuaT\x00";
        // Submitted script itself.
        let e = err_of(run(bin).await);
        assert!(e.to_lowercase().contains("binary") || e.contains("mode"), "{e}");
        // Through `load`, even when the caller asks for mode "b"/"bt".
        let v = run(r#"
            local f1, e1 = load("\27Lua\84\0", "x", "b")
            local f2, e2 = load("\27Lua\84\0", "x", "bt")
            local f3 = load("return 7")
            return { f1 = f1 == nil, f2 = f2 == nil, e1 = e1, seven = f3() }
        "#).await.unwrap();
        assert_eq!(v["f1"], true, "{v}");
        assert_eq!(v["f2"], true, "{v}");
        assert!(v["e1"].as_str().unwrap().contains("binary"), "{v}");
        assert_eq!(v["seven"], 7);
    }

    #[tokio::test]
    async fn infinite_loop_terminates() {
        let e = err_of(run("while true do end").await);
        assert!(e.contains("VM instructions"), "{e}");
    }

    #[tokio::test]
    async fn coroutine_infinite_loop_terminates() {
        // The coroutine escape: mlua's hook only covers the hooked thread.
        // `coroutine` is gone, so this fails immediately instead of spinning.
        let e = err_of(run(r#"
            local co = coroutine.create(function() while true do end end)
            return coroutine.resume(co)
        "#).await);
        assert!(e.contains("coroutine"), "{e}");
    }

    #[tokio::test]
    async fn pcall_cannot_swallow_budget_error() {
        let e = err_of(run(r#"
            while true do pcall(function() while true do end end) end
        "#).await);
        assert!(e.contains("VM instructions"), "{e}");
        let e = err_of(run(r#"
            while true do xpcall(function() while true do end end, function(m) return m end) end
        "#).await);
        assert!(e.contains("VM instructions"), "{e}");
    }

    #[tokio::test]
    async fn memory_is_bounded() {
        let e = err_of(run(r#"
            local t = {}
            for i = 1, 1e9 do t[i] = string.rep("x", 1024) .. i end
        "#).await);
        assert!(e.to_lowercase().contains("memory"), "{e}");
    }

    #[tokio::test]
    async fn gc_finalizers_refused() {
        let e = err_of(run(r#"
            setmetatable({}, { __gc = function() while true do end end })
        "#).await);
        assert!(e.contains("__gc"), "{e}");
    }

    #[tokio::test]
    async fn catastrophic_pattern_refused() {
        let e = err_of(run(r#"
            local s = string.rep("a", 200000)
            return s:find(".-.-.-.-.-b")
        "#).await);
        assert!(e.contains("pattern too expensive"), "{e}");
        // Small patterns on small subjects are fine.
        let v = run(r#"return (("key=value"):match("^(%w+)=(%w+)$"))"#).await.unwrap();
        assert_eq!(v, "key");
    }

    #[tokio::test]
    async fn cyclic_table_returns_error() {
        // The Halborn PoC: a self-referential result used to overflow the stack.
        let e = err_of(run("local t = {}; t.self = t; return t").await);
        assert!(e.contains("cyclic"), "{e}");
        let e = err_of(run("local t = {}; t[1] = t; return t").await);
        assert!(e.contains("cyclic"), "{e}");
    }

    #[tokio::test]
    async fn deep_and_exponential_results_bounded() {
        let e = err_of(run(r#"
            local t = {}
            for i = 1, 10000 do t = { t } end
            return t
        "#).await);
        assert!(e.contains("nested too deeply"), "{e}");
        // DAG: every level references the previous one twice -> 2^40 nodes.
        let e = err_of(run(r#"
            local t = { 1 }
            for i = 1, 40 do t = { t, t } end
            return t
        "#).await);
        assert!(e.contains("too large"), "{e}");
        // Shared (non-cyclic) subtables are still fine.
        let v = run("local s = { 1 }; return { a = s, b = s }").await.unwrap();
        assert_eq!(v["a"], serde_json::json!([1]));
        assert_eq!(v["b"], serde_json::json!([1]));
    }

    #[tokio::test]
    async fn metamethods_do_not_run_during_conversion() {
        let v = run(r#"
            return setmetatable({}, { __len = function() while true do end end,
                                      __index = function() while true do end end })
        "#).await.unwrap();
        assert_eq!(v, serde_json::json!({}));
    }

    #[tokio::test]
    async fn wall_clock_deadline_enforced_across_awaits() {
        // A script parked on an async host function (here: a sleep standing in
        // for a slow backend) burns no instructions; the tokio deadline must
        // still fire.
        let limits = LuaLimits { wall_clock: Duration::from_millis(300), ..small_limits() };
        let budget = Budget::new();
        let lua = new_sandboxed_lua(&limits, &budget, |lua| {
            let f = lua.create_async_function(|_, ()| async {
                tokio::time::sleep(Duration::from_secs(3600)).await;
                Ok(())
            })?;
            lua.globals().set("slow", f)
        })
        .unwrap();
        let t = Instant::now();
        let e = err_of(run_sandboxed(&lua, "while true do slow() end", &limits, &budget).await);
        assert!(e.contains("deadline"), "{e}");
        assert!(t.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn shipped_multicall_script_runs_in_sandbox() {
        // lua/multicall.lua calls async `_RPC` functions INSIDE pcall; the
        // pcall wrapper must stay yieldable for that to work.
        let limits = small_limits();
        let budget = Budget::new();
        let lua = new_sandboxed_lua(&limits, &budget, |lua| {
            let rpc = lua.create_table()?;
            rpc.set(
                "btc_getblockcount",
                lua.create_async_function(|_, ()| async {
                    tokio::task::yield_now().await;
                    Ok(840000)
                })?,
            )?;
            rpc.set(
                "boom",
                lua.create_async_function(|_, ()| async {
                    tokio::task::yield_now().await;
                    Err::<(), _>(mlua::Error::RuntimeError("backend down".into()))
                })?,
            )?;
            lua.globals().set("_RPC", rpc)?;
            let args = lua.create_table()?;
            args.set(1, json_to_lua(lua, &serde_json::json!(["btc_getblockcount", []]))?)?;
            args.set(2, json_to_lua(lua, &serde_json::json!(["boom", []]))?)?;
            lua.globals().set("args", args)
        })
        .unwrap();
        let script = include_str!("../../../lua/multicall.lua");
        let v = run_sandboxed(&lua, script, &limits, &budget).await.unwrap();
        assert_eq!(v[0]["result"], 840000, "{v}");
        assert!(v[1]["error"]["message"].as_str().unwrap().contains("backend down"), "{v}");
    }

    #[test]
    fn pattern_cost_estimate() {
        assert!(pattern_cost_ok(1_000_000, b"abc"));
        assert!(pattern_cost_ok(10_000, b"%d+"));
        assert!(!pattern_cost_ok(200_000, b".-.-b"));
        assert_eq!(pattern_quantifiers(b"%*%+a*"), 1);
        assert_eq!(pattern_quantifiers(b"%b()"), 1);
    }
}
