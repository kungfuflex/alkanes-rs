//! Database configuration

use anyhow::Result;
use std::path::PathBuf;

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub optimize_for_performance: bool,
}

impl DatabaseConfig {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            optimize_for_performance: true,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.path.as_os_str().is_empty() {
            return Err(anyhow::anyhow!("Database path cannot be empty"));
        }
        
        // Check if parent directory exists or can be created
        if let Some(parent) = self.path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow::anyhow!("Cannot create database directory: {}", e))?;
            }
        }
        
        Ok(())
    }
}