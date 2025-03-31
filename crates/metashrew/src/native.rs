//! Native runtime support for Metashrew.
//!
//! This module provides the necessary components to build native standalone binaries
//! from indexer programs created with `metashrew-lib`.

use crate::indexer::{Indexer, KeyValueStore};
use hex;
use tokio;
use actix_web;
use actix_cors;
use metashrew_support::proto::metashrew::KeyValueFlush;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use log::{debug, error, info, warn};
use protobuf::Message;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

/// Native implementation of the host functions
pub struct NativeRuntime<T: Indexer> {
    indexer: T,
    store: HashMap<Vec<u8>, Vec<u8>>,
    height: u32,
    block: Vec<u8>,
}

impl<T: Indexer> NativeRuntime<T> {
    /// Create a new NativeRuntime
    pub fn new(indexer: T) -> Self {
        Self {
            indexer,
            store: HashMap::new(),
            height: 0,
            block: Vec::new(),
        }
    }

    /// Process a block
    pub fn process_block(&mut self, height: u32, block: &[u8]) -> Result<()> {
        self.height = height;
        self.block = block.to_vec();
        
        // Process the block using the indexer
        self.indexer.index_block(height, block)?;
        
        // Get the key-value pairs from the indexer
        let pairs = self.indexer.flush()?;
        
        // Store the key-value pairs
        for (key, value) in pairs {
            self.store.insert(key, value);
        }
        
        Ok(())
    }
    
    /// Get a value from the store
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.store.get(key).cloned()
    }
    
    /// Get the current height
    pub fn height(&self) -> u32 {
        self.height
    }
    
    /// Execute a view function
    pub fn execute_view<I, O>(&self, name: &str, input: &[u8]) -> Result<Vec<u8>>
    where
        I: for<'de> serde::Deserialize<'de>,
        O: serde::Serialize,
        T: crate::view::ViewFunction<I, O>,
    {
        let input: I = serde_json::from_slice(input)?;
        let output = self.indexer.execute(name, input)?;
        let result = serde_json::to_vec(&output)?;
        Ok(result)
    }
    
    /// Execute a view function with Protocol Buffer messages
    pub fn execute_proto_view<I, O>(&self, name: &str, input: &[u8]) -> Result<Vec<u8>>
    where
        I: protobuf::Message,
        O: protobuf::Message,
        T: crate::view::ProtoViewFunction<I, O>,
    {
        let input = I::parse_from_bytes(input)?;
        let output = self.indexer.execute_proto(name, input)?;
        let result = output.write_to_bytes()?;
        Ok(result)
    }
}

/// Command line arguments for the native runtime
#[derive(Parser, Debug)]
pub struct NativeRuntimeArgs {
    /// Bitcoin RPC URL
    #[arg(long)]
    pub daemon_rpc_url: String,
    
    /// Path to the database
    #[arg(long)]
    pub db_path: String,
    
    /// Starting block height
    #[arg(long)]
    pub start_block: Option<u32>,
    
    /// Bitcoin RPC authentication (username:password)
    #[arg(long)]
    pub auth: Option<String>,
    
    /// Database label
    #[arg(long)]
    pub label: Option<String>,
    
    /// Exit at block height
    #[arg(long)]
    pub exit_at: Option<u32>,
    
    /// JSON-RPC server host
    #[arg(long, env = "HOST", default_value = "127.0.0.1")]
    pub host: String,
    
    /// JSON-RPC server port
    #[arg(long, env = "PORT", default_value_t = 8080)]
    pub port: u16,
    
    /// CORS allowed origins
    #[arg(long, help = "CORS allowed origins (e.g., '*' for all origins, or specific domains)")]
    pub cors: Option<String>,
    
    /// Pipeline size
    #[arg(long, default_value_t = 5)]
    pub pipeline_size: usize,
}

/// Native runtime server
pub struct NativeRuntimeServer<T: Indexer + Clone + Send + Sync + 'static> {
    runtime: Arc<Mutex<NativeRuntime<T>>>,
    args: NativeRuntimeArgs,
}

impl<T: Indexer + Clone + Send + Sync + 'static> NativeRuntimeServer<T> {
    /// Create a new NativeRuntimeServer
    pub fn new(indexer: T, args: NativeRuntimeArgs) -> Self {
        Self {
            runtime: Arc::new(Mutex::new(NativeRuntime::new(indexer))),
            args,
        }
    }
    
    /// Run the server
    pub async fn run(&self) -> Result<()> {
        // Start the indexer
        let indexer_handle = {
            let runtime = self.runtime.clone();
            let args = self.args.clone();
            tokio::spawn(async move {
                let mut indexer = IndexerState {
                    runtime,
                    args: Arc::new(args),
                    start_block: 0,
                };
                indexer.run_pipeline().await
            })
        };
        
        // Start the JSON-RPC server
        let server_handle = {
            let runtime = self.runtime.clone();
            let args = self.args.clone();
            tokio::spawn(async move {
                let app_state = AppState { runtime };
                
                let cors = match args.cors {
                    Some(cors) => {
                        let mut cors_middleware = actix_cors::Cors::default();
                        if cors == "*" {
                            cors_middleware.allow_any_origin();
                        } else {
                            for origin in cors.split(',') {
                                cors_middleware = cors_middleware.allowed_origin(origin.trim());
                            }
                        }
                        cors_middleware.allowed_methods(vec!["POST"])
                            .allowed_headers(vec![actix_web::http::header::CONTENT_TYPE])
                            .max_age(3600)
                    },
                    None => {
                        actix_cors::Cors::default()
                            .allowed_origin("http://localhost:3000")
                            .allowed_origin("http://127.0.0.1:3000")
                            .allowed_methods(vec!["POST"])
                            .allowed_headers(vec![actix_web::http::header::CONTENT_TYPE])
                            .max_age(3600)
                    }
                };
                
                actix_web::HttpServer::new(move || {
                    actix_web::App::new()
                        .wrap(cors.clone())
                        .app_data(actix_web::web::Data::new(app_state.clone()))
                        .service(handle_jsonrpc)
                })
                .bind((args.host, args.port))
                .map_err(|e| anyhow!("Failed to bind server: {}", e))?
                .run()
                .await
                .map_err(|e| anyhow!("Server error: {}", e))
            })
        };
        
        // Wait for both handles to complete
        let (indexer_result, server_result) = tokio::join!(indexer_handle, server_handle);
        indexer_result??;
        server_result??;
        
        Ok(())
    }
}

// Helper structs and functions for the native runtime server

#[derive(Clone)]
struct AppState<T: Indexer + Clone + Send + Sync + 'static> {
    runtime: Arc<Mutex<NativeRuntime<T>>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct JsonRpcRequest {
    id: u32,
    method: String,
    params: Vec<serde_json::Value>,
    jsonrpc: String,
}

#[derive(serde::Serialize)]
struct JsonRpcResult {
    id: u32,
    result: String,
    jsonrpc: String,
}

#[derive(serde::Serialize)]
struct JsonRpcError {
    id: u32,
    error: JsonRpcErrorObject,
    jsonrpc: String,
}

#[derive(serde::Serialize)]
struct JsonRpcErrorObject {
    code: i32,
    message: String,
    data: Option<String>,
}

#[actix_web::post("/")]
async fn handle_jsonrpc<T: Indexer + Clone + Send + Sync + 'static>(
    req: actix_web::web::Json<JsonRpcRequest>,
    data: actix_web::web::Data<AppState<T>>,
) -> actix_web::Result<impl actix_web::Responder> {
    let request = req.into_inner();
    
    // Handle the request
    match request.method.as_str() {
        "metashrew.view" => {
            if request.params.len() < 2 {
                return Ok(actix_web::HttpResponse::Ok().json(JsonRpcError {
                    id: request.id,
                    error: JsonRpcErrorObject {
                        code: -32602,
                        message: "Invalid params".to_string(),
                        data: None,
                    },
                    jsonrpc: "2.0".to_string(),
                }));
            }
            
            let function_name = match request.params[0].as_str() {
                Some(name) => name,
                None => {
                    return Ok(actix_web::HttpResponse::Ok().json(JsonRpcError {
                        id: request.id,
                        error: JsonRpcErrorObject {
                            code: -32602,
                            message: "Invalid function name".to_string(),
                            data: None,
                        },
                        jsonrpc: "2.0".to_string(),
                    }));
                }
            };
            
            let input_hex = match request.params[1].as_str() {
                Some(hex) => hex,
                None => {
                    return Ok(actix_web::HttpResponse::Ok().json(JsonRpcError {
                        id: request.id,
                        error: JsonRpcErrorObject {
                            code: -32602,
                            message: "Invalid input hex".to_string(),
                            data: None,
                        },
                        jsonrpc: "2.0".to_string(),
                    }));
                }
            };
            
            let input = match hex::decode(input_hex) {
                Ok(bytes) => bytes,
                Err(e) => {
                    return Ok(actix_web::HttpResponse::Ok().json(JsonRpcError {
                        id: request.id,
                        error: JsonRpcErrorObject {
                            code: -32602,
                            message: format!("Invalid hex: {}", e),
                            data: None,
                        },
                        jsonrpc: "2.0".to_string(),
                    }));
                }
            };
            
            // Execute the view function
            let runtime = data.runtime.lock().unwrap();
            let result = runtime.execute_proto_view(function_name, &input);
            
            match result {
                Ok(output) => {
                    Ok(actix_web::HttpResponse::Ok().json(JsonRpcResult {
                        id: request.id,
                        result: hex::encode(output),
                        jsonrpc: "2.0".to_string(),
                    }))
                },
                Err(e) => {
                    Ok(actix_web::HttpResponse::Ok().json(JsonRpcError {
                        id: request.id,
                        error: JsonRpcErrorObject {
                            code: -32603,
                            message: format!("Internal error: {}", e),
                            data: None,
                        },
                        jsonrpc: "2.0".to_string(),
                    }))
                }
            }
        },
        _ => {
            Ok(actix_web::HttpResponse::Ok().json(JsonRpcError {
                id: request.id,
                error: JsonRpcErrorObject {
                    code: -32601,
                    message: "Method not found".to_string(),
                    data: None,
                },
                jsonrpc: "2.0".to_string(),
            }))
        }
    }
}

struct IndexerState<T: Indexer + Clone + Send + Sync + 'static> {
    runtime: Arc<Mutex<NativeRuntime<T>>>,
    args: Arc<NativeRuntimeArgs>,
    start_block: u32,
}

impl<T: Indexer + Clone + Send + Sync + 'static> IndexerState<T> {
    // Implementation of the indexer state methods
    // This would be similar to the IndexerState implementation in rockshrew-mono
    // but adapted to work with the NativeRuntime
    
    async fn run_pipeline(&mut self) -> Result<()> {
        // Implementation would be similar to the run_pipeline method in rockshrew-mono
        // but adapted to work with the NativeRuntime
        
        // For now, we'll just provide a placeholder implementation
        info!("Starting native indexer pipeline");
        
        // TODO: Implement the full pipeline logic
        
        Ok(())
    }
}

/// Add native runtime support to the Indexer trait
pub trait NativeIndexer: Indexer {
    /// Execute a view function
    fn execute<I, O>(&self, name: &str, input: I) -> Result<O>
    where
        I: for<'de> serde::Deserialize<'de>,
        O: serde::Serialize;
    
    /// Execute a view function with Protocol Buffer messages
    fn execute_proto<I, O>(&self, name: &str, input: I) -> Result<O>
    where
        I: protobuf::Message,
        O: protobuf::Message;
}

/// Extend the ViewFunction trait to support native execution
pub trait ViewFunction<I, O> {
    /// Execute the view function with the given input
    fn execute(&self, input: I) -> Result<O>;
}

/// Extend the ViewFunction trait to support native execution with Protocol Buffer messages
pub trait ProtoViewFunction<I, O>
where
    I: protobuf::Message,
    O: protobuf::Message,
{
    /// Execute the view function with the given input
    fn execute_proto(&self, input: I) -> Result<O>;
}
