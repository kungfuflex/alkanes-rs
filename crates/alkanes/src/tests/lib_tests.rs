use crate::{
    configure_network,
    message::AlkaneMessageContext,
    tests::test_runtime::TestRuntime,
};
use bitcoin::{
    consensus::{Decodable, Decodable as consensus_decode},
    Block, OutPoint,
};
use prost::Message;
use protorune::{
    view::{protorune_outpoint_to_outpoint_response, runes_by_address, runes_by_height},
    Protorune,
};
use protorune_support::proto::protorune::{RunesByHeightRequest, Uint128, WalletRequest};
use std::io::Cursor;

#[test]
fn test_decode_block() {
    let block_data = include_bytes!("static/849236.txt").to_vec();

    assert!(block_data.len() > 0);

    let data = block_data;
    let height = u32::from_le_bytes((&data[0..4]).try_into().unwrap());
    let reader = &data[4..];
    let block: Block =
        consensus_decode::<Block>(&mut Cursor::<Vec<u8>>::new(reader.to_vec())).unwrap();
    assert!(height == 849236);

    // calling index_block directly fails since genesis(&block).unwrap(); gets segfault
    // index_block(&block, height).unwrap();
    configure_network();
    let mut test_runtime = TestRuntime::default();
    Protorune::index_block::<AlkaneMessageContext<TestRuntime>>(&mut test_runtime, block.clone(), height.into()).unwrap();

    let req_height: Vec<u8> = (RunesByHeightRequest {
        height: 849236,
    })
    .encode_to_vec();
    let runes = runes_by_height::<TestRuntime>(&req_height, &mut test_runtime).unwrap();
    assert!(runes.runes.len() == 2);

    // TODO: figure out what address to use for runesbyaddress
    let req_wallet: Vec<u8> = (WalletRequest {
        wallet: String::from("bc1pfs5dhzwk32xa53cjx8fx4dqy7hm4m6tys8zyvemqffz8ua4tytqs8vjdgr")
            .as_bytes()
            .to_vec(),
    })
    .encode_to_vec();

    let runes_for_addr = runes_by_address::<TestRuntime>(&req_wallet, &mut test_runtime).unwrap();
    // assert!(runes_for_addr.balances > 0);
    TestRuntime::log(format!("RUNES by addr: {:?}", runes_for_addr));

    let outpoint_res = protorune_outpoint_to_outpoint_response::<TestRuntime>(
        &(OutPoint {
            txid: block.txdata[298].compute_txid(),
            vout: 2,
        }),
        0,
        &mut test_runtime,
    )
    .unwrap();
    let quorum_rune = outpoint_res.balances.unwrap().entries[0].clone();
    let balance = quorum_rune.balance.unwrap();
    let mut expected_balance = Uint128::default();
    expected_balance.lo = 21000000;
    assert!(balance == expected_balance);
    // TODO: Assert rune
    TestRuntime::log(format!(" with rune {:?}", quorum_rune.rune.unwrap()));

    // assert!(false);
}