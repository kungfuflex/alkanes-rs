//! AMM deployment helpers — deploy factory, pool, beacon, auth token contracts.
//!
//! Ports the deployment sequence from ~/espo/src/test_utils/amm_helpers.rs.

use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::Block;
use std::collections::HashMap;

use crate::block_builder::{create_block_with_deploys, DeployPair};
use crate::fixtures;
use crate::runtime::TestRuntime;

pub struct AmmDeployment {
    pub pool_template_id: AlkaneId,
    pub auth_token_factory_id: AlkaneId,
    pub factory_logic_id: AlkaneId,
    pub beacon_proxy_id: AlkaneId,
    pub upgradeable_beacon_id: AlkaneId,
    pub factory_proxy_id: AlkaneId,
    pub blocks: HashMap<u32, Block>,
}

/// Deploy the full AMM infrastructure.
///
/// Uses 6 blocks starting at `start_height`:
/// 1. Pool template
/// 2. Auth token factory
/// 3. Factory logic
/// 4. Beacon proxy
/// 5. Upgradeable beacon (points to pool template)
/// 6. Factory proxy + init
pub fn setup_amm(runtime: &TestRuntime, start_height: u32) -> Result<AmmDeployment> {
    let mut blocks = HashMap::new();
    let mut h = start_height;

    // Block 1: Pool template
    let pool_block = create_block_with_deploys(
        h,
        vec![DeployPair::new(
            fixtures::POOL,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![0],
            },
        )],
    );
    runtime.index_block(&pool_block, h)?;
    let pool_template_id = AlkaneId {
        block: (h + 2) as u128,
        tx: 1,
    };
    blocks.insert(h, pool_block);
    h += 1;

    // Block 2: Auth token factory
    let auth_block = create_block_with_deploys(
        h,
        vec![DeployPair::new(
            fixtures::AUTH_TOKEN,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![0],
            },
        )],
    );
    runtime.index_block(&auth_block, h)?;
    let auth_token_factory_id = AlkaneId {
        block: (h + 2) as u128,
        tx: 1,
    };
    blocks.insert(h, auth_block);
    h += 1;

    // Block 3: Factory logic
    let factory_block = create_block_with_deploys(
        h,
        vec![DeployPair::new(
            fixtures::FACTORY,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![0],
            },
        )],
    );
    runtime.index_block(&factory_block, h)?;
    let factory_logic_id = AlkaneId {
        block: (h + 2) as u128,
        tx: 1,
    };
    blocks.insert(h, factory_block);
    h += 1;

    // Block 4: Beacon proxy
    let beacon_proxy_block = create_block_with_deploys(
        h,
        vec![DeployPair::new(
            fixtures::BEACON_PROXY,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![0],
            },
        )],
    );
    runtime.index_block(&beacon_proxy_block, h)?;
    let beacon_proxy_id = AlkaneId {
        block: (h + 2) as u128,
        tx: 1,
    };
    blocks.insert(h, beacon_proxy_block);
    h += 1;

    // Block 5: Upgradeable beacon (points to pool template)
    let ub_block = create_block_with_deploys(
        h,
        vec![DeployPair::new(
            fixtures::UPGRADEABLE_BEACON,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![
                    0x7fff,
                    pool_template_id.block,
                    pool_template_id.tx,
                    1,
                ],
            },
        )],
    );
    runtime.index_block(&ub_block, h)?;
    let upgradeable_beacon_id = AlkaneId {
        block: (h + 2) as u128,
        tx: 1,
    };
    blocks.insert(h, ub_block);
    h += 1;

    // Block 6: Factory proxy (upgradeable) + init
    let proxy_block = create_block_with_deploys(
        h,
        vec![
            // Deploy upgradeable proxy
            DeployPair::new(
                fixtures::UPGRADEABLE,
                Cellpack {
                    target: AlkaneId { block: 1, tx: 0 },
                    inputs: vec![
                        0x7fff,
                        factory_logic_id.block,
                        factory_logic_id.tx,
                        1,
                    ],
                },
            ),
            // Init the factory via proxy
            DeployPair::call_only(Cellpack {
                target: AlkaneId {
                    block: (h + 2) as u128,
                    tx: 1,
                },
                inputs: vec![
                    0,
                    beacon_proxy_id.block,
                    beacon_proxy_id.tx,
                    upgradeable_beacon_id.block,
                    upgradeable_beacon_id.tx,
                ],
            }),
        ],
    );
    runtime.index_block(&proxy_block, h)?;
    let factory_proxy_id = AlkaneId {
        block: (h + 2) as u128,
        tx: 1,
    };
    blocks.insert(h, proxy_block);

    Ok(AmmDeployment {
        pool_template_id,
        auth_token_factory_id,
        factory_logic_id,
        beacon_proxy_id,
        upgradeable_beacon_id,
        factory_proxy_id,
        blocks,
    })
}
