// Copyright (c) 2024 The Qubitcoin developers
// Distributed under the MIT software license.

//! qubitcoin-rpc: JSON-RPC server for Qubitcoin.
//!
//! This crate provides the JSON-RPC 2.0 server framework and method
//! implementations that expose Qubitcoin node functionality over RPC.
//!
//! # Modules
//!
//! - [`server`] -- Core RPC types (request, response, error), handler
//!   registry, and raw request processing.
//! - [`methods`] -- Stub implementations of all RPC methods (blockchain,
//!   mining, network, mempool, utility).
//! - [`http_server`] -- HTTP/1.1 server that accepts JSON-RPC requests
//!   over TCP with optional Basic Auth.
//! - [`node_rpc`] -- RPC method handlers wired to shared, mutable
//!   [`node_rpc::NodeState`] for live node data.

/// HTTP/1.1 server that accepts JSON-RPC requests over TCP with optional Basic Auth.
pub mod http_server;
/// Stub implementations of all RPC methods (blockchain, mining, network, mempool, utility).
pub mod methods;
/// RPC method handlers wired to shared, mutable `NodeState` for live node data.
pub mod node_rpc;
/// Core RPC types (request, response, error), `RpcRegistry` handler dispatch, and request processing.
pub mod server;
