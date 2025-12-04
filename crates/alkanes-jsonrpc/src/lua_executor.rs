use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse};
use crate::proxy::ProxyClient;
use anyhow::{anyhow, Result};
use mlua::prelude::*;
use moka::sync::Cache;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

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

/// Context for RPC calls made from Lua
#[derive(Clone)]
struct RpcContext {
    proxy: Arc<ProxyClient>,
    call_count: Arc<Mutex<usize>>,
}

impl RpcContext {
    fn new(proxy: Arc<ProxyClient>) -> Self {
        Self {
            proxy,
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    async fn increment_calls(&self) {
        let mut count = self.call_count.lock().await;
        *count += 1;
    }

    async fn get_call_count(&self) -> usize {
        *self.call_count.lock().await
    }

    async fn call_rpc(&self, method: &str, params: Vec<Value>) -> Result<Value> {
        self.increment_calls().await;
        
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: serde_json::Value::Number(1.into()),
        };

        let response = crate::handler::handle_request(&request, &self.proxy).await?;
        
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
    proxy: &ProxyClient,
) -> Result<LuaExecutionResult> {
    let start = Instant::now();
    let rpc_context = RpcContext::new(Arc::new(proxy.clone()));

    let lua = Lua::new();

    // Create flat _RPC table with all methods
    let rpc_table = lua.create_table()?;

    // Add all RPC methods to the flat table
    add_all_rpc_methods(&lua, &rpc_table, rpc_context.clone())?;

    // Set _RPC as global
    lua.globals().set("_RPC", rpc_table)?;

    // Set args as global table
    let args_table = lua.create_table()?;
    for (i, arg) in args.iter().enumerate() {
        let lua_value = json_to_lua(&lua, arg)?;
        args_table.set(i + 1, lua_value)?;
    }
    lua.globals().set("args", args_table)?;

    // Execute the script (async)
    let result_value: LuaValue = lua.load(script).eval_async().await?;

    // Convert result to JSON
    let result = lua_to_json(&result_value, &lua)?;
    
    let runtime = start.elapsed().as_millis() as u64;
    let calls = rpc_context.get_call_count().await;

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
                let json_val = lua_to_json(&arg, lua)?;
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
    rpc_table.set("btc_getnettotals", create_rpc_function(lua, "btc_getnettotals", rpc_context.clone())?)?;
    rpc_table.set("btc_getpeerinfo", create_rpc_function(lua, "btc_getpeerinfo", rpc_context.clone())?)?;
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

/// Convert Lua Value to JSON Value
fn lua_to_json(value: &LuaValue, lua: &Lua) -> LuaResult<Value> {
    match value {
        LuaValue::Nil => Ok(Value::Null),
        LuaValue::Boolean(b) => Ok(Value::Bool(*b)),
        LuaValue::Integer(i) => Ok(Value::Number((*i).into())),
        LuaValue::Number(n) => {
            serde_json::Number::from_f64(*n)
                .map(Value::Number)
                .ok_or_else(|| mlua::Error::RuntimeError("Invalid number conversion".to_string()))
        }
        LuaValue::String(s) => {
            let string = s.to_str()?.to_string();
            Ok(Value::String(string))
        }
        LuaValue::Table(table) => {
            // Check if it's an array (consecutive integer keys starting from 1)
            let len = table.len()?;
            if len > 0 {
                let mut arr = Vec::new();
                let mut is_array = true;

                for i in 1..=len {
                    match table.get::<_, LuaValue>(i) {
                        Ok(val) => arr.push(lua_to_json(&val, lua)?),
                        Err(_) => {
                            is_array = false;
                            break;
                        }
                    }
                }

                if is_array {
                    return Ok(Value::Array(arr));
                }
            }

            // Build as object
            let mut obj = serde_json::Map::new();
            for pair in table.clone().pairs::<LuaValue, LuaValue>() {
                let (k, v) = pair?;
                let key = match k {
                    LuaValue::String(s) => s.to_str()?.to_string(),
                    LuaValue::Integer(i) => i.to_string(),
                    LuaValue::Number(n) => n.to_string(),
                    _ => continue,
                };
                obj.insert(key, lua_to_json(&v, lua)?);
            }
            Ok(Value::Object(obj))
        }
        _ => Ok(Value::Null),
    }
}
