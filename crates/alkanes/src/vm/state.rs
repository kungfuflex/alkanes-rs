use super::AlkanesRuntimeContext;
use metashrew_sync::traits::RuntimeAdapter;
use std::sync::{Arc, Mutex};
use wasmi::*;

pub struct AlkanesState<'a, E: RuntimeAdapter + Clone> {
    pub(super) had_failure: bool,
    pub(super) context: Arc<Mutex<AlkanesRuntimeContext<E>>>,
    pub(super) limiter: StoreLimits,
    pub(super) env: &'a mut E,
}