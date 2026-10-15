//! `getalias` view: the factory alias stored at `/alkanes/<id>`.
//!
//! Covers the three shapes the view has to tell apart:
//! - a clone made by a top-level `[5, n]` cellpack (2:n template);
//! - a clone made INSIDE a contract through an extcall to `[6, n]`, the way
//!   FIRE staking (2:77631) cloned 2:103199 from template 4:520. The trace
//!   of that call never carries the `[6, n]` target, so this view is the
//!   only place the template can be read back from;
//! - alkanes that are not clones (own code, a precompiled built-in, an id
//!   with nothing deployed), which must return an empty response.

use crate::index_block;
use crate::tests::helpers::{self as alkane_helpers, assert_id_points_to_alkane_id};
use crate::tests::std::alkanes_std_test_build;
use alkane_helpers::clear;
use alkanes::view;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::id::AlkaneId;
use alkanes_support::proto::alkanes::{AlkaneAliasRequest, AlkaneAliasResponse};
use anyhow::Result;
#[allow(unused_imports)]
use metashrew_core::{
    println,
    stdio::{stdout, Write},
};
use prost::Message;
use wasm_bindgen_test::wasm_bindgen_test;

/// The reserved template the extcall clones, like FIRE's 4:520.
const TEMPLATE_TX: u128 = 520;

fn getalias(id: AlkaneId, height: u32) -> Result<Vec<u8>> {
    let request = AlkaneAliasRequest {
        id: Some(id.into()),
    };
    view::getalias(&request.encode_to_vec(), height)
}

fn alias_of(id: AlkaneId, height: u32) -> Result<Option<AlkaneId>> {
    let bytes = getalias(id, height)?;
    let response = AlkaneAliasResponse::decode(bytes.as_slice())?;
    Ok(response.alias.map(Into::into))
}

#[wasm_bindgen_test]
fn test_getalias_top_level_extcall_and_non_clones() -> Result<()> {
    clear();
    let block_height = 0;

    // tx0: reserve the template at 4:520 ([3, 520] + binary).
    let reserve_template = Cellpack {
        target: AlkaneId {
            block: 3,
            tx: TEMPLATE_TX,
        },
        inputs: vec![50],
    };
    // tx1: deploy a contract with its own code at 2:1.
    let deploy_caller = Cellpack {
        target: AlkaneId { block: 1, tx: 0 },
        inputs: vec![50],
    };
    // tx2: 2:1 extcalls [6, 520] (opcode 31 = TestExtCall { target, inputs }),
    // which clones the 4:520 template to 2:2 from inside the contract.
    let extcall_clone = Cellpack {
        target: AlkaneId { block: 2, tx: 1 },
        inputs: vec![31, 6, TEMPLATE_TX, 1, 50],
    };
    // tx3: a top-level [5, 1] clone of 2:1 -> 2:3.
    let top_level_clone = Cellpack {
        target: AlkaneId { block: 5, tx: 1 },
        inputs: vec![50],
    };
    // tx4: a clone of the clone, [5, 2] -> 2:4 (alias 2:2, one hop only).
    let clone_of_clone = Cellpack {
        target: AlkaneId { block: 5, tx: 2 },
        inputs: vec![50],
    };

    let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [
            alkanes_std_test_build::get_bytes(),
            alkanes_std_test_build::get_bytes(),
            [].into(),
            [].into(),
            [].into(),
        ]
        .into(),
        [
            reserve_template,
            deploy_caller,
            extcall_clone,
            top_level_clone,
            clone_of_clone,
        ]
        .into(),
    );
    index_block(&test_block, block_height)?;

    let template = AlkaneId {
        block: 4,
        tx: TEMPLATE_TX,
    };
    let caller = AlkaneId { block: 2, tx: 1 };
    let extcall_child = AlkaneId { block: 2, tx: 2 };
    let top_level_child = AlkaneId { block: 2, tx: 3 };
    let grandchild = AlkaneId { block: 2, tx: 4 };

    // The raw storage the view reads, checked independently of the view.
    assert_id_points_to_alkane_id(extcall_child.clone(), template.clone())?;
    assert_id_points_to_alkane_id(top_level_child.clone(), caller.clone())?;
    assert_id_points_to_alkane_id(grandchild.clone(), extcall_child.clone())?;

    // Clone made inside a contract via extcall to [6, 520].
    assert_eq!(alias_of(extcall_child.clone(), block_height)?, Some(template.clone()));
    // Top-level [5, 1] clone.
    assert_eq!(alias_of(top_level_child.clone(), block_height)?, Some(caller.clone()));
    // A chain returns the one stored hop, not the end of the chain.
    assert_eq!(alias_of(grandchild.clone(), block_height)?, Some(extcall_child.clone()));

    // Wire format: an alias is a nested AlkaneId at field 1.
    let wire = getalias(extcall_child.clone(), block_height)?;
    assert_eq!(
        wire,
        AlkaneAliasResponse {
            alias: Some(template.clone().into())
        }
        .encode_to_vec()
    );

    // Not clones: own code, reserved template, precompiled built-in, nothing.
    for id in [
        caller.clone(),
        template.clone(),
        AlkaneId { block: 2, tx: 0 },
        AlkaneId { block: 2, tx: 999 },
        AlkaneId { block: 4, tx: 521 },
    ] {
        assert!(
            getalias(id.clone(), block_height)?.is_empty(),
            "{:?} has no alias, expected an empty response",
            id
        );
        assert_eq!(alias_of(id, block_height)?, None);
    }

    // A request without an id is an error (the wasm export turns it into an
    // empty response, as getbytecode does).
    assert!(view::getalias(&AlkaneAliasRequest { id: None }.encode_to_vec(), block_height).is_err());
    Ok(())
}
