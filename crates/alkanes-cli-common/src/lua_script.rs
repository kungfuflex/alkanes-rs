//! Lua script execution with automatic hash-based caching
//!
//! This module provides a unified interface for executing Lua scripts that works
//! in both native and WASM environments. It automatically handles:
//! - Computing SHA-256 hash of script content
//! - Attempting cached execution via lua_evalsaved first
//! - Falling back to lua_evalscript if script not cached
//! - Works with both embedded scripts (include_str!) and runtime strings

use crate::{AlkanesError, Result};
use serde_json::Value as JsonValue;
use sha2::{Sha256, Digest};

/// Represents a Lua script that can be executed
#[derive(Clone, Debug)]
pub struct LuaScript {
    /// The Lua script content
    content: String,
    /// SHA-256 hash of the script (used for caching)
    hash: String,
}

impl LuaScript {
    /// Create a new LuaScript from string content
    pub fn from_string(content: String) -> Self {
        let hash = Self::compute_hash(&content);
        Self { content, hash }
    }

    /// Create a LuaScript from a static string (e.g., include_str!)
    pub fn from_static(content: &'static str) -> Self {
        Self::from_string(content.to_string())
    }

    /// Get the script content
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the script hash (for lua_evalsaved)
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Compute SHA-256 hash of script content
    fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Extension trait for providers that support Lua script execution
#[async_trait::async_trait(?Send)]
pub trait LuaScriptExecutor {
    /// Execute a Lua script with automatic caching
    /// 
    /// This method:
    /// 1. Computes the hash of the script
    /// 2. Tries lua_evalsaved with the hash first (cached execution)
    /// 3. Falls back to lua_evalscript with full content if not cached
    /// 
    /// # Arguments
    /// * `script` - The Lua script to execute
    /// * `args` - Arguments to pass to the script
    /// 
    /// # Returns
    /// The JSON result from script execution
    async fn execute_lua_script(
        &self,
        script: &LuaScript,
        args: Vec<JsonValue>,
    ) -> Result<JsonValue>;

    /// Low-level method to call lua_evalsaved
    async fn lua_evalsaved(
        &self,
        script_hash: &str,
        args: Vec<JsonValue>,
    ) -> Result<JsonValue>;

    /// Low-level method to call lua_evalscript
    async fn lua_evalscript(
        &self,
        script_content: &str,
        args: Vec<JsonValue>,
    ) -> Result<JsonValue>;
}

// Blanket implementation for Box<T>
#[async_trait::async_trait(?Send)]
impl<T: LuaScriptExecutor + ?Sized> LuaScriptExecutor for Box<T> {
    async fn execute_lua_script(
        &self,
        script: &LuaScript,
        args: Vec<JsonValue>,
    ) -> Result<JsonValue> {
        (**self).execute_lua_script(script, args).await
    }

    async fn lua_evalsaved(
        &self,
        script_hash: &str,
        args: Vec<JsonValue>,
    ) -> Result<JsonValue> {
        (**self).lua_evalsaved(script_hash, args).await
    }

    async fn lua_evalscript(
        &self,
        script_content: &str,
        args: Vec<JsonValue>,
    ) -> Result<JsonValue> {
        (**self).lua_evalscript(script_content, args).await
    }
}

// Embedded Lua scripts
/// Batch UTXO balance fetching script
pub const BATCH_UTXO_BALANCES: &str = include_str!("../../../lua/batch_utxo_balances.lua");

/// Comprehensive balance information script (replacement for sandshrew_balances)
pub const BALANCES: &str = include_str!("../../../lua/balances.lua");

/// Multicall script (replacement for sandshrew_multicall)
pub const MULTICALL: &str = include_str!("../../../lua/multicall.lua");

/// Address UTXOs with full transaction data (batched esplora_tx calls)
pub const ADDRESS_UTXOS_WITH_TXS: &str = include_str!("../../../lua/address_utxos_with_txs.lua");

/// Spendable UTXOs script (filters out immature coinbase outputs)
pub const SPENDABLE_UTXOS: &str = include_str!("../../../lua/spendable_utxos.lua");

/// Lazy-initialized static script instances
pub mod scripts {
    use super::LuaScript;
    use once_cell::sync::Lazy;

    /// Batch UTXO balances script
    pub static BATCH_UTXO_BALANCES: Lazy<LuaScript> = 
        Lazy::new(|| LuaScript::from_static(super::BATCH_UTXO_BALANCES));

    /// Balances script
    pub static BALANCES: Lazy<LuaScript> = 
        Lazy::new(|| LuaScript::from_static(super::BALANCES));

    /// Multicall script
    pub static MULTICALL: Lazy<LuaScript> = 
        Lazy::new(|| LuaScript::from_static(super::MULTICALL));

    /// Address UTXOs with transaction data script
    pub static ADDRESS_UTXOS_WITH_TXS: Lazy<LuaScript> =
        Lazy::new(|| LuaScript::from_static(super::ADDRESS_UTXOS_WITH_TXS));

    /// Spendable UTXOs script (filters out immature coinbase)
    pub static SPENDABLE_UTXOS: Lazy<LuaScript> =
        Lazy::new(|| LuaScript::from_static(super::SPENDABLE_UTXOS));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lua_script_hash() {
        let script = LuaScript::from_string("return 42".to_string());
        assert_eq!(script.content(), "return 42");
        // Hash should be deterministic
        let script2 = LuaScript::from_string("return 42".to_string());
        assert_eq!(script.hash(), script2.hash());
    }

    #[test]
    fn test_different_scripts_different_hashes() {
        let script1 = LuaScript::from_string("return 42".to_string());
        let script2 = LuaScript::from_string("return 43".to_string());
        assert_ne!(script1.hash(), script2.hash());
    }

    #[test]
    fn test_embedded_scripts_load() {
        // Just verify they compile and load
        let _ = &scripts::BATCH_UTXO_BALANCES;
        let _ = &scripts::BALANCES;
        let _ = &scripts::MULTICALL;
        let _ = &scripts::SPENDABLE_UTXOS;
    }
}
