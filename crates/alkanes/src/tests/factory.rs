use crate::indexer::{index_block, configure_network};
use crate::tests::helpers::{
    self as alkane_helpers, assert_binary_deployed_to_id, assert_id_points_to_alkane_id,
    get_sheet_for_runtime,
};
use crate::tests::std::alkanes_std_test_build;
use crate::tests::test_runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use metashrew_support::environment::RuntimeEnvironment;
use protorune_support::balance_sheet::BalanceSheetOperations;

#[test]
fn test_factory_wasm_load() -> Result<()> {
    configure_network();
    let mut env = TestRuntime::default();
    alkane_helpers::clear::<TestRuntime>(&mut env);
    let block_height = 0;

    // Create a cellpack to call the process_numbers method (opcode 11)
    let arb_mint_cellpack = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![30, 2, 1, 1_000_000],
    };

    let send_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![3],
    };

    let create_another_cellpack = Cellpack {
        target: AlkaneId { block: 5, tx: 1 },
        inputs: vec![50],
    };

    let steal_cellpack = Cellpack {
        target: AlkaneId { block: 2, tx: 2 },
        inputs: vec![30, 2, 2, 1_000_000],
    };

    // Initialize the contract and execute the cellpacks
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_test_build::get_bytes(),
            [].into(),
            [].into(),
            [].into(),
        ]
        .into(),
        [
            arb_mint_cellpack,
            send_cellpack,
            create_another_cellpack,
            steal_cellpack,
        ]
        .into(),
    );

    index_block::<TestRuntime>(&mut env, &test_block, block_height)?;

    let sheet = alkane_helpers::get_last_outpoint_sheet::<TestRuntime>(&mut env, &test_block)?;

    let runtime_sheet = get_sheet_for_runtime::<TestRuntime>(&mut env);
    let orig_alkane = AlkaneId { block: 2, tx: 1 };
    let copy_alkane = AlkaneId { block: 2, tx: 2 };

    assert_eq!(
        runtime_sheet.get(&orig_alkane.clone().into(), &mut env),
        1000000
    );
    assert_eq!(sheet.get(&orig_alkane.clone().into(), &mut env), 0);
    assert_eq!(sheet.get(&copy_alkane.clone().into(), &mut env), 1000000);

    let _ = assert_binary_deployed_to_id::<TestRuntime>(
        &mut env,
        orig_alkane.clone(),
        alkanes_std_test_build::get_bytes(),
    );

    assert_id_points_to_alkane_id::<TestRuntime>(&mut env, copy_alkane.clone(), orig_alkane.clone())?;
    Ok(())
}