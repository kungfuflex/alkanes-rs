// Copyright (c) 2024 The Qubitcoin developers
// Distributed under the MIT software license.

//! JSON-RPC server framework for Qubitcoin.
//!
//! Provides JSON-RPC 2.0 compliant request/response types, an RPC method
//! registry for dispatching handlers, and request processing utilities.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// JSON-RPC types
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 request.
#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    /// The JSON-RPC version string (should be "2.0").
    pub jsonrpc: Option<String>,
    /// The name of the method to invoke.
    pub method: String,
    /// Optional parameters for the method.
    pub params: Option<serde_json::Value>,
    /// Request identifier (number, string, or null).
    pub id: serde_json::Value,
}

/// JSON-RPC version detected from the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonRpcVersion {
    /// JSON-RPC 1.0 (legacy): include both result and error, no "jsonrpc" field.
    V1Legacy,
    /// JSON-RPC 2.0: include "jsonrpc":"2.0", omit null result/error.
    V2,
}

/// A JSON-RPC response.
///
/// Supports both JSON-RPC 1.0 (legacy) and 2.0 formatting:
/// - V1: Both `result` and `error` always present (one as null), no `jsonrpc`.
/// - V2: `jsonrpc` is `"2.0"`, null fields omitted.
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcResponse {
    /// "2.0" for V2, omitted for V1 legacy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jsonrpc: Option<String>,
    /// The result on success, `None` on error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// The error on failure, `None` on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    /// The id matching the request.
    pub id: serde_json::Value,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    /// Numeric error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
    /// Optional additional data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Standard JSON-RPC error codes (spec-defined)
// ---------------------------------------------------------------------------

/// Invalid JSON was received by the server.
pub const RPC_PARSE_ERROR: i32 = -32700;
/// The JSON sent is not a valid Request object.
pub const RPC_INVALID_REQUEST: i32 = -32600;
/// The method does not exist / is not available.
pub const RPC_METHOD_NOT_FOUND: i32 = -32601;
/// Invalid method parameter(s).
pub const RPC_INVALID_PARAMS: i32 = -32602;
/// Internal JSON-RPC error.
pub const RPC_INTERNAL_ERROR: i32 = -32603;

// ---------------------------------------------------------------------------
// Bitcoin-compatible application error codes
// ---------------------------------------------------------------------------

/// Miscellaneous error.
pub const RPC_MISC_ERROR: i32 = -1;
/// Unexpected type was passed as parameter.
pub const RPC_TYPE_ERROR: i32 = -3;
/// Invalid address or key.
pub const RPC_INVALID_ADDRESS_OR_KEY: i32 = -5;
/// Ran out of memory during operation.
pub const RPC_OUT_OF_MEMORY: i32 = -7;
/// Invalid, missing, or duplicate parameter.
pub const RPC_INVALID_PARAMETER: i32 = -8;
/// Database error.
pub const RPC_DATABASE_ERROR: i32 = -20;
/// Error parsing or validating structure in raw format.
pub const RPC_DESERIALIZATION_ERROR: i32 = -22;
/// General error during transaction or block submission.
pub const RPC_VERIFY_ERROR: i32 = -25;
/// Transaction or block was rejected by network rules.
pub const RPC_VERIFY_REJECTED: i32 = -26;
/// Server is still warming up.
pub const RPC_IN_WARMUP: i32 = -28;

// ---------------------------------------------------------------------------
// RpcResponse constructors
// ---------------------------------------------------------------------------

impl RpcResponse {
    /// Detect JSON-RPC version from a request.
    pub fn detect_version(req: &RpcRequest) -> JsonRpcVersion {
        match req.jsonrpc.as_deref() {
            Some("2.0") => JsonRpcVersion::V2,
            _ => JsonRpcVersion::V1Legacy,
        }
    }

    /// Create a successful response (defaults to V2 format).
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        RpcResponse {
            jsonrpc: Some("2.0".to_string()),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response (defaults to V2 format).
    pub fn error(id: serde_json::Value, code: i32, message: String) -> Self {
        RpcResponse {
            jsonrpc: Some("2.0".to_string()),
            result: None,
            error: Some(RpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }

    /// Create an error response with additional data.
    pub fn error_with_data(
        id: serde_json::Value,
        code: i32,
        message: String,
        data: serde_json::Value,
    ) -> Self {
        RpcResponse {
            jsonrpc: Some("2.0".to_string()),
            result: None,
            error: Some(RpcError {
                code,
                message,
                data: Some(data),
            }),
            id,
        }
    }
}

// ---------------------------------------------------------------------------
// Auth tier
// ---------------------------------------------------------------------------

/// Authentication tier for RPC methods.
///
/// Determines whether a method requires HTTP Basic Auth credentials.
/// Designed for reverse-proxy setups where Public methods are exposed
/// to the world and Admin methods are restricted to operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthTier {
    /// No authentication required. Safe for public reverse-proxy exposure.
    Public,
    /// Requires rpcuser/rpcpassword HTTP Basic Auth credentials.
    Admin,
}

// ---------------------------------------------------------------------------
// RPC handler type and registry
// ---------------------------------------------------------------------------

/// An RPC method handler: receives a request reference and returns a response.
pub type RpcHandler = Box<dyn Fn(&RpcRequest) -> RpcResponse + Send + Sync>;

/// Registry that maps method names to handler functions with auth tiers.
///
/// Supports optional per-user method whitelisting via `rpcwhitelist`,
/// matching Bitcoin Core's `-rpcwhitelist=user:method1,method2` pattern.
pub struct RpcRegistry {
    methods: HashMap<String, (AuthTier, RpcHandler)>,
    /// Optional per-user method whitelist. If set for a user, only listed
    /// methods are allowed. Users not in the map have no restrictions.
    pub rpcwhitelist: Option<HashMap<String, HashSet<String>>>,
}

impl RpcRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        RpcRegistry {
            methods: HashMap::new(),
            rpcwhitelist: None,
        }
    }

    /// Register a handler with an explicit auth tier.
    pub fn register_with_tier<F>(&mut self, method: &str, tier: AuthTier, handler: F)
    where
        F: Fn(&RpcRequest) -> RpcResponse + Send + Sync + 'static,
    {
        self.methods
            .insert(method.to_string(), (tier, Box::new(handler)));
    }

    /// Register a Public (no auth required) handler.
    pub fn register_public<F>(&mut self, method: &str, handler: F)
    where
        F: Fn(&RpcRequest) -> RpcResponse + Send + Sync + 'static,
    {
        self.register_with_tier(method, AuthTier::Public, handler);
    }

    /// Register an Admin (auth required) handler.
    pub fn register_admin<F>(&mut self, method: &str, handler: F)
    where
        F: Fn(&RpcRequest) -> RpcResponse + Send + Sync + 'static,
    {
        self.register_with_tier(method, AuthTier::Admin, handler);
    }

    /// Backwards-compatible register — defaults to Public tier.
    pub fn register<F>(&mut self, method: &str, handler: F)
    where
        F: Fn(&RpcRequest) -> RpcResponse + Send + Sync + 'static,
    {
        self.register_public(method, handler);
    }

    /// Query the auth tier for a method. Returns `None` if unregistered.
    pub fn auth_tier_for(&self, method: &str) -> Option<AuthTier> {
        self.methods.get(method).map(|(tier, _)| *tier)
    }

    /// Dispatch a request to the appropriate handler.
    ///
    /// Returns a `method not found` error if no handler is registered.
    pub fn dispatch(&self, request: &RpcRequest) -> RpcResponse {
        match self.methods.get(&request.method) {
            Some((_tier, handler)) => handler(request),
            None => {
                let method_bytes: Vec<u8> = request.method.bytes().collect();
                eprintln!(
                    "[RPC dispatch] Method not found: {:?} (bytes: {:?}, registry has {} methods)",
                    request.method,
                    &method_bytes[..method_bytes.len().min(30)],
                    self.methods.len()
                );
                RpcResponse::error(
                    request.id.clone(),
                    RPC_METHOD_NOT_FOUND,
                    format!("Method not found: {}", request.method),
                )
            }
        }
    }

    /// Check if a user is allowed to call a method per `rpcwhitelist`.
    ///
    /// Returns `true` if no whitelist is configured, or if the user is not
    /// in the whitelist map (unrestricted), or if the method is in their
    /// allowed set.
    pub fn user_allowed(&self, user: &str, method: &str) -> bool {
        match &self.rpcwhitelist {
            None => true,
            Some(whitelist) => match whitelist.get(user) {
                None => true, // user not in whitelist → unrestricted
                Some(allowed) => allowed.contains(method),
            },
        }
    }

    /// Return the number of registered methods.
    pub fn method_count(&self) -> usize {
        self.methods.len()
    }

    /// Check whether a method is registered.
    pub fn has_method(&self, method: &str) -> bool {
        self.methods.contains_key(method)
    }

    /// Return a sorted list of registered method names.
    pub fn method_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.methods.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for RpcRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Raw request processing
// ---------------------------------------------------------------------------

/// Parse a raw JSON string as an RPC request, dispatch it through the
/// registry, and return the serialized JSON response string.
pub fn process_request(registry: &RpcRegistry, raw: &str) -> String {
    let request = match parse_rpc_request(raw) {
        Ok(req) => req,
        Err(resp_str) => return resp_str,
    };
    let response = registry.dispatch(&request);
    serde_json::to_string(&response).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error"},"id":null}"#
            .to_string()
    })
}

/// Parse and validate a raw JSON string into an `RpcRequest`.
///
/// Returns `Ok(request)` or `Err(serialized_error_response)`.
pub fn parse_rpc_request(raw: &str) -> Result<RpcRequest, String> {
    let request: RpcRequest = match serde_json::from_str(raw) {
        Ok(req) => req,
        Err(e) => {
            let resp = RpcResponse::error(
                serde_json::Value::Null,
                RPC_PARSE_ERROR,
                format!("Parse error: {}", e),
            );
            return Err(serde_json::to_string(&resp).unwrap_or_else(|_| {
                r#"{"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"},"id":null}"#
                    .to_string()
            }));
        }
    };

    if request.method.is_empty() {
        let resp = RpcResponse::error(
            request.id.clone(),
            RPC_INVALID_REQUEST,
            "Invalid request: method is empty".to_string(),
        );
        return Err(serde_json::to_string(&resp).unwrap_or_default());
    }

    Ok(request)
}

/// HTTP 403 Forbidden error code for RPC whitelist denials.
pub const RPC_FORBIDDEN: i32 = -32604;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_rpc_response_success() {
        let resp = RpcResponse::success(json!(1), json!({"height": 100}));
        assert_eq!(resp.jsonrpc, Some("2.0".to_string()));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        assert_eq!(resp.id, json!(1));

        let serialized = serde_json::to_string(&resp).unwrap();
        assert!(serialized.contains("\"jsonrpc\":\"2.0\""));
        assert!(serialized.contains("\"height\":100"));
        assert!(!serialized.contains("\"error\""));
    }

    #[test]
    fn test_rpc_response_error() {
        let resp = RpcResponse::error(json!("abc"), RPC_METHOD_NOT_FOUND, "Not found".into());
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        let err = resp.error.as_ref().unwrap();
        assert_eq!(err.code, RPC_METHOD_NOT_FOUND);
        assert_eq!(err.message, "Not found");
        assert!(err.data.is_none());
    }

    #[test]
    fn test_rpc_response_error_with_data() {
        let resp = RpcResponse::error_with_data(
            json!(2),
            RPC_INVALID_PARAMS,
            "Bad param".into(),
            json!({"param": "height"}),
        );
        let err = resp.error.as_ref().unwrap();
        assert_eq!(err.code, RPC_INVALID_PARAMS);
        assert!(err.data.is_some());
    }

    #[test]
    fn test_rpc_request_deserialization() {
        let raw = r#"{"jsonrpc":"2.0","method":"getblockcount","params":[],"id":1}"#;
        let req: RpcRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.method, "getblockcount");
        assert_eq!(req.id, json!(1));
        assert_eq!(req.jsonrpc, Some("2.0".to_string()));
    }

    #[test]
    fn test_rpc_request_without_jsonrpc_field() {
        let raw = r#"{"method":"help","id":"test-1"}"#;
        let req: RpcRequest = serde_json::from_str(raw).unwrap();
        assert!(req.jsonrpc.is_none());
        assert!(req.params.is_none());
    }

    #[test]
    fn test_registry_register_and_dispatch() {
        let mut reg = RpcRegistry::new();
        reg.register("echo", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!("echoed"))
        });

        assert!(reg.has_method("echo"));
        assert!(!reg.has_method("nonexistent"));

        let req = RpcRequest {
            jsonrpc: Some("2.0".into()),
            method: "echo".into(),
            params: None,
            id: json!(42),
        };
        let resp = reg.dispatch(&req);
        assert_eq!(resp.result, Some(json!("echoed")));
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_registry_method_not_found() {
        let reg = RpcRegistry::new();
        let req = RpcRequest {
            jsonrpc: Some("2.0".into()),
            method: "nonexistent".into(),
            params: None,
            id: json!(1),
        };
        let resp = reg.dispatch(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, RPC_METHOD_NOT_FOUND);
    }

    #[test]
    fn test_registry_method_names_sorted() {
        let mut reg = RpcRegistry::new();
        reg.register("zmethod", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!(null))
        });
        reg.register("amethod", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!(null))
        });
        reg.register("mmethod", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!(null))
        });

        let names = reg.method_names();
        assert_eq!(names, vec!["amethod", "mmethod", "zmethod"]);
    }

    #[test]
    fn test_registry_replace_handler() {
        let mut reg = RpcRegistry::new();
        reg.register("test", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!("first"))
        });
        reg.register("test", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!("second"))
        });

        let req = RpcRequest {
            jsonrpc: Some("2.0".into()),
            method: "test".into(),
            params: None,
            id: json!(1),
        };
        let resp = reg.dispatch(&req);
        assert_eq!(resp.result, Some(json!("second")));
    }

    #[test]
    fn test_process_request_valid() {
        let mut reg = RpcRegistry::new();
        reg.register("ping", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!("pong"))
        });

        let raw = r#"{"method":"ping","id":1}"#;
        let resp_str = process_request(&reg, raw);
        let resp: RpcResponse = serde_json::from_str(&resp_str).unwrap();
        assert_eq!(resp.result, Some(json!("pong")));
    }

    #[test]
    fn test_process_request_parse_error() {
        let reg = RpcRegistry::new();
        let raw = "this is not json";
        let resp_str = process_request(&reg, raw);
        let resp: RpcResponse = serde_json::from_str(&resp_str).unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, RPC_PARSE_ERROR);
        assert_eq!(resp.id, json!(null));
    }

    #[test]
    fn test_process_request_empty_method() {
        let reg = RpcRegistry::new();
        let raw = r#"{"method":"","id":5}"#;
        let resp_str = process_request(&reg, raw);
        let resp: RpcResponse = serde_json::from_str(&resp_str).unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, RPC_INVALID_REQUEST);
        assert_eq!(resp.id, json!(5));
    }

    #[test]
    fn test_process_request_method_not_found() {
        let reg = RpcRegistry::new();
        let raw = r#"{"method":"doesntexist","id":99}"#;
        let resp_str = process_request(&reg, raw);
        let resp: RpcResponse = serde_json::from_str(&resp_str).unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, RPC_METHOD_NOT_FOUND);
    }

    #[test]
    fn test_response_roundtrip_serialization() {
        let original = RpcResponse::success(json!(1), json!({"key": "value"}));
        let json_str = serde_json::to_string(&original).unwrap();
        let deserialized: RpcResponse = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.jsonrpc, Some("2.0".to_string()));
        assert_eq!(deserialized.result, Some(json!({"key": "value"})));
        assert_eq!(deserialized.id, json!(1));
    }

    // -- Auth tier tests ---------------------------------------------------

    #[test]
    fn test_register_public_sets_public_tier() {
        let mut reg = RpcRegistry::new();
        reg.register_public("getblockcount", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!(42))
        });
        assert_eq!(reg.auth_tier_for("getblockcount"), Some(AuthTier::Public));
    }

    #[test]
    fn test_register_admin_sets_admin_tier() {
        let mut reg = RpcRegistry::new();
        reg.register_admin("stop", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!("stopping"))
        });
        assert_eq!(reg.auth_tier_for("stop"), Some(AuthTier::Admin));
    }

    #[test]
    fn test_auth_tier_for_unregistered() {
        let reg = RpcRegistry::new();
        assert_eq!(reg.auth_tier_for("nonexistent"), None);
    }

    #[test]
    fn test_register_defaults_to_public() {
        let mut reg = RpcRegistry::new();
        reg.register("help", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!("help text"))
        });
        assert_eq!(reg.auth_tier_for("help"), Some(AuthTier::Public));
    }

    #[test]
    fn test_mixed_public_and_admin() {
        let mut reg = RpcRegistry::new();
        reg.register_public("getblockcount", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!(100))
        });
        reg.register_admin("stop", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!("ok"))
        });
        assert_eq!(reg.auth_tier_for("getblockcount"), Some(AuthTier::Public));
        assert_eq!(reg.auth_tier_for("stop"), Some(AuthTier::Admin));
        assert_eq!(reg.method_count(), 2);
    }

    #[test]
    fn test_dispatch_works_regardless_of_tier() {
        let mut reg = RpcRegistry::new();
        reg.register_admin("admin_method", |req: &RpcRequest| {
            RpcResponse::success(req.id.clone(), json!("admin_result"))
        });
        let req = RpcRequest {
            jsonrpc: Some("2.0".into()),
            method: "admin_method".into(),
            params: None,
            id: json!(1),
        };
        let resp = reg.dispatch(&req);
        assert_eq!(resp.result, Some(json!("admin_result")));
    }

    // -- rpcwhitelist tests ------------------------------------------------

    #[test]
    fn test_user_allowed_no_whitelist() {
        let reg = RpcRegistry::new();
        assert!(reg.user_allowed("anyone", "anything"));
    }

    #[test]
    fn test_user_allowed_user_not_in_whitelist() {
        let mut reg = RpcRegistry::new();
        let mut wl = HashMap::new();
        wl.insert(
            "restricted_user".to_string(),
            HashSet::from(["getblockcount".to_string()]),
        );
        reg.rpcwhitelist = Some(wl);
        // User "other" is not in the whitelist → unrestricted.
        assert!(reg.user_allowed("other", "stop"));
    }

    #[test]
    fn test_user_allowed_method_in_whitelist() {
        let mut reg = RpcRegistry::new();
        let mut wl = HashMap::new();
        wl.insert(
            "viewer".to_string(),
            HashSet::from(["getblockcount".to_string(), "help".to_string()]),
        );
        reg.rpcwhitelist = Some(wl);
        assert!(reg.user_allowed("viewer", "getblockcount"));
        assert!(reg.user_allowed("viewer", "help"));
    }

    #[test]
    fn test_user_allowed_method_not_in_whitelist() {
        let mut reg = RpcRegistry::new();
        let mut wl = HashMap::new();
        wl.insert(
            "viewer".to_string(),
            HashSet::from(["getblockcount".to_string()]),
        );
        reg.rpcwhitelist = Some(wl);
        assert!(!reg.user_allowed("viewer", "stop"));
        assert!(!reg.user_allowed("viewer", "indexerpause"));
    }

    // -- parse_rpc_request tests -------------------------------------------

    #[test]
    fn test_parse_rpc_request_valid() {
        let raw = r#"{"method":"ping","id":1}"#;
        let req = parse_rpc_request(raw).unwrap();
        assert_eq!(req.method, "ping");
        assert_eq!(req.id, json!(1));
    }

    #[test]
    fn test_parse_rpc_request_invalid_json() {
        let result = parse_rpc_request("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_rpc_request_empty_method() {
        let result = parse_rpc_request(r#"{"method":"","id":1}"#);
        assert!(result.is_err());
    }
}
