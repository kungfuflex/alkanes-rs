use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_host: String,
    pub server_port: u16,
    
    pub bitcoin_rpc_url: String,
    pub bitcoin_rpc_user: String,
    pub bitcoin_rpc_password: String,
    
    pub metashrew_url: String,
    pub memshrew_url: String,
    
    pub ord_url: String,
    pub esplora_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("SERVER_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(18888),
            
            bitcoin_rpc_url: env::var("BITCOIN_RPC_URL")
                .unwrap_or_else(|_| "http://localhost:8332".to_string()),
            bitcoin_rpc_user: env::var("BITCOIN_RPC_USER")
                .unwrap_or_else(|_| "bitcoinrpc".to_string()),
            bitcoin_rpc_password: env::var("BITCOIN_RPC_PASSWORD")
                .unwrap_or_else(|_| "bitcoinrpc".to_string()),
            
            metashrew_url: env::var("METASHREW_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            memshrew_url: env::var("MEMSHREW_URL")
                .unwrap_or_else(|_| "http://localhost:8081".to_string()),
            
            ord_url: env::var("ORD_URL")
                .unwrap_or_else(|_| "http://localhost:8090".to_string()),
            esplora_url: env::var("ESPLORA_URL")
                .unwrap_or_else(|_| "http://localhost:50010".to_string()),
        }
    }

    pub fn bitcoin_rpc_auth(&self) -> String {
        format!("{}:{}", self.bitcoin_rpc_user, self.bitcoin_rpc_password)
    }

    pub fn bitcoin_rpc_auth_header(&self) -> String {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(self.bitcoin_rpc_auth().as_bytes());
        format!("Basic {}", encoded)
    }
}
