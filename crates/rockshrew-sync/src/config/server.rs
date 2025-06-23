//! Server configuration

use anyhow::Result;

/// Server configuration for HTTP/JSON-RPC server
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

impl ServerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_host(mut self, host: String) -> Self {
        self.host = host;
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_cors(mut self, cors: Option<String>) -> Self {
        self.cors = cors;
        self
    }

    pub fn validate(&self) -> Result<()> {
        // Validate host
        if self.host.is_empty() {
            return Err(anyhow::anyhow!("Host cannot be empty"));
        }

        // Validate port
        if self.port == 0 {
            return Err(anyhow::anyhow!("Port must be greater than 0"));
        }

        Ok(())
    }
}