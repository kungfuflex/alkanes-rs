# Swap with BTC Wrap/Unwrap

## Overview

The `alkanes swap` command now supports using `B` as a token identifier in the path to automatically wrap/unwrap BTC ↔ frBTC.

## Usage

### Wrap BTC → Swap → Token
```bash
alkanes swap --path B,2:0,32:0 --input 10000000
```
- Wraps 10M sats of BTC → frBTC
- Swaps frBTC through path 2:0 → 32:0
- Outputs final token

### Token → Swap → Unwrap BTC
```bash
alkanes swap --path 32:0,2:0,B --input 50000000
```
- Swaps input token through path 32:0 → 2:0 → frBTC  
- Unwraps frBTC → BTC
- Outputs BTC to recipient address

### Wrap → Swap → Unwrap
```bash
alkanes swap --path B,2:0,32:0,B --input 10000000
```
- Wraps BTC → frBTC
- Swaps through 2:0 → 32:0
- Unwraps back to BTC

## Implementation

### Wrap (BTC → frBTC)

**Protostone #1: Wrap**
- Cellpack: Call frBTC (32:0) opcode 77
- Bitcoin transfer: Send BTC to subfrost address (output 0)
- Pointer: Points to output where frBTC should be minted
- Refund pointer: Same as pointer

**Protostone #2: Swap**
- Consumes frBTC from wrap protostone output
- Executes swap through path
- Outputs result tokens

### Unwrap (frBTC → BTC)

**Protostone #1: Swap**
- Executes swap through path
- Outputs frBTC via pointer to unwrap protostone's virtual vout

**Protostone #2: Unwrap**
- Receives frBTC from swap protostone
- Calldata: `[32, 0, 78, dustOutputIndex, unwrapAmount]`
  - 32:0 = frBTC alkane
  - 78 = UNWRAP opcode
  - dustOutputIndex = index of dust output to subfrost
  - unwrapAmount = amount of frBTC to unwrap
- Pointer: Output that receives unwrapped BTC
- Refund pointer: Output for failed unwrap (receives frBTC back)
- Cellpack: Must include the dust output to subfrost as an input

**Transaction Structure:**
- Output 0: Recipient address for alkanes (546 sats)
- Output 1: BTC recipient address (546 sats) ← pointer for unwrap
- Output 2: Subfrost address dust (546 sats) ← used in cellpack
- Output 3: OP_RETURN with protostones
- Output 4+: Change outputs

## Constants

```rust
pub const FRBTC_BLOCK: u64 = 32;
pub const FRBTC_TX: u64 = 0;
pub const WRAP_OPCODE: u128 = 77;
pub const UNWRAP_OPCODE: u128 = 78;
```

## Error Handling

- `B` can only appear at start or end of path, not in middle
- Minimum 2 tokens required in path
- If unwrap fails (insufficient frBTC), tokens are refunded to refund pointer
