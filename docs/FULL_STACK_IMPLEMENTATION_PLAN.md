# Full Stack Implementation Plan: Complete Data API Ecosystem

## Overview

This document provides a comprehensive implementation plan to build the **complete data API ecosystem** from backend to all client interfaces:

1. ✅ **Backend** - alkanes-contract-indexer + alkanes-data-api (Server)
2. 🔨 **Client Library** - alkanes-cli-common (Rust client)
3. 🔨 **CLI Interface** - alkanes-cli-sys + alkanes-cli (Command line)
4. 🔨 **Web Interface** - alkanes-web-sys (WASM)
5. 🔨 **FFI Bindings** - alkanes-ffi (C/C++/Swift/etc)
6. 🔨 **JNI Bindings** - alkanes-jni (Java/Kotlin/Android)
7. 🔨 **TypeScript SDK** - ts-sdk (Node.js/Browser)

---

## Part 1: Backend Implementation (Weeks 1-5)

See `ALKANES_DATA_API_BUILDOUT_PLAN.md` for detailed backend implementation.

**Summary**:
- Phase 1: Balance tracking system
- Phase 2: Storage indexing system
- Phase 3: Enhanced AMM data
- Result: 11 API endpoints

---

## Part 2: Rust Client Library (Week 6)

### Location: `crates/alkanes-cli-common/src/dataapi/`

### Objective
Create a comprehensive Rust client library for the data API that can be used by CLI, Web, FFI, and JNI interfaces.

### Step 2.1: Client Structure

**Files to Create**:
```
crates/alkanes-cli-common/src/dataapi/
├── mod.rs                  # Module exports
├── client.rs               # Main client struct
├── balance.rs              # Balance API methods
├── storage.rs              # Storage API methods
├── amm.rs                  # AMM API methods
├── types.rs                # Request/response types
└── error.rs                # Error types
```

### Step 2.2: Core Client Implementation

**File**: `crates/alkanes-cli-common/src/dataapi/client.rs`

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Main DataAPI client
#[derive(Clone, Debug)]
pub struct DataApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl DataApiClient {
    /// Create a new DataAPI client
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        
        Ok(Self {
            base_url: base_url.into(),
            client,
        })
    }
    
    /// Create client with custom reqwest client
    pub fn with_client(base_url: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            base_url: base_url.into(),
            client,
        }
    }
    
    /// Build full URL from path
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
    
    /// GET request helper
    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: Option<&impl Serialize>,
    ) -> Result<T> {
        let mut req = self.client.get(&self.url(path));
        if let Some(q) = query {
            req = req.query(q);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await?;
            anyhow::bail!("HTTP {}: {}", status, text);
        }
        Ok(resp.json().await?)
    }
    
    /// POST request helper
    async fn post<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T> {
        let resp = self.client
            .post(&self.url(path))
            .json(body)
            .send()
            .await?;
        
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await?;
            anyhow::bail!("HTTP {}: {}", status, text);
        }
        Ok(resp.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_client_creation() {
        let client = DataApiClient::new("http://localhost:3000").unwrap();
        assert_eq!(client.base_url, "http://localhost:3000");
    }
}
```

### Step 2.3: Balance API Methods

**File**: `crates/alkanes-cli-common/src/dataapi/balance.rs`

```rust
use super::client::DataApiClient;
use super::types::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

impl DataApiClient {
    /// Get all balances for a Bitcoin address
    pub async fn get_address_balances(
        &self,
        address: &str,
        include_outpoints: bool,
    ) -> Result<AddressBalancesResponse> {
        #[derive(Serialize)]
        struct Query<'a> {
            address: &'a str,
            include_outpoints: bool,
        }
        
        self.get("/balance/address", Some(&Query {
            address,
            include_outpoints,
        })).await
    }
    
    /// Get balances for a specific UTXO
    pub async fn get_outpoint_balances(
        &self,
        outpoint: &str,
    ) -> Result<OutpointBalancesResponse> {
        #[derive(Serialize)]
        struct Query<'a> {
            outpoint: &'a str,
        }
        
        self.get("/balance/outpoint", Some(&Query { outpoint })).await
    }
    
    /// Get holders for an alkane (paginated)
    pub async fn get_holders(
        &self,
        alkane: &str,
        page: Option<usize>,
        limit: Option<usize>,
    ) -> Result<HoldersResponse> {
        #[derive(Serialize)]
        struct Query<'a> {
            alkane: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            page: Option<usize>,
            #[serde(skip_serializing_if = "Option::is_none")]
            limit: Option<usize>,
        }
        
        self.get("/balance/holders", Some(&Query {
            alkane,
            page,
            limit,
        })).await
    }
    
    /// Get total holder count for an alkane
    pub async fn get_holders_count(&self, alkane: &str) -> Result<HolderCountResponse> {
        #[derive(Serialize)]
        struct Query<'a> {
            alkane: &'a str,
        }
        
        self.get("/balance/holders/count", Some(&Query { alkane })).await
    }
    
    /// Get all UTXOs with balances for an address
    pub async fn get_address_outpoints(&self, address: &str) -> Result<AddressOutpointsResponse> {
        #[derive(Serialize)]
        struct Query<'a> {
            address: &'a str,
        }
        
        self.get("/balance/address/outpoints", Some(&Query { address })).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressBalancesResponse {
    pub ok: bool,
    pub address: String,
    pub balances: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outpoints: Option<Vec<OutpointInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutpointInfo {
    pub outpoint: String,
    pub entries: Vec<BalanceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceEntry {
    pub alkane: String,
    pub amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutpointBalancesResponse {
    pub ok: bool,
    pub outpoint: String,
    pub items: Vec<OutpointInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldersResponse {
    pub ok: bool,
    pub alkane: String,
    pub page: usize,
    pub limit: usize,
    pub total: usize,
    pub has_more: bool,
    pub items: Vec<HolderInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolderInfo {
    pub address: String,
    pub amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolderCountResponse {
    pub ok: bool,
    pub alkane: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressOutpointsResponse {
    pub ok: bool,
    pub address: String,
    pub outpoints: Vec<OutpointInfo>,
}
```

### Step 2.4: Storage API Methods

**File**: `crates/alkanes-cli-common/src/dataapi/storage.rs`

```rust
use super::client::DataApiClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

impl DataApiClient {
    /// Get storage keys for a contract
    pub async fn get_keys(
        &self,
        alkane: &str,
        keys: Option<Vec<String>>,
        page: Option<usize>,
        limit: Option<usize>,
        try_decode_utf8: Option<bool>,
    ) -> Result<GetKeysResponse> {
        #[derive(Serialize)]
        struct Query<'a> {
            alkane: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            keys: Option<String>, // Comma-separated
            #[serde(skip_serializing_if = "Option::is_none")]
            page: Option<usize>,
            #[serde(skip_serializing_if = "Option::is_none")]
            limit: Option<usize>,
            #[serde(skip_serializing_if = "Option::is_none")]
            try_decode_utf8: Option<bool>,
        }
        
        let keys_str = keys.map(|k| k.join(","));
        
        self.get("/storage/keys", Some(&Query {
            alkane,
            keys: keys_str,
            page,
            limit,
            try_decode_utf8,
        })).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetKeysResponse {
    pub ok: bool,
    pub alkane: String,
    pub page: usize,
    pub limit: usize,
    pub total: usize,
    pub has_more: bool,
    pub items: HashMap<String, StorageValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageValue {
    pub key_hex: String,
    pub key_str: Option<String>,
    pub value_hex: String,
    pub value_str: Option<String>,
    pub value_u128: Option<String>,
    pub last_txid: String,
}
```

### Step 2.5: AMM API Methods

**File**: `crates/alkanes-cli-common/src/dataapi/amm.rs`

```rust
use super::client::DataApiClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

impl DataApiClient {
    /// Get OHLCV candles for a pool
    pub async fn get_candles(
        &self,
        pool: &str,
        timeframe: Option<&str>,
        side: Option<&str>,
        page: Option<usize>,
        limit: Option<usize>,
        now: Option<u64>,
    ) -> Result<CandlesResponse> {
        #[derive(Serialize)]
        struct Query<'a> {
            pool: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            timeframe: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            side: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            page: Option<usize>,
            #[serde(skip_serializing_if = "Option::is_none")]
            limit: Option<usize>,
            #[serde(skip_serializing_if = "Option::is_none")]
            now: Option<u64>,
        }
        
        self.get("/amm/candles", Some(&Query {
            pool,
            timeframe,
            side,
            page,
            limit,
            now,
        })).await
    }
    
    /// Get trade history for a pool
    pub async fn get_trades(
        &self,
        pool: &str,
        params: GetTradesParams,
    ) -> Result<TradesResponse> {
        self.get("/amm/trades", Some(&params.with_pool(pool))).await
    }
    
    /// Get all pools with reserves
    pub async fn get_pools(
        &self,
        page: Option<usize>,
        limit: Option<usize>,
    ) -> Result<PoolsResponse> {
        #[derive(Serialize)]
        struct Query {
            #[serde(skip_serializing_if = "Option::is_none")]
            page: Option<usize>,
            #[serde(skip_serializing_if = "Option::is_none")]
            limit: Option<usize>,
        }
        
        self.get("/amm/pools", Some(&Query { page, limit })).await
    }
    
    /// Find best swap path
    pub async fn find_best_swap_path(
        &self,
        request: SwapPathRequest,
    ) -> Result<SwapPathResponse> {
        self.post("/amm/find_best_swap_path", &request).await
    }
    
    /// Find best MEV opportunity
    pub async fn get_best_mev_swap(
        &self,
        request: MevSwapRequest,
    ) -> Result<MevSwapResponse> {
        self.post("/amm/get_best_mev_swap", &request).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTradesParams {
    #[serde(skip)]
    pool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl GetTradesParams {
    fn with_pool(mut self, pool: &str) -> GetTradesParamsWithPool {
        GetTradesParamsWithPool {
            pool: pool.to_string(),
            inner: self,
        }
    }
}

#[derive(Serialize)]
struct GetTradesParamsWithPool {
    pool: String,
    #[serde(flatten)]
    inner: GetTradesParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlesResponse {
    pub ok: bool,
    pub pool: String,
    pub timeframe: String,
    pub side: String,
    pub page: usize,
    pub limit: usize,
    pub total: usize,
    pub has_more: bool,
    pub candles: Vec<Candle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub ts: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradesResponse {
    pub ok: bool,
    pub pool: String,
    pub side: String,
    pub filter_side: String,
    pub sort: String,
    pub dir: String,
    pub page: usize,
    pub limit: usize,
    pub total: usize,
    pub has_more: bool,
    pub trades: Vec<Trade>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub ts: u64,
    pub side: String,
    pub amount_base: String,
    pub amount_quote: String,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolsResponse {
    pub ok: bool,
    pub page: usize,
    pub limit: usize,
    pub total: usize,
    pub has_more: bool,
    pub pools: HashMap<String, PoolInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolInfo {
    pub base: String,
    pub quote: String,
    pub base_reserve: String,
    pub quote_reserve: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapPathRequest {
    pub mode: String, // "exact_in", "exact_out", "implicit"
    pub token_in: String,
    pub token_out: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_in: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_out: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_out_min: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_in_max: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_hops: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapPathResponse {
    pub ok: bool,
    pub mode: String,
    pub token_in: String,
    pub token_out: String,
    pub fee_bps: u32,
    pub max_hops: usize,
    pub amount_in: String,
    pub amount_out: String,
    pub hops: Vec<SwapHop>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapHop {
    pub pool: String,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub amount_out: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MevSwapRequest {
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_hops: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MevSwapResponse {
    pub ok: bool,
    pub token: String,
    pub fee_bps: u32,
    pub max_hops: usize,
    pub amount_in: String,
    pub amount_out: String,
    pub profit: String,
    pub hops: Vec<SwapHop>,
}
```

### Step 2.6: Module Exports

**File**: `crates/alkanes-cli-common/src/dataapi/mod.rs`

```rust
//! Data API client library
//!
//! Provides a Rust client for the alkanes-data-api service.

mod client;
mod balance;
mod storage;
mod amm;

pub use client::DataApiClient;
pub use balance::*;
pub use storage::*;
pub use amm::*;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // Only run with real server
    async fn test_integration() {
        let client = DataApiClient::new("http://localhost:3000").unwrap();
        
        // Test balance query
        let result = client.get_address_balances("bc1q...", false).await;
        println!("{:?}", result);
    }
}
```

**File**: `crates/alkanes-cli-common/src/lib.rs`

```rust
// Add to existing exports
pub mod dataapi;
```

---

## Part 3: CLI Integration (Week 7)

### Location: `crates/alkanes-cli/src/main.rs`

### Objective
Add CLI commands for all data API functionality.

### Step 3.1: Add Subcommands

**File**: `crates/alkanes-cli/src/main.rs`

```rust
#[derive(Subcommand)]
enum Alkanes {
    // ... existing commands ...
    
    /// Balance and holder queries
    #[command(subcommand)]
    Balance(BalanceCommands),
    
    /// Storage queries
    #[command(subcommand)]
    Storage(StorageCommands),
    
    /// AMM/DEX queries
    #[command(subcommand)]
    Amm(AmmCommands),
}

#[derive(Subcommand)]
enum BalanceCommands {
    /// Get balances for an address
    Address {
        address: String,
        #[arg(long)]
        include_outpoints: bool,
        #[arg(long)]
        raw: bool,
    },
    
    /// Get balances for a UTXO
    Outpoint {
        outpoint: String,
        #[arg(long)]
        raw: bool,
    },
    
    /// Get holders for an alkane
    Holders {
        alkane: String,
        #[arg(long, default_value = "1")]
        page: usize,
        #[arg(long, default_value = "100")]
        limit: usize,
        #[arg(long)]
        raw: bool,
    },
    
    /// Get holder count
    HoldersCount {
        alkane: String,
        #[arg(long)]
        raw: bool,
    },
    
    /// Get UTXOs for an address
    AddressOutpoints {
        address: String,
        #[arg(long)]
        raw: bool,
    },
}

#[derive(Subcommand)]
enum StorageCommands {
    /// Get storage keys for a contract
    Keys {
        alkane: String,
        #[arg(long)]
        keys: Option<Vec<String>>,
        #[arg(long, default_value = "1")]
        page: usize,
        #[arg(long, default_value = "100")]
        limit: usize,
        #[arg(long)]
        raw: bool,
    },
}

#[derive(Subcommand)]
enum AmmCommands {
    /// Get OHLCV candles
    Candles {
        pool: String,
        #[arg(long, default_value = "1h")]
        timeframe: String,
        #[arg(long, default_value = "base")]
        side: String,
        #[arg(long, default_value = "1")]
        page: usize,
        #[arg(long, default_value = "120")]
        limit: usize,
        #[arg(long)]
        raw: bool,
    },
    
    /// Get trade history
    Trades {
        pool: String,
        #[arg(long, default_value = "base")]
        side: String,
        #[arg(long, default_value = "all")]
        filter_side: String,
        #[arg(long, default_value = "ts")]
        sort: String,
        #[arg(long, default_value = "desc")]
        dir: String,
        #[arg(long, default_value = "1")]
        page: usize,
        #[arg(long, default_value = "50")]
        limit: usize,
        #[arg(long)]
        raw: bool,
    },
    
    /// Get all pools
    Pools {
        #[arg(long)]
        page: Option<usize>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        raw: bool,
    },
    
    /// Find best swap path
    FindPath {
        #[arg(long, default_value = "exact_in")]
        mode: String,
        #[arg(long)]
        token_in: String,
        #[arg(long)]
        token_out: String,
        #[arg(long)]
        amount_in: Option<String>,
        #[arg(long)]
        amount_out: Option<String>,
        #[arg(long)]
        amount_out_min: Option<String>,
        #[arg(long)]
        amount_in_max: Option<String>,
        #[arg(long)]
        fee_bps: Option<u32>,
        #[arg(long)]
        max_hops: Option<usize>,
        #[arg(long)]
        raw: bool,
    },
    
    /// Find MEV opportunity
    FindMev {
        token: String,
        #[arg(long)]
        fee_bps: Option<u32>,
        #[arg(long)]
        max_hops: Option<usize>,
        #[arg(long)]
        raw: bool,
    },
}
```

### Step 3.2: Implement Handlers

**File**: `crates/alkanes-cli/src/main.rs` (continued)

```rust
async fn handle_balance_commands(
    client: &DataApiClient,
    cmd: BalanceCommands,
) -> Result<()> {
    match cmd {
        BalanceCommands::Address { address, include_outpoints, raw } => {
            let result = client.get_address_balances(&address, include_outpoints).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                print_address_balances(&result);
            }
        }
        BalanceCommands::Outpoint { outpoint, raw } => {
            let result = client.get_outpoint_balances(&outpoint).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                print_outpoint_balances(&result);
            }
        }
        BalanceCommands::Holders { alkane, page, limit, raw } => {
            let result = client.get_holders(&alkane, Some(page), Some(limit)).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                print_holders(&result);
            }
        }
        BalanceCommands::HoldersCount { alkane, raw } => {
            let result = client.get_holders_count(&alkane).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Alkane: {}", result.alkane);
                println!("Holders: {}", result.count);
            }
        }
        BalanceCommands::AddressOutpoints { address, raw } => {
            let result = client.get_address_outpoints(&address).await?;
            if raw {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                print_address_outpoints(&result);
            }
        }
    }
    Ok(())
}

// Pretty print functions
fn print_address_balances(result: &AddressBalancesResponse) {
    println!("Address: {}", result.address);
    println!("\nBalances:");
    for (alkane, amount) in &result.balances {
        println!("  {}: {}", alkane, amount);
    }
    
    if let Some(outpoints) = &result.outpoints {
        println!("\nUTXOs ({}):", outpoints.len());
        for op in outpoints {
            println!("  {}:", op.outpoint);
            for entry in &op.entries {
                println!("    {}: {}", entry.alkane, entry.amount);
            }
        }
    }
}

// ... similar for other print functions ...
```

---

## Part 4: Web (WASM) Integration (Week 8)

### Location: `crates/alkanes-web-sys/src/`

### Objective
Expose data API client to JavaScript/TypeScript via WASM.

### Step 4.1: WASM Wrapper

**File**: `crates/alkanes-web-sys/src/dataapi.rs`

```rust
use wasm_bindgen::prelude::*;
use alkanes_cli_common::dataapi::DataApiClient;
use js_sys::Promise;
use wasm_bindgen_futures::future_to_promise;

#[wasm_bindgen]
pub struct DataApiWasm {
    client: DataApiClient,
}

#[wasm_bindgen]
impl DataApiWasm {
    #[wasm_bindgen(constructor)]
    pub fn new(base_url: String) -> Result<DataApiWasm, JsValue> {
        let client = DataApiClient::new(base_url)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { client })
    }
    
    #[wasm_bindgen(js_name = getAddressBalances)]
    pub fn get_address_balances(
        &self,
        address: String,
        include_outpoints: bool,
    ) -> Promise {
        let client = self.client.clone();
        future_to_promise(async move {
            let result = client.get_address_balances(&address, include_outpoints)
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            
            let js_value = serde_wasm_bindgen::to_value(&result)?;
            Ok(js_value)
        })
    }
    
    #[wasm_bindgen(js_name = getOutpointBalances)]
    pub fn get_outpoint_balances(&self, outpoint: String) -> Promise {
        let client = self.client.clone();
        future_to_promise(async move {
            let result = client.get_outpoint_balances(&outpoint)
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            
            let js_value = serde_wasm_bindgen::to_value(&result)?;
            Ok(js_value)
        })
    }
    
    #[wasm_bindgen(js_name = getHolders)]
    pub fn get_holders(
        &self,
        alkane: String,
        page: Option<usize>,
        limit: Option<usize>,
    ) -> Promise {
        let client = self.client.clone();
        future_to_promise(async move {
            let result = client.get_holders(&alkane, page, limit)
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            
            let js_value = serde_wasm_bindgen::to_value(&result)?;
            Ok(js_value)
        })
    }
    
    // ... similar methods for all other endpoints ...
}
```

---

## Part 5: FFI Bindings (Week 9)

### Location: `crates/alkanes-ffi/src/`

### Objective
Expose data API to C/C++/Swift/Objective-C/etc.

### Step 5.1: FFI Wrapper

**File**: `crates/alkanes-ffi/src/dataapi.rs`

```rust
use alkanes_cli_common::dataapi::DataApiClient;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

#[repr(C)]
pub struct AlkanesDataApiClient {
    client: Box<DataApiClient>,
}

/// Create a new DataAPI client
#[no_mangle]
pub extern "C" fn alkanes_dataapi_client_new(
    base_url: *const c_char,
) -> *mut AlkanesDataApiClient {
    let base_url = unsafe {
        if base_url.is_null() {
            return ptr::null_mut();
        }
        match CStr::from_ptr(base_url).to_str() {
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        }
    };
    
    match DataApiClient::new(base_url) {
        Ok(client) => Box::into_raw(Box::new(AlkanesDataApiClient {
            client: Box::new(client),
        })),
        Err(_) => ptr::null_mut(),
    }
}

/// Free a DataAPI client
#[no_mangle]
pub extern "C" fn alkanes_dataapi_client_free(client: *mut AlkanesDataApiClient) {
    if !client.is_null() {
        unsafe {
            let _ = Box::from_raw(client);
        }
    }
}

/// Get address balances (returns JSON string)
#[no_mangle]
pub extern "C" fn alkanes_dataapi_get_address_balances(
    client: *mut AlkanesDataApiClient,
    address: *const c_char,
    include_outpoints: bool,
) -> *mut c_char {
    let client = unsafe {
        if client.is_null() {
            return ptr::null_mut();
        }
        &(*client).client
    };
    
    let address = unsafe {
        if address.is_null() {
            return ptr::null_mut();
        }
        match CStr::from_ptr(address).to_str() {
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        }
    };
    
    // Run async in blocking context
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return ptr::null_mut(),
    };
    
    let result = runtime.block_on(async {
        client.get_address_balances(address, include_outpoints).await
    });
    
    match result {
        Ok(data) => {
            match serde_json::to_string(&data) {
                Ok(json) => {
                    match CString::new(json) {
                        Ok(cstr) => cstr.into_raw(),
                        Err(_) => ptr::null_mut(),
                    }
                }
                Err(_) => ptr::null_mut(),
            }
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Free a JSON string returned by the API
#[no_mangle]
pub extern "C" fn alkanes_dataapi_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

// ... similar functions for all other endpoints ...
```

**File**: `crates/alkanes-ffi/include/alkanes_dataapi.h`

```c
#ifndef ALKANES_DATAAPI_H
#define ALKANES_DATAAPI_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct AlkanesDataApiClient AlkanesDataApiClient;

/**
 * Create a new DataAPI client
 * @param base_url Base URL of the API server
 * @return Pointer to client or NULL on error
 */
AlkanesDataApiClient* alkanes_dataapi_client_new(const char* base_url);

/**
 * Free a DataAPI client
 * @param client Client to free
 */
void alkanes_dataapi_client_free(AlkanesDataApiClient* client);

/**
 * Get balances for an address
 * @param client DataAPI client
 * @param address Bitcoin address
 * @param include_outpoints Whether to include UTXO breakdown
 * @return JSON string (must be freed with alkanes_dataapi_free_string)
 */
char* alkanes_dataapi_get_address_balances(
    AlkanesDataApiClient* client,
    const char* address,
    bool include_outpoints
);

/**
 * Free a string returned by the API
 * @param s String to free
 */
void alkanes_dataapi_free_string(char* s);

// ... declarations for all other functions ...

#ifdef __cplusplus
}
#endif

#endif // ALKANES_DATAAPI_H
```

---

## Part 6: JNI Bindings (Week 10)

### Location: `crates/alkanes-jni/src/`

### Objective
Expose data API to Java/Kotlin/Android.

### Step 6.1: JNI Wrapper

**File**: `crates/alkanes-jni/src/dataapi.rs`

```rust
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jlong, jstring, jboolean};
use alkanes_cli_common::dataapi::DataApiClient;
use std::sync::Arc;

#[no_mangle]
pub extern "system" fn Java_io_alkanes_DataApiClient_nativeNew(
    env: JNIEnv,
    _class: JClass,
    base_url: JString,
) -> jlong {
    let base_url: String = match env.get_string(base_url) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };
    
    match DataApiClient::new(base_url) {
        Ok(client) => Box::into_raw(Box::new(Arc::new(client))) as jlong,
        Err(_) => 0,
    }
}

#[no_mangle]
pub extern "system" fn Java_io_alkanes_DataApiClient_nativeFree(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        unsafe {
            let _ = Box::from_raw(handle as *mut Arc<DataApiClient>);
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_io_alkanes_DataApiClient_nativeGetAddressBalances(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    address: JString,
    include_outpoints: jboolean,
) -> jstring {
    let client = unsafe {
        if handle == 0 {
            return JObject::null().into_inner();
        }
        &*(handle as *const Arc<DataApiClient>)
    };
    
    let address: String = match env.get_string(address) {
        Ok(s) => s.into(),
        Err(_) => return JObject::null().into_inner(),
    };
    
    let include_outpoints = include_outpoints != 0;
    
    // Run async in blocking context
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return JObject::null().into_inner(),
    };
    
    let result = runtime.block_on(async {
        client.get_address_balances(&address, include_outpoints).await
    });
    
    match result {
        Ok(data) => {
            match serde_json::to_string(&data) {
                Ok(json) => {
                    match env.new_string(json) {
                        Ok(jstr) => jstr.into_inner(),
                        Err(_) => JObject::null().into_inner(),
                    }
                }
                Err(_) => JObject::null().into_inner(),
            }
        }
        Err(_) => JObject::null().into_inner(),
    }
}

// ... similar functions for all other endpoints ...
```

**File**: `java/io/alkanes/DataApiClient.java`

```java
package io.alkanes;

import com.google.gson.Gson;
import com.google.gson.JsonObject;

public class DataApiClient {
    static {
        System.loadLibrary("alkanes_jni");
    }
    
    private long nativeHandle;
    private Gson gson = new Gson();
    
    public DataApiClient(String baseUrl) {
        this.nativeHandle = nativeNew(baseUrl);
        if (this.nativeHandle == 0) {
            throw new RuntimeException("Failed to create DataApiClient");
        }
    }
    
    public AddressBalancesResponse getAddressBalances(String address, boolean includeOutpoints) {
        String json = nativeGetAddressBalances(nativeHandle, address, includeOutpoints);
        if (json == null) {
            throw new RuntimeException("API call failed");
        }
        return gson.fromJson(json, AddressBalancesResponse.class);
    }
    
    // ... public methods for all endpoints ...
    
    @Override
    protected void finalize() {
        if (nativeHandle != 0) {
            nativeFree(nativeHandle);
            nativeHandle = 0;
        }
    }
    
    // Native method declarations
    private static native long nativeNew(String baseUrl);
    private static native void nativeFree(long handle);
    private static native String nativeGetAddressBalances(
        long handle,
        String address,
        boolean includeOutpoints
    );
    // ... native declarations for all other methods ...
}
```

---

## Part 7: TypeScript SDK (Week 11)

### Location: `ts-sdk/`

### Objective
Provide a first-class TypeScript/JavaScript SDK.

### Step 7.1: TypeScript Client

**File**: `ts-sdk/src/DataApiClient.ts`

```typescript
import axios, { AxiosInstance, AxiosRequestConfig } from 'axios';

export interface AddressBalancesResponse {
  ok: boolean;
  address: string;
  balances: Record<string, string>;
  outpoints?: Array<{
    outpoint: string;
    entries: Array<{
      alkane: string;
      amount: string;
    }>;
  }>;
}

export interface OutpointBalancesResponse {
  ok: boolean;
  outpoint: string;
  items: Array<{
    outpoint: string;
    address?: string;
    entries: Array<{
      alkane: string;
      amount: string;
    }>;
  }>;
}

export interface HoldersResponse {
  ok: boolean;
  alkane: string;
  page: number;
  limit: number;
  total: number;
  has_more: boolean;
  items: Array<{
    address: string;
    amount: string;
  }>;
}

export interface HolderCountResponse {
  ok: boolean;
  alkane: string;
  count: number;
}

// ... more response types ...

export class DataApiClient {
  private client: AxiosInstance;
  
  constructor(baseURL: string, config?: AxiosRequestConfig) {
    this.client = axios.create({
      baseURL,
      timeout: 30000,
      ...config,
    });
  }
  
  /**
   * Get all balances for a Bitcoin address
   */
  async getAddressBalances(
    address: string,
    includeOutpoints: boolean = false
  ): Promise<AddressBalancesResponse> {
    const response = await this.client.get('/balance/address', {
      params: { address, include_outpoints: includeOutpoints },
    });
    return response.data;
  }
  
  /**
   * Get balances for a specific UTXO
   */
  async getOutpointBalances(outpoint: string): Promise<OutpointBalancesResponse> {
    const response = await this.client.get('/balance/outpoint', {
      params: { outpoint },
    });
    return response.data;
  }
  
  /**
   * Get holders for an alkane (paginated)
   */
  async getHolders(
    alkane: string,
    page?: number,
    limit?: number
  ): Promise<HoldersResponse> {
    const response = await this.client.get('/balance/holders', {
      params: { alkane, page, limit },
    });
    return response.data;
  }
  
  /**
   * Get total holder count
   */
  async getHoldersCount(alkane: string): Promise<HolderCountResponse> {
    const response = await this.client.get('/balance/holders/count', {
      params: { alkane },
    });
    return response.data;
  }
  
  // ... methods for all other endpoints ...
  
  /**
   * Find best swap path
   */
  async findBestSwapPath(request: SwapPathRequest): Promise<SwapPathResponse> {
    const response = await this.client.post('/amm/find_best_swap_path', request);
    return response.data;
  }
  
  /**
   * Find MEV opportunity
   */
  async getBestMevSwap(request: MevSwapRequest): Promise<MevSwapResponse> {
    const response = await this.client.post('/amm/get_best_mev_swap', request);
    return response.data;
  }
}

// Default export
export default DataApiClient;
```

**File**: `ts-sdk/package.json`

```json
{
  "name": "@alkanes/data-api-client",
  "version": "1.0.0",
  "description": "TypeScript client for Alkanes Data API",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "scripts": {
    "build": "tsc",
    "test": "jest",
    "prepublishOnly": "npm run build"
  },
  "dependencies": {
    "axios": "^1.6.0"
  },
  "devDependencies": {
    "@types/node": "^20.0.0",
    "typescript": "^5.0.0",
    "jest": "^29.0.0"
  }
}
```

---

## Timeline Summary

| Phase | Duration | Deliverables |
|-------|----------|--------------|
| **Backend** (Weeks 1-5) | 5 weeks | 11 API endpoints, indexing system |
| **Rust Client** (Week 6) | 1 week | alkanes-cli-common dataapi module |
| **CLI** (Week 7) | 1 week | CLI commands for all endpoints |
| **Web/WASM** (Week 8) | 1 week | JavaScript bindings |
| **FFI** (Week 9) | 1 week | C/C++/Swift bindings |
| **JNI** (Week 10) | 1 week | Java/Kotlin/Android bindings |
| **TypeScript SDK** (Week 11) | 1 week | npm package |
| **Testing & Docs** (Week 12) | 1 week | Integration tests, documentation |
| **TOTAL** | **12 weeks** | **Complete ecosystem** |

---

## Testing Strategy

### Backend Tests
- Unit tests for extractors
- Integration tests for pipeline
- API endpoint tests
- Performance tests

### Client Library Tests
- Unit tests for each method
- Mock server tests
- Integration tests with real API

### CLI Tests
- Command execution tests
- Output format tests
- Error handling tests

### WASM Tests
- Browser compatibility tests
- Node.js compatibility tests
- Performance tests

### FFI Tests
- C integration tests
- Swift integration tests
- Memory leak tests

### JNI Tests
- Java integration tests
- Android compatibility tests
- Thread safety tests

### TypeScript SDK Tests
- Unit tests
- Integration tests
- Type checking tests

---

## Success Criteria

- ✅ All 11 API endpoints functional
- ✅ <100ms response time for balance queries
- ✅ All client libraries working
- ✅ CLI commands functional
- ✅ WASM working in browser and Node.js
- ✅ FFI working with C/Swift examples
- ✅ JNI working with Java/Android examples
- ✅ TypeScript SDK published to npm
- ✅ Complete API documentation
- ✅ Integration tests passing
- ✅ Example apps in each language

---

## Deployment Checklist

### Backend
- [ ] Database migrations applied
- [ ] Indexer running and syncing
- [ ] API server deployed
- [ ] Load balancer configured
- [ ] Monitoring/alerting setup

### Client Libraries
- [ ] Rust crate published to crates.io
- [ ] CLI binary releases for all platforms
- [ ] WASM package published to npm
- [ ] FFI library builds for all platforms
- [ ] JNI library builds for all platforms
- [ ] TypeScript SDK published to npm

### Documentation
- [ ] API reference docs complete
- [ ] Client library docs complete
- [ ] Example apps for each platform
- [ ] Migration guide from other APIs
- [ ] Performance tuning guide

---

## Next Steps

1. ✅ Review this comprehensive plan
2. Begin Phase 1: Backend implementation
3. Progress through each phase sequentially
4. Test thoroughly at each stage
5. Document as you go
6. Deploy progressively (backend first, then clients)

This plan delivers a **complete, production-ready data API ecosystem** accessible from every major programming language and platform!
