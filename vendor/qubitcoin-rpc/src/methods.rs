// Copyright (c) 2024 The Qubitcoin developers
// Distributed under the MIT software license.

//! RPC method stub implementations for Qubitcoin.
//!
//! Each method returns plausible static/default data. These stubs will be
//! wired up to the real node subsystems in a future phase.

use crate::server::{
    RpcRegistry, RpcRequest, RpcResponse, RPC_INVALID_PARAMS, RPC_METHOD_NOT_FOUND,
};
use serde_json::json;

/// The genesis block hash used in stub responses.
const GENESIS_HASH: &str = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";

/// Register all RPC methods on the given registry.
pub fn register_all(registry: &mut RpcRegistry) {
    register_blockchain_rpcs(registry);
    register_mining_rpcs(registry);
    register_network_rpcs(registry);
    register_mempool_rpcs(registry);
    register_util_rpcs(registry);
}

// ---------------------------------------------------------------------------
// Blockchain RPCs
// ---------------------------------------------------------------------------

fn register_blockchain_rpcs(registry: &mut RpcRegistry) {
    // getblockchaininfo
    registry.register("getblockchaininfo", |req: &RpcRequest| {
        RpcResponse::success(
            req.id.clone(),
            json!({
                "chain": "main",
                "blocks": 0,
                "headers": 0,
                "bestblockhash": GENESIS_HASH,
                "difficulty": 1.0,
                "mediantime": 1231006505_u64,
                "verificationprogress": 1.0,
                "initialblockdownload": false,
                "chainwork": "0000000000000000000000000000000000000000000000000000000100010001",
                "size_on_disk": 285,
                "pruned": false,
                "warnings": ""
            }),
        )
    });

    // getblockcount
    registry.register("getblockcount", |req: &RpcRequest| {
        RpcResponse::success(req.id.clone(), json!(0))
    });

    // getblockhash
    registry.register("getblockhash", |req: &RpcRequest| {
        let height = req
            .params
            .as_ref()
            .and_then(|p| {
                if let Some(arr) = p.as_array() {
                    arr.first().and_then(|v| v.as_i64())
                } else {
                    p.get("height").and_then(|v| v.as_i64())
                }
            })
            .unwrap_or(-1);

        if height == 0 {
            RpcResponse::success(req.id.clone(), json!(GENESIS_HASH))
        } else {
            RpcResponse::error(
                req.id.clone(),
                RPC_INVALID_PARAMS,
                format!("Block height {} out of range", height),
            )
        }
    });

    // getblock
    registry.register("getblock", |req: &RpcRequest| {
        let _blockhash = req.params.as_ref().and_then(|p| {
            if let Some(arr) = p.as_array() {
                arr.first().and_then(|v| v.as_str())
            } else {
                p.get("blockhash").and_then(|v| v.as_str())
            }
        });

        RpcResponse::success(
            req.id.clone(),
            json!({
                "hash": GENESIS_HASH,
                "confirmations": 1,
                "height": 0,
                "version": 1,
                "versionHex": "00000001",
                "merkleroot": "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
                "time": 1231006505_u64,
                "mediantime": 1231006505_u64,
                "nonce": 2083236893_u64,
                "bits": "1d00ffff",
                "difficulty": 1.0,
                "chainwork": "0000000000000000000000000000000000000000000000000000000100010001",
                "nTx": 1,
                "previousblockhash": null,
                "nextblockhash": null,
                "size": 285,
                "weight": 1140,
                "tx": [
                    "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
                ]
            }),
        )
    });

    // getbestblockhash
    registry.register("getbestblockhash", |req: &RpcRequest| {
        RpcResponse::success(req.id.clone(), json!(GENESIS_HASH))
    });

    // gettxout
    registry.register("gettxout", |req: &RpcRequest| {
        // For the stub, return null (no UTXO found) which is a valid response.
        RpcResponse::success(req.id.clone(), json!(null))
    });

    // getdifficulty
    registry.register("getdifficulty", |req: &RpcRequest| {
        RpcResponse::success(req.id.clone(), json!(1.0))
    });
}

// ---------------------------------------------------------------------------
// Mining RPCs
// ---------------------------------------------------------------------------

fn register_mining_rpcs(registry: &mut RpcRegistry) {
    // getmininginfo
    registry.register("getmininginfo", |req: &RpcRequest| {
        RpcResponse::success(
            req.id.clone(),
            json!({
                "blocks": 0,
                "difficulty": 1.0,
                "networkhashps": 0,
                "pooledtx": 0,
                "chain": "main",
                "warnings": ""
            }),
        )
    });

    // getnetworkhashps
    registry.register("getnetworkhashps", |req: &RpcRequest| {
        RpcResponse::success(req.id.clone(), json!(0))
    });

    // generatetoaddress (regtest only stub)
    registry.register("generatetoaddress", |req: &RpcRequest| {
        let nblocks = req
            .params
            .as_ref()
            .and_then(|p| {
                if let Some(arr) = p.as_array() {
                    arr.first().and_then(|v| v.as_u64())
                } else {
                    p.get("nblocks").and_then(|v| v.as_u64())
                }
            })
            .unwrap_or(0);

        // Return a list of fake block hashes.
        let hashes: Vec<String> = (0..nblocks).map(|i| format!("{:064x}", i + 1)).collect();

        RpcResponse::success(req.id.clone(), json!(hashes))
    });
}

// ---------------------------------------------------------------------------
// Network RPCs
// ---------------------------------------------------------------------------

fn register_network_rpcs(registry: &mut RpcRegistry) {
    // getnetworkinfo
    registry.register("getnetworkinfo", |req: &RpcRequest| {
        RpcResponse::success(
            req.id.clone(),
            json!({
                "version": 10000,
                "subversion": "/Qubitcoin:0.1.0/",
                "protocolversion": 70016,
                "localservices": "0000000000000409",
                "localservicesnames": ["NETWORK", "WITNESS", "NETWORK_LIMITED"],
                "localrelay": true,
                "timeoffset": 0,
                "networkactive": true,
                "connections": 0,
                "connections_in": 0,
                "connections_out": 0,
                "networks": [
                    {
                        "name": "ipv4",
                        "limited": false,
                        "reachable": true,
                        "proxy": "",
                        "proxy_randomize_credentials": false
                    },
                    {
                        "name": "ipv6",
                        "limited": false,
                        "reachable": true,
                        "proxy": "",
                        "proxy_randomize_credentials": false
                    }
                ],
                "relayfee": 0.00001000,
                "incrementalfee": 0.00001000,
                "localaddresses": [],
                "warnings": ""
            }),
        )
    });

    // getpeerinfo
    registry.register("getpeerinfo", |req: &RpcRequest| {
        // No peers connected in stub.
        RpcResponse::success(req.id.clone(), json!([]))
    });

    // getconnectioncount
    registry.register("getconnectioncount", |req: &RpcRequest| {
        RpcResponse::success(req.id.clone(), json!(0))
    });

    // getnettotals
    registry.register("getnettotals", |req: &RpcRequest| {
        RpcResponse::success(
            req.id.clone(),
            json!({
                "totalbytesrecv": 0,
                "totalbytessent": 0,
                "timemillis": 1231006505000_u64,
                "uploadtarget": {
                    "timeframe": 86400,
                    "target": 0,
                    "target_reached": false,
                    "serve_historical_blocks": true,
                    "bytes_left_in_cycle": 0,
                    "time_left_in_cycle": 0
                }
            }),
        )
    });
}

// ---------------------------------------------------------------------------
// Mempool RPCs
// ---------------------------------------------------------------------------

fn register_mempool_rpcs(registry: &mut RpcRegistry) {
    // getmempoolinfo
    registry.register("getmempoolinfo", |req: &RpcRequest| {
        RpcResponse::success(
            req.id.clone(),
            json!({
                "loaded": true,
                "size": 0,
                "bytes": 0,
                "usage": 0,
                "total_fee": 0.0,
                "maxmempool": 300000000,
                "mempoolminfee": 0.00001000,
                "minrelaytxfee": 0.00001000,
                "incrementalrelayfee": 0.00001000,
                "unbroadcastcount": 0,
                "fullrbf": false
            }),
        )
    });

    // getrawmempool
    registry.register("getrawmempool", |req: &RpcRequest| {
        // Determine verbosity from params.
        let verbose = req
            .params
            .as_ref()
            .and_then(|p| {
                if let Some(arr) = p.as_array() {
                    arr.first().and_then(|v| v.as_bool())
                } else {
                    p.get("verbose").and_then(|v| v.as_bool())
                }
            })
            .unwrap_or(false);

        if verbose {
            // Return an empty object (no transactions).
            RpcResponse::success(req.id.clone(), json!({}))
        } else {
            // Return an empty array.
            RpcResponse::success(req.id.clone(), json!([]))
        }
    });

    // getmempoolentry
    registry.register("getmempoolentry", |req: &RpcRequest| {
        let _txid = req.params.as_ref().and_then(|p| {
            if let Some(arr) = p.as_array() {
                arr.first().and_then(|v| v.as_str())
            } else {
                p.get("txid").and_then(|v| v.as_str())
            }
        });

        // Stub: transaction not in mempool.
        RpcResponse::error(
            req.id.clone(),
            RPC_INVALID_PARAMS,
            "Transaction not in mempool".to_string(),
        )
    });
}

// ---------------------------------------------------------------------------
// Utility RPCs
// ---------------------------------------------------------------------------

fn register_util_rpcs(registry: &mut RpcRegistry) {
    // validateaddress
    registry.register("validateaddress", |req: &RpcRequest| {
        let address = req.params.as_ref().and_then(|p| {
            if let Some(arr) = p.as_array() {
                arr.first().and_then(|v| v.as_str()).map(|s| s.to_string())
            } else {
                p.get("address")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            }
        });

        match address {
            Some(addr) => {
                // Very basic stub validation: non-empty is "valid".
                let is_valid = !addr.is_empty();
                RpcResponse::success(
                    req.id.clone(),
                    json!({
                        "isvalid": is_valid,
                        "address": addr,
                        "scriptPubKey": "",
                        "isscript": false,
                        "iswitness": false
                    }),
                )
            }
            None => RpcResponse::error(
                req.id.clone(),
                RPC_INVALID_PARAMS,
                "Missing required parameter: address".to_string(),
            ),
        }
    });

    // getinfo (deprecated but commonly used)
    registry.register("getinfo", |req: &RpcRequest| {
        RpcResponse::success(
            req.id.clone(),
            json!({
                "deprecation-warning": "WARNING: getinfo is deprecated and will be removed in a future version. Use getblockchaininfo, getnetworkinfo, or getmininginfo instead.",
                "version": 10000,
                "protocolversion": 70016,
                "blocks": 0,
                "timeoffset": 0,
                "connections": 0,
                "proxy": "",
                "difficulty": 1.0,
                "testnet": false,
                "paytxfee": 0.0,
                "relayfee": 0.00001000,
                "errors": ""
            }),
        )
    });

    // help
    // We need access to the registry's method list, but the registry is borrowed
    // mutably during registration. We solve this by building the help text
    // dynamically inside the handler using a known static list.
    registry.register("help", |req: &RpcRequest| {
        let command = req.params.as_ref().and_then(|p| {
            if let Some(arr) = p.as_array() {
                arr.first().and_then(|v| v.as_str()).map(|s| s.to_string())
            } else {
                p.get("command")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            }
        });

        match command {
            Some(cmd) => {
                let help_text = get_method_help(&cmd);
                match help_text {
                    Some(text) => RpcResponse::success(req.id.clone(), json!(text)),
                    None => RpcResponse::error(
                        req.id.clone(),
                        RPC_METHOD_NOT_FOUND,
                        format!("help: unknown command: {}", cmd),
                    ),
                }
            }
            None => {
                // List all available methods.
                let methods = vec![
                    "== Blockchain ==",
                    "getbestblockhash",
                    "getblock \"blockhash\" ( verbosity )",
                    "getblockchaininfo",
                    "getblockcount",
                    "getblockhash height",
                    "getdifficulty",
                    "gettxout \"txid\" n ( include_mempool )",
                    "",
                    "== Mining ==",
                    "generatetoaddress nblocks \"address\" ( maxtries )",
                    "getmininginfo",
                    "getnetworkhashps ( nblocks height )",
                    "",
                    "== Network ==",
                    "getconnectioncount",
                    "getnettotals",
                    "getnetworkinfo",
                    "getpeerinfo",
                    "",
                    "== Mempool ==",
                    "getmempoolentry \"txid\"",
                    "getmempoolinfo",
                    "getrawmempool ( verbose )",
                    "",
                    "== Util ==",
                    "getinfo",
                    "help ( \"command\" )",
                    "stop",
                    "validateaddress \"address\"",
                ];
                RpcResponse::success(req.id.clone(), json!(methods.join("\n")))
            }
        }
    });

    // stop
    registry.register("stop", |req: &RpcRequest| {
        RpcResponse::success(req.id.clone(), json!("Qubitcoin server stopping"))
    });
}

/// Return help text for a specific method, or `None` if unknown.
fn get_method_help(method: &str) -> Option<&'static str> {
    match method {
        "getblockchaininfo" => Some(
            "getblockchaininfo\n\
             \n\
             Returns an object containing various state info regarding blockchain processing.",
        ),
        "getblockcount" => Some(
            "getblockcount\n\
             \n\
             Returns the height of the most-work fully-validated chain.",
        ),
        "getblockhash" => Some(
            "getblockhash height\n\
             \n\
             Returns hash of block in best-block-chain at height provided.",
        ),
        "getblock" => Some(
            "getblock \"blockhash\" ( verbosity )\n\
             \n\
             Returns block data for the given block hash.",
        ),
        "getbestblockhash" => Some(
            "getbestblockhash\n\
             \n\
             Returns the hash of the best (tip) block in the most-work fully-validated chain.",
        ),
        "gettxout" => Some(
            "gettxout \"txid\" n ( include_mempool )\n\
             \n\
             Returns details about an unspent transaction output.",
        ),
        "getdifficulty" => Some(
            "getdifficulty\n\
             \n\
             Returns the proof-of-work difficulty as a multiple of the minimum difficulty.",
        ),
        "getmininginfo" => Some(
            "getmininginfo\n\
             \n\
             Returns a json object containing mining-related information.",
        ),
        "getnetworkhashps" => Some(
            "getnetworkhashps ( nblocks height )\n\
             \n\
             Returns the estimated network hashes per second.",
        ),
        "generatetoaddress" => Some(
            "generatetoaddress nblocks \"address\" ( maxtries )\n\
             \n\
             Mine blocks immediately to a specified address (for regtest mode only).",
        ),
        "getnetworkinfo" => Some(
            "getnetworkinfo\n\
             \n\
             Returns an object containing various state info regarding P2P networking.",
        ),
        "getpeerinfo" => Some(
            "getpeerinfo\n\
             \n\
             Returns data about each connected network peer.",
        ),
        "getconnectioncount" => Some(
            "getconnectioncount\n\
             \n\
             Returns the number of connections to other nodes.",
        ),
        "getnettotals" => Some(
            "getnettotals\n\
             \n\
             Returns information about network traffic.",
        ),
        "getmempoolinfo" => Some(
            "getmempoolinfo\n\
             \n\
             Returns details on the active state of the TX memory pool.",
        ),
        "getrawmempool" => Some(
            "getrawmempool ( verbose )\n\
             \n\
             Returns all transaction ids in memory pool.",
        ),
        "getmempoolentry" => Some(
            "getmempoolentry \"txid\"\n\
             \n\
             Returns mempool data for given transaction.",
        ),
        "validateaddress" => Some(
            "validateaddress \"address\"\n\
             \n\
             Returns information about the given qubitcoin address.",
        ),
        "getinfo" => Some(
            "getinfo\n\
             \n\
             DEPRECATED. Returns an object containing various state info.",
        ),
        "help" => Some(
            "help ( \"command\" )\n\
             \n\
             List all commands, or get help for a specified command.",
        ),
        "stop" => Some(
            "stop\n\
             \n\
             Request a graceful shutdown of Qubitcoin server.",
        ),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: create a fully-wired registry and dispatch a request.
    fn dispatch(method: &str, params: Option<serde_json::Value>) -> RpcResponse {
        let mut registry = RpcRegistry::new();
        register_all(&mut registry);
        let req = RpcRequest {
            jsonrpc: Some("2.0".into()),
            method: method.into(),
            params,
            id: json!(1),
        };
        registry.dispatch(&req)
    }

    fn assert_success(resp: &RpcResponse) {
        assert!(
            resp.error.is_none(),
            "Expected success but got error: {:?}",
            resp.error
        );
        assert!(resp.result.is_some(), "Expected a result value");
    }

    // -- Blockchain RPCs --

    #[test]
    fn test_getblockchaininfo() {
        let resp = dispatch("getblockchaininfo", None);
        assert_success(&resp);
        let result = resp.result.unwrap();
        assert_eq!(result["chain"], "main");
        assert_eq!(result["blocks"], 0);
        assert!(result["bestblockhash"].is_string());
    }

    #[test]
    fn test_getblockcount() {
        let resp = dispatch("getblockcount", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!(0));
    }

    #[test]
    fn test_getblockhash_genesis() {
        let resp = dispatch("getblockhash", Some(json!([0])));
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!(GENESIS_HASH));
    }

    #[test]
    fn test_getblockhash_out_of_range() {
        let resp = dispatch("getblockhash", Some(json!([999])));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, RPC_INVALID_PARAMS);
    }

    #[test]
    fn test_getblock() {
        let resp = dispatch("getblock", Some(json!([GENESIS_HASH])));
        assert_success(&resp);
        let result = resp.result.unwrap();
        assert_eq!(result["height"], 0);
        assert_eq!(result["hash"], GENESIS_HASH);
        assert!(result["tx"].is_array());
    }

    #[test]
    fn test_getbestblockhash() {
        let resp = dispatch("getbestblockhash", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!(GENESIS_HASH));
    }

    #[test]
    fn test_gettxout() {
        let resp = dispatch("gettxout", Some(json!(["txid", 0])));
        assert_success(&resp);
        // Stub returns null (not found).
        assert_eq!(resp.result.unwrap(), json!(null));
    }

    #[test]
    fn test_getdifficulty() {
        let resp = dispatch("getdifficulty", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!(1.0));
    }

    // -- Mining RPCs --

    #[test]
    fn test_getmininginfo() {
        let resp = dispatch("getmininginfo", None);
        assert_success(&resp);
        let result = resp.result.unwrap();
        assert_eq!(result["blocks"], 0);
        assert_eq!(result["chain"], "main");
    }

    #[test]
    fn test_getnetworkhashps() {
        let resp = dispatch("getnetworkhashps", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!(0));
    }

    #[test]
    fn test_generatetoaddress() {
        let resp = dispatch("generatetoaddress", Some(json!([3, "qc1qaddresshere"])));
        assert_success(&resp);
        let hashes = resp.result.unwrap();
        assert!(hashes.is_array());
        assert_eq!(hashes.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_generatetoaddress_zero() {
        let resp = dispatch("generatetoaddress", Some(json!([0, "qc1qaddresshere"])));
        assert_success(&resp);
        let hashes = resp.result.unwrap();
        assert_eq!(hashes.as_array().unwrap().len(), 0);
    }

    // -- Network RPCs --

    #[test]
    fn test_getnetworkinfo() {
        let resp = dispatch("getnetworkinfo", None);
        assert_success(&resp);
        let result = resp.result.unwrap();
        assert!(result["subversion"].as_str().unwrap().contains("Qubitcoin"));
        assert!(result["networks"].is_array());
    }

    #[test]
    fn test_getpeerinfo() {
        let resp = dispatch("getpeerinfo", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!([]));
    }

    #[test]
    fn test_getconnectioncount() {
        let resp = dispatch("getconnectioncount", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!(0));
    }

    #[test]
    fn test_getnettotals() {
        let resp = dispatch("getnettotals", None);
        assert_success(&resp);
        let result = resp.result.unwrap();
        assert_eq!(result["totalbytesrecv"], 0);
        assert!(result["uploadtarget"].is_object());
    }

    // -- Mempool RPCs --

    #[test]
    fn test_getmempoolinfo() {
        let resp = dispatch("getmempoolinfo", None);
        assert_success(&resp);
        let result = resp.result.unwrap();
        assert_eq!(result["size"], 0);
        assert_eq!(result["loaded"], true);
    }

    #[test]
    fn test_getrawmempool_default() {
        let resp = dispatch("getrawmempool", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!([]));
    }

    #[test]
    fn test_getrawmempool_verbose() {
        let resp = dispatch("getrawmempool", Some(json!([true])));
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!({}));
    }

    #[test]
    fn test_getmempoolentry_not_found() {
        let resp = dispatch("getmempoolentry", Some(json!(["abc123"])));
        assert!(resp.error.is_some());
    }

    // -- Utility RPCs --

    #[test]
    fn test_validateaddress_valid() {
        let resp = dispatch("validateaddress", Some(json!(["qc1qsomeaddress"])));
        assert_success(&resp);
        let result = resp.result.unwrap();
        assert_eq!(result["isvalid"], true);
        assert_eq!(result["address"], "qc1qsomeaddress");
    }

    #[test]
    fn test_validateaddress_missing() {
        let resp = dispatch("validateaddress", None);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, RPC_INVALID_PARAMS);
    }

    #[test]
    fn test_getinfo() {
        let resp = dispatch("getinfo", None);
        assert_success(&resp);
        let result = resp.result.unwrap();
        assert!(result["deprecation-warning"].is_string());
        assert_eq!(result["blocks"], 0);
    }

    #[test]
    fn test_help_all() {
        let resp = dispatch("help", None);
        assert_success(&resp);
        let text = resp.result.unwrap();
        let text_str = text.as_str().unwrap();
        assert!(text_str.contains("getblockchaininfo"));
        assert!(text_str.contains("getmininginfo"));
        assert!(text_str.contains("stop"));
    }

    #[test]
    fn test_help_specific_command() {
        let resp = dispatch("help", Some(json!(["getblockcount"])));
        assert_success(&resp);
        let text = resp.result.unwrap();
        assert!(text.as_str().unwrap().contains("getblockcount"));
    }

    #[test]
    fn test_help_unknown_command() {
        let resp = dispatch("help", Some(json!(["nosuchmethod"])));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, RPC_METHOD_NOT_FOUND);
    }

    #[test]
    fn test_stop() {
        let resp = dispatch("stop", None);
        assert_success(&resp);
        assert!(resp.result.unwrap().as_str().unwrap().contains("stopping"));
    }

    // -- Integration: all methods registered --

    #[test]
    fn test_all_methods_registered() {
        let mut registry = RpcRegistry::new();
        register_all(&mut registry);

        let expected = vec![
            "getblockchaininfo",
            "getblockcount",
            "getblockhash",
            "getblock",
            "getbestblockhash",
            "gettxout",
            "getdifficulty",
            "getmininginfo",
            "getnetworkhashps",
            "generatetoaddress",
            "getnetworkinfo",
            "getpeerinfo",
            "getconnectioncount",
            "getnettotals",
            "getmempoolinfo",
            "getrawmempool",
            "getmempoolentry",
            "validateaddress",
            "getinfo",
            "help",
            "stop",
        ];

        for method in &expected {
            assert!(
                registry.has_method(method),
                "Method '{}' should be registered",
                method
            );
        }
    }

    #[test]
    fn test_each_stub_returns_valid_json() {
        let mut registry = RpcRegistry::new();
        register_all(&mut registry);

        for name in registry.method_names() {
            let req = RpcRequest {
                jsonrpc: Some("2.0".into()),
                method: name.clone(),
                params: None,
                id: json!(1),
            };
            let resp = registry.dispatch(&req);
            // Every response must serialize to valid JSON.
            let json_str = serde_json::to_string(&resp).unwrap_or_else(|e| {
                panic!(
                    "Method '{}' produced non-serializable response: {}",
                    name, e
                );
            });
            assert!(
                !json_str.is_empty(),
                "Method '{}' produced empty response",
                name
            );
        }
    }
}
