use super::AlkanesRuntimeContext;
use std::sync::{Arc, Mutex};
use wasmi::*;

pub struct AlkanesState {
    pub(super) had_failure: bool,
    pub(super) context: Arc<Mutex<AlkanesRuntimeContext>>,
    pub(super) limiter: StoreLimits,
}
