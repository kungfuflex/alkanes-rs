use alkanes_integ_tests::block_builder::{create_block_with_deploys, DeployPair};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::harness::FullStackHarness;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use prost::Message;

/// Uses protorune test helper blocks for genesis (same as TestRuntime) but stores in RocksDB.
/// This isolates whether the issue is genesis block format or storage backend.
#[test]
fn deploy_with_protorune_genesis() -> anyhow::Result<()> {
    let _ = env_logger::try_init();

    // Use TestRuntime (protorune genesis blocks + HashMap storage) as baseline
    let runtime = alkanes_integ_tests::runtime::TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    let block = create_block_with_deploys(4, vec![DeployPair::new(
        fixtures::AUTH_TOKEN.to_vec(),
        Cellpack { target: AlkaneId { block: 3, tx: 65517 }, inputs: vec![100] },
    )]);
    runtime.index_block(&block, 4)?;

    // Check getbytecode first
    let mut bc_req = alkanes_support::proto::alkanes::BytecodeRequest::default();
    bc_req.id = Some(alkanes_support::proto::alkanes::AlkaneId {
        block: Some(alkanes_support::proto::alkanes::Uint128 { lo: 4, hi: 0 }),
        tx: Some(alkanes_support::proto::alkanes::Uint128 { lo: 65517, hi: 0 }),
    });
    let mut bc_buf = Vec::new();
    bc_req.encode(&mut bc_buf).unwrap();
    match runtime.alkanes_view("getbytecode", &bc_buf, 4) {
        Ok(data) => println!("getbytecode: {} bytes", data.len()),
        Err(e) => println!("getbytecode failed: {}", e),
    }

    // Call trace on the deploy tx output (vout=1 for the OP_RETURN)
    let deploy_txid = block.txdata[1].compute_txid();
    let txid_bytes: Vec<u8> = AsRef::<[u8]>::as_ref(&deploy_txid).to_vec();
    let mut trace_req = protorune_support::proto::protorune::Outpoint::default();
    trace_req.txid = txid_bytes;
    trace_req.vout = 1;
    let mut trace_buf = Vec::new();
    prost::Message::encode(&trace_req, &mut trace_buf).unwrap();
    match runtime.alkanes_view("trace", &trace_buf, 4) {
        Ok(data) => {
            println!("trace response: {} bytes", data.len());
            if let Ok(resp) = alkanes_support::proto::alkanes::AlkanesTrace::decode(data.as_slice()) {
                println!("trace: {} events", resp.events.len());
                for (i, evt) in resp.events.iter().enumerate() {
                    println!("  event[{}]: {:?}", i, evt);
                }
            } else {
                println!("Raw trace: {}", hex::encode(&data[..std::cmp::min(data.len(), 200)]));
            }
        }
        Err(e) => println!("trace failed: {}", e),
    }

    // Also try vout=0
    let mut trace_req2 = protorune_support::proto::protorune::Outpoint::default();
    trace_req2.txid = AsRef::<[u8]>::as_ref(&deploy_txid).to_vec();
    trace_req2.vout = 0;
    let mut trace_buf2 = Vec::new();
    prost::Message::encode(&trace_req2, &mut trace_buf2).unwrap();
    match runtime.alkanes_view("trace", &trace_buf2, 4) {
        Ok(data) => {
            println!("trace(vout=0): {} bytes", data.len());
            if let Ok(resp) = alkanes_support::proto::alkanes::AlkanesTrace::decode(data.as_slice()) {
                println!("trace(vout=0): {} events", resp.events.len());
                for (i, evt) in resp.events.iter().enumerate() {
                    println!("  event[{}]: {:?}", i, evt);
                }
            }
        }
        Err(e) => println!("trace(vout=0) failed: {}", e),
    }

    // Simulate
    let cellpack = Cellpack { target: AlkaneId { block: 4, tx: 65517 }, inputs: vec![99] };
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cellpack.encipher();
    let mut sim_buf = Vec::new();
    parcel.encode(&mut sim_buf)?;
    let result = runtime.alkanes_view("simulate", &sim_buf, 4)?;
    let resp = alkanes_support::proto::alkanes::SimulateResponse::decode(result.as_slice())?;

    println!("TestRuntime simulate error: '{}'", resp.error);
    if resp.error.is_empty() || resp.error.contains("already") {
        println!("DEPLOY WORKS!");
    } else {
        println!("DEPLOY FAILED: {}", resp.error);
    }
    Ok(())
}

/// Uses FullStackHarness (TestChain genesis blocks + RocksDB) to test deploy.
#[test]
fn deploy_with_qblock_genesis() -> anyhow::Result<()> {
    let _ = env_logger::try_init();
    let mut harness = FullStackHarness::new()?;

    // Use mine_empty_blocks (QBlock format) like cli_factory_init does
    harness.mine_empty_blocks(4)?;
    let height = harness.height() as u32 + 1;

    println!("Deploying at height {}", height);

    let block = create_block_with_deploys(height, vec![DeployPair::new(
        fixtures::AUTH_TOKEN.to_vec(),
        Cellpack { target: AlkaneId { block: 3, tx: 65517 }, inputs: vec![100] },
    )]);
    harness.index_bitcoin_block(&block, height)?;

    let cellpack = Cellpack { target: AlkaneId { block: 4, tx: 65517 }, inputs: vec![99] };
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cellpack.encipher();
    let mut sim_buf = Vec::new();
    parcel.encode(&mut sim_buf)?;
    let result = harness.alkanes_view("simulate", &sim_buf)?;
    let resp = alkanes_support::proto::alkanes::SimulateResponse::decode(result.as_slice())?;

    println!("FullStackHarness simulate error: '{}'", resp.error);

    assert!(resp.error.is_empty() || resp.error.contains("already"),
        "Should work: {}", resp.error);
    Ok(())
}

#[test]
fn can_simulate_diesel() -> anyhow::Result<()> {
    let mut harness = FullStackHarness::new()?;
    harness.mine_empty_blocks(4)?;
    
    // DIESEL is the genesis alkane at 2:0. If we can simulate it, the alkanes index works.
    let cellpack = alkanes_support::cellpack::Cellpack { 
        target: alkanes_support::id::AlkaneId { block: 2, tx: 0 }, 
        inputs: vec![99] // get_name
    };
    let mut parcel = alkanes_support::proto::alkanes::MessageContextParcel::default();
    parcel.calldata = cellpack.encipher();
    let mut sim_buf = Vec::new();
    prost::Message::encode(&parcel, &mut sim_buf)?;
    
    let result = harness.alkanes_view("simulate", &sim_buf)?;
    let resp = alkanes_support::proto::alkanes::SimulateResponse::decode(result.as_slice())?;
    
    println!("DIESEL simulate: error='{}' gas={}", resp.error, resp.gas_used);
    if let Some(exec) = &resp.execution {
        let name = String::from_utf8_lossy(&exec.data);
        println!("  Data: '{}'", name);
    }
    
    assert!(resp.error.is_empty(), "DIESEL should be queryable: {}", resp.error);
    Ok(())
}
