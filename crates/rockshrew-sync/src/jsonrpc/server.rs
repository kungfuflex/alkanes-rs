//! Generic JSON-RPC server implementation

use super::handlers::JsonRpcHandlers;
use super::types::{JsonRpcRequest, JsonRpcError};
use crate::{StorageAdapter, RuntimeAdapter};
use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Result as ActixResult};
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::RwLock;
use log::debug;

/// Generic JSON-RPC server that works with any storage and runtime adapters
pub struct MetashrewJsonRpcServer<S, R> 
where 
    S: StorageAdapter,
    R: RuntimeAdapter,
{
    handlers: JsonRpcHandlers<S, R>,
    config: ServerConfig,
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub cors: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            cors: None,
        }
    }
}

impl<S, R> MetashrewJsonRpcServer<S, R>
where
    S: StorageAdapter + 'static,
    R: RuntimeAdapter + 'static,
{
    pub fn new(
        storage: Arc<RwLock<S>>,
        runtime: Arc<RwLock<R>>,
        current_height: Arc<AtomicU32>,
        config: ServerConfig,
    ) -> Self {
        let handlers = JsonRpcHandlers::new(storage, runtime, current_height);
        Self { handlers, config }
    }

    /// Start the JSON-RPC server
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error>> {
        let handlers = Arc::new(self.handlers);
        let config = self.config.clone();

        let server = HttpServer::new(move || {
            let cors = Self::configure_cors(&config.cors);
            let handlers_clone = handlers.clone();

            App::new()
                .wrap(cors)
                .app_data(web::Data::new(handlers_clone))
                .route("/", web::post().to(Self::handle_jsonrpc_request))
        })
        .bind((config.host.as_str(), config.port))?
        .run();

        log::info!(
            "JSON-RPC server running at http://{}:{}",
            config.host, config.port
        );
        log::info!("Available RPC methods: metashrew_view, metashrew_preview, metashrew_height, metashrew_getblockhash, metashrew_stateroot, metashrew_snapshot");

        server.await?;
        Ok(())
    }

    /// Handle JSON-RPC requests
    async fn handle_jsonrpc_request(
        body: web::Json<JsonRpcRequest>,
        handlers: web::Data<Arc<JsonRpcHandlers<S, R>>>,
    ) -> ActixResult<HttpResponse> {
        debug!("RPC request: {}", serde_json::to_string(&body).unwrap_or_default());

        match handlers.handle_request(body.into_inner()).await {
            Ok(result) => Ok(HttpResponse::Ok().json(result)),
            Err(error) => Ok(HttpResponse::Ok().json(error)),
        }
    }

    /// Configure CORS based on configuration
    fn configure_cors(cors_config: &Option<String>) -> Cors {
        match cors_config {
            Some(cors_value) if cors_value == "*" => {
                // Allow all origins
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
            }
            Some(cors_value) => {
                // Allow specific origins
                let mut cors_builder = Cors::default();
                for origin in cors_value.split(',') {
                    cors_builder = cors_builder.allowed_origin(origin.trim());
                }
                cors_builder
            }
            None => {
                // Default: only allow localhost
                Cors::default().allowed_origin_fn(|origin, _| {
                    if let Ok(origin_str) = origin.to_str() {
                        origin_str.starts_with("http://localhost:")
                    } else {
                        false
                    }
                })
            }
        }
    }
}


/// Error wrapper for Actix Web compatibility
#[derive(Debug)]
struct IndexerError(anyhow::Error);

impl std::fmt::Display for IndexerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<anyhow::Error> for IndexerError {
    fn from(err: anyhow::Error) -> Self {
        IndexerError(err)
    }
}

impl actix_web::error::ResponseError for IndexerError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::Ok().json(JsonRpcError::internal_error(
            0, // Generic ID since we lost context
            self.0.to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{MockStorage, MockRuntime};
    use std::sync::atomic::AtomicU32;

    #[tokio::test]
    async fn test_server_creation() {
        let storage = Arc::new(RwLock::new(MockStorage::new()));
        let runtime = Arc::new(RwLock::new(MockRuntime::new()));
        let current_height = Arc::new(AtomicU32::new(0));
        let config = ServerConfig::default();

        let _server = MetashrewJsonRpcServer::new(storage, runtime, current_height, config);
        // Server creation should succeed
    }

    #[test]
    fn test_cors_configuration() {
        // Test wildcard CORS
        let cors = MetashrewJsonRpcServer::<MockStorage, MockRuntime>::configure_cors(&Some("*".to_string()));
        // Should not panic

        // Test specific origins
        let cors = MetashrewJsonRpcServer::<MockStorage, MockRuntime>::configure_cors(&Some("http://localhost:3000".to_string()));
        // Should not panic

        // Test default (None)
        let cors = MetashrewJsonRpcServer::<MockStorage, MockRuntime>::configure_cors(&None);
        // Should not panic
    }
}