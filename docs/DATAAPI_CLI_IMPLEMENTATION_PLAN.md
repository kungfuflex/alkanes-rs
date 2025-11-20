# DataAPI CLI & AMM Operations Implementation Plan

## Overview

Implement comprehensive CLI commands for:
1. Data API client integration (`alkanes-cli dataapi`)
2. AMM operations (init-pool, swap, wrap-btc)
3. Deploy script for regtest diesel pool

## Architecture

```
alkanes-cli-common (core logic)
    ↓
alkanes-cli-sys (system integration)
    ↓
┌─────────────────┬──────────────────┐
│   alkanes-cli   │  alkanes-web-sys │
│  (native CLI)   │   (WASM/web)     │
└─────────────────┴──────────────────┘
```

---

## Part 1: DataAPI Client (`alkanes-cli dataapi`)

### Location: `crates/alkanes-cli-common/src/dataapi/`

### Files to Create:

#### 1. `dataapi/mod.rs`
```rust
pub mod client;
pub mod types;
pub mod commands;

pub use client::DataApiClient;
pub use types::*;
```

#### 2. `dataapi/types.rs`
Mirror types from alkanes-data-api:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlkaneToken {
    pub id: AlkaneId,
    pub name: Option<String>,
    pub symbol: Option<String>,
    // ... match alkanes-data-api types
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pool {
    pub id: String,
    pub factory_block_id: String,
    // ... match alkanes-data-api types
}

// All response types from API
```

#### 3. `dataapi/client.rs`
```rust
pub struct DataApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl DataApiClient {
    pub fn new(base_url: String) -> Self;
    
    // Alkanes endpoints
    pub async fn get_alkanes(&self, params: GetAlkanesParams) -> Result<AlkanesResponse>;
    pub async fn get_alkanes_by_address(&self, address: &str) -> Result<Vec<AlkaneToken>>;
    pub async fn get_alkane_details(&self, id: &AlkaneId) -> Result<AlkaneToken>;
    
    // Pool endpoints
    pub async fn get_pools(&self, factory_id: &AlkaneId) -> Result<Vec<Pool>>;
    pub async fn get_pool_by_id(&self, pool_id: &AlkaneId) -> Result<Option<Pool>>;
    pub async fn get_pool_history(&self, params: PoolHistoryParams) -> Result<HistoryResponse>;
    
    // History endpoints
    pub async fn get_all_history(&self, params: HistoryParams) -> Result<HistoryResponse>;
    pub async fn get_swap_history(&self, params: SwapHistoryParams) -> Result<SwapHistoryResponse>;
    pub async fn get_mint_history(&self, params: MintHistoryParams) -> Result<MintHistoryResponse>;
    pub async fn get_burn_history(&self, params: BurnHistoryParams) -> Result<BurnHistoryResponse>;
    
    // Price endpoints
    pub async fn get_bitcoin_price(&self) -> Result<BitcoinPrice>;
    pub async fn get_bitcoin_market_chart(&self, days: u32) -> Result<MarketChart>;
    
    // Health
    pub async fn health(&self) -> Result<()>;
}
```

#### 4. `dataapi/commands.rs`
```rust
pub async fn execute_dataapi_command<T: DeezelProvider>(
    provider: &T,
    command: DataApiCommand,
    base_url: String,
) -> Result<()> {
    let client = DataApiClient::new(base_url);
    
    match command {
        DataApiCommand::GetAlkanes { limit, offset, sort_by, order, search } => {
            let response = client.get_alkanes(GetAlkanesParams {
                limit, offset, sort_by, order, search_query: search
            }).await?;
            print_alkanes_table(&response);
        }
        DataApiCommand::GetAlkanesByAddress { address } => {
            let alkanes = client.get_alkanes_by_address(&address).await?;
            print_alkanes_tree(&alkanes);
        }
        // ... all other commands
    }
    Ok(())
}
```

---

## Part 2: AMM Operations

### Location: `crates/alkanes-cli-common/src/alkanes/amm.rs` (already exists)

### Enhancements Needed:

#### 1. Init Pool (`init-pool`)

```rust
pub struct InitPoolParams {
    pub pair: (AlkaneId, AlkaneId),           // e.g., 2:0,32:0
    pub liquidity: (u128, u128),               // e.g., 300000000:50000
    pub to: AddressIdentifier,                 // p2tr:0
    pub from: AddressIdentifier,               // p2tr:0
    pub change: Option<AddressIdentifier>,     // default to from
    pub minimum: Option<u128>,                 // minimum LP tokens
    pub fee_rate: Option<f64>,
    pub trace: bool,
}

pub async fn init_pool<T: DeezelProvider>(
    provider: &T,
    params: InitPoolParams,
) -> Result<String> {
    // 1. Get factory ID from provider config
    let factory_id = provider.get_factory_id()?;
    
    // 2. Find UTXOs with required token amounts for both tokens
    let (token0_utxos, token1_utxos) = find_token_utxos(
        provider,
        &params.pair.0,
        params.liquidity.0,
        &params.pair.1,
        params.liquidity.1,
    ).await?;
    
    // 3. Calculate minimum LP tokens if not provided
    let minimum_lp = params.minimum.unwrap_or_else(|| {
        // sqrt(amount0 * amount1) - MINIMUM_LIQUIDITY (1000)
        let product = params.liquidity.0 * params.liquidity.1;
        (product as f64).sqrt() as u128 - 1000
    });
    
    // 4. Build alkanes execute call
    // Format: [factory, 0, <token0Block>, <token0Tx>, <token1Block>, <token1Tx>]:amount0:amount1:minimumLp
    let calldata = format!(
        "[{},{},0,{},{},{},{}]:{}:{}:{}",
        factory_id.block, factory_id.tx,
        params.pair.0.block, params.pair.0.tx,
        params.pair.1.block, params.pair.1.tx,
        params.liquidity.0,
        params.liquidity.1,
        minimum_lp
    );
    
    // 5. Execute transaction
    let execute_params = ExecuteParams {
        calldata,
        to: params.to,
        from: params.from,
        change: params.change,
        fee_rate: params.fee_rate,
        auto_confirm: false,
    };
    
    let txid = execute_alkanes(provider, execute_params).await?;
    
    // 6. If trace flag, wait for confirmation and print trace
    if params.trace {
        wait_for_confirmation(provider, &txid).await?;
        let trace = provider.alkanes_trace(&txid).await?;
        print_trace(&trace);
    }
    
    Ok(txid)
}
```

#### 2. Swap (`swap`)

```rust
pub struct SwapParams {
    pub path: Vec<AlkaneId>,                   // e.g., [2:0, 32:0]
    pub input: u128,                           // input amount
    pub minimum: u128,                         // minimum output (default 1)
    pub expires: Option<u64>,                  // expiry block (default current + 10000)
    pub to: AddressIdentifier,
    pub from: AddressIdentifier,
    pub change: Option<AddressIdentifier>,
    pub fee_rate: Option<f64>,
    pub trace: bool,
}

pub async fn swap<T: DeezelProvider>(
    provider: &T,
    params: SwapParams,
) -> Result<String> {
    if params.path.len() < 2 {
        return Err(anyhow::anyhow!("Path must have at least 2 tokens"));
    }
    
    // 1. Get current height for expiry
    let current_height = provider.get_height().await?;
    let expires = params.expires.unwrap_or(current_height + 10000);
    
    // 2. Get factory ID
    let factory_id = provider.get_factory_id()?;
    
    // 3. Find UTXOs with input token
    let input_token = &params.path[0];
    let utxos = find_token_utxos_for_amount(
        provider,
        input_token,
        params.input,
    ).await?;
    
    // 4. Build swap calldata
    // Format for single hop: [factory, 1, <pool>, <tokenIn>, <tokenOut>]:amount:minimum:expires
    // For multi-hop: chain multiple swaps
    let calldata = if params.path.len() == 2 {
        // Single hop
        format!(
            "[{},{},1]:{}:{}:{}",
            factory_id.block, factory_id.tx,
            params.input,
            params.minimum,
            expires
        )
    } else {
        // Multi-hop (implement routing)
        build_multihop_swap_calldata(factory_id, &params.path, params.input, params.minimum, expires)?
    };
    
    // 5. Execute transaction
    let execute_params = ExecuteParams {
        calldata,
        to: params.to,
        from: params.from,
        change: params.change,
        fee_rate: params.fee_rate,
        auto_confirm: false,
    };
    
    let txid = execute_alkanes(provider, execute_params).await?;
    
    // 6. If trace flag, show trace
    if params.trace {
        wait_for_confirmation(provider, &txid).await?;
        let trace = provider.alkanes_trace(&txid).await?;
        print_trace(&trace);
    }
    
    Ok(txid)
}
```

#### 3. Wrap BTC Enhancement

Already exists at `crates/alkanes-cli-common/src/alkanes/wrap_btc.rs`

Add `--trace` flag support:
```rust
pub struct WrapBtcParams {
    // ... existing fields
    pub trace: bool,
}

// In wrap_btc function, after transaction:
if params.trace {
    wait_for_confirmation(provider, &txid).await?;
    let trace = provider.alkanes_trace(&txid).await?;
    print_trace(&trace);
}
```

---

## Part 3: CLI Commands

### Location: `crates/alkanes-cli/src/commands.rs`

Add new subcommands:

```rust
#[derive(Subcommand)]
pub enum AlkanesCommands {
    // ... existing commands
    
    /// Query data from alkanes-data-api
    Dataapi {
        #[command(subcommand)]
        command: DataApiCommand,
        
        /// Data API base URL
        #[arg(long, env = "ALKANES_DATA_API_URL", default_value = "http://localhost:4000")]
        api_url: String,
    },
    
    /// Initialize a liquidity pool
    InitPool {
        /// Token pair in format: BLOCK:TX,BLOCK:TX (e.g., 2:0,32:0)
        #[arg(long)]
        pair: String,
        
        /// Initial liquidity amounts in format: AMOUNT0:AMOUNT1 (e.g., 300000000:50000)
        #[arg(long)]
        liquidity: String,
        
        /// Recipient address identifier (e.g., p2tr:0)
        #[arg(long)]
        to: String,
        
        /// Sender address identifier (e.g., p2tr:0)
        #[arg(long)]
        from: String,
        
        /// Change address identifier (defaults to --from)
        #[arg(long)]
        change: Option<String>,
        
        /// Minimum LP tokens to receive (optional)
        #[arg(long)]
        minimum: Option<u128>,
        
        /// Fee rate in sat/vB (optional)
        #[arg(long)]
        fee_rate: Option<f64>,
        
        /// Show trace after transaction confirms
        #[arg(long)]
        trace: bool,
    },
    
    /// Execute a swap on the AMM
    Swap {
        /// Swap path in format: BLOCK:TX:BLOCK:TX (e.g., 2:0:32:0 for DIESEL->frBTC)
        #[arg(long)]
        path: String,
        
        /// Input token amount
        #[arg(long)]
        input: u128,
        
        /// Minimum output amount (default: 1)
        #[arg(long, default_value = "1")]
        minimum: u128,
        
        /// Expiry block height (default: current + 10000)
        #[arg(long)]
        expires: Option<u64>,
        
        /// Recipient address identifier
        #[arg(long)]
        to: String,
        
        /// Sender address identifier
        #[arg(long)]
        from: String,
        
        /// Change address identifier (defaults to --from)
        #[arg(long)]
        change: Option<String>,
        
        /// Fee rate in sat/vB (optional)
        #[arg(long)]
        fee_rate: Option<f64>,
        
        /// Show trace after transaction confirms
        #[arg(long)]
        trace: bool,
    },
    
    /// Wrap BTC into frBTC (Subfrost)
    WrapBtc {
        /// Amount in BTC (e.g., 1.0, 0.5)
        #[arg(long)]
        amount: String,
        
        /// Recipient address identifier
        #[arg(long)]
        to: String,
        
        /// Sender address identifier
        #[arg(long)]
        from: String,
        
        /// Change address identifier (defaults to --from)
        #[arg(long)]
        change: Option<String>,
        
        /// Fee rate in sat/vB (optional)
        #[arg(long)]
        fee_rate: Option<f64>,
        
        /// Show trace after transaction confirms
        #[arg(long)]
        trace: bool,
    },
}

#[derive(Subcommand)]
pub enum DataApiCommand {
    /// Get all alkanes
    GetAlkanes {
        #[arg(long)]
        limit: Option<i32>,
        #[arg(long)]
        offset: Option<i32>,
        #[arg(long)]
        sort_by: Option<String>,
        #[arg(long)]
        order: Option<String>,
        #[arg(long)]
        search: Option<String>,
    },
    
    /// Get alkanes for an address
    GetAlkanesByAddress {
        address: String,
    },
    
    /// Get alkane details
    GetAlkaneDetails {
        /// Alkane ID in format BLOCK:TX (e.g., 2:0)
        id: String,
    },
    
    /// Get all pools
    GetPools {
        /// Factory ID in format BLOCK:TX (e.g., 4:65522)
        #[arg(long)]
        factory: String,
    },
    
    /// Get pool details
    GetPoolById {
        /// Pool ID in format BLOCK:TX
        id: String,
    },
    
    /// Get pool history
    GetPoolHistory {
        /// Pool ID in format BLOCK:TX
        pool_id: String,
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        limit: Option<i32>,
        #[arg(long)]
        offset: Option<i32>,
    },
    
    /// Get swap history
    GetSwapHistory {
        #[arg(long)]
        pool_id: Option<String>,
        #[arg(long)]
        limit: Option<i32>,
        #[arg(long)]
        offset: Option<i32>,
    },
    
    /// Get Bitcoin price
    GetBitcoinPrice,
    
    /// Get Bitcoin market chart
    GetMarketChart {
        /// Number of days (1, 7, 14, 30, 90, 180, 365, max)
        days: String,
    },
    
    /// Health check
    Health,
}
```

---

## Part 4: Deploy Script

### Location: `scripts/deploy-regtest-diesel-pool.sh`

```bash
#!/bin/bash
set -e

echo "🚀 Deploying Regtest DIESEL/frBTC Pool"
echo "========================================"

# Configuration
FACTORY="4:65522"
DIESEL_ID="2:0"
FRBTC_ID="32:0"
DIESEL_AMOUNT="300000000"  # 300M DIESEL
FRBTC_AMOUNT="50000"       # 0.0005 BTC in sats
ADDR="p2tr:0"

# Step 1: Mine DIESEL
echo ""
echo "📦 Step 1: Mining DIESEL tokens..."
alkanes-cli alkanes execute "[2,0,77]:v0:v0" \
    --to $ADDR \
    --from $ADDR \
    --change $ADDR \
    --auto-confirm

echo "✅ DIESEL mined"

# Step 2: Wrap BTC for frBTC
echo ""
echo "🔄 Step 2: Wrapping BTC to frBTC..."
alkanes-cli alkanes wrap-btc \
    --amount 1.0 \
    --to $ADDR \
    --from $ADDR \
    --change $ADDR

echo "✅ frBTC wrapped"

# Wait for confirmations
echo ""
echo "⏳ Waiting for confirmations..."
sleep 5

# Step 3: Create the pool
echo ""
echo "🏊 Step 3: Creating DIESEL/frBTC pool..."
alkanes-cli alkanes init-pool \
    --pair "$DIESEL_ID,$FRBTC_ID" \
    --liquidity "$DIESEL_AMOUNT:$FRBTC_AMOUNT" \
    --to $ADDR \
    --from $ADDR \
    --change $ADDR \
    --trace

echo ""
echo "✅ Pool created successfully!"
echo ""
echo "🎉 Deployment complete!"
echo ""
echo "You can now query the pool:"
echo "  alkanes-cli alkanes dataapi get-pools --factory $FACTORY"
```

---

## Part 5: Pretty Printing

### Location: `crates/alkanes-cli/src/pretty_print.rs`

Add functions for new types:

```rust
pub fn print_alkanes_table(response: &AlkanesResponse) {
    let mut table = Table::new();
    table.set_header(vec!["ID", "Symbol", "Name", "Balance", "Price USD"]);
    
    for token in &response.data.tokens {
        table.add_row(vec![
            format!("{}:{}", token.id.block, token.id.tx),
            token.symbol.as_deref().unwrap_or(""),
            token.name.as_deref().unwrap_or(""),
            token.balance.as_deref().unwrap_or("0"),
            token.price_usd.map(|p| format!("${:.2}", p)).unwrap_or_default(),
        ]);
    }
    
    println!("{}", table);
    println!("\n📊 Total: {} tokens", response.data.total);
}

pub fn print_pools_table(pools: &[Pool]) {
    let mut table = Table::new();
    table.set_header(vec!["Pool ID", "Token0", "Token1", "Reserves", "TVL"]);
    
    for pool in pools {
        table.add_row(vec![
            format!("{}:{}", pool.pool_block_id, pool.pool_tx_id),
            format!("{}:{}", pool.token0_block_id, pool.token0_tx_id),
            format!("{}:{}", pool.token1_block_id, pool.token1_tx_id),
            format!("{} / {}", pool.token0_amount.as_deref().unwrap_or("0"), 
                              pool.token1_amount.as_deref().unwrap_or("0")),
            "TBD", // Calculate from reserves + prices
        ]);
    }
    
    println!("{}", table);
}

pub fn print_swap_result(txid: &str, trace: Option<&TraceResult>) {
    println!("🔄 Swap executed!");
    println!("📝 Transaction: {}", txid);
    
    if let Some(t) = trace {
        println!("\n📊 Trace:");
        print_trace(t);
    }
}
```

---

## Implementation Order

1. **Phase 1: DataAPI Client** (alkanes-cli-common)
   - Create `dataapi/types.rs` with all response types
   - Create `dataapi/client.rs` with HTTP client
   - Create `dataapi/commands.rs` with command execution

2. **Phase 2: AMM Operations** (alkanes-cli-common)
   - Implement `init_pool()` in `alkanes/amm.rs`
   - Implement `swap()` in `alkanes/amm.rs`
   - Add `--trace` support to `wrap_btc()`

3. **Phase 3: CLI Integration** (alkanes-cli)
   - Add `DataApiCommand` enum to `commands.rs`
   - Add `InitPool`, `Swap` commands to `AlkanesCommands`
   - Update `execute_alkanes_command()` handler

4. **Phase 4: Web/Sys Integration**
   - Update `alkanes-cli-sys` with new traits/methods
   - Update `alkanes-web-sys` with WASM bindings

5. **Phase 5: Testing**
   - Create `deploy-regtest-diesel-pool.sh`
   - Test all commands end-to-end
   - Document usage

---

## Dependencies to Add

### `alkanes-cli-common/Cargo.toml`
```toml
[dependencies]
reqwest = { version = "0.11", features = ["json"], optional = true }

[features]
std = ["reqwest"]
```

### `alkanes-data-api/Cargo.toml`
Consider extracting types to a shared crate:
```toml
[dependencies]
alkanes-types = { path = "../alkanes-types" }  # shared types
```

---

## Testing Plan

```bash
# 1. Start services
docker-compose up -d

# 2. Test DataAPI commands
alkanes-cli alkanes dataapi health
alkanes-cli alkanes dataapi get-bitcoin-price
alkanes-cli alkanes dataapi get-pools --factory 4:65522

# 3. Test AMM operations
./scripts/deploy-regtest-diesel-pool.sh

# 4. Test swap
alkanes-cli alkanes swap \
    --path 2:0:32:0 \
    --input 1000000 \
    --minimum 100 \
    --to p2tr:0 \
    --from p2tr:0 \
    --trace

# 5. Verify with DataAPI
alkanes-cli alkanes dataapi get-swap-history --limit 10
```

---

## Next Steps

Due to token/time constraints, this plan provides the complete architecture. The implementation would proceed phase by phase as outlined above. Would you like me to start implementing any specific phase?
