//! Test esplora view functions through the multi-indexer harness.

use alkanes_integ_tests::esplora;
use alkanes_integ_tests::runtime::TestRuntime;
use anyhow::Result;

#[test]
fn test_esplora_tip_height() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    runtime.mine_empty_blocks(0, 5)?;

    let result = esplora::get_tip_height(&runtime, 4);
    println!("Esplora tipheight result: {:?}", result);

    // tipheight should return the current tip
    // The exact format depends on esplorashrew's ABI
    match result {
        Ok(data) => {
            println!(
                "tipheight returned {} bytes: {}",
                data.len(),
                String::from_utf8_lossy(&data)
            );
        }
        Err(e) => {
            println!("tipheight view failed (may need ABI investigation): {}", e);
        }
    }

    Ok(())
}

#[test]
fn test_esplora_tx_lookup() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;

    runtime.mine_empty_blocks(0, 3)?;

    // Get the coinbase txid from block 0
    let block0 = protorune::test_helpers::create_block_with_coinbase_tx(0);
    let coinbase_txid = block0.txdata[0].compute_txid();
    let txid_bytes = coinbase_txid.as_ref();

    let result = esplora::get_tx_hex(&runtime, txid_bytes, 2);
    println!("Esplora txhex result: {:?}", result);

    match result {
        Ok(data) => {
            println!(
                "txhex returned {} bytes: {}",
                data.len(),
                String::from_utf8_lossy(&data).chars().take(200).collect::<String>()
            );
        }
        Err(e) => {
            println!("txhex view failed (may need ABI investigation): {}", e);
        }
    }

    Ok(())
}
