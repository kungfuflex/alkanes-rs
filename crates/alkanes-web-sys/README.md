# deezel-web

Web-compatible implementation of deezel-common traits using web-sys APIs for browser environments, with comprehensive browser wallet integration.

## Overview

`deezel-web` provides web-compatible implementations of all deezel-common traits, enabling deezel functionality to run in web browsers and WASM environments. It includes a comprehensive browser wallet provider system that wraps injected wallet extensions while leveraging our sandshrew RPC infrastructure.

## Features

- **Complete trait implementation**: Implements all deezel-common provider traits
- **Browser wallet integration**: Works with 13+ Bitcoin wallet extensions (Unisat, Xverse, Phantom, OKX, etc.)
- **Minimal wallet usage**: Wallets used only for signing, blockchain operations use our RPC
- **Web-standard APIs**: Uses fetch API, localStorage, Web Crypto API, etc.
- **WASM optimization**: Optimized for WebAssembly execution
- **Cross-platform compatibility**: Works in all modern web browsers
- **Rebar Labs Shield support**: Private transaction broadcasting for mainnet
- **Enhanced privacy**: Reduced wallet API dependencies, local UTXO management

## Architecture

The library provides two main provider types:

### WebProvider
Basic web-compatible provider using browser APIs:
- `JsonRpcProvider`: Uses fetch API for HTTP requests
- `StorageProvider`: Uses localStorage for persistent storage
- `NetworkProvider`: Uses fetch API for general HTTP operations
- `CryptoProvider`: Uses Web Crypto API for cryptographic operations
- `TimeProvider`: Uses Performance API for timing
- `LogProvider`: Uses console API for logging
- `WalletProvider`: Browser-compatible wallet operations

### BrowserWalletProvider
Advanced provider that wraps injected browser wallets:
- **Wallet detection**: Automatically detects available wallet extensions
- **Minimal wallet usage**: Only uses wallets for signing and key operations
- **Sandshrew integration**: All blockchain operations use our RPC infrastructure
- **Multi-wallet support**: Works with any injected Bitcoin wallet
- **PSBT signing**: Full support for Partially Signed Bitcoin Transactions

## Supported Wallets

| Wallet | PSBT | Taproot | Ordinals | Mobile | Deep Link |
|--------|------|---------|----------|--------|-----------|
| Unisat | ✅ | ✅ | ✅ | ❌ | ❌ |
| Xverse | ✅ | ✅ | ✅ | ✅ | `xverse://` |
| Phantom | ✅ | ✅ | ❌ | ✅ | `phantom://` |
| OKX | ✅ | ✅ | ✅ | ✅ | `okx://` |
| Leather | ✅ | ✅ | ✅ | ❌ | ❌ |
| Magic Eden | ✅ | ✅ | ✅ | ✅ | `magiceden://` |

*Additional wallets can be easily added by extending the supported wallets list.*

## Usage

### Basic Web Provider

```rust
use deezel_web::WebProvider;
use deezel_common::*;

async fn example() -> Result<()> {
    // Create a web provider instance
    let provider = WebProvider::new(
        "http://localhost:8332".to_string(),
        "http://localhost:8080".to_string(),
        "regtest".to_string(),
    ).await?;

    // Initialize the provider
    provider.initialize().await?;

    // Use any deezel-common functionality
    let balance = WalletProvider::get_balance(&provider).await?;
    println!("Balance: {} sats", balance.confirmed);

    Ok(())
}
```

### Browser Wallet Integration

```rust
use deezel_web::wallet_provider::*;
use deezel_common::*;

async fn connect_wallet() -> Result<BrowserWalletProvider> {
    // Detect available wallets
    let connector = WalletConnector::new();
    let available_wallets = connector.detect_wallets().await?;
    
    if let Some(wallet_info) = available_wallets.first() {
        // Connect to the first available wallet
        let provider = BrowserWalletProvider::connect(
            wallet_info.clone(),
            "http://localhost:8332".to_string(),
            "http://localhost:8080".to_string(),
            "mainnet".to_string(),
        ).await?;
        
        // Initialize the provider
        provider.initialize().await?;
        
        Ok(provider)
    } else {
        Err(DeezelError::Wallet("No wallets detected".to_string()))
    }
}

async fn use_wallet_provider(provider: &BrowserWalletProvider) -> Result<()> {
    // Get balance using our sandshrew RPC (not wallet's limited API)
    let balance = WalletProvider::get_balance(provider).await?;
    println!("Balance: {} sats", balance.confirmed);
    
    // Get UTXOs using our Esplora provider
    let utxos = WalletProvider::get_utxos(provider, false, None).await?;
    println!("Found {} UTXOs", utxos.len());
    
    // Execute alkanes contracts
    let execute_params = AlkanesExecuteParams {
        inputs: "auto".to_string(),
        to: "bc1q...".to_string(),
        protostones: "contract_call_data".to_string(),
        // ... other params
    };
    
    let result = AlkanesProvider::execute(provider, execute_params).await?;
    println!("Contract executed: {}", result.reveal_txid);
    
    Ok(())
}
```

### Leptos Integration

```rust
use leptos::*;
use deezel_web::wallet_provider::*;

#[component]
pub fn WalletConnector() -> impl IntoView {
    let (wallet_state, set_wallet_state) = create_signal(None::<BrowserWalletProvider>);
    let (available_wallets, set_available_wallets) = create_signal(Vec::<WalletInfo>::new());
    
    // Detect wallets on component mount
    create_effect(move |_| {
        spawn_local(async move {
            let connector = WalletConnector::new();
            if let Ok(wallets) = connector.detect_wallets().await {
                set_available_wallets.set(wallets);
            }
        });
    });
    
    let connect_wallet = move |wallet_info: WalletInfo| {
        spawn_local(async move {
            if let Ok(provider) = BrowserWalletProvider::connect(
                wallet_info,
                "http://localhost:8332".to_string(),
                "http://localhost:8080".to_string(),
                "mainnet".to_string(),
            ).await {
                set_wallet_state.set(Some(provider));
            }
        });
    };
    
    view! {
        <div class="wallet-connector">
            <h3>"Available Wallets"</h3>
            <For
                each=move || available_wallets.get()
                key=|wallet| wallet.id.clone()
                children=move |wallet| {
                    let wallet_clone = wallet.clone();
                    view! {
                        <button
                            on:click=move |_| connect_wallet(wallet_clone.clone())
                            class="wallet-button"
                        >
                            <img src=wallet.icon alt=wallet.name.clone() />
                            {wallet.name}
                        </button>
                    }
                }
            />
        </div>
    }
}
```

### WASM Integration

```rust
use wasm_bindgen::prelude::*;
use deezel_web::prelude::*;

#[wasm_bindgen]
pub async fn run_browser_wallet_example() -> Result<(), JsValue> {
    // Detect and connect to wallets
    let connector = WalletConnector::new();
    let available_wallets = connector.detect_wallets().await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    
    if let Some(wallet_info) = available_wallets.first() {
        let provider = BrowserWalletProvider::connect(
            wallet_info.clone(),
            "http://localhost:8332".to_string(),
            "http://localhost:8080".to_string(),
            "mainnet".to_string(),
        ).await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        // Use provider for Bitcoin operations
        let address = WalletProvider::get_address(&provider).await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        web_sys::console::log_1(&format!("Connected to wallet: {}", address).into());
    }
    
    Ok(())
}
```

## Building for Web

### Prerequisites

```bash
# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Install a local HTTP server (for testing)
npm install -g http-server
```

### Build Commands

```bash
# Build for web target
wasm-pack build --target web --out-dir pkg crates/deezel-web

# Build for bundler target (webpack, etc.)
wasm-pack build --target bundler --out-dir pkg crates/deezel-web

# Build for Node.js
wasm-pack build --target nodejs --out-dir pkg crates/deezel-web
```

### HTML Integration

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Deezel Web App</title>
</head>
<body>
    <script type="module">
        import init, { run_browser_wallet_example } from './pkg/deezel_web.js';
        
        async function run() {
            await init();
            await run_browser_wallet_example();
        }
        
        run();
    </script>
</body>
</html>
```

## Key Benefits

### 1. Minimal Wallet Usage
- **Signing only**: Wallets used only for transaction and PSBT signing
- **Key operations**: Public key retrieval and account management
- **Blockchain operations**: All handled by our sandshrew RPC infrastructure

### 2. Enhanced Privacy
- **Rebar Labs Shield**: Private transaction broadcasting for mainnet
- **Reduced API calls**: Less data leakage to wallet providers
- **Local UTXO management**: Better privacy than wallet-based tracking

### 3. Superior Performance
- **Sandshrew RPC**: Faster and more reliable than wallet APIs
- **Comprehensive data**: Includes ordinals, runes, and alkanes information
- **Advanced features**: Better fee estimation and UTXO selection

### 4. Full Compatibility
- **All deezel-common traits**: Complete compatibility with existing code
- **Seamless integration**: Drop-in replacement for other providers
- **Consistent API**: Same interface regardless of wallet backend

## API Reference

### WebProvider

```rust
impl WebProvider {
    pub async fn new(
        bitcoin_rpc_url: String,
        metashrew_rpc_url: String,
        network_str: String,
    ) -> Result<Self>;
    
    pub fn get_wallet_config(&self) -> WalletConfig;
    pub fn network(&self) -> Network;
    pub fn bitcoin_rpc_url(&self) -> &str;
    pub fn metashrew_rpc_url(&self) -> &str;
    pub async fn broadcast_via_rebar_shield(&self, tx_hex: &str) -> Result<String>;
}
```

### BrowserWalletProvider

```rust
impl BrowserWalletProvider {
    pub async fn connect(
        wallet_info: WalletInfo,
        bitcoin_rpc_url: String,
        metashrew_rpc_url: String,
        network_str: String,
    ) -> Result<Self>;
    
    pub fn connection_status(&self) -> &WalletConnectionStatus;
    pub fn current_account(&self) -> Option<&WalletAccount>;
    pub fn wallet_info(&self) -> &WalletInfo;
    pub async fn disconnect(&mut self) -> Result<()>;
    pub async fn switch_network(&mut self, network: &str) -> Result<()>;
    pub fn web_provider(&self) -> &WebProvider;
}
```

### WalletConnector

```rust
impl WalletConnector {
    pub fn new() -> Self;
    pub async fn detect_wallets(&self) -> Result<Vec<WalletInfo>>;
    pub fn get_wallet_info(&self, wallet_id: &str) -> Option<&WalletInfo>;
    pub fn create_injected_wallet(&self, wallet_info: WalletInfo) -> Result<InjectedWallet>;
}
```

## Examples

See the `examples/` directory for complete examples:

- `web_example.rs`: Basic web provider usage
- `browser_wallet_example.rs`: Browser wallet integration examples
- `wallet_demo.html`: Interactive HTML demo with wallet detection

## Testing

Run tests in a browser environment:

```bash
wasm-pack test --headless --firefox crates/deezel-web
wasm-pack test --headless --chrome crates/deezel-web
```

Test with specific features:

```bash
# Test wallet provider functionality
wasm-pack test --headless --firefox crates/deezel-web -- --test wallet_provider_tests
```

## Browser Compatibility

- **Chrome/Chromium**: Full support
- **Firefox**: Full support  
- **Safari**: Full support (iOS 14+)
- **Edge**: Full support

## Limitations

- No access to file system (uses localStorage instead)
- Network requests subject to CORS policies
- Some cryptographic operations may be slower than native
- Limited to browser security model
- Wallet extensions must be installed and enabled

## Documentation

For comprehensive documentation on the browser wallet provider system, see:
- [`docs/browser-wallet-provider-system.md`](../../docs/browser-wallet-provider-system.md)

## Contributing

1. Ensure all tests pass: `wasm-pack test --headless --firefox`
2. Test in multiple browsers
3. Test with actual wallet extensions
4. Update documentation for any API changes
5. Follow the existing code style

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.