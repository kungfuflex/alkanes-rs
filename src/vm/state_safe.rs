use crate::vm::errors::{IndexerResult, lock_error};
use crate::vm::validation::{ValidationLayer, ResourceTracker};
use std::sync::{Arc, Mutex};
use wasmi::*;
use super::AlkanesRuntimeContext;

pub struct AlkanesStateSafe {
    pub(crate) had_failure: bool,
    pub(crate) context: Arc<Mutex<AlkanesRuntimeContext>>,
    pub(crate) limiter: StoreLimits,
    pub(crate) validation: ValidationLayer,
    pub(crate) metrics: Arc<Mutex<ResourceTracker>>,
}

impl AlkanesStateSafe {
    pub fn new(
        context: Arc<Mutex<AlkanesRuntimeContext>>,
        limiter: StoreLimits,
        validation: ValidationLayer,
    ) -> Self {
        Self {
            had_failure: false,
            context,
            limiter,
            validation,
            metrics: Arc::new(Mutex::new(ResourceTracker::new())),
        }
    }

    pub fn track_memory(&self, size: usize) -> IndexerResult<()> {
        self.metrics
            .lock()
            .map_err(lock_error)?
            .track_memory_allocation(size)
    }

    pub fn track_instruction(&self, count: u64) -> IndexerResult<()> {
        self.metrics
            .lock()
            .map_err(lock_error)?
            .track_instruction(count)
    }

    pub fn get_context(&self) -> IndexerResult<std::sync::MutexGuard<AlkanesRuntimeContext>> {
        self.context.lock().map_err(lock_error)
    }

    pub fn record_error(&self) {
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.record_error();
        }
    }

    pub fn set_failure(&mut self) {
        self.had_failure = true;
        self.record_error();
    }
}