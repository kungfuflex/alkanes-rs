//! Sync configuration

use anyhow::Result;

/// Sync configuration for blockchain synchronization
#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub start_block: u32,
    pub exit_at: Option<u32>,
    pub pipeline_size: Option<usize>,
    pub max_reorg_depth: u32,
    pub reorg_check_threshold: u32,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            start_block: 0,
            exit_at: None,
            pipeline_size: None,
            max_reorg_depth: 100,
            reorg_check_threshold: 6,
        }
    }
}

impl SyncConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_start_block(mut self, start_block: u32) -> Self {
        self.start_block = start_block;
        self
    }

    pub fn with_exit_at(mut self, exit_at: Option<u32>) -> Self {
        self.exit_at = exit_at;
        self
    }

    pub fn with_pipeline_size(mut self, pipeline_size: Option<usize>) -> Self {
        self.pipeline_size = pipeline_size;
        self
    }

    pub fn with_reorg_settings(mut self, max_depth: u32, check_threshold: u32) -> Self {
        self.max_reorg_depth = max_depth;
        self.reorg_check_threshold = check_threshold;
        self
    }

    pub fn validate(&self) -> Result<()> {
        // Validate exit_at is greater than start_block if specified
        if let Some(exit_at) = self.exit_at {
            if exit_at <= self.start_block {
                return Err(anyhow::anyhow!("Exit block must be greater than start block"));
            }
        }

        // Validate pipeline size if specified
        if let Some(pipeline_size) = self.pipeline_size {
            if pipeline_size == 0 {
                return Err(anyhow::anyhow!("Pipeline size must be greater than 0"));
            }
        }

        // Validate reorg settings
        if self.max_reorg_depth == 0 {
            return Err(anyhow::anyhow!("Max reorg depth must be greater than 0"));
        }

        if self.reorg_check_threshold == 0 {
            return Err(anyhow::anyhow!("Reorg check threshold must be greater than 0"));
        }

        Ok(())
    }
}