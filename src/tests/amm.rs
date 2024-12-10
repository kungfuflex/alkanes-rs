use crate::tests::std::{alkanes_std_amm_pool_build, alkanes_std_auth_token_build};
use alkanes::message::AlkaneMessageContext;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::constants::{AMM_FACTORY_ID, AUTH_TOKEN_FACTORY_ID};
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::address::NetworkChecked;
use bitcoin::blockdata::transaction::OutPoint;
use bitcoin::{Address, Amount, Block, ScriptBuf, Sequence, TxIn, TxOut, Witness};
use metashrew_support::index_pointer::KeyValuePointer;
use num::integer::Roots;
use protorune::test_helpers::create_block_with_coinbase_tx;
use protorune::{balance_sheet::load_sheet, message::MessageContext, tables::RuneTable};
use protorune_support::balance_sheet::BalanceSheet;
use protorune_support::protostone::Protostone;
use protorune_support::protostone::ProtostoneEdict;
use protorune_support::utils::consensus_encode;

use crate::index_block;
use crate::tests::helpers::{
    self as alkane_helpers, assert_binary_deployed_to_id, assert_token_id_has_no_deployment,
};
use crate::tests::std::{alkanes_std_amm_factory_build, alkanes_std_owned_token_build};
use alkane_helpers::clear;
#[allow(unused_imports)]
use metashrew::{get_cache, index_pointer::IndexPointer, println, stdio::stdout};
use std::fmt::Write;
use wasm_bindgen_test::wasm_bindgen_test;

struct AmmTestDeploymentIds {
    amm_pool_factory: AlkaneId,
    auth_token_factory: AlkaneId,
    amm_factory_deployment: AlkaneId,
    owned_token_1_deployment: AlkaneId,
    auth_token_1_deployment: AlkaneId,
    owned_token_2_deployment: AlkaneId,
    auth_token_2_deployment: AlkaneId,
    amm_pool_deployment: AlkaneId,
}

// per uniswap docs, the first 1e3 wei of lp token minted are burned to mitigate attacks where the value of a lp token is raised too high easily
pub const MINIMUM_LIQUIDITY: u128 = 1000;

fn init_block_with_amm_pool() -> Result<(Block, AmmTestDeploymentIds)> {
    let cellpacks: Vec<Cellpack> = [
        //amm pool factory init
        Cellpack {
            target: AlkaneId {
                block: 3,
                tx: AMM_FACTORY_ID,
            },
            inputs: vec![50],
        },
        //auth token factory init
        Cellpack {
            target: AlkaneId {
                block: 3,
                tx: AUTH_TOKEN_FACTORY_ID,
            },
            inputs: vec![100],
        },
        Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0],
        },
        // token 1 init 1 auth token and mint 1000000 owned tokens
        Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0, 1, 1000000],
        },
        // token 2 init 1 auth token and mint 1000000 owned tokens
        Cellpack {
            target: AlkaneId { block: 5, tx: 2 }, // factory creation of owned token using {2, 1} as the factory. Then it deploys to {2,3}
            inputs: vec![0, 1, 1000000],
        },
    ]
    .into();
    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_amm_pool_build::get_bytes(),
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_amm_factory_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
            [].into(),
        ]
        .into(),
        cellpacks,
    );
    let deployed_ids = AmmTestDeploymentIds {
        amm_pool_factory: AlkaneId {
            block: 4,
            tx: AMM_FACTORY_ID,
        },
        auth_token_factory: AlkaneId {
            block: 4,
            tx: AUTH_TOKEN_FACTORY_ID,
        },
        amm_factory_deployment: AlkaneId { block: 2, tx: 1 },
        owned_token_1_deployment: AlkaneId { block: 2, tx: 2 },
        auth_token_1_deployment: AlkaneId { block: 2, tx: 3 },
        owned_token_2_deployment: AlkaneId { block: 2, tx: 4 },
        auth_token_2_deployment: AlkaneId { block: 2, tx: 5 },
        amm_pool_deployment: AlkaneId { block: 2, tx: 6 },
    };
    return Ok((test_block, deployed_ids));
}

fn assert_contracts_correct_ids(deployment_ids: &AmmTestDeploymentIds) -> Result<()> {
    let _ = assert_binary_deployed_to_id(
        deployment_ids.amm_pool_factory.clone(),
        alkanes_std_amm_pool_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        deployment_ids.auth_token_factory.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );

    let _ = assert_binary_deployed_to_id(
        deployment_ids.amm_factory_deployment.clone(),
        alkanes_std_amm_factory_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        deployment_ids.owned_token_1_deployment.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        deployment_ids.owned_token_2_deployment.clone(),
        alkanes_std_owned_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        deployment_ids.auth_token_1_deployment.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        deployment_ids.auth_token_2_deployment.clone(),
        alkanes_std_auth_token_build::get_bytes(),
    );
    let _ = assert_binary_deployed_to_id(
        deployment_ids.amm_pool_deployment.clone(),
        alkanes_std_amm_pool_build::get_bytes(),
    );
    Ok(())
}

fn insert_split_tx(
    test_block: &mut Block,
    deployment_ids: &AmmTestDeploymentIds,
    input_outpoint: OutPoint,
    protostone_edicts: Vec<ProtostoneEdict>,
) {
    let address: Address<NetworkChecked> =
        protorune::test_helpers::get_address(&protorune::test_helpers::ADDRESS1().as_str());
    let script_pubkey = address.script_pubkey();
    let split = alkane_helpers::create_protostone_tx_with_inputs(
        vec![TxIn {
            previous_output: input_outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        vec![
            TxOut {
                value: Amount::from_sat(546),
                script_pubkey: script_pubkey.clone(),
            },
            TxOut {
                value: Amount::from_sat(546),
                script_pubkey: script_pubkey.clone(),
            },
        ],
        Protostone {
            from: None,
            burn: None,
            protocol_tag: 1,
            message: vec![],
            pointer: Some(1),
            refund: None,
            edicts: protostone_edicts,
        },
    );
    test_block.txdata.push(split);
}

fn insert_single_edict_split_tx(
    amount: u128,
    target: AlkaneId,
    test_block: &mut Block,
    deployment_ids: &AmmTestDeploymentIds,
    input_outpoint: OutPoint,
) {
    insert_split_tx(
        test_block,
        deployment_ids,
        input_outpoint,
        vec![ProtostoneEdict {
            id: target.into(),
            amount: amount,
            output: 0,
        }],
    );
}

fn insert_two_edict_split_tx(
    amount1: u128,
    amount2: u128,
    test_block: &mut Block,
    deployment_ids: &AmmTestDeploymentIds,
    input_outpoint: OutPoint,
) {
    insert_split_tx(
        test_block,
        deployment_ids,
        input_outpoint,
        vec![
            ProtostoneEdict {
                id: deployment_ids.owned_token_1_deployment.into(),
                amount: amount1,
                output: 0,
            },
            ProtostoneEdict {
                id: deployment_ids.owned_token_2_deployment.into(),
                amount: amount2,
                output: 0,
            },
        ],
    );
}

fn insert_init_pool_liquidity_txs(
    amount1: u128,
    amount2: u128,
    test_block: &mut Block,
    deployment_ids: &AmmTestDeploymentIds,
) {
    insert_two_edict_split_tx(
        amount1,
        amount2,
        test_block,
        deployment_ids,
        OutPoint {
            txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
            vout: 0,
        },
    );
    test_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![Cellpack {
                target: deployment_ids.amm_factory_deployment,
                inputs: vec![1],
            }],
            OutPoint {
                txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
                vout: 0,
            },
            false,
        ),
    );
}

fn insert_add_liquidity_txs(
    amount1: u128,
    amount2: u128,
    test_block: &mut Block,
    deployment_ids: &AmmTestDeploymentIds,
    input_outpoint: OutPoint,
) {
    insert_two_edict_split_tx(amount1, amount2, test_block, deployment_ids, input_outpoint);
    test_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![Cellpack {
                target: deployment_ids.amm_pool_deployment,
                inputs: vec![1],
            }],
            OutPoint {
                txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
                vout: 0,
            },
            false,
        ),
    );
}

fn insert_remove_liquidity_txs(
    amount: u128,
    test_block: &mut Block,
    deployment_ids: &AmmTestDeploymentIds,
    input_outpoint: OutPoint,
) {
    insert_single_edict_split_tx(
        amount,
        deployment_ids.amm_pool_deployment,
        test_block,
        deployment_ids,
        input_outpoint,
    );
    test_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![Cellpack {
                target: deployment_ids.amm_pool_deployment,
                inputs: vec![2],
            }],
            OutPoint {
                txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
                vout: 0,
            },
            false,
        ),
    );
}

fn insert_swap_txs(
    amount: u128,
    target: AlkaneId,
    min_out: u128,
    test_block: &mut Block,
    deployment_ids: &AmmTestDeploymentIds,
    input_outpoint: OutPoint,
) {
    insert_single_edict_split_tx(amount, target, test_block, deployment_ids, input_outpoint);
    test_block.txdata.push(
        alkane_helpers::create_multiple_cellpack_with_witness_and_in(
            Witness::new(),
            vec![Cellpack {
                target: deployment_ids.amm_pool_deployment,
                inputs: vec![3, min_out],
            }],
            OutPoint {
                txid: test_block.txdata[test_block.txdata.len() - 1].compute_txid(),
                vout: 0,
            },
            false,
        ),
    );
}

fn calc_lp_balance_from_pool_init(amount1: u128, amount2: u128) -> u128 {
    if (amount1 * amount2).sqrt() < MINIMUM_LIQUIDITY {
        return 0;
    }
    return (amount1 * amount2).sqrt() - MINIMUM_LIQUIDITY;
}

fn calc_lp_balance_from_add_liquidity(
    prev_amount1: u128,
    prev_amount2: u128,
    added_amount1: u128,
    added_amount2: u128,
    total_supply: u128,
) -> u128 {
    let root_k = ((prev_amount1 + added_amount1) * (prev_amount2 + added_amount2)).sqrt();
    let root_k_last = (prev_amount1 * prev_amount2).sqrt();
    let numerator = total_supply * (root_k - root_k_last);
    let denominator = root_k * 5 + root_k_last;
    numerator / denominator
}

fn calc_swapped_balance(amount: u128, reserve_from: u128, reserve_to: u128) -> Result<u128> {
    Ok((reserve_to - (reserve_from * reserve_to / (reserve_from + amount))) * 997 / 1000)
}

fn get_sheet_for_outpoint(test_block: &Block, tx_num: usize, vout: u32) -> Result<BalanceSheet> {
    let outpoint = OutPoint {
        txid: test_block.txdata[tx_num].compute_txid(),
        vout,
    };
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(&outpoint)?);
    let sheet = load_sheet(&ptr);
    println!(
        "balances at outpoint tx {} vout {}: {:?}",
        tx_num, vout, sheet
    );
    Ok(sheet)
}

fn get_last_outpoint_sheet(test_block: &Block) -> Result<BalanceSheet> {
    let len = test_block.txdata.len();
    get_sheet_for_outpoint(test_block, len - 1, 0)
}

fn get_sheet_with_remaining_lp_after_burn(test_block: &Block) -> Result<BalanceSheet> {
    let len = test_block.txdata.len();
    get_sheet_for_outpoint(test_block, len - 2, 1)
}

fn check_init_liquidity_lp_balance(
    amount1: u128,
    amount2: u128,
    test_block: &Block,
    deployment_ids: &AmmTestDeploymentIds,
) -> Result<()> {
    let sheet = get_last_outpoint_sheet(test_block)?;
    let expected_amount = calc_lp_balance_from_pool_init(amount1, amount2);
    println!("expected amt from init {:?}", expected_amount);
    assert_eq!(
        sheet.get(&deployment_ids.amm_pool_deployment.into()),
        expected_amount
    );
    Ok(())
}

fn check_add_liquidity_lp_balance(
    prev_amount1: u128,
    prev_amount2: u128,
    added_amount1: u128,
    added_amount2: u128,
    total_supply: u128,
    test_block: &Block,
    deployment_ids: &AmmTestDeploymentIds,
) -> Result<()> {
    let sheet = get_last_outpoint_sheet(test_block)?;
    let expected_amount = calc_lp_balance_from_add_liquidity(
        prev_amount1,
        prev_amount2,
        added_amount1,
        added_amount2,
        total_supply,
    );
    println!("expected amt from adding liquidity {:?}", expected_amount);
    assert_eq!(
        sheet.get(&deployment_ids.amm_pool_deployment.into()),
        expected_amount
    );
    Ok(())
}

fn check_swap_lp_balance(
    prev_reserve_from: u128,
    prev_reserve_to: u128,
    swap_amount: u128,
    swap_target_token: AlkaneId,
    test_block: &Block,
    deployment_ids: &AmmTestDeploymentIds,
) -> Result<()> {
    let sheet = get_last_outpoint_sheet(test_block)?;
    let expected_amount = calc_swapped_balance(swap_amount, prev_reserve_from, prev_reserve_to)?;
    println!("expected amt from swapping {:?}", expected_amount);
    assert_eq!(sheet.get(&swap_target_token.into()), expected_amount);
    Ok(())
}

fn test_amm_pool_init_fixture(
    amount1: u128,
    amount2: u128,
) -> Result<(Block, AmmTestDeploymentIds)> {
    let block_height = 840_000;
    let (mut test_block, deployment_ids) = init_block_with_amm_pool()?;
    insert_init_pool_liquidity_txs(amount1, amount2, &mut test_block, &deployment_ids);
    index_block(&test_block, block_height)?;
    assert_contracts_correct_ids(&deployment_ids)?;
    check_init_liquidity_lp_balance(amount1, amount2, &test_block, &deployment_ids)?;
    Ok((test_block, deployment_ids))
}

fn test_amm_burn_fixture(amount_burn: u128) -> Result<()> {
    let (amount1, amount2) = (1000000, 1000000);
    let total_lp = calc_lp_balance_from_pool_init(1000000, 1000000);
    let total_supply = (amount1 * amount2).sqrt();
    let (mut init_block, deployment_ids) = test_amm_pool_init_fixture(amount1, amount2)?;

    let block_height = 840_001;
    let mut test_block = create_block_with_coinbase_tx(block_height);
    let input_outpoint = OutPoint {
        txid: init_block.txdata[init_block.txdata.len() - 1].compute_txid(),
        vout: 0,
    };
    insert_remove_liquidity_txs(
        amount_burn,
        &mut test_block,
        &deployment_ids,
        input_outpoint,
    );
    index_block(&test_block, block_height)?;

    let sheet = get_sheet_with_remaining_lp_after_burn(&test_block)?;
    let amount_burned_true = std::cmp::min(amount_burn, total_lp);
    assert_eq!(
        sheet.get(&deployment_ids.amm_pool_deployment.into()),
        total_lp - amount_burned_true
    );

    let owned_alkane_sheets = get_last_outpoint_sheet(&test_block)?;
    assert_eq!(
        owned_alkane_sheets.get(&deployment_ids.owned_token_1_deployment.into()),
        amount_burned_true * amount1 / total_supply
    );
    assert_eq!(
        owned_alkane_sheets.get(&deployment_ids.owned_token_2_deployment.into()),
        amount_burned_true * amount2 / total_supply
    );
    Ok(())
}

#[wasm_bindgen_test]
fn test_amm_pool_normal_init() -> Result<()> {
    clear();
    test_amm_pool_init_fixture(1000000, 1000000)?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_amm_pool_skewed_init() -> Result<()> {
    clear();
    test_amm_pool_init_fixture(1000000 / 2, 1000000)?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_amm_pool_zero_init() -> Result<()> {
    clear();
    test_amm_pool_init_fixture(1000000, 1)?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_amm_pool_bad_init() -> Result<()> {
    clear();
    let block_height = 840_000;
    let (mut test_block, deployment_ids) = init_block_with_amm_pool()?;
    insert_init_pool_liquidity_txs(10000, 1, &mut test_block, &deployment_ids);
    index_block(&test_block, block_height)?;
    assert_token_id_has_no_deployment(deployment_ids.amm_pool_deployment);
    let sheet = get_last_outpoint_sheet(&test_block)?;
    assert_eq!(sheet.get(&deployment_ids.amm_pool_deployment.into()), 0);
    Ok(())
}

#[wasm_bindgen_test]
fn test_amm_pool_burn_all() -> Result<()> {
    clear();
    let total_lp = calc_lp_balance_from_pool_init(1000000, 1000000);
    test_amm_burn_fixture(total_lp)?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_amm_pool_burn_some() -> Result<()> {
    clear();
    let total_lp = calc_lp_balance_from_pool_init(1000000, 1000000);
    let burn_amount = total_lp / 3;
    test_amm_burn_fixture(burn_amount)?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_amm_pool_burn_more_than_owned() -> Result<()> {
    clear();
    let total_lp = calc_lp_balance_from_pool_init(1000000, 1000000);
    test_amm_burn_fixture(total_lp * 2)?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_amm_pool_add_more_liquidity() -> Result<()> {
    clear();
    let (amount1, amount2) = (500000, 500000);
    let total_supply = (amount1 * amount2).sqrt();
    let (init_block, deployment_ids) = test_amm_pool_init_fixture(amount1, amount2)?;
    let block_height = 840_001;
    let mut add_liquidity_block = create_block_with_coinbase_tx(block_height);
    // split init tx puts 1000000 / 2 in vout 0, and the other is unspent at vout 1. The split tx is now 2 from the tail
    let input_outpoint = OutPoint {
        txid: init_block.txdata[init_block.txdata.len() - 2].compute_txid(),
        vout: 1,
    };
    insert_add_liquidity_txs(
        amount1,
        amount2,
        &mut add_liquidity_block,
        &deployment_ids,
        input_outpoint,
    );
    index_block(&add_liquidity_block, block_height)?;

    check_add_liquidity_lp_balance(
        amount1,
        amount2,
        amount1,
        amount2,
        total_supply,
        &add_liquidity_block,
        &deployment_ids,
    )?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_amm_pool_swap() -> Result<()> {
    clear();
    let (amount1, amount2) = (500000, 500000);
    let (init_block, deployment_ids) = test_amm_pool_init_fixture(amount1, amount2)?;
    let block_height = 840_001;
    let mut swap_block = create_block_with_coinbase_tx(block_height);
    // split init tx puts 1000000 / 2 in vout 0, and the other is unspent at vout 1. The split tx is now 2 from the tail
    let input_outpoint = OutPoint {
        txid: init_block.txdata[init_block.txdata.len() - 2].compute_txid(),
        vout: 1,
    };
    let amount_to_swap = 10000;
    insert_swap_txs(
        amount_to_swap,
        deployment_ids.owned_token_1_deployment,
        0,
        &mut swap_block,
        &deployment_ids,
        input_outpoint,
    );
    index_block(&swap_block, block_height)?;

    check_swap_lp_balance(
        amount1,
        amount2,
        amount_to_swap,
        deployment_ids.owned_token_2_deployment,
        &swap_block,
        &deployment_ids,
    )?;
    Ok(())
}

#[wasm_bindgen_test]
fn test_amm_pool_swap_large() -> Result<()> {
    clear();
    let (amount1, amount2) = (500000, 500000);
    let (init_block, deployment_ids) = test_amm_pool_init_fixture(amount1, amount2)?;
    let block_height = 840_001;
    let mut swap_block = create_block_with_coinbase_tx(block_height);
    // split init tx puts 1000000 / 2 in vout 0, and the other is unspent at vout 1. The split tx is now 2 from the tail
    let input_outpoint = OutPoint {
        txid: init_block.txdata[init_block.txdata.len() - 2].compute_txid(),
        vout: 1,
    };
    let amount_to_swap = 500000;
    insert_swap_txs(
        amount_to_swap,
        deployment_ids.owned_token_1_deployment,
        0,
        &mut swap_block,
        &deployment_ids,
        input_outpoint,
    );
    index_block(&swap_block, block_height)?;

    check_swap_lp_balance(
        amount1,
        amount2,
        amount_to_swap,
        deployment_ids.owned_token_2_deployment,
        &swap_block,
        &deployment_ids,
    )?;
    Ok(())
}
