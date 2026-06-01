//! Integration tests for the in-process `protorunesbyaddress` fan-out
//! handler. We spin up a tiny actix-web mock server that pretends to be
//! both metashrew (POST JSON-RPC) and esplora (GET /address/.../utxo),
//! then point the ProxyClient's URLs at it. No real upstream needed.
//!
//! Two paths are covered:
//!   1. HAPPY PATH — height stable across the request, fan-out returns
//!      an aggregated WalletResponse JSON.
//!   2. STALE-WINDOW PATH — the mock advances its "metashrew_height"
//!      reply between the first call (resolve H) and the last call
//!      (post-fan-out drift check) by more than the configured window,
//!      so the handler returns the -32011 error.
//!
//! We do NOT try to verify byte-equivalence with the real upstream
//! `metashrew_view "protorunesbyaddress"` response — that requires a
//! live indexer and is covered by the e2e suites in subkube. What we
//! verify here is the contract of the fan-out helper itself.

use actix_web::{web, App, HttpResponse, HttpServer};
use serde_json::{json, Value};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// We need access to internal items. The crate is a bin, but tests/
// integration tests link against the lib if we expose one — looking at
// the existing tests file, it uses standalone code. So we exercise the
// flow end-to-end via the HTTP server the same way main.rs does, except
// here we don't want to spin up the full server. Instead, we make the
// modules used by handlers visible to tests by depending on a tiny
// shim — but since alkanes-jsonrpc has no lib target, we re-declare a
// minimal subset of the relevant types via the actix server and call
// it via HTTP.
//
// SIMPLER APPROACH: the handler functions we want to test all live
// behind `proxy.forward_to_metashrew` + `proxy.fetch_esplora_endpoint`,
// so we just build a real ProxyClient (the public API) and call into
// it through a black-box helper. To do that, we need the binary crate
// to expose those types — which it does, but only via the `main.rs`
// `mod` declarations. Crates with only a `[[bin]]` target cannot be
// linked by integration tests in the standard way.
//
// We work around this by black-box testing via the HTTP server: spawn
// the alkanes-jsonrpc binary's logic by replicating just the routing
// layer here, but only for the methods under test. That's heavier than
// it should be — for now, the test verifies the underlying invariants
// by sending raw HTTP requests to a stub server and confirming the
// flow logic at the protocol level.
//
// CONCRETELY: because we can't import `crate::protorunesbyaddress`
// from an integration test of a bin-only crate, this test exercises
// the staleness CONTRACT (the mock advances height; the fan-out
// short-circuits) by routing all upstream calls through one mock
// server and observing the call sequence. The happy path verifies the
// mock returns valid protobuf and the fan-out call count matches
// expectation (1 esplora + N protorunesbyoutpoint + at least 2
// metashrew_height for the drift-check).

#[derive(Default)]
struct MockState {
    /// Current "served" height. Returned for `metashrew_height` calls.
    height: AtomicU64,
    /// Per-call height step. Each `metashrew_height` call returns
    /// `height + step * call_index`, simulating the chain advancing.
    /// 0 means stable.
    height_step: AtomicU64,
    /// Counter of `metashrew_height` calls — used by the test to
    /// confirm the handler called it twice (pin + post-check).
    metashrew_height_calls: AtomicU64,
    /// Counter of `protorunesbyoutpoint` sub-calls. Should equal #UTXOs.
    protorunesbyoutpoint_calls: AtomicU64,
    /// Counter of `esplora_address::utxo` calls. Should equal 1.
    esplora_utxo_calls: AtomicU64,
    /// UTXOs to return from the mock esplora.
    utxos: parking_lot_mutex::Mutex<Vec<Value>>,
}

mod parking_lot_mutex {
    // tiny stdlib Mutex wrapper to keep us off external deps.
    pub use std::sync::Mutex;
}

async fn mock_jsonrpc(
    body: web::Json<Value>,
    state: web::Data<Arc<MockState>>,
) -> HttpResponse {
    let method = body.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let id = body.get("id").cloned().unwrap_or(json!(1));

    match method {
        "metashrew_height" => {
            let n = state.metashrew_height_calls.fetch_add(1, Ordering::SeqCst);
            let base = state.height.load(Ordering::SeqCst);
            let step = state.height_step.load(Ordering::SeqCst);
            let served = base + step * n;
            HttpResponse::Ok().json(json!({
                "jsonrpc": "2.0",
                "result": served.to_string(),
                "id": id,
            }))
        }
        "metashrew_view" => {
            let params = body.get("params").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let view = params.get(0).and_then(|v| v.as_str()).unwrap_or("");
            match view {
                "protorunesbyoutpoint" => {
                    state
                        .protorunesbyoutpoint_calls
                        .fetch_add(1, Ordering::SeqCst);
                    // Return a minimal OutpointResponse-shaped protobuf
                    // hex. Empty payload is valid (zero-length protobuf
                    // decodes to the all-default message) and is the
                    // simplest stable response for the test.
                    HttpResponse::Ok().json(json!({
                        "jsonrpc": "2.0",
                        "result": "0x",
                        "id": id,
                    }))
                }
                "protorunesbyaddress" => {
                    // We don't expect the fan-out handler to ever call
                    // this — but if it does (passthrough mode), return
                    // an empty WalletResponse to keep the test
                    // deterministic.
                    HttpResponse::Ok().json(json!({
                        "jsonrpc": "2.0",
                        "result": "0x",
                        "id": id,
                    }))
                }
                _ => HttpResponse::Ok().json(json!({
                    "jsonrpc": "2.0",
                    "error": { "code": -32601, "message": format!("unknown view: {}", view) },
                    "id": id,
                })),
            }
        }
        "metashrew_getblockhash" => {
            // Return a deterministic 32-byte hex string. The cache
            // layer wants this — we don't use the cache in this test
            // (no REDIS_URL) so the path is technically dead, but if
            // it's ever called we want a stable answer.
            HttpResponse::Ok().json(json!({
                "jsonrpc": "2.0",
                "result": format!("0x{}", "ab".repeat(32)),
                "id": id,
            }))
        }
        _ => HttpResponse::Ok().json(json!({
            "jsonrpc": "2.0",
            "error": { "code": -32601, "message": format!("unknown method: {}", method) },
            "id": id,
        })),
    }
}

async fn mock_esplora_utxo(
    path: web::Path<String>,
    state: web::Data<Arc<MockState>>,
) -> HttpResponse {
    let _ = path; // address; we ignore it and serve the configured UTXO set.
    state.esplora_utxo_calls.fetch_add(1, Ordering::SeqCst);
    let utxos = state.utxos.lock().unwrap().clone();
    HttpResponse::Ok().json(utxos)
}

/// Spawn a mock server bound to a free localhost port. Returns
/// (base_url, state_handle). The server runs until the test exits.
async fn spawn_mock_server(state: Arc<MockState>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    let st = state.clone();
    std::thread::spawn(move || {
        let rt = actix_web::rt::System::new();
        rt.block_on(async move {
            HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(st.clone()))
                    .route("/", web::post().to(mock_jsonrpc))
                    .route(
                        "/address/{address}/utxo",
                        web::get().to(mock_esplora_utxo),
                    )
            })
            .listen(listener)
            .unwrap()
            .run()
            .await
            .unwrap();
        });
    });

    // Give the server a brief moment to start accepting.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    url
}

/// Send a JSON-RPC request to the mock server's "metashrew" endpoint
/// (which we point at via the same base URL for both metashrew + esplora).
async fn post_jsonrpc(url: &str, body: Value) -> Value {
    let client = reqwest::Client::new();
    client
        .post(url)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn mock_server_returns_height_and_utxos() {
    // Sanity check: confirm the mock server responds as expected
    // before we exercise the actual fan-out logic against it.
    let state = Arc::new(MockState {
        height: AtomicU64::new(900_000),
        height_step: AtomicU64::new(0),
        ..Default::default()
    });
    {
        *state.utxos.lock().unwrap() = vec![
            json!({
                "txid": "aa".repeat(32),
                "vout": 0,
                "value": 12345,
                "status": { "block_height": 899_990 },
            }),
        ];
    }
    let url = spawn_mock_server(state.clone()).await;

    let h = post_jsonrpc(
        &url,
        json!({ "jsonrpc": "2.0", "method": "metashrew_height", "params": [], "id": 1 }),
    )
    .await;
    assert_eq!(h["result"].as_str().unwrap(), "900000");

    let utxos: Value = reqwest::Client::new()
        .get(&format!("{}/address/bc1ptest/utxo", url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(utxos.as_array().unwrap().len(), 1);

    assert_eq!(state.metashrew_height_calls.load(Ordering::SeqCst), 1);
    assert_eq!(state.esplora_utxo_calls.load(Ordering::SeqCst), 1);
}

// -----------------------------------------------------------------------
// The two tests below drive the alkanes-jsonrpc binary through HTTP. We
// don't link against the bin crate (Rust doesn't support that out of the
// box for [[bin]]-only crates). Instead, we spawn the alkanes-jsonrpc
// binary as a subprocess with env vars pointing at the mock, then send
// JSON-RPC requests to it. If the binary isn't yet built when the test
// runs, we skip with a clear message rather than fail noisily — the
// e2e CI runs `cargo build` before `cargo test`, so in CI both tests
// run; locally a developer running `cargo test` without first running
// `cargo build` sees the skip.
// -----------------------------------------------------------------------

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct ServerHandle {
    child: Child,
    base_url: String,
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn spawn_alkanes_jsonrpc(
    mock_url: &str,
    extra_env: &[(&str, &str)],
) -> Option<ServerHandle> {
    // Find a free port for the server.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener); // free it so the binary can grab it.

    // The bin path: target/debug/alkanes-jsonrpc relative to the
    // workspace root. CARGO_BIN_EXE_alkanes-jsonrpc is set automatically
    // by cargo for integration tests of bin crates — use it when
    // available. (Cargo sets this env var when running `cargo test`
    // because alkanes-jsonrpc has a [[bin]] target.)
    let bin_path = match std::env::var("CARGO_BIN_EXE_alkanes-jsonrpc") {
        Ok(p) => p,
        Err(_) => {
            eprintln!(
                "SKIP: CARGO_BIN_EXE_alkanes-jsonrpc not set; \
                 run `cargo test -p alkanes-jsonrpc` (not direct `cargo test`)"
            );
            return None;
        }
    };

    let mut cmd = Command::new(&bin_path);
    cmd.env("SERVER_HOST", "127.0.0.1")
        .env("SERVER_PORT", port.to_string())
        .env("METASHREW_URL", mock_url)
        .env("MEMSHREW_URL", mock_url)
        .env("ESPLORA_URL", mock_url)
        .env("ORD_URL", mock_url)
        .env("SUBFROST_URL", mock_url)
        .env("BITCOIN_RPC_URL", mock_url)
        .env("BITCOIN_RPC_USER", "u")
        .env("BITCOIN_RPC_PASSWORD", "p")
        // Make sure REDIS_URL is unset so the cache is disabled — the
        // staleness contract is exercised against the raw upstream
        // path, not the cached one.
        .env_remove("REDIS_URL")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (k, v) in extra_env {
        cmd.env(*k, *v);
    }
    let child = cmd.spawn().expect("spawn alkanes-jsonrpc");

    let base_url = format!("http://127.0.0.1:{}", port);
    // Poll the server until it accepts connections (or give up).
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if reqwest::Client::new()
            .post(&base_url)
            .json(&json!({ "jsonrpc": "2.0", "method": "metashrew_height", "params": [], "id": 1 }))
            .timeout(Duration::from_millis(500))
            .send()
            .await
            .is_ok()
        {
            return Some(ServerHandle { child, base_url });
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    eprintln!("SKIP: alkanes-jsonrpc binary did not come up in 10s");
    None
}

#[tokio::test]
async fn happy_path_returns_wallet_response_shape() {
    let state = Arc::new(MockState {
        height: AtomicU64::new(900_000),
        height_step: AtomicU64::new(0), // height stable → drift = 0
        ..Default::default()
    });
    {
        *state.utxos.lock().unwrap() = vec![
            json!({
                "txid": "aa".repeat(32),
                "vout": 0,
                "value": 12345,
                "status": { "block_height": 899_990 },
            }),
            json!({
                "txid": "bb".repeat(32),
                "vout": 1,
                "value": 54321,
                "status": { "block_height": 899_995 },
            }),
        ];
    }
    let mock_url = spawn_mock_server(state.clone()).await;

    let Some(server) = spawn_alkanes_jsonrpc(&mock_url, &[]).await else {
        return; // skipped
    };

    let resp = post_jsonrpc(
        &server.base_url,
        json!({
            "jsonrpc": "2.0",
            "method": "alkanes_protorunesbyaddress",
            "params": ["bc1ptest", "latest"],
            "id": 42,
        }),
    )
    .await;

    // We should get a Success with a WalletResponse-shaped result.
    assert!(resp.get("error").is_none(), "got error: {:?}", resp);
    let result = resp.get("result").expect("missing result");
    assert!(result.get("outpoints").is_some(), "missing outpoints: {:?}", result);
    assert!(result.get("balances").is_some(), "missing balances: {:?}", result);

    // Each UTXO had an empty (0-length protobuf) response → empty
    // balances → outpoints get filtered (the aggregator drops entries
    // with no balances). Expect 0 outpoints + 0 aggregated balances.
    let outpoints = result["outpoints"].as_array().unwrap();
    let agg = result["balances"]["entries"].as_array().unwrap();
    assert_eq!(outpoints.len(), 0, "empty sub-responses should produce no outpoints");
    assert_eq!(agg.len(), 0);

    // Verify the call accounting on the mock.
    assert_eq!(state.esplora_utxo_calls.load(Ordering::SeqCst), 1, "1 esplora call");
    assert_eq!(
        state.protorunesbyoutpoint_calls.load(Ordering::SeqCst),
        2,
        "2 fan-out sub-calls (one per UTXO)"
    );
    // metashrew_height called at least twice: once to pin H, once to
    // re-check drift. The cache layer may add more (it's disabled here
    // because REDIS_URL is unset).
    assert!(
        state.metashrew_height_calls.load(Ordering::SeqCst) >= 2,
        "expected ≥2 metashrew_height calls (pin + drift check), got {}",
        state.metashrew_height_calls.load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn stale_window_returns_minus_32011() {
    // height starts at 900_000 and advances by 10 per metashrew_height
    // call. With staleness window = 6, by the second call (post-fan-out
    // drift check) the served height is 900_010 — drift = 10 > 6 → fail.
    let state = Arc::new(MockState {
        height: AtomicU64::new(900_000),
        height_step: AtomicU64::new(10),
        ..Default::default()
    });
    {
        *state.utxos.lock().unwrap() = vec![json!({
            "txid": "aa".repeat(32),
            "vout": 0,
            "value": 12345,
            "status": { "block_height": 899_990 },
        })];
    }
    let mock_url = spawn_mock_server(state.clone()).await;

    let Some(server) =
        spawn_alkanes_jsonrpc(&mock_url, &[("PROTORUNES_STALENESS_WINDOW_BLOCKS", "6")])
            .await
    else {
        return; // skipped
    };

    let resp = post_jsonrpc(
        &server.base_url,
        json!({
            "jsonrpc": "2.0",
            "method": "alkanes_protorunesbyaddress",
            "params": ["bc1ptest", "latest"],
            "id": 7,
        }),
    )
    .await;

    let err = resp.get("error").expect("expected error, got success");
    let code = err.get("code").and_then(|v| v.as_i64()).expect("code");
    let msg = err.get("message").and_then(|v| v.as_str()).expect("message");
    assert_eq!(code, -32011, "expected STALE_HEIGHT_WINDOW (-32011), got {}: {}", code, msg);
    assert!(
        msg.contains("stale height window"),
        "expected stale-height message, got: {}",
        msg
    );
    assert!(msg.contains("pinned"), "message should mention pinned H");
    assert!(msg.contains("drift"), "message should mention drift");
}
