use anyhow::Result;
use alkanes_cli_common::traits::{MetashrewRpcProvider, BitcoinRpcProvider, EsploraProvider, JsonRpcProvider, DeezelProvider};
use tracing::{info, error};

use crate::{pipeline::{BlockContext, Pipeline}, progress::ProgressStore, helpers::block::canonical_tip_height};

pub struct CatchUpCoordinator<P: MetashrewRpcProvider + BitcoinRpcProvider + EsploraProvider + JsonRpcProvider + DeezelProvider + Send + Sync> {
    provider: P,
    pipeline: Pipeline,
    progress: ProgressStore,
    start_height: Option<u64>,
}

impl<P: MetashrewRpcProvider + BitcoinRpcProvider + EsploraProvider + JsonRpcProvider + DeezelProvider + Send + Sync> CatchUpCoordinator<P> {
    pub fn new(provider: P, pipeline: Pipeline, progress: ProgressStore, start_height: Option<u64>) -> Self {
        Self { provider, pipeline, progress, start_height }
    }

    /// Run a single pass: check our current position and process the next block
    /// This is deterministic - we always check our position first before fetching the next block
    pub async fn run_once(&self) -> Result<()> {
        // First, check our current position
        let position = self.progress.get_position().await?;

        // Determine the next block to process
        // If we have a position, continue from there
        // If no position, use start_height or default to 0
        let next = match &position {
            Some(pos) => pos.height.saturating_add(1),
            None => self.start_height.unwrap_or(0),
        };

        // Check the metashrew tip to see if there's a block to process
        let tip = canonical_tip_height(&self.provider).await?;
        if next > tip {
            return Ok(()); // Nothing to process yet
        }

        // Process blocks one at a time
        // Position is updated atomically inside process_block_sequential
        for h in next..=tip {
            info!(height = h, "catch-up: processing block sequentially");

            // Process the block - position is updated atomically inside
            match self.pipeline.process_block_sequential(&self.provider, BlockContext { height: h, emit_publish: false }).await {
                Ok(block_hash) => {
                    info!(height = h, %block_hash, "catch-up: block processed");
                }
                Err(e) => {
                    error!(height = h, error = %e, "catch-up block processing failed");
                    // Stop here - position wasn't updated due to atomic transaction
                    break;
                }
            }
        }
        Ok(())
    }
}


