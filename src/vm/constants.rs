pub(super) const MEMORY_LIMIT: usize = 43554432;

// Maximum checkpoint (extcall) depth before an extcall is rejected as
// possible infinite recursion. This is consensus-critical: changing it
// alters which call chains revert, so it must match deployed indexers.
//
// Each nested extcall recurses on the host call stack. Under Node
// (wasm-bindgen-test) V8's stack overflows around depth 68, before this
// limit can fire, so tests exercise the revert path via the cfg(test)
// override below rather than by lowering this value.
pub const MAX_CHECKPOINT_DEPTH: usize = 75;

#[cfg(test)]
static MAX_CHECKPOINT_DEPTH_OVERRIDE: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(MAX_CHECKPOINT_DEPTH);

#[cfg(test)]
pub fn set_max_checkpoint_depth(depth: usize) {
    MAX_CHECKPOINT_DEPTH_OVERRIDE.store(depth, core::sync::atomic::Ordering::Relaxed);
}

pub fn max_checkpoint_depth() -> usize {
    #[cfg(test)]
    {
        MAX_CHECKPOINT_DEPTH_OVERRIDE.load(core::sync::atomic::Ordering::Relaxed)
    }
    #[cfg(not(test))]
    {
        MAX_CHECKPOINT_DEPTH
    }
}
