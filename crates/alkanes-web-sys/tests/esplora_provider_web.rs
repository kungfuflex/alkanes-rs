//! Integration tests for the EsploraProvider on WebProvider
//!
//! These tests validate that the WebProvider correctly implements the EsploraProvider
//! trait by mocking the JavaScript `fetch` API and ensuring that the correct
//! JSON-RPC requests are constructed and sent.

#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;
wasm_bindgen_test_configure!(run_in_browser);
use alkanes_cli_common::{EsploraProvider, Result};
use deezel_web::provider::WebProvider;
use serde_json::json;
use web_sys::{Response, ResponseInit};

// A more robust mocking setup using closures.
// This avoids inline JS and gives us more control from Rust.
thread_local! {
    static MOCK_RESPONSE: Rc<RefCell<JsValue>> = Rc::new(RefCell::new(JsValue::NULL));
    static LAST_REQUEST_URL: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    static LAST_REQUEST_BODY: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
}

struct FetchMock {
    original_fetch: JsValue,
    _closure: Closure<dyn FnMut(String, JsValue) -> js_sys::Promise>,
}

impl FetchMock {
    fn new() -> Self {
        let window = web_sys::window().unwrap();
        let original_fetch = js_sys::Reflect::get(&window, &"fetch".into()).unwrap();

        let fetch_closure = Closure::wrap(Box::new(move |url: String, options: JsValue| {
            LAST_REQUEST_URL.with(|cell| *cell.borrow_mut() = url);

            if let Ok(body_val) = js_sys::Reflect::get(&options, &"body".into()) {
                if let Some(body_str) = body_val.as_string() {
                    LAST_REQUEST_BODY.with(|cell| *cell.borrow_mut() = body_str);
                }
            }

            let promise = js_sys::Promise::new(&mut |resolve, _| {
                let mut response_init = ResponseInit::new();
                response_init.set_status(200);
                let response_body = MOCK_RESPONSE.with(|cell| {
                    let js_val = cell.borrow().clone();
                    if js_val.is_null() || js_val.is_undefined() {
                        None
                    } else {
                        // The mock response is already a JSON stringified value
                        js_val.as_string()
                    }
                });

                let response = Response::new_with_opt_str_and_init(response_body.as_deref(), &response_init).unwrap();
                resolve.call1(&JsValue::UNDEFINED, &response).unwrap();
            });
            promise
        }) as Box<dyn FnMut(String, JsValue) -> js_sys::Promise>);

        let fetch_js_val = fetch_closure.as_ref().clone();
        js_sys::Reflect::set(&window, &"fetch".into(), &fetch_js_val).unwrap();

        FetchMock {
            original_fetch,
            _closure: fetch_closure,
        }
    }

    fn set_response(&self, response: &serde_json::Value) {
        let response_str = serde_json::to_string(response).unwrap();
        MOCK_RESPONSE.with(|cell| *cell.borrow_mut() = JsValue::from_str(&response_str));
    }

    fn last_request_url(&self) -> String {
        LAST_REQUEST_URL.with(|cell| cell.borrow().clone())
    }

    fn last_request_body(&self) -> String {
        LAST_REQUEST_BODY.with(|cell| cell.borrow().clone())
    }
}

impl Drop for FetchMock {
    fn drop(&mut self) {
        let window = web_sys::window().unwrap();
        js_sys::Reflect::set(&window, &"fetch".into(), &self.original_fetch).unwrap();
    }
}

async fn setup() -> Result<WebProvider> {
    WebProvider::new("regtest".to_string()).await
}

#[wasm_bindgen_test]
pub async fn test_get_blocks_tip_hash_web() {
    let mock = FetchMock::new();
    let provider = setup().await.unwrap();

    let mock_hash = "0000000000000000000abcde".to_string();
    let rpc_response = json!({
        "jsonrpc": "2.0",
        "result": mock_hash,
        "id": 1
    });
    mock.set_response(&rpc_response);

    let result = provider.get_blocks_tip_hash().await;

    assert!(result.is_ok(), "get_blocks_tip_hash failed: {:?}", result.err());
    assert_eq!(result.unwrap(), mock_hash);

    // Verify the request details
    assert_eq!(mock.last_request_url(), provider.esplora_rpc_url().unwrap());
    let body: serde_json::Value = serde_json::from_str(&mock.last_request_body()).unwrap();
    assert_eq!(body["method"], "esplora_blocks:tip:hash");
    assert_eq!(body["params"], json!([]));
}

#[wasm_bindgen_test]
pub async fn test_get_blocks_tip_height_web() {
    let mock = FetchMock::new();
    let provider = setup().await.unwrap();

    let mock_height = 300000;
    let rpc_response = json!({
        "jsonrpc": "2.0",
        "result": mock_height,
        "id": 1
    });
    mock.set_response(&rpc_response);

    let result = provider.get_blocks_tip_height().await;

    assert!(result.is_ok(), "get_blocks_tip_height failed: {:?}", result.err());
    assert_eq!(result.unwrap(), mock_height);

    let body: serde_json::Value = serde_json::from_str(&mock.last_request_body()).unwrap();
    assert_eq!(body["method"], "esplora_blocks:tip:height");
    assert_eq!(body["params"], json!([]));
}

#[wasm_bindgen_test]
pub async fn test_get_block_by_hash_web() {
    let mock = FetchMock::new();
    let provider = setup().await.unwrap();
    let hash = "0000000000000000000abcde".to_string();

    let mock_block: serde_json::Value = json!({
      "id": "0000000000000000000abcde",
      "height": 123,
      "version": 1,
      "timestamp": 1234567890,
      "tx_count": 10,
      "size": 1000,
      "weight": 4000,
      "merkle_root": "mr",
      "previousblockhash": "prev",
      "nonce": 1234,
      "bits": 5678,
      "difficulty": 9876
    });
    let rpc_response = json!({
        "jsonrpc": "2.0",
        "result": mock_block,
        "id": 1
    });
    mock.set_response(&rpc_response);

    let result = provider.get_block(&hash).await;

    assert!(result.is_ok(), "get_block failed: {:?}", result.err());
    assert_eq!(result.unwrap(), mock_block);

    let body: serde_json::Value = serde_json::from_str(&mock.last_request_body()).unwrap();
    assert_eq!(body["method"], "esplora_block");
    assert_eq!(body["params"], json!([hash]));
}

#[wasm_bindgen_test]
pub async fn test_get_block_by_height_web() {
    let mock = FetchMock::new();
    let provider = setup().await.unwrap();
    let height = 123;
    let mock_hash = "0000000000000000000abcde".to_string();

    let rpc_response = json!({
        "jsonrpc": "2.0",
        "result": mock_hash,
        "id": 1
    });
    mock.set_response(&rpc_response);

    let result = provider.get_block_by_height(height).await;

    assert!(result.is_ok(), "get_block_by_height failed: {:?}", result.err());
    assert_eq!(result.unwrap(), mock_hash);

    let body: serde_json::Value = serde_json::from_str(&mock.last_request_body()).unwrap();
    assert_eq!(body["method"], "esplora_block:height");
    assert_eq!(body["params"], json!([height]));
}

#[wasm_bindgen_test]
pub async fn test_get_transaction_web() {
    let mock = FetchMock::new();
    let provider = setup().await.unwrap();
    let txid = "abcdef1234567890".to_string();

    let mock_tx: serde_json::Value = json!({
        "txid": "abcdef1234567890",
        "version": 1,
        "locktime": 0,
        "vin": [],
        "vout": [],
        "size": 100,
        "weight": 400,
        "fee": 1000,
        "status": {
            "confirmed": true,
            "block_height": 123,
            "block_hash": "0000000000000000000abcde",
            "block_time": 1234567890
        }
    });
    let rpc_response = json!({
        "jsonrpc": "2.0",
        "result": mock_tx,
        "id": 1
    });
    mock.set_response(&rpc_response);

    let result = provider.get_tx(&txid).await;

    assert!(result.is_ok(), "get_tx failed: {:?}", result.err());
    assert_eq!(result.unwrap(), mock_tx);

    let body: serde_json::Value = serde_json::from_str(&mock.last_request_body()).unwrap();
    assert_eq!(body["method"], "esplora_tx");
    assert_eq!(body["params"], json!([txid]));
}

#[wasm_bindgen_test]
pub async fn test_get_transaction_status_web() {
    let mock = FetchMock::new();
    let provider = setup().await.unwrap();
    let txid = "abcdef1234567890".to_string();

    let mock_status: serde_json::Value = json!({
        "confirmed": true,
        "block_height": 123,
        "block_hash": "0000000000000000000abcde",
        "block_time": 1234567890
    });
    let rpc_response = json!({
        "jsonrpc": "2.0",
        "result": mock_status,
        "id": 1
    });
    mock.set_response(&rpc_response);

    let result = provider.get_tx_status(&txid).await;

    assert!(result.is_ok(), "get_tx_status failed: {:?}", result.err());
    assert_eq!(result.unwrap(), mock_status);

    let body: serde_json::Value = serde_json::from_str(&mock.last_request_body()).unwrap();
    assert_eq!(body["method"], "esplora_tx:status");
    assert_eq!(body["params"], json!([txid]));
}

#[wasm_bindgen_test]
pub async fn test_get_merkle_proof_web() {
    let mock = FetchMock::new();
    let provider = setup().await.unwrap();
    let txid = "abcdef1234567890".to_string();

    let mock_proof: serde_json::Value = json!({
        "block_height": 123,
        "merkle": ["abc", "def"],
        "pos": 1
    });
    let rpc_response = json!({
        "jsonrpc": "2.0",
        "result": mock_proof,
        "id": 1
    });
    mock.set_response(&rpc_response);

    let result = provider.get_tx_merkle_proof(&txid).await;

    assert!(result.is_ok(), "get_tx_merkle_proof failed: {:?}", result.err());
    assert_eq!(result.unwrap(), mock_proof);

    let body: serde_json::Value = serde_json::from_str(&mock.last_request_body()).unwrap();
    assert_eq!(body["method"], "esplora_tx:merkle-proof");
    assert_eq!(body["params"], json!([txid]));
}

#[wasm_bindgen_test]
pub async fn test_get_fee_estimates_web() {
    let mock = FetchMock::new();
    let provider = setup().await.unwrap();

    let mock_fees: serde_json::Value = json!({
        "1": 100.0,
        "6": 50.0,
        "144": 10.0
    });
    let rpc_response = json!({
        "jsonrpc": "2.0",
        "result": mock_fees,
        "id": 1
    });
    mock.set_response(&rpc_response);

    let result = provider.get_fee_estimates().await;

    assert!(result.is_ok(), "get_fee_estimates failed: {:?}", result.err());

    let body: serde_json::Value = serde_json::from_str(&mock.last_request_body()).unwrap();
    assert_eq!(body["method"], "esplora_fee-estimates");
    assert_eq!(body["params"], json!([]));
}

#[wasm_bindgen_test]
pub async fn test_broadcast_transaction_web() {
    let mock = FetchMock::new();
    let provider = setup().await.unwrap();
    let tx_hex = "0100000001...".to_string();

    let mock_txid = "abcdef1234567890".to_string();
    let rpc_response = json!({
        "jsonrpc": "2.0",
        "result": mock_txid,
        "id": 1
    });
    mock.set_response(&rpc_response);

    let result = provider.broadcast(&tx_hex).await;

    assert!(result.is_ok(), "broadcast failed: {:?}", result.err());
    assert_eq!(result.unwrap(), mock_txid);

    let body: serde_json::Value = serde_json::from_str(&mock.last_request_body()).unwrap();
    assert_eq!(body["method"], "esplora_broadcast");
    assert_eq!(body["params"], json!([tx_hex]));
}

#[wasm_bindgen_test]
pub async fn test_get_address_utxo_web() {
    let mock = FetchMock::new();
    let provider = setup().await.unwrap();
    let address = "bc1q...";

    let mock_utxos: serde_json::Value = json!([
        {
            "txid": "abcdef1234567890",
            "vout": 0,
            "status": {
                "confirmed": true,
                "block_height": 123,
                "block_hash": "0000000000000000000abcde",
                "block_time": 1234567890
            },
            "value": 10000
        }
    ]);
    let rpc_response = json!({
        "jsonrpc": "2.0",
        "result": mock_utxos,
        "id": 1
    });
    mock.set_response(&rpc_response);

    let result = provider.get_address_utxo(address).await;

    assert!(result.is_ok(), "get_address_utxo failed: {:?}", result.err());
    assert_eq!(result.unwrap(), mock_utxos);

    let body: serde_json::Value = serde_json::from_str(&mock.last_request_body()).unwrap();
    assert_eq!(body["method"], "esplora_address:utxo");
    assert_eq!(body["params"], json!([address]));
}

#[wasm_bindgen_test]
pub async fn test_get_address_info_web() {
    let mock = FetchMock::new();
    let provider = setup().await.unwrap();
    let address = "bc1q_test_address";

    let mock_info = json!({
        "address": address,
        "chain_stats": {
            "funded_txo_count": 10,
            "funded_txo_sum": 500000,
            "spent_txo_count": 5,
            "spent_txo_sum": 250000,
            "tx_count": 15
        },
        "mempool_stats": {
            "funded_txo_count": 1,
            "funded_txo_sum": 10000,
            "spent_txo_count": 0,
            "spent_txo_sum": 0,
            "tx_count": 1
        }
    });

    let rpc_response = json!({
        "jsonrpc": "2.0",
        "result": mock_info,
        "id": 1
    });
    mock.set_response(&rpc_response);

    let result = provider.get_address_info(address).await;

    assert!(result.is_ok(), "get_address_info failed: {:?}", result.err());
    assert_eq!(result.unwrap(), mock_info);

    let body: serde_json::Value = serde_json::from_str(&mock.last_request_body()).unwrap();
    assert_eq!(body["method"], "esplora_address");
    assert_eq!(body["params"], json!([address]));
}
