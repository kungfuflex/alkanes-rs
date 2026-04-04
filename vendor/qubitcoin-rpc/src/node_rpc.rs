// Copyright (c) 2024 The Qubitcoin developers
// Distributed under the MIT software license.

//! RPC methods wired to real node state.
//!
//! `NodeState` holds the shared, mutable state that the running node
//! updates (block height, best hash, peer count, etc.).  The
//! `register_node_rpcs` function registers every supported RPC method
//! so that each handler reads live data from the shared state.

use crate::server::{RpcRegistry, RpcRequest, RpcResponse, RPC_INVALID_PARAMS, RPC_MISC_ERROR};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Peer info for getpeerinfo RPC
// ---------------------------------------------------------------------------

/// Per-peer information returned by the `getpeerinfo` RPC.
///
/// Maps to the JSON object returned by Bitcoin Core's `getpeerinfo`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer index.
    pub id: u64,
    /// IP address and port of the peer.
    pub addr: String,
    /// Local bind address for this connection.
    pub addrbind: String,
    /// Network (ipv4, ipv6, onion, i2p, cjdns).
    pub network: String,
    /// Services offered by the peer (hex string).
    pub services: String,
    /// Whether we connected to the peer (true) or they connected to us (false).
    pub connection_type: String,
    /// Protocol version negotiated.
    pub version: u32,
    /// User agent string.
    pub subver: String,
    /// Whether this is an inbound connection.
    pub inbound: bool,
    /// Starting height advertised by the peer.
    pub startingheight: i32,
    /// Bytes sent to this peer.
    pub bytessent: u64,
    /// Bytes received from this peer.
    pub bytesrecv: u64,
    /// Connection time (Unix timestamp).
    pub conntime: u64,
    /// Time offset (seconds).
    pub timeoffset: i64,
    /// Ping time (seconds, -1 if not yet available).
    pub pingtime: f64,
    /// Synced headers with this peer.
    pub synced_headers: i32,
    /// Synced blocks with this peer.
    pub synced_blocks: i32,
}

/// Mempool transaction entry for getrawmempool verbose mode.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MempoolEntry {
    /// Transaction ID (hex).
    pub txid: String,
    /// Virtual size in vbytes.
    pub vsize: u64,
    /// Weight in weight units.
    pub weight: u64,
    /// Fee in BTC.
    pub fee: f64,
    /// Time the transaction entered the mempool (Unix timestamp).
    pub time: u64,
    /// Block height when the transaction entered the mempool.
    pub height: i32,
    /// Number of in-mempool descendant transactions.
    pub descendantcount: u64,
    /// Number of in-mempool ancestor transactions.
    pub ancestorcount: u64,
}

// ---------------------------------------------------------------------------
// Shared node state
// ---------------------------------------------------------------------------

/// Shared node state that RPC methods can access.
///
/// Every field that the node mutates at runtime is wrapped in a [`RwLock`]
/// so that RPC handlers (which only need read access) never block the
/// writer for longer than a single field copy.
pub struct NodeState {
    /// Current chain height (-1 means no blocks yet).
    pub chain_height: RwLock<i32>,
    /// Number of headers received (may be ahead of chain_height during IBD).
    pub headers_count: RwLock<i32>,
    /// Hash of the current best block.
    pub best_block_hash: RwLock<String>,
    /// Name of the active chain (e.g. "main", "test", "regtest").
    pub chain_name: String,
    /// Number of connected peers.
    pub peer_count: RwLock<usize>,
    /// Number of transactions in the mempool.
    pub mempool_size: RwLock<usize>,
    /// Total size of mempool transactions in bytes.
    pub mempool_bytes: RwLock<u64>,
    /// Current mining difficulty.
    pub difficulty: RwLock<f64>,
    /// Estimated network hash rate (hashes per second).
    pub network_hashps: RwLock<f64>,
    /// Software version string.
    pub version: String,
    /// Protocol version number.
    pub protocol_version: u32,
    /// Number of connections (may differ from `peer_count` in direction).
    pub connections: RwLock<usize>,
    /// Known block hashes indexed by height.
    pub block_hashes: RwLock<Vec<String>>,
    /// Total bytes received from the network.
    pub total_bytes_recv: RwLock<u64>,
    /// Total bytes sent to the network.
    pub total_bytes_sent: RwLock<u64>,
    /// Whether the node is still performing initial block download.
    pub is_initial_block_download: RwLock<bool>,
    /// Warning string shown in several RPC responses.
    pub warnings: RwLock<String>,
    /// Per-peer information for `getpeerinfo`.
    pub peers: RwLock<Vec<PeerInfo>>,
    /// Transaction IDs in the mempool for `getrawmempool`.
    pub mempool_txids: RwLock<Vec<String>>,
    /// Detailed mempool entries for `getrawmempool verbose`.
    pub mempool_entries: RwLock<Vec<MempoolEntry>>,
    /// Node start time (Unix timestamp) for `uptime`.
    pub start_time: u64,
}

impl NodeState {
    /// Create a new `NodeState` with sensible defaults for the given chain.
    pub fn new(chain_name: &str) -> Self {
        NodeState {
            chain_height: RwLock::new(-1),
            headers_count: RwLock::new(-1),
            best_block_hash: RwLock::new(
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            ),
            chain_name: chain_name.to_string(),
            peer_count: RwLock::new(0),
            mempool_size: RwLock::new(0),
            mempool_bytes: RwLock::new(0),
            difficulty: RwLock::new(1.0),
            network_hashps: RwLock::new(0.0),
            version: "0.1.0".to_string(),
            protocol_version: 70016,
            connections: RwLock::new(0),
            block_hashes: RwLock::new(Vec::new()),
            total_bytes_recv: RwLock::new(0),
            total_bytes_sent: RwLock::new(0),
            is_initial_block_download: RwLock::new(true),
            warnings: RwLock::new(String::new()),
            peers: RwLock::new(Vec::new()),
            mempool_txids: RwLock::new(Vec::new()),
            mempool_entries: RwLock::new(Vec::new()),
            start_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all RPC methods that read from live `NodeState`.
pub fn register_node_rpcs(registry: &mut RpcRegistry, state: Arc<NodeState>) {
    // -- getblockchaininfo --------------------------------------------------
    let s = state.clone();
    registry.register("getblockchaininfo", move |req: &RpcRequest| {
        let height = *s.chain_height.read();
        let headers = *s.headers_count.read();
        let hash = s.best_block_hash.read().clone();
        let difficulty = *s.difficulty.read();
        let ibd = *s.is_initial_block_download.read();
        let warnings = s.warnings.read().clone();

        RpcResponse::success(
            req.id.clone(),
            serde_json::json!({
                "chain": s.chain_name,
                "blocks": height,
                "headers": headers,
                "bestblockhash": hash,
                "difficulty": difficulty,
                "time": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                "mediantime": 0,
                "verificationprogress": if ibd { 0.0 } else { 1.0 },
                "initialblockdownload": ibd,
                "chainwork": "0000000000000000000000000000000000000000000000000000000000000000",
                "size_on_disk": 0,
                "pruned": false,
                "warnings": warnings,
            }),
        )
    });

    // -- getblockcount ------------------------------------------------------
    let s = state.clone();
    registry.register("getblockcount", move |req: &RpcRequest| {
        RpcResponse::success(req.id.clone(), serde_json::json!(*s.chain_height.read()))
    });

    // -- getbestblockhash ---------------------------------------------------
    let s = state.clone();
    registry.register("getbestblockhash", move |req: &RpcRequest| {
        RpcResponse::success(req.id.clone(), serde_json::json!(*s.best_block_hash.read()))
    });

    // -- getblockhash -------------------------------------------------------
    let s = state.clone();
    registry.register("getblockhash", move |req: &RpcRequest| {
        let height = match req
            .params
            .as_ref()
            .and_then(|p| p.get(0))
            .and_then(|v| v.as_i64())
        {
            Some(h) => h as i32,
            None => {
                return RpcResponse::error(
                    req.id.clone(),
                    RPC_INVALID_PARAMS,
                    "Missing height parameter".into(),
                )
            }
        };
        let hashes = s.block_hashes.read();
        if height < 0 || height as usize >= hashes.len() {
            return RpcResponse::error(
                req.id.clone(),
                RPC_INVALID_PARAMS,
                "Block height out of range".into(),
            );
        }
        RpcResponse::success(req.id.clone(), serde_json::json!(hashes[height as usize]))
    });

    // -- getnetworkinfo -----------------------------------------------------
    let s = state.clone();
    registry.register("getnetworkinfo", move |req: &RpcRequest| {
        RpcResponse::success(
            req.id.clone(),
            serde_json::json!({
                "version": 10000,
                "subversion": format!("/Qubitcoin:{}/", s.version),
                "protocolversion": s.protocol_version,
                "localservices": "000000000000040d",
                "localservicesnames": ["NETWORK", "BLOOM", "WITNESS", "NETWORK_LIMITED"],
                "localrelay": true,
                "timeoffset": 0,
                "networkactive": true,
                "connections": *s.connections.read(),
                "connections_in": 0,
                "connections_out": *s.connections.read(),
                "networks": [],
                "relayfee": 0.00001000,
                "incrementalfee": 0.00001000,
                "localaddresses": [],
                "warnings": ""
            }),
        )
    });

    // -- getpeerinfo --------------------------------------------------------
    let s = state.clone();
    registry.register("getpeerinfo", move |req: &RpcRequest| {
        let peers = s.peers.read();
        let peer_json: Vec<serde_json::Value> = peers
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "addr": p.addr,
                    "addrbind": p.addrbind,
                    "network": p.network,
                    "services": p.services,
                    "connection_type": p.connection_type,
                    "version": p.version,
                    "subver": p.subver,
                    "inbound": p.inbound,
                    "startingheight": p.startingheight,
                    "bytessent": p.bytessent,
                    "bytesrecv": p.bytesrecv,
                    "conntime": p.conntime,
                    "timeoffset": p.timeoffset,
                    "pingtime": p.pingtime,
                    "synced_headers": p.synced_headers,
                    "synced_blocks": p.synced_blocks,
                })
            })
            .collect();
        RpcResponse::success(req.id.clone(), serde_json::json!(peer_json))
    });

    // -- getconnectioncount -------------------------------------------------
    let s = state.clone();
    registry.register("getconnectioncount", move |req: &RpcRequest| {
        RpcResponse::success(req.id.clone(), serde_json::json!(*s.connections.read()))
    });

    // -- getnettotals -------------------------------------------------------
    let s = state.clone();
    registry.register("getnettotals", move |req: &RpcRequest| {
        RpcResponse::success(
            req.id.clone(),
            serde_json::json!({
                "totalbytesrecv": *s.total_bytes_recv.read(),
                "totalbytessent": *s.total_bytes_sent.read(),
                "timemillis": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
            }),
        )
    });

    // -- getmempoolinfo -----------------------------------------------------
    let s = state.clone();
    registry.register("getmempoolinfo", move |req: &RpcRequest| {
        RpcResponse::success(
            req.id.clone(),
            serde_json::json!({
                "loaded": true,
                "size": *s.mempool_size.read(),
                "bytes": *s.mempool_bytes.read(),
                "usage": *s.mempool_bytes.read(),
                "total_fee": 0.0,
                "maxmempool": 300000000,
                "mempoolminfee": 0.00001000,
                "minrelaytxfee": 0.00001000,
            }),
        )
    });

    // -- getrawmempool ------------------------------------------------------
    let s = state.clone();
    registry.register("getrawmempool", move |req: &RpcRequest| {
        // Check if verbose mode is requested (second param or first param).
        let verbose = req
            .params
            .as_ref()
            .and_then(|p| p.get(0))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if verbose {
            // Return detailed mempool entries as an object keyed by txid.
            let entries = s.mempool_entries.read();
            let mut result = serde_json::Map::new();
            for entry in entries.iter() {
                result.insert(
                    entry.txid.clone(),
                    serde_json::json!({
                        "vsize": entry.vsize,
                        "weight": entry.weight,
                        "fee": entry.fee,
                        "time": entry.time,
                        "height": entry.height,
                        "descendantcount": entry.descendantcount,
                        "ancestorcount": entry.ancestorcount,
                    }),
                );
            }
            RpcResponse::success(req.id.clone(), serde_json::Value::Object(result))
        } else {
            // Return array of txids.
            let txids = s.mempool_txids.read();
            RpcResponse::success(req.id.clone(), serde_json::json!(*txids))
        }
    });

    // -- getdifficulty ------------------------------------------------------
    let s = state.clone();
    registry.register("getdifficulty", move |req: &RpcRequest| {
        RpcResponse::success(req.id.clone(), serde_json::json!(*s.difficulty.read()))
    });

    // -- getmininginfo ------------------------------------------------------
    let s = state.clone();
    registry.register("getmininginfo", move |req: &RpcRequest| {
        RpcResponse::success(
            req.id.clone(),
            serde_json::json!({
                "blocks": *s.chain_height.read(),
                "difficulty": *s.difficulty.read(),
                "networkhashps": *s.network_hashps.read(),
                "pooledtx": *s.mempool_size.read(),
                "chain": s.chain_name,
                "warnings": "",
            }),
        )
    });

    // -- help ---------------------------------------------------------------
    registry.register("help", move |req: &RpcRequest| {
        let methods = vec![
            "getblockchaininfo",
            "getblockcount",
            "getbestblockhash",
            "getblockhash",
            "getnetworkinfo",
            "getpeerinfo",
            "getconnectioncount",
            "getnettotals",
            "getmempoolinfo",
            "getrawmempool",
            "getdifficulty",
            "getmininginfo",
            "uptime",
            "help",
            "stop",
        ];
        if let Some(cmd) = req
            .params
            .as_ref()
            .and_then(|p| p.get(0))
            .and_then(|v| v.as_str())
        {
            if methods.contains(&cmd) {
                return RpcResponse::success(
                    req.id.clone(),
                    serde_json::json!(format!("{} - Qubitcoin RPC method", cmd)),
                );
            }
            return RpcResponse::error(
                req.id.clone(),
                RPC_MISC_ERROR,
                format!("help: unknown command: {}", cmd),
            );
        }
        RpcResponse::success(req.id.clone(), serde_json::json!(methods.join("\n")))
    });

    // -- uptime -------------------------------------------------------------
    let s = state.clone();
    registry.register("uptime", move |req: &RpcRequest| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let uptime = now.saturating_sub(s.start_time);
        RpcResponse::success(req.id.clone(), serde_json::json!(uptime))
    });

    // -- stop ---------------------------------------------------------------
    registry.register("stop", move |req: &RpcRequest| {
        RpcResponse::success(
            req.id.clone(),
            serde_json::json!("Qubitcoin server stopping"),
        )
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{RpcRegistry, RpcRequest, RpcResponse};
    use serde_json::json;

    // -- NodeState creation -------------------------------------------------

    #[test]
    fn test_node_state_new_defaults() {
        let state = NodeState::new("main");
        assert_eq!(state.chain_name, "main");
        assert_eq!(*state.chain_height.read(), -1);
        assert_eq!(
            *state.best_block_hash.read(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(*state.peer_count.read(), 0);
        assert_eq!(*state.mempool_size.read(), 0);
        assert_eq!(*state.mempool_bytes.read(), 0);
        assert_eq!(*state.difficulty.read(), 1.0);
        assert_eq!(*state.network_hashps.read(), 0.0);
        assert_eq!(state.version, "0.1.0");
        assert_eq!(state.protocol_version, 70016);
        assert_eq!(*state.connections.read(), 0);
        assert!(state.block_hashes.read().is_empty());
        assert_eq!(*state.total_bytes_recv.read(), 0);
        assert_eq!(*state.total_bytes_sent.read(), 0);
        assert_eq!(*state.is_initial_block_download.read(), true);
        assert!(state.warnings.read().is_empty());
    }

    #[test]
    fn test_node_state_new_testnet() {
        let state = NodeState::new("test");
        assert_eq!(state.chain_name, "test");
    }

    // -- NodeState updates --------------------------------------------------

    #[test]
    fn test_node_state_update_chain_height() {
        let state = NodeState::new("main");
        *state.chain_height.write() = 100;
        assert_eq!(*state.chain_height.read(), 100);
    }

    #[test]
    fn test_node_state_update_best_block_hash() {
        let state = NodeState::new("main");
        let hash = "00000000000000000001abcdef1234567890abcdef1234567890abcdef123456".to_string();
        *state.best_block_hash.write() = hash.clone();
        assert_eq!(*state.best_block_hash.read(), hash);
    }

    #[test]
    fn test_node_state_update_connections() {
        let state = NodeState::new("main");
        *state.connections.write() = 8;
        *state.peer_count.write() = 8;
        assert_eq!(*state.connections.read(), 8);
        assert_eq!(*state.peer_count.read(), 8);
    }

    #[test]
    fn test_node_state_update_mempool() {
        let state = NodeState::new("main");
        *state.mempool_size.write() = 42;
        *state.mempool_bytes.write() = 123456;
        assert_eq!(*state.mempool_size.read(), 42);
        assert_eq!(*state.mempool_bytes.read(), 123456);
    }

    #[test]
    fn test_node_state_update_block_hashes() {
        let state = NodeState::new("main");
        let mut hashes = state.block_hashes.write();
        hashes.push("genesis_hash".to_string());
        hashes.push("block_1_hash".to_string());
        drop(hashes);
        assert_eq!(state.block_hashes.read().len(), 2);
        assert_eq!(state.block_hashes.read()[0], "genesis_hash");
    }

    #[test]
    fn test_node_state_update_ibd() {
        let state = NodeState::new("main");
        assert_eq!(*state.is_initial_block_download.read(), true);
        *state.is_initial_block_download.write() = false;
        assert_eq!(*state.is_initial_block_download.read(), false);
    }

    // -- Helper: build a registry with node RPCs ----------------------------

    fn make_registry(state: Arc<NodeState>) -> RpcRegistry {
        let mut registry = RpcRegistry::new();
        register_node_rpcs(&mut registry, state);
        registry
    }

    fn dispatch(
        registry: &RpcRegistry,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> RpcResponse {
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

    // -- RPC method tests: each returns valid JSON --------------------------

    #[test]
    fn test_register_node_rpcs_all_methods() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);

        let expected = vec![
            "getblockchaininfo",
            "getblockcount",
            "getbestblockhash",
            "getblockhash",
            "getnetworkinfo",
            "getpeerinfo",
            "getconnectioncount",
            "getnettotals",
            "getmempoolinfo",
            "getrawmempool",
            "getdifficulty",
            "getmininginfo",
            "uptime",
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
    fn test_each_node_rpc_returns_valid_json() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);

        for name in registry.method_names() {
            // Skip getblockhash which requires params.
            if name == "getblockhash" {
                continue;
            }
            let resp = dispatch(&registry, &name, None);
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

    // -- Individual method tests --------------------------------------------

    #[test]
    fn test_getblockchaininfo_reads_state() {
        let state = Arc::new(NodeState::new("regtest"));
        *state.chain_height.write() = 50;
        *state.best_block_hash.write() = "abc123".to_string();
        *state.difficulty.write() = 2.5;
        *state.is_initial_block_download.write() = false;

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getblockchaininfo", None);
        assert_success(&resp);

        let result = resp.result.unwrap();
        assert_eq!(result["chain"], "regtest");
        assert_eq!(result["blocks"], 50);
        assert_eq!(result["bestblockhash"], "abc123");
        assert_eq!(result["difficulty"], 2.5);
        assert_eq!(result["initialblockdownload"], false);
        assert_eq!(result["verificationprogress"], 1.0);
    }

    #[test]
    fn test_getblockchaininfo_ibd_progress() {
        let state = Arc::new(NodeState::new("main"));
        // Default is IBD=true
        let registry = make_registry(state);
        let resp = dispatch(&registry, "getblockchaininfo", None);
        assert_success(&resp);
        let result = resp.result.unwrap();
        assert_eq!(result["initialblockdownload"], true);
        assert_eq!(result["verificationprogress"], 0.0);
    }

    #[test]
    fn test_getblockcount_reads_state() {
        let state = Arc::new(NodeState::new("main"));
        *state.chain_height.write() = 12345;

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getblockcount", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!(12345));
    }

    #[test]
    fn test_getbestblockhash_reads_state() {
        let state = Arc::new(NodeState::new("main"));
        *state.best_block_hash.write() = "deadbeef".to_string();

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getbestblockhash", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!("deadbeef"));
    }

    #[test]
    fn test_getblockhash_valid() {
        let state = Arc::new(NodeState::new("main"));
        {
            let mut hashes = state.block_hashes.write();
            hashes.push("genesis_hash".to_string());
            hashes.push("block1_hash".to_string());
        }

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getblockhash", Some(json!([0])));
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!("genesis_hash"));

        let resp = dispatch(&registry, "getblockhash", Some(json!([1])));
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!("block1_hash"));
    }

    #[test]
    fn test_getblockhash_out_of_range() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);
        let resp = dispatch(&registry, "getblockhash", Some(json!([999])));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, RPC_INVALID_PARAMS);
    }

    #[test]
    fn test_getblockhash_negative_height() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);
        let resp = dispatch(&registry, "getblockhash", Some(json!([-1])));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, RPC_INVALID_PARAMS);
    }

    #[test]
    fn test_getblockhash_missing_param() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);
        let resp = dispatch(&registry, "getblockhash", None);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, RPC_INVALID_PARAMS);
    }

    #[test]
    fn test_getnetworkinfo_reads_state() {
        let state = Arc::new(NodeState::new("main"));
        *state.connections.write() = 12;

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getnetworkinfo", None);
        assert_success(&resp);

        let result = resp.result.unwrap();
        assert_eq!(result["connections"], 12);
        assert_eq!(result["connections_out"], 12);
        assert!(result["subversion"].as_str().unwrap().contains("Qubitcoin"));
        assert_eq!(result["protocolversion"], 70016);
    }

    #[test]
    fn test_getpeerinfo_returns_empty() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);
        let resp = dispatch(&registry, "getpeerinfo", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!([]));
    }

    #[test]
    fn test_getconnectioncount_reads_state() {
        let state = Arc::new(NodeState::new("main"));
        *state.connections.write() = 5;

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getconnectioncount", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!(5));
    }

    #[test]
    fn test_getnettotals_reads_state() {
        let state = Arc::new(NodeState::new("main"));
        *state.total_bytes_recv.write() = 1000;
        *state.total_bytes_sent.write() = 2000;

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getnettotals", None);
        assert_success(&resp);

        let result = resp.result.unwrap();
        assert_eq!(result["totalbytesrecv"], 1000);
        assert_eq!(result["totalbytessent"], 2000);
        assert!(result["timemillis"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_getmempoolinfo_reads_state() {
        let state = Arc::new(NodeState::new("main"));
        *state.mempool_size.write() = 10;
        *state.mempool_bytes.write() = 5000;

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getmempoolinfo", None);
        assert_success(&resp);

        let result = resp.result.unwrap();
        assert_eq!(result["size"], 10);
        assert_eq!(result["bytes"], 5000);
        assert_eq!(result["usage"], 5000);
        assert_eq!(result["loaded"], true);
    }

    #[test]
    fn test_getrawmempool_returns_empty() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);
        let resp = dispatch(&registry, "getrawmempool", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!([]));
    }

    #[test]
    fn test_getdifficulty_reads_state() {
        let state = Arc::new(NodeState::new("main"));
        *state.difficulty.write() = 99.5;

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getdifficulty", None);
        assert_success(&resp);
        assert_eq!(resp.result.unwrap(), json!(99.5));
    }

    #[test]
    fn test_getmininginfo_reads_state() {
        let state = Arc::new(NodeState::new("regtest"));
        *state.chain_height.write() = 200;
        *state.difficulty.write() = 3.14;
        *state.network_hashps.write() = 1000000.0;
        *state.mempool_size.write() = 7;

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getmininginfo", None);
        assert_success(&resp);

        let result = resp.result.unwrap();
        assert_eq!(result["blocks"], 200);
        assert_eq!(result["difficulty"], 3.14);
        assert_eq!(result["networkhashps"], 1000000.0);
        assert_eq!(result["pooledtx"], 7);
        assert_eq!(result["chain"], "regtest");
    }

    #[test]
    fn test_help_lists_all_methods() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);
        let resp = dispatch(&registry, "help", None);
        assert_success(&resp);
        let text = resp.result.unwrap();
        let text_str = text.as_str().unwrap();
        assert!(text_str.contains("getblockchaininfo"));
        assert!(text_str.contains("getblockcount"));
        assert!(text_str.contains("stop"));
        assert!(text_str.contains("help"));
    }

    #[test]
    fn test_help_specific_known_method() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);
        let resp = dispatch(&registry, "help", Some(json!(["getblockcount"])));
        assert_success(&resp);
        let text = resp.result.unwrap();
        assert!(text.as_str().unwrap().contains("getblockcount"));
    }

    #[test]
    fn test_help_specific_unknown_method() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);
        let resp = dispatch(&registry, "help", Some(json!(["nosuchmethod"])));
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, RPC_MISC_ERROR);
    }

    #[test]
    fn test_stop() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);
        let resp = dispatch(&registry, "stop", None);
        assert_success(&resp);
        assert!(resp.result.unwrap().as_str().unwrap().contains("stopping"));
    }

    // -- getpeerinfo with data ----------------------------------------------

    #[test]
    fn test_getpeerinfo_with_peers() {
        let state = Arc::new(NodeState::new("main"));
        {
            let mut peers = state.peers.write();
            peers.push(PeerInfo {
                id: 1,
                addr: "1.2.3.4:8333".to_string(),
                addrbind: "0.0.0.0:8333".to_string(),
                network: "ipv4".to_string(),
                services: "0000000000000409".to_string(),
                connection_type: "outbound-full-relay".to_string(),
                version: 70016,
                subver: "/Satoshi:25.0.0/".to_string(),
                inbound: false,
                startingheight: 800000,
                bytessent: 1024,
                bytesrecv: 2048,
                conntime: 1700000000,
                timeoffset: -3,
                pingtime: 0.05,
                synced_headers: 800000,
                synced_blocks: 799999,
            });
            peers.push(PeerInfo {
                id: 2,
                addr: "5.6.7.8:8333".to_string(),
                addrbind: "0.0.0.0:8333".to_string(),
                network: "ipv4".to_string(),
                services: "0000000000000409".to_string(),
                connection_type: "inbound".to_string(),
                version: 70016,
                subver: "/Qubitcoin:0.1.0/".to_string(),
                inbound: true,
                startingheight: 799500,
                bytessent: 512,
                bytesrecv: 256,
                conntime: 1700001000,
                timeoffset: 0,
                pingtime: 0.12,
                synced_headers: 799500,
                synced_blocks: 799500,
            });
        }

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getpeerinfo", None);
        assert_success(&resp);

        let result = resp.result.unwrap();
        let peers = result.as_array().unwrap();
        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0]["id"], 1);
        assert_eq!(peers[0]["addr"], "1.2.3.4:8333");
        assert_eq!(peers[0]["version"], 70016);
        assert_eq!(peers[0]["inbound"], false);
        assert_eq!(peers[1]["id"], 2);
        assert_eq!(peers[1]["inbound"], true);
    }

    // -- getrawmempool with data --------------------------------------------

    #[test]
    fn test_getrawmempool_with_txids() {
        let state = Arc::new(NodeState::new("main"));
        {
            let mut txids = state.mempool_txids.write();
            txids.push(
                "aabb00112233445566778899aabb00112233445566778899aabb001122334455".to_string(),
            );
            txids.push(
                "ccdd00112233445566778899ccdd00112233445566778899ccdd001122334455".to_string(),
            );
        }

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getrawmempool", None);
        assert_success(&resp);

        let result = resp.result.unwrap();
        let txids = result.as_array().unwrap();
        assert_eq!(txids.len(), 2);
        assert!(txids[0].as_str().unwrap().starts_with("aabb"));
    }

    #[test]
    fn test_getrawmempool_verbose() {
        let state = Arc::new(NodeState::new("main"));
        {
            let mut entries = state.mempool_entries.write();
            entries.push(MempoolEntry {
                txid: "aabb001122".to_string(),
                vsize: 225,
                weight: 900,
                fee: 0.00001000,
                time: 1700000000,
                height: 800000,
                descendantcount: 1,
                ancestorcount: 1,
            });
        }

        let registry = make_registry(state);
        let resp = dispatch(&registry, "getrawmempool", Some(json!([true])));
        assert_success(&resp);

        let result = resp.result.unwrap();
        assert!(result.is_object());
        let entry = &result["aabb001122"];
        assert_eq!(entry["vsize"], 225);
        assert_eq!(entry["weight"], 900);
    }

    // -- uptime -------------------------------------------------------------

    #[test]
    fn test_uptime() {
        let state = Arc::new(NodeState::new("main"));
        let registry = make_registry(state);
        let resp = dispatch(&registry, "uptime", None);
        assert_success(&resp);
        // Uptime should be 0 or very small since we just created the state.
        let uptime = resp.result.unwrap().as_u64().unwrap();
        assert!(uptime < 5);
    }

    // -- Concurrent state updates -------------------------------------------

    #[test]
    fn test_concurrent_state_reads() {
        use std::thread;

        let state = Arc::new(NodeState::new("main"));
        *state.chain_height.write() = 100;

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let s = state.clone();
                thread::spawn(move || {
                    assert_eq!(*s.chain_height.read(), 100);
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
    }
}
