//! JSON-RPC type definitions

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC request structure
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcRequest {
    pub id: u32,
    pub jsonrpc: String,
    pub method: String,
    pub params: Vec<Value>,
}

/// JSON-RPC success response
#[derive(Serialize, Debug)]
pub struct JsonRpcResult {
    pub id: u32,
    pub result: Value,
    pub jsonrpc: String,
}

/// JSON-RPC error response
#[derive(Serialize, Debug)]
pub struct JsonRpcError {
    pub id: u32,
    pub error: JsonRpcErrorObject,
    pub jsonrpc: String,
}

/// JSON-RPC error object
#[derive(Serialize, Debug)]
pub struct JsonRpcErrorObject {
    pub code: i32,
    pub message: String,
    pub data: Option<String>,
}

/// JSON-RPC response (for internal use)
#[derive(Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub id: u32,
    pub result: Option<Value>,
    pub error: Option<JsonRpcErrorInternal>,
    pub jsonrpc: String,
}

/// JSON-RPC error (for internal use)
#[derive(Deserialize, Debug)]
pub struct JsonRpcErrorInternal {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl JsonRpcResult {
    pub fn new(id: u32, result: Value) -> Self {
        Self {
            id,
            result,
            jsonrpc: "2.0".to_string(),
        }
    }

    pub fn success(id: u32, result: String) -> Self {
        Self::new(id, Value::String(result))
    }

    pub fn success_json(id: u32, result: serde_json::Value) -> Self {
        Self::new(id, result)
    }
}

impl JsonRpcError {
    pub fn new(id: u32, code: i32, message: String) -> Self {
        Self {
            id,
            error: JsonRpcErrorObject {
                code,
                message,
                data: None,
            },
            jsonrpc: "2.0".to_string(),
        }
    }

    pub fn invalid_params(id: u32, message: String) -> Self {
        Self::new(id, -32602, message)
    }

    pub fn method_not_found(id: u32, method: String) -> Self {
        Self::new(id, -32601, format!("Method '{}' not found", method))
    }

    pub fn internal_error(id: u32, message: String) -> Self {
        Self::new(id, -32000, message)
    }
}

/// Standard JSON-RPC error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const SERVER_ERROR: i32 = -32000;
}