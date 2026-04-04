//! Balance query helpers using alkanes view functions.

use anyhow::Result;

use crate::runtime::TestRuntime;

/// Call alkanes_simulate view function.
pub fn simulate(
    runtime: &TestRuntime,
    target_block: u128,
    target_tx: u128,
    inputs: &[u128],
    height: u32,
) -> Result<Vec<u8>> {
    // Build the simulate request as the alkanes view expects:
    // The exact input format depends on the alkanes.wasm view ABI.
    // For now, expose the raw view call — typed wrappers added as we discover the ABI.
    let _ = (target_block, target_tx, inputs);
    runtime.alkanes_view("alkanes_simulate", &[], height)
}
