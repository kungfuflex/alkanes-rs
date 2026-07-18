// Integration test for flexible protostone parsing
use alkanes_cli_common::alkanes::parsing::parse_protostones;
use alkanes_cli_common::alkanes::types::OutputTarget;

#[test]
fn test_standard_order() {
    let result = parse_protostones("[3,100]:v0:v1:[2:1:100:v0]");
    assert!(result.is_ok(), "Should parse standard order");
    let specs = result.unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].pointer, Some(OutputTarget::Output(0)));
    assert_eq!(specs[0].refund, Some(OutputTarget::Output(1)));
    assert!(specs[0].cellpack.is_some());
    assert_eq!(specs[0].edicts.len(), 1);
}

#[test]
fn test_swapped_order() {
    let result = parse_protostones("[2:1:100:v0]:v0:v1:[3,100]");
    assert!(result.is_ok(), "Should parse swapped order");
    let specs = result.unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].pointer, Some(OutputTarget::Output(0)));
    assert_eq!(specs[0].refund, Some(OutputTarget::Output(1)));
    assert!(specs[0].cellpack.is_some());
    assert_eq!(specs[0].edicts.len(), 1);
}

#[test]
fn test_pointer_first() {
    let result = parse_protostones("v0:v1:[3,100]:[2:1:100:v0]");
    assert!(result.is_ok(), "Should parse with pointer first");
    let specs = result.unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].pointer, Some(OutputTarget::Output(0)));
    assert_eq!(specs[0].refund, Some(OutputTarget::Output(1)));
    assert!(specs[0].cellpack.is_some());
    assert_eq!(specs[0].edicts.len(), 1);
}

#[test]
fn test_refund_defaults_to_pointer() {
    let result = parse_protostones("[3,100]:v0:[2:1:100:v0]");
    assert!(result.is_ok(), "Should parse with omitted refund");
    let specs = result.unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].pointer, Some(OutputTarget::Output(0)));
    assert_eq!(specs[0].refund, Some(OutputTarget::Output(0)), "Refund should default to pointer");
}

#[test]
fn test_both_default_to_v0() {
    let result = parse_protostones("[3,100]:[2:1:100:v0]");
    assert!(result.is_ok(), "Should parse with omitted pointer and refund");
    let specs = result.unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].pointer, Some(OutputTarget::Output(0)), "Pointer should default to v0");
    assert_eq!(specs[0].refund, Some(OutputTarget::Output(0)), "Refund should default to v0");
}

#[test]
fn test_only_cellpack() {
    let result = parse_protostones("[3,100]");
    assert!(result.is_ok(), "Should parse cellpack only");
    let specs = result.unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].pointer, Some(OutputTarget::Output(0)));
    assert_eq!(specs[0].refund, Some(OutputTarget::Output(0)));
    assert!(specs[0].cellpack.is_some());
}

#[test]
fn test_multiple_edicts() {
    let result = parse_protostones("[2:1:50:v0]:[2:1:50:v1]:[3,100]:v0");
    assert!(result.is_ok(), "Should parse multiple edicts");
    let specs = result.unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].edicts.len(), 2, "Should have 2 edicts");
    assert!(specs[0].cellpack.is_some());
}

#[test]
fn test_no_cellpack() {
    let result = parse_protostones("[2:1:100:v0]:v0:v0");
    assert!(result.is_ok(), "Should parse without cellpack");
    let specs = result.unwrap();
    assert_eq!(specs.len(), 1);
    assert!(specs[0].cellpack.is_none());
    assert_eq!(specs[0].edicts.len(), 1);
}

#[test]
fn test_protostone_target() {
    let result = parse_protostones("[2:1:100:p0]:v0:v0");
    assert!(result.is_ok(), "Should parse protostone target");
    let specs = result.unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].edicts[0].target, OutputTarget::Protostone(0));
}

#[test]
fn test_multiple_protostones() {
    let result = parse_protostones("[3,100]:v0:v0,[2:1:100:v0]:v1:v1");
    assert!(result.is_ok(), "Should parse multiple protostones");
    let specs = result.unwrap();
    assert_eq!(specs.len(), 2, "Should have 2 protostones");
}
