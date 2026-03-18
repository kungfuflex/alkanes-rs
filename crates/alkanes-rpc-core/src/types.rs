use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    /// JSON-RPC params — supports both positional (array) and named (object).
    /// Object params are wrapped in a single-element array for uniform handling.
    #[serde(deserialize_with = "deserialize_params", default)]
    pub params: Vec<Value>,
    pub id: Value,
}

/// Accept both `[...]` and `{...}` for JSON-RPC params.
fn deserialize_params<'de, D>(deserializer: D) -> std::result::Result<Vec<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Array(arr) => Ok(arr),
        Value::Object(_) => Ok(vec![value]),
        Value::Null => Ok(vec![]),
        other => Ok(vec![other]),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcResponse {
    Success {
        jsonrpc: String,
        result: Value,
        id: Value,
    },
    Error {
        jsonrpc: String,
        error: JsonRpcError,
        id: Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(result: Value, id: Value) -> Self {
        Self::Success {
            jsonrpc: "2.0".to_string(),
            result,
            id,
        }
    }

    pub fn error(code: i32, message: String, id: Value) -> Self {
        Self::Error {
            jsonrpc: "2.0".to_string(),
            error: JsonRpcError {
                code,
                message,
                data: None,
            },
            id,
        }
    }

    pub fn error_with_data(code: i32, message: String, data: Value, id: Value) -> Self {
        Self::Error {
            jsonrpc: "2.0".to_string(),
            error: JsonRpcError {
                code,
                message,
                data: Some(data),
            },
            id,
        }
    }
}

pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;
