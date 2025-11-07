use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers, assert_revert_context};
use crate::tests::std::{
    alkanes_std_auth_token_build, alkanes_std_beacon_proxy_build, alkanes_std_test_2_build,
    alkanes_std_test_build, alkanes_std_upgradeable_beacon_build, alkanes_std_upgradeable_build,
};
use alkane_helpers::clear;
use alkanes::view;
use alkanes::vm::utils::sequence_pointer;
use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{Trace, TraceEvent};
use alkanes_support::{cellpack::Cellpack, constants::AUTH_TOKEN_FACTORY_ID};
use anyhow::Result;
use bitcoin::block::Header;
use bitcoin::{Block, Transaction};
use bitcoin::{OutPoint, Witness};
use metashrew_core::index_pointer::AtomicPointer;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::test_helpers::{create_block_with_coinbase_tx, create_coinbase_transaction};
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::utils::consensus_decode;
use wasm_bindgen_test::wasm_bindgen_test;

pub const BEACON_ID: u128 = 0xbeac0;

fn setup_env() -> Result<Block> {
    clear();
    let block_height = 0;
    let auth_cellpack = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        inputs: vec![100],
    };
    let test = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![0], // even though calling initialize here, this should not affect the proxies
    };

    // Initialize the contract and execute the cellpacks
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_test_build::get_bytes(),
            alkanes_std_test_2_build::get_bytes(),
        ]
        .into(),
        [auth_cellpack, test.clone(), test.clone()].into(),
    );

    index_block(&test_block, block_height)?;

    Ok(test_block)
}

fn deploy_upgradeable_beacon() -> Result<Block> {
    let block_height = 0;
    let beacon = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: BEACON_ID,
        },
        inputs: vec![0x7fff, 2, 1, 1],
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_upgradeable_beacon_build::get_bytes()].into(),
        [beacon].into(),
    );

    index_block(&test_block, block_height)?;

    Ok(test_block)
}

fn deploy_upgradeable_proxy(
    proxy_build: Vec<u8>,
    block_height: u32,
    delegate_target: AlkaneId,
) -> Result<(Block, u128)> {
    let mut next_sequence_pointer = sequence_pointer(&mut AtomicPointer::default());
    let proxy_sequence = next_sequence_pointer.get_value::<u128>();
    let proxy = alkane_helpers::BinaryAndCellpack {
        binary: proxy_build,
        cellpack: Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0x7fff, delegate_target.block, delegate_target.tx, 1],
        },
    };

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_cellpack_pairs([proxy].into());

    index_block(&test_block, block_height)?;

    Ok((test_block, proxy_sequence))
}

fn upgradeability_harness(
    proxy_sequence: u128,
    block_height: u32,
    delegate_target: AlkaneId,
) -> Result<()> {
    let initialize = alkane_helpers::BinaryAndCellpack::cellpack_only(Cellpack {
        target: AlkaneId {
            block: 2,
            tx: proxy_sequence,
        },
        inputs: vec![0],
    });
    let set_claimable = alkane_helpers::BinaryAndCellpack::cellpack_only(Cellpack {
        target: AlkaneId {
            block: 2,
            tx: proxy_sequence,
        },
        inputs: vec![104, 10],
    });
    let mint = alkane_helpers::BinaryAndCellpack::cellpack_only(Cellpack {
        target: AlkaneId {
            block: 2,
            tx: proxy_sequence,
        },
        inputs: vec![22, 1_000_000],
    });
    let double_init = alkane_helpers::BinaryAndCellpack::cellpack_only(Cellpack {
        target: AlkaneId {
            block: 2,
            tx: proxy_sequence,
        },
        inputs: vec![0x7fff, delegate_target.block, delegate_target.tx, 1],
    });

    // Initialize the contract and execute the cellpacks
    let mut test_block = alkane_helpers::init_with_cellpack_pairs(
        [initialize, set_claimable, mint, double_init].into(),
    );

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;
    assert_eq!(
        sheet.get_cached(&ProtoruneRuneId {
            block: 2,
            tx: proxy_sequence
        }),
        1_000_000
    );
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);
    assert_revert_context(
        &OutPoint {
            txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
            vout: 3,
        },
        "proxy already initialized",
    )?;

    let proxy_through_extcall = alkane_helpers::BinaryAndCellpack::cellpack_only(Cellpack {
        target: AlkaneId {
            block: 2,
            tx: 1, // test contract
        },
        inputs: vec![110, 2, proxy_sequence, 2, 22, 1_000_000],
    });

    // Initialize the contract and execute the cellpacks
    let mut test_block2 = alkane_helpers::init_with_cellpack_pairs([proxy_through_extcall].into());

    index_block(&test_block2, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block2)?;
    assert_eq!(
        sheet.get_cached(&ProtoruneRuneId {
            block: 2,
            tx: proxy_sequence
        }),
        1_000_000
    );
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);
    Ok(())
}

fn upgrade_implementation(
    block_height: u32,
    input_outpoint: OutPoint,
    contract_to_upgrade: AlkaneId,
) -> Result<()> {
    let upgrade = Cellpack {
        target: contract_to_upgrade,
        inputs: vec![0x7ffe, 2, 2],
    };

    let mut test_block = create_block_with_coinbase_tx(block_height);

    test_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![upgrade],
            input_outpoint,
            false,
        ),
    );

    index_block(&test_block, block_height)?;
    Ok(())
}

fn check_after_upgrade(block_height: u32, proxy_sequence: u128) -> Result<()> {
    let incr = Cellpack {
        target: AlkaneId {
            block: 2,
            tx: proxy_sequence,
        },
        inputs: vec![105],
    };
    let initialize = Cellpack {
        target: AlkaneId {
            block: 2,
            tx: proxy_sequence,
        },
        inputs: vec![0],
    };
    let get_claimable = Cellpack {
        target: AlkaneId {
            block: 2,
            tx: proxy_sequence,
        },
        inputs: vec![103],
    };
    let mint = Cellpack {
        target: AlkaneId {
            block: 2,
            tx: proxy_sequence,
        },
        inputs: vec![22, 1_000_000],
    };

    let mut test_block = create_block_with_coinbase_tx(block_height);

    test_block
        .txdata
        .push(alkane_helpers::create_multiple_cellpack_with_witness(
            Witness::new(),
            vec![incr, get_claimable, mint, initialize],
            false,
        ));

    index_block(&test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet(&test_block)?;
    assert_eq!(
        sheet.get_cached(&ProtoruneRuneId {
            block: 2,
            tx: proxy_sequence
        }),
        1_000_000
    );
    assert_eq!(sheet.get_cached(&ProtoruneRuneId { block: 2, tx: 1 }), 0);

    let outpoint = OutPoint {
        txid: test_block.txdata[1].compute_txid(),
        vout: 4,
    };

    alkane_helpers::assert_return_context(&outpoint, |trace_response| {
        let data = trace_response.inner.data;

        assert_eq!(data[0], 12);
        Ok(())
    })?;

    assert_revert_context(
        &OutPoint {
            txid: test_block.txdata[1].compute_txid(),
            vout: 6,
        },
        "already initialized",
    )?;

    Ok(())
}

#[wasm_bindgen_test]
fn test_proxy() -> Result<()> {
    setup_env()?;
    let (_, proxy_sequence) = deploy_upgradeable_proxy(
        alkanes_std_upgradeable_build::get_bytes(),
        0,
        AlkaneId { block: 2, tx: 1 },
    )?;
    upgradeability_harness(proxy_sequence, 0, AlkaneId { block: 2, tx: 1 })?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_upgradeability() -> Result<()> {
    setup_env()?;
    let (init_block, proxy_sequence) = deploy_upgradeable_proxy(
        alkanes_std_upgradeable_build::get_bytes(),
        0,
        AlkaneId { block: 2, tx: 1 },
    )?;
    upgradeability_harness(proxy_sequence, 0, AlkaneId { block: 2, tx: 1 })?;
    upgrade_implementation(
        0,
        OutPoint {
            txid: init_block.txdata[init_block.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
        AlkaneId {
            block: 2,
            tx: proxy_sequence,
        },
    )?;
    check_after_upgrade(0, proxy_sequence)
}

#[wasm_bindgen_test]
fn test_beacon_proxy() -> Result<()> {
    setup_env()?;
    let init_block = deploy_upgradeable_beacon()?;
    println!("deployed upgradeable beacon");
    let (_, proxy_sequence_1) = deploy_upgradeable_proxy(
        alkanes_std_beacon_proxy_build::get_bytes(),
        0,
        AlkaneId {
            block: 4,
            tx: BEACON_ID,
        },
    )?;
    println!("deployed first beacon proxy");
    upgradeability_harness(
        proxy_sequence_1,
        0,
        AlkaneId {
            block: 4,
            tx: BEACON_ID,
        },
    )?;
    println!("tested first beacon proxy");

    let (_, proxy_sequence_2) = deploy_upgradeable_proxy(
        alkanes_std_beacon_proxy_build::get_bytes(),
        0,
        AlkaneId {
            block: 4,
            tx: BEACON_ID,
        },
    )?;
    println!("deployed second beacon proxy");
    upgradeability_harness(
        proxy_sequence_2,
        0,
        AlkaneId {
            block: 4,
            tx: BEACON_ID,
        },
    )?;
    println!("tested second beacon proxy");
    upgrade_implementation(
        0,
        OutPoint {
            txid: init_block.txdata[init_block.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
        AlkaneId {
            block: 4,
            tx: BEACON_ID,
        },
    )?;
    check_after_upgrade(0, proxy_sequence_1)?;
    check_after_upgrade(0, proxy_sequence_2)
}
