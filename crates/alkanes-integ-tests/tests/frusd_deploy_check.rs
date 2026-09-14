use alkanes_integ_tests::block_builder::{create_block_with_deploys, DeployPair};
use alkanes_integ_tests::harness::FullStackHarness;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use prost::Message;

const FRUSD_AUTH_WASM: &[u8] = include_bytes!("../test_data/frusd_auth_token.wasm");

#[test]
fn deploy_frusd_auth_token_in_alkanes_integ() -> anyhow::Result<()> {
    let mut harness = FullStackHarness::new()?;
    harness.mine_empty_blocks(4)?;
    let height = harness.height() as u32 + 1;

    let block = create_block_with_deploys(
        height,
        vec![DeployPair::new(
            FRUSD_AUTH_WASM.to_vec(),
            Cellpack { target: AlkaneId { block: 3, tx: 8200 }, inputs: vec![0] },
        )],
    );
    harness.index_bitcoin_block(&block, height)?;

    // Check bytecode
    let mut bc_req = alkanes_support::proto::alkanes::BytecodeRequest::default();
    bc_req.id = Some(alkanes_support::proto::alkanes::AlkaneId {
        block: Some(alkanes_support::proto::alkanes::Uint128 { lo: 4, hi: 0 }),
        tx: Some(alkanes_support::proto::alkanes::Uint128 { lo: 8200, hi: 0 }),
    });
    let mut buf = Vec::new();
    bc_req.encode(&mut buf)?;
    
    let bytecode = harness.alkanes_view("getbytecode", &buf)?;
    println!("Bytecode: {} bytes", bytecode.len());
    assert!(bytecode.len() > 100, "WASM should be stored");
    
    Ok(())
}
