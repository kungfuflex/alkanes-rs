//! RPC configuration

use anyhow::Result;

/// RPC configuration for Bitcoin daemon connection
#[derive(Debug, Clone)]
pub struct RpcConfig {
    pub url: String,
    pub auth: Option<String>,
    pub bypass_ssl: bool,
    pub tunnel_config: Option<TunnelConfig>,
}

/// SSH tunnel configuration for RPC connections
#[derive(Debug, Clone)]
pub struct TunnelConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: Option<String>,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:8332".to_string(),
            auth: None,
            bypass_ssl: false,
            tunnel_config: None,
        }
    }
}

impl RpcConfig {
    pub fn new(url: String) -> Self {
        Self {
            url,
            ..Default::default()
        }
    }

    pub fn with_auth(mut self, auth: String) -> Self {
        self.auth = Some(auth);
        self
    }

    pub fn with_ssl_bypass(mut self, bypass: bool) -> Self {
        self.bypass_ssl = bypass;
        self
    }

    pub fn with_tunnel(mut self, config: TunnelConfig) -> Self {
        self.tunnel_config = Some(config);
        self
    }

    pub fn validate(&self) -> Result<()> {
        // Validate URL
        if self.url.is_empty() {
            return Err(anyhow::anyhow!("RPC URL cannot be empty"));
        }

        // Basic URL validation
        if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
            return Err(anyhow::anyhow!("RPC URL must start with http:// or https://"));
        }

        // Validate tunnel config if present
        if let Some(ref tunnel) = self.tunnel_config {
            tunnel.validate()?;
        }

        Ok(())
    }
}

impl TunnelConfig {
    pub fn validate(&self) -> Result<()> {
        if self.host.is_empty() {
            return Err(anyhow::anyhow!("Tunnel host cannot be empty"));
        }

        if self.port == 0 {
            return Err(anyhow::anyhow!("Tunnel port must be greater than 0"));
        }

        if self.user.is_empty() {
            return Err(anyhow::anyhow!("Tunnel user cannot be empty"));
        }

        Ok(())
    }
}