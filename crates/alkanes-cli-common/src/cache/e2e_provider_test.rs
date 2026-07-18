//! End-to-end test for cache integration in [`crate::provider::ConcreteProvider`].
//!
//! We can't easily spin up a mock metashrew server in-process here, but we
//! can still prove the cache path is wired by:
//!
//! 1. Constructing a `ConcreteProvider` pointed at an *invalid* URL.
//! 2. Attaching an `InMemoryCache` via `with_cache`.
//! 3. Pre-populating the cache for a `getbytecode` call.
//! 4. Calling `metashrew_view_call("getbytecode", …)` and asserting it
//!    returns the cached bytes *without* attempting to hit the network.
//!
//! If the cache path isn't wired correctly, the call will fail with a
//! connection error trying to reach the bogus URL. A cache HIT short-
//! circuits the HTTP call.

#![cfg(all(test, feature = "std"))]

use std::sync::Arc;

use crate::cache::{
    in_memory::InMemoryCache, integration::make_key, AlkanesCache, CacheScope,
};
use crate::commands::Commands;
use crate::network::RpcConfig;
use crate::provider::ConcreteProvider;

fn rpc_config_with_bogus_url() -> RpcConfig {
    RpcConfig {
        provider: "mainnet".to_string(),
        bitcoin_rpc_url: None,
        jsonrpc_url: None,
        titan_api_url: None,
        esplora_url: None,
        ord_url: None,
        // Port 1 is "tcpmux" — guaranteed-unused on dev boxes. Any HTTP
        // request to it should fail fast with ECONNREFUSED.
        metashrew_rpc_url: Some("http://127.0.0.1:1/jsonrpc".to_string()),
        brc20_prog_rpc_url: None,
        data_api_url: None,
        espo_rpc_url: None,
        qubitcoin_rpc_url: None,
        quzec_rpc_url: None,
        subfrost_api_key: None,
        timeout_seconds: 2,
        jsonrpc_headers: Vec::new(),
    }
}

fn dummy_command() -> Commands {
    Commands::Wallet {
        command: crate::commands::WalletCommands::Info,
    }
}

#[tokio::test]
async fn cache_hit_short_circuits_network_call_for_getbytecode() {
    let cache: Arc<dyn AlkanesCache> = Arc::new(InMemoryCache::new());

    // Pre-populate the cache with the exact key shape that
    // `cached_view_call` will look up: namespace="getbytecode",
    // network="mainnet", suffix=params_hex.as_bytes(), scope=Immutable.
    let params_hex = "0a090a0208041203088004";
    let key = make_key("getbytecode", "mainnet", params_hex);
    cache
        .put(&key, &CacheScope::Immutable, b"PRE_CACHED_WASM".to_vec(), None)
        .await
        .unwrap();

    let provider = ConcreteProvider::new_for_test(rpc_config_with_bogus_url(), dummy_command())
        .with_cache(cache.clone());

    // If the cache is wired through `metashrew_view_call`, this call returns
    // the cached bytes without touching the network. If it's NOT wired,
    // we'll get a connection error trying to reach 127.0.0.1:1.
    let result = provider
        .metashrew_view_call("getbytecode", params_hex, "latest")
        .await
        .expect("cache HIT should short-circuit the network call");

    assert_eq!(result, b"PRE_CACHED_WASM");
}

#[tokio::test]
async fn cache_miss_attempts_network_call_then_fails() {
    // Sanity-check the opposite direction: with no cache entry, the call
    // SHOULD attempt the network and fail. Confirms our test setup actually
    // exercises the network path when expected.
    let cache: Arc<dyn AlkanesCache> = Arc::new(InMemoryCache::new());
    let provider = ConcreteProvider::new_for_test(rpc_config_with_bogus_url(), dummy_command())
        .with_cache(cache.clone());

    let params_hex = "0a090a0208041203088004";
    let result = provider
        .metashrew_view_call("getbytecode", params_hex, "latest")
        .await;

    assert!(
        result.is_err(),
        "expected a network error on cache miss with bogus URL, got Ok"
    );
}

#[tokio::test]
async fn tip_method_caches_when_tip_set() {
    let cache: Arc<dyn AlkanesCache> = Arc::new(InMemoryCache::new());
    let provider = ConcreteProvider::new_for_test(rpc_config_with_bogus_url(), dummy_command())
        .with_cache(cache.clone());

    let tip = [0xAB; 32];
    provider.set_current_tip(tip).await;

    // Pre-populate as Tip-scoped.
    let params_hex = "deadbeef";
    let key = make_key("simulate", "mainnet", params_hex);
    cache
        .put(&key, &CacheScope::Tip(tip), b"SIM_RESULT".to_vec(), None)
        .await
        .unwrap();

    let got = provider
        .metashrew_view_call("simulate", params_hex, "latest")
        .await
        .expect("Tip-scoped cache hit should short-circuit");
    assert_eq!(got, b"SIM_RESULT");
}

#[tokio::test]
async fn set_current_tip_triggers_reorg_when_changing() {
    let cache: Arc<dyn AlkanesCache> = Arc::new(InMemoryCache::new());
    let provider = ConcreteProvider::new_for_test(rpc_config_with_bogus_url(), dummy_command())
        .with_cache(cache.clone());

    let h1 = [1u8; 32];
    let h2 = [2u8; 32];

    provider.set_current_tip(h1).await;

    // Populate Tip(h1) entry.
    let key = make_key("simulate", "mainnet", "xx");
    cache
        .put(&key, &CacheScope::Tip(h1), b"OLD".to_vec(), None)
        .await
        .unwrap();
    assert_eq!(
        cache.get(&key, &CacheScope::Tip(h1)).await.unwrap().as_deref(),
        Some(b"OLD".as_ref())
    );

    // Reorg to h2 should drop the h1 entry.
    provider.set_current_tip(h2).await;
    assert!(cache.get(&key, &CacheScope::Tip(h1)).await.unwrap().is_none());
}

#[tokio::test]
async fn provider_without_cache_falls_through_to_network() {
    // No `with_cache` call — should bypass cache entirely and attempt the
    // network (which fails because the URL is bogus). This guards against
    // accidentally requiring a cache.
    let provider = ConcreteProvider::new_for_test(rpc_config_with_bogus_url(), dummy_command());
    let result = provider
        .metashrew_view_call("getbytecode", "0a01", "latest")
        .await;
    assert!(result.is_err());
}
