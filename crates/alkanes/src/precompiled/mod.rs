pub mod alkanes_std_auth_token_build;
pub mod alkanes_std_beacon_proxy_build;
pub mod alkanes_std_genesis_alkane_bellscoin_build;
pub mod alkanes_std_genesis_alkane_dogecoin_build;
pub mod alkanes_std_genesis_alkane_fractal_build;
pub mod alkanes_std_genesis_alkane_luckycoin_build;
pub mod alkanes_std_genesis_alkane_mainnet_build;
pub mod alkanes_std_genesis_alkane_regtest_build;
pub mod alkanes_std_genesis_alkane_upgraded_eoa_mainnet_build;
pub mod alkanes_std_genesis_alkane_upgraded_eoa_regtest_build;
pub mod alkanes_std_genesis_alkane_upgraded_mainnet_build;
pub mod alkanes_std_genesis_alkane_upgraded_regtest_build;
pub mod alkanes_std_owned_token_build;
pub mod alkanes_std_proxy_build;
pub mod alkanes_std_upgradeable_beacon_build;
pub mod alkanes_std_upgradeable_build;
pub mod fr_btc_build;
pub mod fr_btc_build_v1_1_0;
pub mod fr_btc_build_v1_2_0;
pub mod fr_btc_build_v1_3_0;
pub mod fr_btc_build_v1_3_1;
pub mod fr_sigil_build;
pub mod free_mint_build;
pub mod alkanes_std_recycle_build;

use alkanes_support::id::AlkaneId;
use anyhow::{anyhow, Result};
use std::sync::Arc;

/// Resolve the embedded binary for an `8:*` precompiled "life WASM". `8:dead` is
/// the recycle bin (`alkanes-std-recycle`). Called from `run_special_cellpacks`.
pub fn precompiled_life_wasm(target: &AlkaneId) -> Result<Arc<Vec<u8>>> {
    if target.block == crate::recycle::RECYCLE_ALKANE_ID.block
        && target.tx == crate::recycle::RECYCLE_ALKANE_ID.tx
    {
        let bytes = alkanes_std_recycle_build::get_bytes();
        if bytes.is_empty() {
            return Err(anyhow!(
                "recycle precompiled (8:dead) not yet embedded — build alkanes-std-recycle to wasm"
            ));
        }
        Ok(Arc::new(bytes))
    } else {
        Err(anyhow!(
            "no precompiled life wasm at {}:{}",
            target.block,
            target.tx
        ))
    }
}
