# Protostone Pointer and Refund Specification

## Overview

This document describes how to specify pointer and refund targets for protostones in the `alkanes execute` command.

## Syntax

```bash
alkanes execute [cellpack,vector,u128s]:pointer:refund[:edicts...]
```

### Pointer and Refund Formats

- **`v{N}`**: Physical output N (e.g., `v0`, `v1`, `v2`)
- **`p{N}`**: Shadow protostone output N (e.g., `p0`, `p1`, `p2`)

### Examples

#### 1. Basic Deployment - Physical Output
```bash
[cellpack,vector,u128s]:v0:v0
```
- Pointer: v0 (physical output 0)
- Refund: v0 (physical output 0)

#### 2. Chaining Protostones - Shadow Outputs
```bash
[first,cellpack]:p0:v0 [second,cellpack]:v1:v1
```
- First protostone:
  - Pointer: p0 (shadow output = second protostone)
  - Refund: v0 (physical output 0)
- Second protostone:
  - Pointer: v1 (physical output 1)
  - Refund: v1 (physical output 1)

#### 3. Multiple Shadow References
```bash
[a]:p1:v0 [b]:p2:v0 [c]:v0:v0
```
- Protostone 0: Points to protostone 1 (p1)
- Protostone 1: Points to protostone 2 (p2)
- Protostone 2: Points to physical output 0 (v0)

## Implementation Details

### Physical Outputs (`v{N}`)

Physical outputs are actual transaction outputs that appear on-chain.

**Mapping**: `v{N}` → output index `N`

Example with 3 outputs:
- `v0` → output 0
- `v1` → output 1  
- `v2` → output 2

### Shadow Outputs (`p{N}`)

Shadow outputs are virtual outputs representing protostone results. They allow chaining protostones together.

**Mapping**: `p{N}` → output index `num_physical_outputs + N`

Example with 3 physical outputs and 2 protostones:
- `p0` → output 3 (first protostone's shadow output)
- `p1` → output 4 (second protostone's shadow output)

### Calculation Formula

```rust
match target {
    OutputTarget::Output(n) => n,  // Physical: use directly
    OutputTarget::Protostone(n) => num_physical_outputs + n,  // Shadow: offset by physical count
}
```

## Code Structure

### Data Structures

**`ProtostoneSpec`** (`types.rs`):
```rust
pub struct ProtostoneSpec {
    pub cellpack: Option<Cellpack>,
    pub edicts: Vec<ProtostoneEdict>,
    pub bitcoin_transfer: Option<BitcoinTransfer>,
    pub pointer: Option<OutputTarget>,  // NEW
    pub refund: Option<OutputTarget>,   // NEW
}
```

**`OutputTarget`** (`types.rs`):
```rust
pub enum OutputTarget {
    Output(u32),      // v{N} - Physical output
    Protostone(u32),  // p{N} - Shadow protostone output
    Split,            // Special case for splitting
}
```

### Parsing (`parsing.rs`)

The parser extracts pointer and refund from the colon-separated format:

```rust
// Input: "[cellpack]:v0:v1"
// Parts: ["[cellpack]", "v0", "v1"]

fn parse_single_protostone(spec_str: &str) -> Result<ProtostoneSpec> {
    // Split by ':' respecting brackets
    // Parse cellpack from first part
    // Parse pointer from second part (v0)
    // Parse refund from third part (v1)
    // ...
}
```

### Conversion (`execute.rs`)

The converter calculates actual output indices:

```rust
fn convert_protostone_specs_with_output_count(
    &self, 
    specs: &[ProtostoneSpec], 
    num_physical_outputs: u32
) -> Result<Vec<Protostone>> {
    specs.iter().enumerate().map(|(i, spec)| {
        // Convert pointer
        let pointer = match &spec.pointer {
            Some(OutputTarget::Output(v)) => Some(*v),
            Some(OutputTarget::Protostone(p)) => Some(num_physical_outputs + p),
            None => Some(0),  // Default
        };
        
        // Convert refund (same logic)
        // ...
    })
}
```

## Use Cases

### 1. Simple Contract Call
```bash
alkanes execute [4,7936,0,32,0]:v0:v0
```
- Calls contract at [4, 7936]
- Result goes to output 0
- Refunds go to output 0

### 2. Token Mint and Transfer
```bash
alkanes execute [mint,cellpack]:p0:v0 [transfer,cellpack]:v1:v1
```
- First protostone: Mint tokens, send to second protostone (p0)
- Second protostone: Transfer tokens, send to output 1 (v1)

### 3. Complex DeFi Operation
```bash
alkanes execute [wrap]:p0:v0 [stake]:v1:v1
```
- First protostone: Wrap BTC to frBTC, send to second protostone
- Second protostone: Stake frBTC in vault, result to output 1

## Validation

The executor validates pointer and refund targets:

```rust
pub fn validate_protostones(&self, protostones: &[ProtostoneSpec], num_outputs: usize) -> Result<()> {
    for (i, protostone) in protostones.iter().enumerate() {
        // Validate edicts can only point forward to later protostones
        for edict in &protostone.edicts {
            if let OutputTarget::Protostone(p) = edict.target {
                if p <= i as u32 {
                    return Err("Protostone {i} refers to protostone {p} which is not allowed");
                }
            }
        }
        
        // Validate pointer and refund are within bounds
        // (num_physical_outputs + num_protostones)
    }
}
```

## Default Behavior

If pointer or refund are not specified:
- **Pointer**: Defaults to `Some(0)` (output 0)
- **Refund**: Defaults to `Some(0)` (output 0)

This ensures compatibility with the alkanes indexer which requires both fields.

## Examples from Code

### Wrap BTC Example (`wrap_btc.rs`)

```rust
let first_protostone = ProtostoneSpec {
    cellpack: Some(Cellpack { /* frBTC wrap call */ }),
    pointer: Some(OutputTarget::Protostone(0)),  // Send to second protostone
    refund: Some(OutputTarget::Output(0)),
    // ...
};

let second_protostone = ProtostoneSpec {
    cellpack: Some(Cellpack { /* vault lock call */ }),
    pointer: Some(OutputTarget::Output(0)),  // Send to physical output
    refund: Some(OutputTarget::Output(0)),
    // ...
};
```

This creates a chain: frBTC wrap → vault lock → output 0

## Common Patterns

### Pattern 1: Single Operation
```
[calldata]:v0:v0
```
Result and refunds both go to output 0.

### Pattern 2: Linear Chain
```
[op1]:p0:v0 [op2]:p1:v0 [op3]:v0:v0
```
op1 → op2 → op3 → output 0

### Pattern 3: Fan-out
```
[op1]:v0:v0 [op2]:v1:v1
```
Two independent operations to different outputs.

### Pattern 4: Gather
```
[op1]:p2:v0 [op2]:p2:v0 [op3]:v0:v0
```
op1 and op2 both send to op3, which sends to output 0.

## Testing

```bash
# Deploy a contract with explicit pointer/refund
./target/release/alkanes-cli \
  -p regtest \
  alkanes execute \
  --protostones "[3,7936,0,32,0,4,7937,4,7970,4,7956]:v0:v0" \
  --envelope-file wasm/mycontract.wasm \
  --to bcrt1q... \
  --mine

# Deploy with shadow outputs (chaining)
./target/release/alkanes-cli \
  -p regtest \
  alkanes execute \
  --protostones "[first]:p0:v0,[second]:v0:v0" \
  --to bcrt1q... \
  --mine
```

## Status

✅ **Implementation Complete**
✅ **Parsing**: Correctly extracts v{N} and p{N} from command line
✅ **Conversion**: Correctly calculates shadow output indices
✅ **Validation**: Ensures forward references only
✅ **Defaults**: Sensible defaults (v0:v0) when not specified
✅ **Testing**: Deployments now work with proper pointers!

## Files Modified

1. `/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/types.rs` - Added pointer/refund fields
2. `/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/parsing.rs` - Store parsed values
3. `/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/execute.rs` - Implement conversion logic
4. `/data/alkanes-rs/crates/alkanes-cli-common/src/alkanes/wrap_btc.rs` - Update wrap_btc example

The pointer/refund system is now fully functional! 🎉
