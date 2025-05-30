use crate::index_block;
use crate::network::genesis;
use crate::tests::helpers as alkane_helpers;
use crate::tests::std::alkanes_std_genesis_alkane_build;
use crate::vm::fuel::{FuelTank, TOTAL_FUEL_START};
use alkane_helpers::clear;
use alkanes::message::AlkaneMessageContext;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::blockdata::transaction::OutPoint;
use bitcoin::hashes::Hash;
#[allow(unused_imports)]
use metashrew_core::{get_cache, index_pointer::IndexPointer, println, stdio::stdout};
use metashrew_support::index_pointer::KeyValuePointer;
use protorune::test_helpers::{create_block_with_coinbase_tx, create_protostone_encoded_tx};
use protorune::view::protorune_outpoint_to_outpoint_response;
use protorune::{balance_sheet::load_sheet, message::MessageContext, tables::RuneTable};
use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};
use protorune_support::protostone::Protostone;
use protorune_support::utils::consensus_encode;
use std::fmt::Write;
use wasm_bindgen_test::wasm_bindgen_test;
// Struct to track fuel benchmarks
struct FuelBenchmark {
    operation: String,
    initial_fuel: u64,
    final_fuel: u64,
    fuel_consumed: u64,
    fuel_percentage: f64,
}

impl FuelBenchmark {
    fn new(operation: &str, initial_fuel: u64, final_fuel: u64) -> Self {
        let fuel_consumed = initial_fuel - final_fuel;
        let fuel_percentage = (fuel_consumed as f64 / TOTAL_FUEL_START as f64) * 100.0;

        Self {
            operation: operation.to_string(),
            initial_fuel,
            final_fuel,
            fuel_consumed,
            fuel_percentage,
        }
    }

    fn display(&self) {
        println!(
            "│ {:<30} │ {:>12} │ {:>12} │ {:>12} │ {:>8.2}% │",
            self.operation,
            self.initial_fuel,
            self.final_fuel,
            self.fuel_consumed,
            self.fuel_percentage
        );
    }
}

fn display_benchmark_header() {
    println!("┌────────────────────────────────┬──────────────┬──────────────┬──────────────┬──────────┐");
    println!("│ Operation                      │ Initial Fuel │  Final Fuel  │ Fuel Consumed│ % of Max │");
    println!("├────────────────────────────────┼──────────────┼──────────────┼──────────────┼──────────┤");
}

fn display_benchmark_footer() {
    println!("└────────────────────────────────┴──────────────┴──────────────┴──────────────┴──────────┘");
}
#[wasm_bindgen_test]
fn test_genesis() -> Result<()> {
    clear();
    let block_height = 850_000;

    // Initialize fuel benchmarks collection
    let mut benchmarks = Vec::new();

    // Track initial fuel state
    let initial_total_fuel = TOTAL_FUEL_START;

    println!(
        "Starting Genesis Test with total fuel: {}",
        initial_total_fuel
    );

    // Genesis block with initialization cellpack
    let cellpacks: Vec<Cellpack> = [
        // Auth token factory init
        Cellpack {
            target: AlkaneId { block: 1, tx: 0 },
            inputs: vec![0],
        },
    ]
    .into();

    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_genesis_alkane_build::get_bytes(), vec![]].into(),
        cellpacks,
    );

    let len = test_block.txdata.len();
    let outpoint = OutPoint {
        txid: test_block.txdata[len - 1].compute_txid(),
        vout: 0,
    };

    println!(
        "Runestone: {}",
        hex::encode(&test_block.txdata[1].output[1].script_pubkey)
    );

    // Initialize FuelTank for the first block
    FuelTank::initialize(&test_block);
    let pre_genesis_fuel = TOTAL_FUEL_START;

    // Process the genesis block
    index_block(&test_block, block_height)?;

    // Get fuel state after genesis block
    let post_genesis_fuel = unsafe {
        match &FuelTank::get_fuel_tank_copy() {
            Some(tank) => tank.block_fuel,
            None => 0,
        }
    };

    // Record benchmark for genesis block
    benchmarks.push(FuelBenchmark::new(
        "Genesis Block Processing",
        pre_genesis_fuel,
        post_genesis_fuel,
    ));

    // Second block with mint operation
    let cellpacks2 = vec![Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![77], // Mint operation
    }];

    let test_block2 = alkane_helpers::init_with_multiple_cellpacks_with_tx([].into(), cellpacks2);

    // Initialize FuelTank for the second block
    FuelTank::initialize(&test_block2);
    let pre_mint_fuel = unsafe {
        match &FuelTank::get_fuel_tank_copy() {
            Some(tank) => tank.block_fuel,
            None => 0,
        }
    };

    // Process the mint block
    index_block(&test_block2, block_height + 1)?;

    // Get fuel state after mint block
    let post_mint_fuel = unsafe {
        match &FuelTank::get_fuel_tank_copy() {
            Some(tank) => tank.block_fuel,
            None => 0,
        }
    };

    // Record benchmark for mint operation
    benchmarks.push(FuelBenchmark::new(
        "Mint Operation Block",
        pre_mint_fuel,
        post_mint_fuel,
    ));

    // Check final balances
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(&outpoint)?);
    let sheet = load_sheet(&ptr);

    println!("Balances at end: {:?}", sheet);

    // Display fuel benchmarks
    println!("\n=== FUEL BENCHMARKS ===");
    display_benchmark_header();
    for benchmark in &benchmarks {
        benchmark.display();
    }

    // Calculate and display total fuel consumption
    let total_consumed = benchmarks.iter().fold(0, |acc, b| acc + b.fuel_consumed);
    let total_percentage = (total_consumed as f64 / initial_total_fuel as f64) * 100.0;

    println!("├────────────────────────────────┼──────────────┼──────────────┼──────────────┼──────────┤");
    println!(
        "│ TOTAL                          │ {:>12} │ {:>12} │ {:>12} │ {:>8.2}% │",
        initial_total_fuel,
        initial_total_fuel - total_consumed,
        total_consumed,
        total_percentage
    );
    display_benchmark_footer();

    Ok(())
}

#[wasm_bindgen_test]
fn test_genesis_alkane_key() -> Result<()> {
    println!(
        "{}",
        (IndexPointer::from_keyword("/alkanes/")
            .select(&(AlkaneId { tx: 2, block: 0 }).into())
            .get()
            .as_ref()
            .len())
    );
    println!(
        "key: {}",
        hex::encode(
            IndexPointer::from_keyword("/alkanes/")
                .select(&(AlkaneId { tx: 2, block: 0 }).into())
                .unwrap()
                .as_ref()
                .clone()
        )
    );
    Ok(())
}

#[wasm_bindgen_test]
fn test_genesis_indexer_premine() -> Result<()> {
    use bitcoin::Txid;

    clear();
    let block_height = 880_000;

    let test_block = create_block_with_coinbase_tx(block_height);

    // Process the genesis block
    index_block(&test_block, block_height)?;
    let outpoint = OutPoint {
        txid: Txid::from_byte_array(
            <Vec<u8> as AsRef<[u8]>>::as_ref(
                &hex::decode(genesis::GENESIS_OUTPOINT)?
                    .iter()
                    .cloned()
                    .rev()
                    .collect::<Vec<u8>>(),
            )
            .try_into()?,
        ),
        vout: 0,
    };
    // Check final balances
    let ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(&outpoint)?);
    let sheet = load_sheet(&ptr);

    println!("Balances at end: {:?}", sheet);
    let genesis_id = ProtoruneRuneId { block: 2, tx: 0 };
    assert_eq!(
        sheet.get(&genesis_id),
        50_000_000u128 * (genesis::GENESIS_BLOCK as u128)
    );
    let out = protorune_outpoint_to_outpoint_response(&outpoint, 1)?;
    let out_sheet: BalanceSheet<IndexPointer> = out.into();
    assert_eq!(sheet, out_sheet);

    // make sure premine is spendable
    let mut spend_block = create_block_with_coinbase_tx(block_height);
    let spend_tx = create_protostone_encoded_tx(
        outpoint.clone(),
        vec![Protostone {
            burn: None,
            edicts: vec![],
            pointer: Some(0),
            refund: None,
            from: None,
            protocol_tag: 1,
            message: vec![],
        }],
    );
    spend_block.txdata.push(spend_tx.clone());
    index_block(&spend_block, 880_001)?;
    let new_outpoint = OutPoint {
        txid: spend_tx.compute_txid(),
        vout: 0,
    };
    let new_ptr = RuneTable::for_protocol(AlkaneMessageContext::protocol_tag())
        .OUTPOINT_TO_RUNES
        .select(&consensus_encode(&new_outpoint)?);
    let new_sheet = load_sheet(&new_ptr);

    let genesis_id = ProtoruneRuneId { block: 2, tx: 0 };
    assert_eq!(
        new_sheet.get(&genesis_id),
        50_000_000u128 * (genesis::GENESIS_BLOCK as u128)
    );
    Ok(())
}
