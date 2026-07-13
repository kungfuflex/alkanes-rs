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

// ---------------------------------------------------------------------------
// JsonRpcResponse — permissive Deserialize (bitcoind JSON-RPC 1.0 compat)
//
// WHY this is NOT a derived `#[serde(untagged)]` Deserialize:
//
// Bitcoin Core's JSON-RPC 1.0 envelopes carry BOTH `result` AND `error`
// members in EVERY response (`"error": null` on success, `"result": null` on
// error). A derived untagged Deserialize tries variants in declaration order,
// so an ERROR envelope like
//   {"jsonrpc":"1.0","result":null,"error":{"code":-26,"message":"min relay fee not met"},"id":1}
// matches `Success` first (`result` is present — even as null — and the
// unknown `error` field is silently dropped). The upstream reject reason is
// DESTROYED: gateways that re-serialize this value hand clients an HTTP 200
// `{"jsonrpc":"1.0","result":null,"id":N}` with no error member at all.
// Verified live on mainnet 2026-07-13 (sendrawtransaction rejects surfacing
// as opaque "Invalid txid response" toasts, -26/-22 codes stripped).
//
// The rigid variants also HARD-FAIL on legitimate 1.0 shapes: pure JSON-RPC
// 1.0 peers may omit the `jsonrpc` member entirely, which matches NEITHER
// variant and produces
//   -32603 "data did not match any variant of untagged enum JsonRpcResponse"
// (observed live on signet for btc_* passthrough).
//
// Fix: deserialize into a permissive raw struct (every member optional,
// any combination tolerated), then branch EXPLICITLY on the JSON-RPC 1.0
// rule — `error` present-and-non-null wins, everything else is a success.
// The enum's public shape and its (untagged) Serialize output are
// byte-identical to before, so call sites and re-serialization paths are
// unaffected; only the decode direction changes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
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

/// Permissive wire shape for INCOMING JSON-RPC response envelopes.
///
/// Every member is optional so any combination parses: 2.0 (`result` XOR
/// `error`), bitcoind 1.0 (both members, one null), and pure 1.0 with no
/// `jsonrpc` member. `error` is captured as a raw `Value` (not
/// `Option<JsonRpcError>`) so a nonconforming error object — string errors,
/// non-i32 codes, missing `message` — degrades to a synthesized
/// `JsonRpcError` that preserves the raw payload instead of hard-failing the
/// whole envelope. A gateway must never destroy the upstream error twice.
#[derive(Deserialize)]
struct RawJsonRpcResponse {
    #[serde(default)]
    jsonrpc: Option<String>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<Value>,
    #[serde(default)]
    id: Option<Value>,
}

impl From<RawJsonRpcResponse> for JsonRpcResponse {
    fn from(raw: RawJsonRpcResponse) -> Self {
        // Preserve the upstream `jsonrpc` tag when present ("1.0" stays
        // "1.0"); default to "2.0" (matching this crate's constructors) when
        // a pure 1.0 peer omitted it.
        let jsonrpc = raw.jsonrpc.unwrap_or_else(|| "2.0".to_string());
        let id = raw.id.unwrap_or(Value::Null);
        match raw.error {
            // JSON-RPC 1.0 rule: a present, non-null `error` member means
            // error — regardless of what `result` holds.
            Some(err_val) if !err_val.is_null() => {
                let error = serde_json::from_value::<JsonRpcError>(err_val.clone())
                    .unwrap_or_else(|_| JsonRpcError {
                        code: INTERNAL_ERROR,
                        message: err_val.to_string(),
                        data: Some(err_val),
                    });
                JsonRpcResponse::Error { jsonrpc, error, id }
            }
            // `error` absent or null → success; `result: null` is a valid
            // success payload (e.g. bitcoind `submitblock`).
            _ => JsonRpcResponse::Success {
                jsonrpc,
                result: raw.result.unwrap_or(Value::Null),
                id,
            },
        }
    }
}

impl<'de> Deserialize<'de> for JsonRpcResponse {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Manual impl (instead of `#[serde(from = "...")]` on the enum) so the
        // untagged Serialize derive above stays exactly as it was — the two
        // attributes' interaction is not something we want to depend on.
        RawJsonRpcResponse::deserialize(deserializer).map(Into::into)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Regression tests for the bitcoind JSON-RPC 1.0 envelope trap. Each wire
    // shape below was observed live (mainnet/signet, 2026-07-13) or is a spec
    // shape the old `#[serde(untagged)]` Deserialize mishandled. See the
    // rationale block above `JsonRpcResponse`.

    /// bitcoind 1.0 ERROR envelope: BOTH members present, `result: null`.
    /// Old behavior: matched `Success` (error member silently dropped) —
    /// clients received `{"result":null}` with the -26 reject reason DESTROYED.
    #[test]
    fn bitcoind_1_0_error_envelope_both_members_preserves_error() {
        let wire = r#"{"jsonrpc":"1.0","result":null,"error":{"code":-26,"message":"min relay fee not met"},"id":1}"#;
        let resp: JsonRpcResponse = serde_json::from_str(wire).expect("must parse");
        match resp {
            JsonRpcResponse::Error { jsonrpc, error, id } => {
                assert_eq!(jsonrpc, "1.0");
                assert_eq!(error.code, -26);
                assert_eq!(error.message, "min relay fee not met");
                assert_eq!(error.data, None);
                assert_eq!(id, json!(1));
            }
            other => panic!("expected Error variant, got {:?}", other),
        }
    }

    /// bitcoind 1.0 SUCCESS envelope: `error: null` present alongside result.
    #[test]
    fn bitcoind_1_0_success_envelope_error_null_keeps_result() {
        let wire = r#"{"jsonrpc":"1.0","result":"deadbeefcafe","error":null,"id":42}"#;
        let resp: JsonRpcResponse = serde_json::from_str(wire).expect("must parse");
        match resp {
            JsonRpcResponse::Success { jsonrpc, result, id } => {
                assert_eq!(jsonrpc, "1.0");
                assert_eq!(result, json!("deadbeefcafe"));
                assert_eq!(id, json!(42));
            }
            other => panic!("expected Success variant, got {:?}", other),
        }
    }

    /// Pure JSON-RPC 1.0: NO `jsonrpc` member at all. This is the suspected
    /// signet hard-fail shape — the old rigid variants matched NEITHER arm and
    /// the gateway surfaced
    ///   -32603 "dispatch error: bitcoind bad json (status 200): data did not
    ///   match any variant of untagged enum JsonRpcResponse".
    #[test]
    fn pure_1_0_envelope_without_jsonrpc_member_parses() {
        // error flavor
        let wire = r#"{"result":null,"error":{"code":-25,"message":"bad-txns-inputs-missingorspent"},"id":"curltest"}"#;
        let resp: JsonRpcResponse = serde_json::from_str(wire).expect("must parse without jsonrpc member");
        match resp {
            JsonRpcResponse::Error { error, id, .. } => {
                assert_eq!(error.code, -25);
                assert_eq!(id, json!("curltest"));
            }
            other => panic!("expected Error variant, got {:?}", other),
        }
        // success flavor
        let wire = r#"{"result":863500,"error":null,"id":7}"#;
        let resp: JsonRpcResponse = serde_json::from_str(wire).expect("must parse without jsonrpc member");
        match resp {
            JsonRpcResponse::Success { result, .. } => assert_eq!(result, json!(863500)),
            other => panic!("expected Success variant, got {:?}", other),
        }
    }

    /// JSON-RPC 2.0 error: no `result` member at all.
    #[test]
    fn jsonrpc_2_0_error_without_result_member() {
        let wire = r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":2}"#;
        let resp: JsonRpcResponse = serde_json::from_str(wire).expect("must parse");
        match resp {
            JsonRpcResponse::Error { jsonrpc, error, id } => {
                assert_eq!(jsonrpc, "2.0");
                assert_eq!(error.code, METHOD_NOT_FOUND);
                assert_eq!(id, json!(2));
            }
            other => panic!("expected Error variant, got {:?}", other),
        }
    }

    /// JSON-RPC 2.0 success: no `error` member at all.
    #[test]
    fn jsonrpc_2_0_success_without_error_member() {
        let wire = r#"{"jsonrpc":"2.0","result":{"ok":true},"id":3}"#;
        let resp: JsonRpcResponse = serde_json::from_str(wire).expect("must parse");
        match resp {
            JsonRpcResponse::Success { result, id, .. } => {
                assert_eq!(result, json!({"ok": true}));
                assert_eq!(id, json!(3));
            }
            other => panic!("expected Success variant, got {:?}", other),
        }
    }

    /// Serialize output must be byte-identical to the pre-fix derived
    /// `#[serde(untagged)]` Serialize (expected JSON constructed explicitly)
    /// so re-serialization paths — gateways included — are unaffected.
    #[test]
    fn serialize_output_byte_compatible_with_pre_fix() {
        let success = JsonRpcResponse::success(json!("abc"), json!(1));
        assert_eq!(
            serde_json::to_string(&success).unwrap(),
            r#"{"jsonrpc":"2.0","result":"abc","id":1}"#
        );

        let error = JsonRpcResponse::error(-26, "min relay fee not met".to_string(), json!(1));
        assert_eq!(
            serde_json::to_string(&error).unwrap(),
            r#"{"jsonrpc":"2.0","error":{"code":-26,"message":"min relay fee not met"},"id":1}"#
        );

        let error_with_data = JsonRpcResponse::error_with_data(
            INVALID_PARAMS,
            "bad params".to_string(),
            json!({"got": 0}),
            json!("x"),
        );
        assert_eq!(
            serde_json::to_string(&error_with_data).unwrap(),
            r#"{"jsonrpc":"2.0","error":{"code":-32602,"message":"bad params","data":{"got":0}},"id":"x"}"#
        );
    }

    /// The exact production failure mode, end to end: a bitcoind reject must
    /// SURVIVE a decode → re-serialize round trip with its error member intact.
    #[test]
    fn error_envelope_survives_decode_reserialize_roundtrip() {
        let wire = r#"{"jsonrpc":"1.0","result":null,"error":{"code":-22,"message":"TX decode failed"},"id":9}"#;
        let resp: JsonRpcResponse = serde_json::from_str(wire).unwrap();
        let out: Value = serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(out["error"]["code"], json!(-22));
        assert_eq!(out["error"]["message"], json!("TX decode failed"));
        assert!(out.get("result").is_none(), "untagged Error variant carries no result member");
    }

    /// A nonconforming error object (e.g. a bare string) degrades to a
    /// synthesized JsonRpcError preserving the raw payload — never a hard
    /// deserialization failure.
    #[test]
    fn malformed_error_member_degrades_instead_of_failing() {
        let wire = r#"{"result":null,"error":"upstream exploded","id":1}"#;
        let resp: JsonRpcResponse = serde_json::from_str(wire).expect("must not hard-fail");
        match resp {
            JsonRpcResponse::Error { error, .. } => {
                assert_eq!(error.code, INTERNAL_ERROR);
                assert!(error.message.contains("upstream exploded"));
                assert_eq!(error.data, Some(json!("upstream exploded")));
            }
            other => panic!("expected Error variant, got {:?}", other),
        }
    }

    /// Missing `id` / missing `result` on success default to null rather than
    /// rejecting the envelope.
    #[test]
    fn missing_members_default_to_null() {
        let resp: JsonRpcResponse = serde_json::from_str(r#"{"result":true}"#).unwrap();
        match resp {
            JsonRpcResponse::Success { jsonrpc, result, id } => {
                assert_eq!(jsonrpc, "2.0"); // constructor-matching default
                assert_eq!(result, json!(true));
                assert_eq!(id, Value::Null);
            }
            other => panic!("expected Success variant, got {:?}", other),
        }
    }
}
