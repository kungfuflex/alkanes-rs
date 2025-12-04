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
        // Only run catch-up when a start height is provided
        if self.start_height.is_none() {
            return Ok(());
        }

        // First, check our current position
        let position = self.progress.get_position().await?;

        // Determine the next block to process
        let next = match (&position, self.start_height) {
            (Some(pos), _) => pos.height.saturating_add(1),
            (None, Some(s)) => s,
            (None, None) => return Ok(()),
        };

        // Check the metashrew tip to see if there's a block to process
        let tip = canonical_tip_height(&self.provider).await?;
        if next > tip {
            return Ok(()); // Nothing to process yet
        }

        // Process blocks one at a time, updating position after each
        for h in next..=tip {
            info!(height = h, "catch-up: processing block sequentially");

            // Process the block and get the block hash back
            match self.pipeline.process_block_sequential(&self.provider, BlockContext { height: h, emit_publish: false }).await {
                Ok(block_hash) => {
                    // Update position only after successful indexing
                    self.progress.set_position(h, &block_hash).await?;
                    info!(height = h, %block_hash, "catch-up: position updated");
                }
                Err(e) => {
                    error!(height = h, error = %e, "catch-up block processing failed");
                    // Stop here - we don't advance position on failure
                    break;
                }
            }
        }
        Ok(())
    }
}


