use hex_lit::hex;

use crate::tests::test_runtime::TestRuntime;
use metashrew_support::environment::RuntimeEnvironment;

use prost::Message;
use protorune_support::proto::protorune::ProtorunesWalletRequest;


#[test]
fn test_decode() {

	let env = TestRuntime::default();
    env.log(&format!("{:?}", ProtorunesWalletRequest::decode(&hex!("0a406263727431703335687775396a306132377a637a6c6468337a36686e796b637972386a3577766837307a706c796a68616e377a647036763577736a6a75716430")[..]).unwrap()));
}