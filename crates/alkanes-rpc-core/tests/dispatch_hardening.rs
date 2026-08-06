//! Regression tests for remotely-triggerable faults in `RpcDispatcher`.
//!
//! Every request built here is something an UNAUTHENTICATED client can post to
//! the public JSON-RPC endpoint, so each of these panicking would be a remote
//! DoS (in the wasip2 edge a panic is a wasm trap -> 5xx).

use std::cell::RefCell;

use alkanes_rpc_core::backend::{
    BitcoinBackend, EsploraBackend, MetashrewBackend, NoOrd,
};
use alkanes_rpc_core::types::{JsonRpcRequest, JsonRpcResponse};
use alkanes_rpc_core::RpcDispatcher;
use anyhow::Result;
use async_trait::async_trait;
use futures::executor::block_on;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Mock backends — count invocations so fan-out can be asserted, and never
// touch the network.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct MockBitcoin {
    calls: RefCell<usize>,
}

#[async_trait(?Send)]
impl BitcoinBackend for MockBitcoin {
    async fn call(&self, _method: &str, _params: Vec<Value>, id: Value) -> Result<JsonRpcResponse> {
        *self.calls.borrow_mut() += 1;
        Ok(JsonRpcResponse::success(json!(1), id))
    }
}

#[derive(Default)]
struct MockMetashrew;

#[async_trait(?Send)]
impl MetashrewBackend for MockMetashrew {
    async fn forward(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        Ok(JsonRpcResponse::success(json!(null), request.id.clone()))
    }
}

#[derive(Default)]
struct MockEsplora {
    paths: RefCell<Vec<String>>,
}

#[async_trait(?Send)]
impl EsploraBackend for MockEsplora {
    async fn fetch(&self, path: &str) -> Result<Value> {
        self.paths.borrow_mut().push(path.to_string());
        Ok(json!([]))
    }
}

type Dispatcher = RpcDispatcher<MockBitcoin, MockMetashrew, MockEsplora, NoOrd>;

fn dispatcher() -> Dispatcher {
    RpcDispatcher::new(
        MockBitcoin::default(),
        MockMetashrew,
        MockEsplora::default(),
        NoOrd,
    )
}

fn req(method: &str, params: Vec<Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: method.to_string(),
        params,
        id: json!(1),
    }
}

fn error_message(resp: &JsonRpcResponse) -> Option<String> {
    match resp {
        JsonRpcResponse::Error { error, .. } => Some(error.message.clone()),
        _ => None,
    }
}

fn results(resp: &JsonRpcResponse) -> Vec<Value> {
    match resp {
        JsonRpcResponse::Success { result, .. } => {
            result.as_array().cloned().expect("multicall returns an array")
        }
        JsonRpcResponse::Error { error, .. } => panic!("expected success, got {:?}", error),
    }
}

// ---------------------------------------------------------------------------
// Bug 1 — empty `params` panicked on `&params[1..]`
//
//   {"jsonrpc":"2.0","id":1,"method":"lua_evalscript","params":[]}
//     -> "range start index 1 out of range for slice of length 0"
// ---------------------------------------------------------------------------

#[test]
fn lua_methods_survive_empty_params() {
    let d = dispatcher();
    for method in [
        "lua_evalscript",
        "lua_evalsaved",
        "sandshrew_evalscript",
        "sandshrew_evalsaved",
    ] {
        // "params": []
        let resp = block_on(d.dispatch(&req(method, vec![]))).expect("dispatch must not fail");
        let msg = error_message(&resp)
            .unwrap_or_else(|| panic!("{method} with empty params should be a JSON-RPC error"));
        assert!(
            msg.contains("Lua script not available"),
            "{method}: unexpected message {msg}"
        );
    }
}

#[test]
fn lua_methods_survive_missing_and_null_params() {
    let d = dispatcher();
    // `params` omitted entirely, and `"params": null` — both hit
    // JsonRpcRequest's `#[serde(default)]` / null-to-empty deserializer and
    // land in handle_lua_method with a zero-length slice.
    for raw in [
        r#"{"jsonrpc":"2.0","id":1,"method":"lua_evalsaved"}"#,
        r#"{"jsonrpc":"2.0","id":1,"method":"lua_evalsaved","params":null}"#,
        r#"{"jsonrpc":"2.0","id":1,"method":"sandshrew_evalscript","params":null}"#,
    ] {
        let request: JsonRpcRequest = serde_json::from_str(raw).expect("parses");
        assert!(request.params.is_empty(), "precondition: {raw}");
        let resp = block_on(d.dispatch(&request)).expect("dispatch must not fail");
        assert!(
            error_message(&resp).is_some(),
            "{raw} should be a JSON-RPC error, not a panic"
        );
    }
}

// ---------------------------------------------------------------------------
// Bug 2 — byte-slicing the caller-controlled identifier on the ERROR path
//
//   &identifier[..identifier.len().min(16)]
//     -> "byte index 16 is not a char boundary"
//
// Fires for ANY unrecognised script, i.e. the normal outcome.
// ---------------------------------------------------------------------------

#[test]
fn unknown_script_identifier_with_multibyte_char_across_byte_16() {
    // 15 ASCII bytes, then a 4-byte U+1F600 occupying bytes 15..19 — so byte
    // index 16 lands strictly INSIDE the character.
    let identifier = "aaaaaaaaaaaaaaa\u{1F600}zzz";
    assert_eq!(identifier.as_bytes().len(), 15 + 4 + 3);
    assert!(
        !identifier.is_char_boundary(16),
        "precondition: byte 16 must not be a char boundary"
    );

    let d = dispatcher();
    for method in ["lua_evalsaved", "lua_evalscript", "sandshrew_evalsaved"] {
        let resp = block_on(d.dispatch(&req(method, vec![json!(identifier)])))
            .expect("dispatch must not fail");
        let msg = error_message(&resp).unwrap_or_else(|| panic!("{method} should error"));
        assert!(
            msg.contains("Lua script not available"),
            "{method}: unexpected message {msg}"
        );
        // Truncated by CHARACTER: 16 chars, the emoji intact.
        assert!(
            msg.contains("aaaaaaaaaaaaaaa\u{1F600}"),
            "{method}: identifier should be char-truncated, got {msg}"
        );
        assert!(!msg.contains('z'), "{method}: should stop at 16 chars, got {msg}");
    }
}

#[test]
fn unknown_script_identifier_multibyte_at_every_offset() {
    // Sweep the emoji across the truncation point so no off-by-one survives.
    let d = dispatcher();
    for pad in 10..24 {
        let identifier = format!("{}\u{1F600}tail", "a".repeat(pad));
        let resp = block_on(d.dispatch(&req("lua_evalsaved", vec![json!(identifier)])))
            .expect("dispatch must not fail");
        assert!(
            error_message(&resp).is_some(),
            "pad={pad} should error, not panic"
        );
    }
}

#[test]
fn non_string_and_empty_identifiers_are_tolerated() {
    let d = dispatcher();
    for params in [
        vec![json!(null)],
        vec![json!(42)],
        vec![json!("")],
        vec![json!({"not": "a string"})],
    ] {
        let resp =
            block_on(d.dispatch(&req("lua_evalsaved", params.clone()))).expect("dispatch must not fail");
        assert!(error_message(&resp).is_some(), "{params:?} should error");
    }
}

// ---------------------------------------------------------------------------
// Bug 3 — unbounded multicall recursion / fan-out
// ---------------------------------------------------------------------------

#[test]
fn flat_multicall_still_works() {
    // A legitimate multicall is the SDK's flat list of [method, params]
    // tuples (lua/multicall.lua). It must be entirely unaffected.
    let d = dispatcher();
    let calls: Vec<Value> = (0..50).map(|_| json!(["btc_getblockcount", []])).collect();
    let resp = block_on(d.dispatch(&req("sandshrew_multicall", calls))).expect("dispatch");
    let out = results(&resp);
    assert_eq!(out.len(), 50);
    assert!(out.iter().all(|r| r.get("result").is_some()), "{out:?}");
    assert_eq!(*d.bitcoin.calls.borrow(), 50);
}

#[test]
fn nested_multicall_is_depth_limited() {
    let d = dispatcher();
    // multicall( multicall( multicall( btc_getblockcount ) ) )
    let inner = json!(["sandshrew_multicall", [["btc_getblockcount", []]]]);
    let mid = json!(["sandshrew_multicall", [inner]]);
    let resp = block_on(d.dispatch(&req("sandshrew_multicall", vec![mid]))).expect("dispatch");

    let rendered = serde_json::to_string(&resp).unwrap();
    assert!(
        rendered.contains("multicall nesting too deep"),
        "expected a depth refusal, got {rendered}"
    );
    // The third level never ran, so the leaf backend was never reached.
    assert_eq!(
        *d.bitcoin.calls.borrow(),
        0,
        "leaf call should have been cut off by the depth guard"
    );
}

#[test]
fn multicall_fanout_is_budget_capped() {
    let d = dispatcher();
    let calls: Vec<Value> = (0..1200).map(|_| json!(["btc_getblockcount", []])).collect();
    let resp = block_on(d.dispatch(&req("sandshrew_multicall", calls))).expect("dispatch");
    let out = results(&resp);

    // Response stays 1:1 with the request…
    assert_eq!(out.len(), 1200);
    // …but only the budgeted prefix actually reached a backend.
    assert_eq!(*d.bitcoin.calls.borrow(), 1024);
    let refused = out
        .iter()
        .filter(|r| {
            r.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(|m| m.contains("sub-call budget exhausted"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(refused, 1200 - 1024);
}

#[test]
fn each_top_level_request_gets_a_fresh_budget() {
    // The budget is per-dispatch, so back-to-back (or concurrently polled)
    // requests must not starve each other.
    let d = dispatcher();
    let calls: Vec<Value> = (0..600).map(|_| json!(["btc_getblockcount", []])).collect();
    for _ in 0..3 {
        let resp = block_on(d.dispatch(&req("sandshrew_multicall", calls.clone()))).expect("dispatch");
        let out = results(&resp);
        assert!(out.iter().all(|r| r.get("result").is_some()), "budget leaked across requests");
    }
    assert_eq!(*d.bitcoin.calls.borrow(), 1800);
}

// ---------------------------------------------------------------------------
// Esplora path assembly — params are path segments and must be encoded.
// ---------------------------------------------------------------------------

#[test]
fn esplora_params_are_percent_encoded() {
    let d = dispatcher();
    let _ = block_on(d.dispatch(&req("esplora_foo", vec![json!("../../whatever")]))).expect("dispatch");
    let paths = d.esplora.paths.borrow();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0], "/foo/..%2F..%2Fwhatever");
    assert!(!paths[0].contains("../"), "traversal survived: {}", paths[0]);
}

#[test]
fn esplora_normal_params_are_unchanged() {
    let d = dispatcher();
    let addr = "bc1pk6nvyxrxmryqsuahhhpal5hkjy4kx6cjmzchqu9xrfhhqfr8dv2skgpwvj";
    let _ = block_on(d.dispatch(&req("esplora_address::utxo", vec![json!(addr)]))).expect("dispatch");
    assert_eq!(d.esplora.paths.borrow()[0], format!("/address/{addr}/utxo"));
}
