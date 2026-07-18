# Swap Wrap/Unwrap Implementation Plan

## Status
- ✅ Parse "B" tokens in path
- ✅ Detect needs_wrap and needs_unwrap
- ✅ Create helper functions `build_wrap_protostone()` and `build_unwrap_protostone()`
- 🚧 Integrate into swap execution (IN PROGRESS)

## Integration Points

### 1. Fetch Subfrost Address (around line 1620)
```rust
// After final_minimum_output is calculated
let subfrost_address = if needs_wrap || needs_unwrap {
    use alkanes_cli_common::subfrost::get_subfrost_address;
    use alkanes_cli_common::alkanes::types::AlkaneId as CliAlkaneId;
    let frbtc_id = CliAlkaneId { block: 32, tx: 0 };
    Some(get_subfrost_address(system.provider(), &frbtc_id).await?)
} else {
    None
};
```

### 2. Build to_addresses Vector (around line 1630)
Current:
```rust
let to_addresses = vec![to.clone()];
```

New logic:
```rust
let mut to_addresses = Vec::new();

if needs_wrap {
    // Output 0: Subfrost address (receives BTC payment)
    to_addresses.push(subfrost_address.clone().unwrap());
    // Output 1: frBTC recipient (pointer destination)
    to_addresses.push(to.clone());
} else if needs_unwrap {
    // Output 0: Alkanes recipient (for intermediate tokens)
    to_addresses.push(to.clone());
    // Output 1: BTC recipient (where unwrapped BTC goes)
    to_addresses.push(to.clone());
    // Output 2: Subfrost dust output (used in cellpack)
    to_addresses.push(subfrost_address.clone().unwrap());
} else {
    // Normal swap: just recipient
    to_addresses.push(to.clone());
}
```

### 3. Build InputRequirement (around line 1633)
Current:
```rust
let input_reqs = vec![
    InputRequirement::Alkanes {
        block: input_token.block,
        tx: input_token.tx,
        amount: input as u64,
    },
];
```

New logic:
```rust
let input_reqs = if needs_wrap {
    // Wrap requires BTC input instead of alkanes input
    vec![InputRequirement::Bitcoin { amount: input }]
} else {
    // Normal swap or unwrap: alkanes input
    vec![InputRequirement::Alkanes {
        block: input_token.block,
        tx: input_token.tx,
        amount: input as u64,
    }]
};
```

### 4. Build Protostones (around line 1641)
Current:
```rust
let protostones = parse_protostones(&calldata)?;
```

New logic:
```rust
let mut protostones = parse_protostones(&calldata)?;  // This is the swap protostone

if needs_wrap {
    // Prepend wrap protostone
    // Wrap outputs to output 1 (where recipient is)
    let wrap_proto = build_wrap_protostone(input, 1);
    protostones.insert(0, wrap_proto);
}

if needs_unwrap {
    // Calculate output indices
    let alkanes_recipient_vout = 0;
    let btc_recipient_vout = 1;
    let subfrost_dust_vout = 2;
    
    // Modify swap protostone to point to unwrap protostone's virtual vout
    // The unwrap protostone will be at index len(outputs) + 1 + len(protostones)
    // We need to update the swap protostone's pointer
    if let Some(swap_proto) = protostones.last_mut() {
        // Point swap to next protostone (unwrap)
        // Virtual vout = num_real_outputs + 1 (for OP_RETURN) + protostone_index
        // The unwrap will be at protostone index = protostones.len()
        let unwrap_vout = to_addresses.len() as u32 + 1 + protostones.len() as u32;
        swap_proto.pointer = Some(OutputTarget::VirtualOutput(unwrap_vout));
    }
    
    // Append unwrap protostone
    let unwrap_proto = build_unwrap_protostone(
        final_minimum_output,  // Amount of frBTC to unwrap
        subfrost_dust_vout,    // Dust output index for cellpack
        btc_recipient_vout,    // Where BTC goes
        alkanes_recipient_vout,  // Refund if unwrap fails
    );
    protostones.push(unwrap_proto);
}
```

## Output Structure Examples

### Wrap Only (B,2:0)
- Output 0: Subfrost address (BTC payment - 10M sats)
- Output 1: Recipient address (frBTC destination - 546 sats)
- Output 2: OP_RETURN (protostones)
- Output 3+: Change

**Protostones:**
1. Wrap: `[32,0,77]` - BTC→frBTC, pointer=v1
2. Swap: `[4,65522,13,...]` - Swap frBTC→token, pointer=v1

### Unwrap Only (2:0,B)
- Output 0: Recipient address (alkanes - 546 sats)
- Output 1: Recipient address (BTC destination - 546 sats)
- Output 2: Subfrost address (dust - 546 sats)
- Output 3: OP_RETURN (protostones)
- Output 4+: Change

**Protostones:**
1. Swap: `[4,65522,13,...]` - Swap token→frBTC, pointer=v4 (unwrap protostone)
2. Unwrap: `[32,0,78,2,amount]` - frBTC→BTC, pointer=v1

### Wrap + Unwrap (B,2:0,B)
- Output 0: Subfrost address (BTC payment wrap)
- Output 1: Recipient address (intermediate - 546 sats)
- Output 2: Recipient address (BTC destination - 546 sats)
- Output 3: Subfrost address (dust unwrap - 546 sats)
- Output 4: OP_RETURN (protostones)
- Output 5+: Change

**Protostones:**
1. Wrap: `[32,0,77]` - BTC→frBTC, pointer=v1
2. Swap: `[4,65522,13,...]` - Swap frBTC→frBTC, pointer=v6 (unwrap protostone)
3. Unwrap: `[32,0,78,3,amount]` - frBTC→BTC, pointer=v2

## Key Considerations

1. **Virtual vout calculation** - Must account for all real outputs + OP_RETURN + protostone index
2. **Pointer chaining** - Wrap points to recipient, Swap points to unwrap, Unwrap points to BTC recipient
3. **Dust outputs** - 546 sats each, must be in transaction for cellpack reference
4. **Refund pointers** - If wrap/unwrap fail, tokens go back to refund address
5. **Change outputs** - Come after OP_RETURN, don't affect vout calculations

## Testing
1. Build and test wrap-only
2. Build and test unwrap-only
3. Build and test wrap+unwrap
4. Verify with --trace flag to see all protostone execution
