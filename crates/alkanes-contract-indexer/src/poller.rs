use deezel_common::provider::ConcreteProvider;
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration, Instant};
use tracing::{error, info, warn};

use crate::{pipeline::{BlockContext, Pipeline}, helpers::block::canonical_tip_height};

pub struct BlockPoller {
    provider: ConcreteProvider,
    pipeline: Pipeline,
    poll_interval_ms: u64,
    init_signal: Option<oneshot::Sender<()>>, // fired once after initial pools refresh + height init
    start_height: Option<u64>,
}

impl BlockPoller {
    pub fn new(
        provider: ConcreteProvider,
        pipeline: Pipeline,
        poll_interval_ms: u64,
        init_signal: Option<oneshot::Sender<()>>,
        start_height: Option<u64>,
    ) -> Self {
        Self { provider, pipeline, poll_interval_ms, init_signal, start_height }
    }

    pub async fn run(mut self) {
        let mut last_height: Option<u64> = None;
        let mut backoff_ms: u64 = self.poll_interval_ms.max(250);
        loop {
            let tick_start = Instant::now();
            match canonical_tip_height(&self.provider).await {
                Ok(height) => {
                    backoff_ms = self.poll_interval_ms; // reset on success
                    match last_height {
                        None => {
                            // On first observation, refresh pools/state once
                            if let Err(e) = self.pipeline.fetch_pools_for_tip(&self.provider, height).await {
                                error!(height, error = %e, "fetch_pools_for_tip failed");
                            }
                            info!(height, "initialized metashrew height");
                            // If we are not running catch-up, begin processing the current tip immediately
                            if self.start_height.is_none() {
                                info!(height, "new block detected");
                                if let Err(e) = self.pipeline.process_block_sequential(&self.provider, BlockContext { height, emit_publish: true }).await {
                                    error!(height, error = %e, "block processing failed");
                                }
                            }
                            last_height = Some(height);
                            if let Some(tx) = self.init_signal.take() {
                                let _ = tx.send(());
                            }
                        }
                        Some(prev) if height > prev => {
                            // Before processing new blocks, update pools and states once at the new tip
                            if let Err(e) = self.pipeline.fetch_pools_for_tip(&self.provider, height).await {
                                error!(height, error = %e, "fetch_pools_for_tip failed");
                            }
                            for h in (prev + 1)..=height {
                                info!(height = h, "new block detected");
                                if let Err(e) = self.pipeline.process_block_sequential(&self.provider, BlockContext { height: h, emit_publish: true }).await {
                                    error!(height = h, error = %e, "block processing failed");
                                    // Stop advancing so we can retry this specific height on next loop
                                    break;
                                } else {
                                    last_height = Some(h);
                                }
                            }
                        }
                        Some(prev) if height < prev => {
                            warn!(current = height, prev, "height decreased, possible reorg; updating pointer");
                            last_height = Some(height);
                        }
                        _ => {}
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


