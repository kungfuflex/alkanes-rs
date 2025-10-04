//! Integration tests for alkanes execute command with edicts.
//!
//! This module tests the parsing of complex protostone strings that include
//! both cellpacks and edicts, ensuring that the `deezel-common` parsing
//! logic correctly interprets them.

use deezel_common::alkanes::execute::{parse_protostones, OutputTarget};

#[test]
fn test_parse_protostone_with_single_edict() {
    let input = "[2,0,77]:v0:v0:[2:0:100:v0]";
    let protostones = parse_protostones(input).unwrap();

    assert_eq!(protostones.len(), 1);
    let spec = &protostones[0];

    // Verify cellpack
    assert!(spec.cellpack.is_some());
    let cellpack = spec.cellpack.as_ref().unwrap();
    assert_eq!(cellpack.target.block, 2);
    assert_eq!(cellpack.target.tx, 0);
    assert_eq!(cellpack.inputs, vec![77]);

    // Verify edicts
    assert_eq!(spec.edicts.len(), 1);
    let edict = &spec.edicts[0];
    assert_eq!(edict.alkane_id.block, 2);
    assert_eq!(edict.alkane_id.tx, 0);
    assert_eq!(edict.amount, 100);
    assert!(matches!(edict.target, OutputTarget::Output(0)));

    // Verify no bitcoin transfer
    assert!(spec.bitcoin_transfer.is_none());
}

#[test]
fn test_parse_protostone_with_multiple_edicts() {
    let input = "[3,1,10]:v1:p2:[3:1:50:v1]:[3:1:50:v2]";
    let protostones = parse_protostones(input).unwrap();

    assert_eq!(protostones.len(), 1);
    let spec = &protostones[0];

    // Verify cellpack
    assert!(spec.cellpack.is_some());
    let cellpack = spec.cellpack.as_ref().unwrap();
    assert_eq!(cellpack.target.block, 3);
    assert_eq!(cellpack.target.tx, 1);
    assert_eq!(cellpack.inputs, vec![10]);

    // Verify edicts
    assert_eq!(spec.edicts.len(), 2);
    let edict1 = &spec.edicts[0];
    assert_eq!(edict1.alkane_id.block, 3);
    assert_eq!(edict1.alkane_id.tx, 1);
    assert_eq!(edict1.amount, 50);
    assert!(matches!(edict1.target, OutputTarget::Output(1)));

    let edict2 = &spec.edicts[1];
    assert_eq!(edict2.alkane_id.block, 3);
    assert_eq!(edict2.alkane_id.tx, 1);
    assert_eq!(edict2.amount, 50);
    assert!(matches!(edict2.target, OutputTarget::Output(2)));
}

#[test]
fn test_parse_multiple_protostones_with_edicts() {
    let input = "[2,0,77]:v0:v0:[2:0:100:v0],[3,1,10]:v1:p2:[3:1:50:v1]";
    let protostones = parse_protostones(input).unwrap();

    assert_eq!(protostones.len(), 2);

    // First protostone
    let spec1 = &protostones[0];
    assert!(spec1.cellpack.is_some());
    assert_eq!(spec1.edicts.len(), 1);
    let edict1 = &spec1.edicts[0];
    assert_eq!(edict1.alkane_id.block, 2);
    assert_eq!(edict1.amount, 100);

    // Second protostone
    let spec2 = &protostones[1];
    assert!(spec2.cellpack.is_some());
    assert_eq!(spec2.edicts.len(), 1);
    let edict2 = &spec2.edicts[0];
    assert_eq!(edict2.alkane_id.block, 3);
    assert_eq!(edict2.amount, 50);
}