//! Generic JSON-RPC framework for Metashrew indexers
//!
//! This module provides a generic JSON-RPC server implementation that can work
//! with any storage and runtime adapters, eliminating code duplication across
//! different implementations.

pub mod server;
pub mod handlers;
pub mod types;

// Re-export main types
pub use server::MetashrewJsonRpcServer;
pub use types::{JsonRpcRequest, JsonRpcResponse, JsonRpcError, JsonRpcResult};
pub use handlers::JsonRpcHandlers;