// Quick test of flexible protostone parsing
use alkanes_cli_common::alkanes::parsing::parse_protostone_specs;
use alkanes_cli_common::alkanes::types::{OutputTarget, ProtostoneSpec};

fn main() {
    println!("Testing flexible protostone parsing...\n");
    
    // Test 1: Standard order
    println!("Test 1: Standard order [3,100]:v0:v1:[2:1:100:v0]");
    match parse_protostone_specs("[3,100]:v0:v1:[2:1:100:v0]") {
        Ok(specs) => {
            println!("  ✅ Parsed successfully");
            println!("  Pointer: {:?}", specs[0].pointer);
            println!("  Refund: {:?}", specs[0].refund);
            println!("  Has cellpack: {}", specs[0].cellpack.is_some());
            println!("  Edicts count: {}", specs[0].edicts.len());
        }
        Err(e) => println!("  ❌ Error: {}", e),
    }
    
    // Test 2: Swapped order
    println!("\nTest 2: Swapped order [2:1:100:v0]:v0:v1:[3,100]");
    match parse_protostone_specs("[2:1:100:v0]:v0:v1:[3,100]") {
        Ok(specs) => {
            println!("  ✅ Parsed successfully");
            println!("  Pointer: {:?}", specs[0].pointer);
            println!("  Refund: {:?}", specs[0].refund);
            println!("  Has cellpack: {}", specs[0].cellpack.is_some());
            println!("  Edicts count: {}", specs[0].edicts.len());
        }
        Err(e) => println!("  ❌ Error: {}", e),
    }
    
    // Test 3: Pointer first
    println!("\nTest 3: Pointer first v0:v1:[3,100]:[2:1:100:v0]");
    match parse_protostone_specs("v0:v1:[3,100]:[2:1:100:v0]") {
        Ok(specs) => {
            println!("  ✅ Parsed successfully");
            println!("  Pointer: {:?}", specs[0].pointer);
            println!("  Refund: {:?}", specs[0].refund);
            println!("  Has cellpack: {}", specs[0].cellpack.is_some());
            println!("  Edicts count: {}", specs[0].edicts.len());
        }
        Err(e) => println!("  ❌ Error: {}", e),
    }
    
    // Test 4: Omit refund (defaults to pointer)
    println!("\nTest 4: Omit refund [3,100]:v0:[2:1:100:v0]");
    match parse_protostone_specs("[3,100]:v0:[2:1:100:v0]") {
        Ok(specs) => {
            println!("  ✅ Parsed successfully");
            println!("  Pointer: {:?}", specs[0].pointer);
            println!("  Refund: {:?}", specs[0].refund);
            assert_eq!(specs[0].pointer, specs[0].refund, "Refund should equal pointer");
            println!("  ✅ Refund correctly defaults to pointer");
        }
        Err(e) => println!("  ❌ Error: {}", e),
    }
    
    // Test 5: Omit both (defaults to v0)
    println!("\nTest 5: Omit both pointer and refund [3,100]:[2:1:100:v0]");
    match parse_protostone_specs("[3,100]:[2:1:100:v0]") {
        Ok(specs) => {
            println!("  ✅ Parsed successfully");
            println!("  Pointer: {:?}", specs[0].pointer);
            println!("  Refund: {:?}", specs[0].refund);
            assert_eq!(specs[0].pointer, Some(OutputTarget::Output(0)), "Pointer should default to v0");
            assert_eq!(specs[0].refund, Some(OutputTarget::Output(0)), "Refund should default to v0");
            println!("  ✅ Both correctly default to v0");
        }
        Err(e) => println!("  ❌ Error: {}", e),
    }
    
    // Test 6: Only cellpack (all defaults)
    println!("\nTest 6: Only cellpack [3,100]");
    match parse_protostone_specs("[3,100]") {
        Ok(specs) => {
            println!("  ✅ Parsed successfully");
            println!("  Pointer: {:?}", specs[0].pointer);
            println!("  Refund: {:?}", specs[0].refund);
            assert_eq!(specs[0].pointer, Some(OutputTarget::Output(0)), "Pointer should default to v0");
            assert_eq!(specs[0].refund, Some(OutputTarget::Output(0)), "Refund should default to v0");
            println!("  ✅ Defaults work correctly");
        }
        Err(e) => println!("  ❌ Error: {}", e),
    }
    
    // Test 7: Multiple edicts in any order
    println!("\nTest 7: Multiple edicts [2:1:50:v0]:[2:1:50:v1]:[3,100]:v0");
    match parse_protostone_specs("[2:1:50:v0]:[2:1:50:v1]:[3,100]:v0") {
        Ok(specs) => {
            println!("  ✅ Parsed successfully");
            println!("  Pointer: {:?}", specs[0].pointer);
            println!("  Refund: {:?}", specs[0].refund);
            println!("  Has cellpack: {}", specs[0].cellpack.is_some());
            println!("  Edicts count: {}", specs[0].edicts.len());
            assert_eq!(specs[0].edicts.len(), 2, "Should have 2 edicts");
            println!("  ✅ Multiple edicts parsed correctly");
        }
        Err(e) => println!("  ❌ Error: {}", e),
    }
    
    println!("\n🎉 All parsing tests completed!");
}
