use alkanes_support::cellpack::Cellpack;
use alkanes_support::envelope::RawEnvelope;
use alkanes_support::id::AlkaneId;
use bitcoin::{
    transaction::Version, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness,
};
use metashrew_support::index_pointer::KeyValuePointer;
use ordinals::Runestone;
use protorune::test_helpers::{create_block_with_coinbase_tx, get_address, ADDRESS1};
use protorune::protostone::Protostones;
use protorune_support::network::{set_network, NetworkParams};
use protorune_support::protostone::Protostone;

#[test]
fn native_deploy_auth_token() {
    set_network(NetworkParams {
        bech32_prefix: "bcrt".into(),
        p2pkh_prefix: 0x64,
        p2sh_prefix: 0xc4,
    });

    // Genesis
    let genesis = create_block_with_coinbase_tx(0);
    alkanes::indexer::index_block(&genesis, 0).expect("genesis");

    for h in 1..=4 {
        let block = create_block_with_coinbase_tx(h);
        alkanes::indexer::index_block(&block, h).expect(&format!("block {h}"));
    }

    // Deploy
    let cellpack = Cellpack {
        target: AlkaneId { block: 3, tx: 65517 },
        inputs: vec![100],
    };
    let envelope = RawEnvelope::from(alkanes_integ_tests::fixtures::AUTH_TOKEN.to_vec());
    let witness = envelope.to_witness(true);

    let protostone = Protostone {
        message: cellpack.encipher(),
        pointer: Some(0),
        refund: Some(0),
        edicts: vec![],
        from: None,
        burn: None,
        protocol_tag: 1,
    };
    let runestone_script = (Runestone {
        edicts: vec![],
        etching: None,
        mint: None,
        pointer: Some(0),
        protocol: vec![protostone].encipher().ok(),
    }).encipher();

    let mut block = create_block_with_coinbase_tx(5);
    block.txdata.push(Transaction {
        version: Version::ONE,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness,
        }],
        output: vec![
            TxOut {
                value: Amount::from_sat(100_000_000),
                script_pubkey: get_address(&ADDRESS1().as_str()).script_pubkey(),
            },
            TxOut {
                value: Amount::from_sat(0),
                script_pubkey: runestone_script,
            },
        ],
    });
    alkanes::indexer::index_block(&block, 5).expect("deploy block");

    // Check bytecode via IndexPointer
    let ptr = metashrew_core::index_pointer::IndexPointer::from_keyword("/alkanes/")
        .select(&<AlkaneId as Into<Vec<u8>>>::into(AlkaneId { block: 4, tx: 65517 }));
    let bytecode = ptr.get();
    println!("Bytecode: {} bytes", bytecode.len());
    assert!(bytecode.len() > 0, "AUTH_TOKEN should be deployed natively");
}
