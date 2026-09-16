//! Port of edict_then_message.rs and forge.rs — edict routing and forgery prevention.

use alkanes_integ_tests::block_builder::{create_block_with_deploys, create_block_with_protostones, last_tx_outpoint, DeployPair};
use alkanes_integ_tests::fixtures;
use alkanes_integ_tests::query;
use alkanes_integ_tests::runtime::TestRuntime;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use anyhow::Result;
use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, Witness};
use protorune_support::balance_sheet::ProtoruneRuneId;
use protorune_support::protostone::{Protostone, ProtostoneEdict};

fn txin_from(outpoint: OutPoint) -> TxIn {
    TxIn {
        previous_output: outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    }
}

/// Test that edicts cannot be forged — minting tokens from thin air via raw edicts.
#[test]
fn test_cant_forge_edicts() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Construct a raw protostone with a forged edict — tries to create
    // 100000 tokens of alkane 2:100 (which doesn't exist)
    let block = create_block_with_protostones(
        4,
        vec![txin_from(OutPoint::null())],
        vec![],
        vec![Protostone {
            protocol_tag: 1,
            from: None,
            edicts: vec![ProtostoneEdict {
                id: ProtoruneRuneId {
                    block: 2,
                    tx: 100,
                },
                amount: 100000,
                output: 0,
            }],
            pointer: Some(0),
            refund: Some(0),
            message: vec![],
            burn: None,
        }],
    );
    runtime.index_block(&block, 4)?;

    // The forged edict should produce 0 balance — you can't create tokens from nothing
    let outpoint = last_tx_outpoint(&block);
    let bal = query::get_alkane_balance(&runtime, &outpoint, 2, 100, 4)?;
    println!("forge test: 2:100 balance = {} (should be 0)", bal);
    assert_eq!(bal, 0, "forged edicts should not create tokens");
    Ok(())
}

/// Test edict + message in same protostone: edict routes 1 token, message calls contract.
#[test]
fn test_edict_message_same_protostone() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Block 4: deploy and mint 1 token
    let deploy_block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![30, 2, 1, 1],
            },
        )],
    );
    runtime.index_block(&deploy_block, 4)?;

    // Block 5: edict routes token from input + message calls contract
    let prev_outpoint = last_tx_outpoint(&deploy_block);
    let block5 = create_block_with_protostones(
        5,
        vec![txin_from(prev_outpoint)],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![5],
            }
            .encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![ProtostoneEdict {
                id: ProtoruneRuneId { block: 2, tx: 1 },
                amount: 1,
                output: 0,
            }],
        }],
    );
    runtime.index_block(&block5, 5)?;

    let outpoint = last_tx_outpoint(&block5);
    let bal = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 5)?;
    println!("edict+message: 2:1 balance = {} (should be 1)", bal);
    assert_eq!(bal, 1, "1 token should arrive at output via edict");
    Ok(())
}

/// Test edict + message revert: edict routes token but message reverts, token refunded.
#[test]
fn test_edict_message_same_protostone_revert() -> Result<()> {
    let _ = env_logger::try_init();
    let runtime = TestRuntime::new()?;
    runtime.mine_empty_blocks(0, 4)?;

    // Block 4: deploy and mint 1 token
    let deploy_block = create_block_with_deploys(
        4,
        vec![DeployPair::new(
            fixtures::TEST_CONTRACT,
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![30, 2, 1, 1],
            },
        )],
    );
    runtime.index_block(&deploy_block, 4)?;

    // Block 5: edict routes token but message reverts (opcode 100 = revert)
    let prev_outpoint = last_tx_outpoint(&deploy_block);
    let block5 = create_block_with_protostones(
        5,
        vec![txin_from(prev_outpoint)],
        vec![],
        vec![Protostone {
            message: Cellpack {
                target: AlkaneId { block: 2, tx: 1 },
                inputs: vec![100], // revert
            }
            .encipher(),
            protocol_tag: 1,
            burn: None,
            from: None,
            pointer: Some(0),
            refund: Some(0),
            edicts: vec![ProtostoneEdict {
                id: ProtoruneRuneId { block: 2, tx: 1 },
                amount: 1,
                output: 0,
            }],
        }],
    );
    runtime.index_block(&block5, 5)?;

    // Token should be refunded to output 0 (refund pointer)
    let outpoint = last_tx_outpoint(&block5);
    let bal = query::get_alkane_balance(&runtime, &outpoint, 2, 1, 5)?;
    println!("edict+revert: 2:1 balance = {} (should be 1, refunded)", bal);
    assert_eq!(bal, 1, "token should be refunded after revert");
    Ok(())
}
