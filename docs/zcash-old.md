# Zcash Support in Alkanes-rs

## Overview

This document describes the implementation of Zcash support in alkanes-rs using the **ord-dogecoin inscription scheme**. This approach enables alkanes smart contracts to run on Zcash's transparent addresses (t-addresses) while maintaining compatibility with existing tooling and infrastructure.

## Architecture

### High-Level Design

```
┌──────────────────────────────────────────────────────────────────┐
│                       ZCASH BLOCKCHAIN                           │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Shielded Pool (z-addr)                                         │
│  ├─ Private balance                                             │
│  ├─ Hidden amounts                                              │
│  └─ NOT tracked by alkanes indexer                              │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────┐                │
│  │  WRAP TRANSACTION (z-addr → t-addr)        │                │
│  │  Input:  z-addr (shielded)                 │                │
│  │  Output 0: OP_RETURN (runestone)           │                │
│  │  Output 1: t-addr (frZEC signer)           │ ← TRACKED      │
│  │  Output 2: t-addr (user change)            │                │
│  └────────────────────────────────────────────┘                │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────┐                │
│  │  ALKANES TRANSPARENT OPERATIONS             │                │
│  │  ├─ Smart contracts in scriptSig            │                │
│  │  ├─ frZEC token transfers                   │                │
│  │  ├─ DeFi operations (AMM, lending, etc.)    │                │
│  │  └─ All on t-addresses only                 │                │
│  └────────────────────────────────────────────┘                │
│                           │                                      │
│                           ▼                                      │
│  ┌────────────────────────────────────────────┐                │
│  │  UNWRAP TRANSACTION (t-addr → z-addr)      │                │
│  │  Input:  t-addr (frZEC burn)               │                │
│  │  Output 0: OP_RETURN (burn message)        │                │
│  │  Output 1: t-addr (tracking for signer)    │                │
│  │  Pointer: t-addr (destination for payout)  │                │
│  └────────────────────────────────────────────┘                │
│                           │                                      │
│                           ▼                                      │
│  Shielded Pool (z-addr)                                         │
│  └─ User receives ZEC back in privacy pool                      │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

### Key Principles

1. **Transparent-Only Tracking**: Alkanes indexer ONLY tracks transparent addresses (t-addresses)
2. **ScriptSig Envelopes**: Smart contract bytecode is embedded in `scriptSig` (not witness)
3. **OP_RETURN Runestones**: Protocol messages use OP_RETURN (within 80-byte limit)
4. **Shielded Pool Boundaries**: frZEC contract enforces entry/exit at pool boundaries

## Ord-Dogecoin Inscription Scheme

### Background

The **ord-dogecoin** project by apezord pioneered inscriptions on Dogecoin using `scriptSig` instead of witness data (since Dogecoin doesn't have SegWit). This same approach is perfect for Zcash.

### Reference Implementation

Location: `./reference/ord-dogecoin/src/inscription.rs`

Key insight from ord-dogecoin:
```rust
pub(crate) fn from_transactions(txs: Vec<Transaction>) -> ParsedInscription {
    let mut sig_scripts = Vec::with_capacity(txs.len());
    for i in 0..txs.len() {
        if txs[i].input.is_empty() {
            return ParsedInscription::None;
        }
        // Extract from scriptSig, not witness
        sig_scripts.push(txs[i].input[0].script_sig.clone());
    }
    InscriptionParser::parse(sig_scripts)
}
```

### Envelope Format in ScriptSig

The envelope structure in scriptSig follows this pattern:

```
OP_FALSE OP_IF
  <protocol_id>     // "BIN" for alkanes (or "ZAK" for Zcash Alkanes)
  <body_tag>        // Empty byte array
  <data_chunk_1>    // Up to 520 bytes
  <data_chunk_2>    // Up to 520 bytes
  ...
  <data_chunk_n>    // Up to 520 bytes
OP_ENDIF
```

This is identical to the witness envelope structure but embedded in `scriptSig` instead.

## Implementation Details

### 1. Feature Flag

```toml
# Cargo.toml
[features]
zcash = []
```

All Zcash-specific code is feature-gated with `#[cfg(feature = "zcash")]`.

### 2. Envelope Extraction (crates/alkanes-support/src/envelope.rs)

#### Current Implementation (Bitcoin/Taproot):
```rust
impl RawEnvelope {
    pub fn from_transaction(transaction: &Transaction) -> Vec<Self> {
        let mut envelopes = Vec::new();
        
        for (i, input) in transaction.input.iter().enumerate() {
            if let Some(tapscript) = input.witness.tapscript() {
                if let Ok(input_envelopes) = Self::from_tapscript(tapscript, i) {
                    envelopes.extend(input_envelopes);
                }
            }
        }
        
        envelopes
    }
}
```

#### Zcash Implementation (ScriptSig-based):
```rust
#[cfg(feature = "zcash")]
impl RawEnvelope {
    pub fn from_transaction(transaction: &Transaction) -> Vec<Self> {
        let mut envelopes = Vec::new();
        
        for (i, input) in transaction.input.iter().enumerate() {
            // Extract from scriptSig instead of witness
            if let Ok(input_envelopes) = Self::from_scriptsig(&input.script_sig, i) {
                envelopes.extend(input_envelopes);
            }
        }
        
        envelopes
    }
    
    fn from_scriptsig(scriptsig: &Script, input: usize) -> Result<Vec<Self>> {
        let mut envelopes = Vec::new();
        let mut instructions = scriptsig.instructions().peekable();
        let mut stuttered = false;
        
        while let Some(instruction) = instructions.next().transpose()? {
            if instruction == PushBytes((&[]).into()) {
                let (stutter, envelope) = 
                    Self::from_instructions(&mut instructions, input, envelopes.len(), stuttered)?;
                if let Some(envelope) = envelope {
                    envelopes.push(envelope);
                } else {
                    stuttered = stutter;
                }
            }
        }
        
        Ok(envelopes)
    }
    
    // from_instructions() remains unchanged - same parsing logic
}
```

### 3. Protocol Identifier (crates/alkanes-support/src/envelope.rs)

```rust
#[cfg(not(feature = "zcash"))]
pub(crate) const PROTOCOL_ID: [u8; 3] = *b"BIN"; // Bitcoin Alkanes

#[cfg(feature = "zcash")]
pub(crate) const PROTOCOL_ID: [u8; 3] = *b"ZAK"; // Zcash Alkanes
```

Alternatively, keep "BIN" for compatibility:
```rust
pub(crate) const PROTOCOL_ID: [u8; 3] = *b"BIN"; // Works for both
```

### 4. Runestone OP_RETURN (crates/ordinals/src/runestone.rs)

Zcash supports OP_RETURN with 80-byte limit. The current runestone implementation already chunks data appropriately:

```rust
#[cfg(feature = "zcash")]
impl Runestone {
    pub const PROTOCOL_ID: &'static [u8] = b"Z"; // Zcash identifier
    
    pub fn encipher(&self) -> ScriptBuf {
        let mut payload = Vec::new();
        
        // ... encode flags, etching, mint, pointer, protocol ...
        
        let mut builder = script::Builder::new()
            .push_opcode(opcodes::all::OP_RETURN)
            .push_slice(b"Z"); // Zcash identifier
            
        for chunk in payload.chunks(MAX_SCRIPT_ELEMENT_SIZE) {
            let push: &script::PushBytes = chunk.try_into().unwrap();
            builder = builder.push_slice(push);
        }
        
        builder.into_script()
    }
}
```

**Note**: Zcash's 80-byte OP_RETURN limit is sufficient for runestone protocol messages. If messages exceed 80 bytes total, they must be encoded more efficiently or moved to scriptSig.

### 5. Address Validation (crates/metashrew-support/src/address.rs)

The address module already supports arbitrary magic bytes. For Zcash:

```rust
#[cfg(feature = "zcash")]
pub fn configure_zcash_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("zs"), // Sapling z-addresses (ignored by alkanes)
        p2pkh_prefix: 0x1c, // t1 addresses (transparent)
        p2sh_prefix: 0x1d,  // t3 addresses (transparent)
    });
}
```

Only `p2pkh_prefix` and `p2sh_prefix` matter for alkanes since we only track transparent addresses.

### 6. Block Parsing (src/block.rs or metashrew-support)

Zcash uses a different block header format, but the reference already includes `AuxpowBlock` support. For Zcash:

```rust
#[cfg(feature = "zcash")]
pub fn parse_zcash_block(data: &[u8]) -> Result<Block> {
    // Zcash blocks have:
    // - Version (4 bytes)
    // - Previous block hash (32 bytes)
    // - Merkle root (32 bytes)
    // - Final sapling root (32 bytes) // ZCASH-SPECIFIC
    // - Timestamp (4 bytes)
    // - Bits (4 bytes)
    // - Nonce (32 bytes) // Equihash nonce
    // - Solution (variable length) // Equihash solution
    
    // Use custom parsing or adapt bitcoin::Block deserialization
    // The transaction list follows standard format
}
```

**Alternative**: If metashrew can provide pre-parsed transactions, block parsing may not be necessary in alkanes-rs.

### 7. Transaction Validation

#### Shielded Transaction Detection

Alkanes must reject or skip transactions that involve shielded inputs/outputs:

```rust
#[cfg(feature = "zcash")]
pub fn is_transparent_only(tx: &Transaction) -> bool {
    // Zcash transactions have different formats
    // Check for JoinSplit, Sapling, Orchard components
    
    // Version 4: has JoinSplits (Sprout) and Sapling
    // Version 5: has Orchard
    
    // For alkanes: only accept version 1-3 transparent-only transactions
    // Or parse version 4/5 and ensure shielded components are empty
    
    tx.version <= 3 // Simple heuristic: only process old-style transparent txs
    // OR more sophisticated: check for empty vJoinSplit, vShieldedSpend, vShieldedOutput
}
```

In the indexer:
```rust
pub fn index_block(block: &Block, height: u32) -> Result<()> {
    #[cfg(feature = "zcash")]
    {
        // Filter out shielded transactions
        for tx in &block.txdata {
            if !is_transparent_only(tx) {
                continue; // Skip this transaction
            }
            // Process transparent transaction normally
        }
    }
    
    #[cfg(not(feature = "zcash"))]
    {
        // Normal Bitcoin processing
    }
}
```

### 8. Contract Size Limits

- **ScriptSig size**: No practical limit (only block size: 2MB)
- **150KB gzipped contract**: ~296 chunks of 520 bytes = ~154KB in scriptSig ✅
- **OP_RETURN limit**: 80 bytes total (sufficient for runestone metadata)

Contract deployment:
```rust
// contracts/alkanes-std-*/src/lib.rs
// No changes needed - compression and chunking already handled

// Deployment transaction:
// Input 0: scriptSig contains [OP_FALSE OP_IF "ZAK" <compressed_wasm_chunks> OP_ENDIF]
// Output 0: OP_RETURN with runestone (CREATE cellpack)
// Output 1: t-address (receives alkane token)
```

## Transparent Address Enforcement

### Critical: Pointer Validation

The frZEC contract (and all alkanes contracts) must enforce that pointers only target transparent addresses:

```rust
// In frZEC unwrap logic:
fn validate_pointer_output(&self, tx: &Transaction, pointer: u32) -> Result<()> {
    let output = &tx.output[pointer as usize];
    
    #[cfg(feature = "zcash")]
    {
        // Check that script_pubkey is P2PKH or P2SH (t-address format)
        if !output.script_pubkey.is_p2pkh() && !output.script_pubkey.is_p2sh() {
            return Err(anyhow!("Pointer must target transparent address (t-addr) on Zcash"));
        }
    }
    
    Ok(())
}
```

### Shielded Pool Interaction Policy

**Allowed**:
- z-addr → t-addr (wrap/deposit)
- t-addr → z-addr (unwrap/withdraw)
- t-addr → t-addr (alkanes operations)

**Rejected**:
- Alkanes tokens sent to z-addr (would be untrackable)
- Protomessages with pointers to z-addr
- Mixed shielded/transparent operations within alkanes context

## Building for Zcash

### Build Command

```bash
cargo build --release --features zcash
```

This produces `alkanes.wasm` with Zcash support.

### Runtime Configuration

When running the indexer:

```bash
metashrew/target/release/rockshrew-mono \
  --daemon-rpc-url http://localhost:8232 \  # Zcash RPC port
  --auth zcashrpc:zcashrpc \
  --db-path ~/.metashrew-zcash \
  --indexer ~/alkanes-rs/target/wasm32-unknown-unknown/release/alkanes.wasm \
  --start-block 0 \  # Or specific Zcash block height
  --host 0.0.0.0 \
  --port 8080 \
  --cors '*'
```

### Network Configuration in Indexer

```rust
#[cfg(feature = "zcash")]
pub fn configure_network() {
    set_network(NetworkParams {
        bech32_prefix: String::from("zs"), // Not used by alkanes
        p2pkh_prefix: 0x1c, // t1
        p2sh_prefix: 0x1d,  // t3
    });
}
```

## Testing

### Unit Tests

```rust
#[cfg(all(test, feature = "zcash"))]
mod zcash_tests {
    use super::*;
    
    #[test]
    fn test_scriptsig_envelope_extraction() {
        // Create a transaction with scriptSig envelope
        let mut script = Vec::new();
        script.push(opcodes::OP_FALSE.to_u8());
        script.push(opcodes::all::OP_IF.to_u8());
        script.extend_from_slice(b"ZAK");
        script.extend_from_slice(&[0]); // body tag
        script.extend_from_slice(b"test contract data");
        script.push(opcodes::all::OP_ENDIF.to_u8());
        
        let tx = Transaction {
            version: 2,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::from_bytes(script),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: vec![],
        };
        
        let envelopes = RawEnvelope::from_transaction(&tx);
        assert_eq!(envelopes.len(), 1);
        assert_eq!(envelopes[0].payload.concat(), b"test contract data");
    }
    
    #[test]
    fn test_transparent_address_detection() {
        // Test that t-addresses are properly validated
    }
    
    #[test]
    fn test_shielded_transaction_rejection() {
        // Test that shielded transactions are skipped
    }
}
```

### Integration Tests

Test with Zcash regtest:
1. Start Zcash regtest node
2. Deploy frZEC contract
3. Wrap ZEC → frZEC
4. Perform alkanes operations
5. Unwrap frZEC → ZEC

## Migration Path from Bitcoin Alkanes

For projects already using Bitcoin alkanes:

1. **Contracts are portable**: Existing WASM contracts work on Zcash without modification
2. **Deployment changes**: Use scriptSig instead of witness for envelope
3. **Tooling updates**: Inscription tools need to target scriptSig (use ord-dogecoin pattern)
4. **Address format**: Use t-addresses instead of bc1/tb1
5. **frZEC required**: Users must wrap ZEC into frZEC to use alkanes DeFi

## Known Limitations

1. **No SegWit/Taproot**: Zcash doesn't have these features (doesn't matter for alkanes)
2. **80-byte OP_RETURN**: May require more efficient runestone encoding for complex operations
3. **Shielded Pool Incompatibility**: Alkanes cannot track shielded transactions (by design)
4. **Privacy Trade-off**: Using alkanes requires transparent addresses (visible on-chain)
5. **Transaction Format Differences**: Requires custom block/tx parsing for Zcash-specific fields

## Security Considerations

1. **Replay Protection**: Ensure transactions are chain-specific (use chainId or network magic)
2. **Transparent Address Privacy**: Users should be aware that alkanes operations are fully visible
3. **Shielded Pool Boundaries**: frZEC contract must enforce proper entry/exit validation
4. **Dust Attacks**: Transparent addresses are vulnerable to dust tracking
5. **Network Isolation**: Zcash alkanes are separate from Bitcoin alkanes (different state)

## Future Enhancements

1. **Shielded Viewing Keys**: Potentially allow read-only tracking of shielded balances
2. **Orchard Support**: Evaluate if newer Zcash shielded protocols allow any tracking
3. **Cross-chain Bridge**: Enable frBTC ↔ frZEC swaps via atomic swaps
4. **Compressed Runestones**: More efficient encoding to fit within 80-byte limit
5. **Zcash-Specific Optimizations**: Leverage 75-second block times for faster finality

## References

- ord-dogecoin: `./reference/ord-dogecoin/` (ScriptSig inscription implementation)
- Zcash Protocol Spec: https://zips.z.cash/protocol/protocol.pdf
- Zcash RPC Documentation: https://zcash.readthedocs.io/
- Alkanes Wiki: https://github.com/kungfuflex/alkanes-rs/wiki
- frBTC Implementation: `./reference/subfrost-alkanes/alkanes/fr-btc/`

## Summary

Zcash support in alkanes-rs is achieved by:
1. Using **ord-dogecoin's scriptSig inscription scheme** for smart contracts
2. Tracking **only transparent addresses** (t-addresses)
3. Enforcing **shielded pool boundaries** via frZEC contract
4. Maintaining **protocol compatibility** with Bitcoin alkanes architecture
5. Leveraging **existing tooling** adapted for scriptSig-based envelopes

This approach enables full DeFi functionality on Zcash while respecting the privacy pool boundary.
