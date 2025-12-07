use alkanes_cli_sys::SystemAlkanes as ConcreteProvider;
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration, Instant};
use tracing::{error, info, warn};

use crate::{pipeline::{BlockContext, Pipeline}, progress::ProgressStore, helpers::block::canonical_tip_height};

pub struct BlockPoller {
    provider: ConcreteProvider,
    pipeline: Pipeline,
    progress: ProgressStore,
    poll_interval_ms: u64,
    init_signal: Option<oneshot::Sender<()>>, // fired once after initial pools refresh + height init
    start_height: Option<u64>,
}

impl BlockPoller {
    pub fn new(
        provider: ConcreteProvider,
        pipeline: Pipeline,
        progress: ProgressStore,
        poll_interval_ms: u64,
        init_signal: Option<oneshot::Sender<()>>,
        start_height: Option<u64>,
    ) -> Self {
        Self { provider, pipeline, progress, poll_interval_ms, init_signal, start_height }
    }

    pub async fn run(mut self) {
        let mut initialized = false;
        let mut backoff_ms: u64 = self.poll_interval_ms.max(250);

        loop {
            let tick_start = Instant::now();

            // First, check our current position from the database
            let position = match self.progress.get_position().await {
                Ok(pos) => pos,
                Err(e) => {
                    error!(error = %e, "failed to get position from database");
                    backoff_ms = (backoff_ms.saturating_mul(2)).min(30_000);
                    sleep(Duration::from_millis(backoff_ms)).await;
                    continue;
                }
            };

            match canonical_tip_height(&self.provider).await {
                Ok(tip_height) => {
                    backoff_ms = self.poll_interval_ms; // reset on success

                    if !initialized {
                        // On first observation, refresh pools/state once
                        if let Err(e) = self.pipeline.fetch_pools_for_tip(&self.provider, tip_height).await {
                            error!(height = tip_height, error = %e, "fetch_pools_for_tip failed");
                        }
                        info!(tip_height, "initialized metashrew height");
                        initialized = true;

                        if let Some(tx) = self.init_signal.take() {
                            let _ = tx.send(());
                        }
                    }

                    // Determine the next block to process based on our position
                    let next_height = match &position {
                        Some(pos) => pos.height.saturating_add(1),
                        None => {
                            // No position yet - start from configured start_height or 0
                            // The coordinator will handle the sequential catch-up
                            self.start_height.unwrap_or(0)
                        }
                    };

                    // Check for reorg: if tip is less than our position, something's wrong
                    if let Some(pos) = &position {
                        if tip_height < pos.height {
                            warn!(
                                tip = tip_height,
                                our_position = pos.height,
                                "tip is behind our position, possible reorg"
                            );
                            // Don't process anything - wait for tip to catch up or handle reorg
                            sleep(Duration::from_millis(self.poll_interval_ms)).await;
                            continue;
                        }
                    }

                    // Process blocks one at a time if we're behind
                    if next_height <= tip_height {
                        // Refresh pools at new tip before processing
                        if let Err(e) = self.pipeline.fetch_pools_for_tip(&self.provider, tip_height).await {
                            error!(height = tip_height, error = %e, "fetch_pools_for_tip failed");
                        }

                        for h in next_height..=tip_height {
                            info!(height = h, "new block detected");

                            // Process the block - position is updated atomically inside
                            match self.pipeline.process_block_sequential(&self.provider, BlockContext { height: h, emit_publish: true }).await {
                                Ok(block_hash) => {
                                    info!(height = h, %block_hash, "block processed");
                                }
                                Err(e) => {
                                    error!(height = h, error = %e, "block processing failed");
                                    // Stop advancing so we can retry this specific height on next loop
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(error = %e, "failed to fetch metashrew height");
                    // Exponential backoff with cap
                    backoff_ms = (backoff_ms.saturating_mul(2)).min(30_000);
                }
            }

            let elapsed = tick_start.elapsed();
            let base = if backoff_ms == self.poll_interval_ms { self.poll_interval_ms } else { backoff_ms };
            let sleep_ms = base.saturating_sub(elapsed.as_millis() as u64);
            sleep(Duration::from_millis(sleep_ms.max(50))).await;
        }
    }
}


