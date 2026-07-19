# Alkanes Deployment Runestone Structure

## Expected Runestone for Envelope Deployment

Based on the code in `execute.rs`, a proper alkanes deployment with envelope should have:

### Protostone Structure
```rust
Protostone {
    protocol_tag: 1,  // ALKANES Metaprotocol
    message: [cellpack_encoded],  // From cellpack.encipher()
    edicts: [],  // Empty for deployment
    pointer: None,
    burn: None,
    refund: None,
    from: None,
}
```

### Cellpack for `[3, 7936]` Deployment

The cellpack `[3, 7936, 0, 32, 0, 4, 7937, 4, 7970, 4, 7956]` represents:
- `3` - block height (reserved space indicator)
- `7936` - tx index (the alkane ID in reserved space)
- `0, 32, 0` - parameters for the deployment
- `4, 7937` - target bytecode location `[4, 7937]`
- `4, 7970` - another pointer
- `4, 7956` - another pointer

When encoded through `cellpack.encipher()`, this gets converted to the `message` field.

### Runestone OP_RETURN Structure

The final OP_RETURN should be:
```
OP_RETURN 
OP_13 (magic number for Runestone)
<encoded_protocol_values>
```

Where `encoded_protocol_values` comes from:
```rust
let protocol_values = converted_protostones.encipher()?;
let runestone = Runestone {
    protocol: Some(protocol_values),
    ..Default::default()
};
runestone.encipher()
```

### What to Check

1. **Protocol Tag**: Should be `1` (ALKANES Metaprotocol, not 2)
2. **Message Present**: The message field should contain the encoded cellpack
3. **Message Content**: Should decode to `[3, 7936, ...]` showing CREATERESERVED operation
4. **Envelope**: Transaction witness should contain:
   - Signature (64 bytes)
   - Reveal script with envelope (75KB+ gzipped WASM)
   - Control block (33 bytes)

### Verification

To verify a deployment is correct:

1. Check the reveal transaction has large witness data (~75KB)
2. Decode the runestone - should show protocol tag 1
3. Decode the message - should show `[3, 7936, ...]` cellpack
4. Extract witness payload - should decompress to valid WASM
5. Check alkanes indexer deploys bytecode to `[4, 7936]`
