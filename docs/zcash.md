# Zcash Support in Alkanes-rs (CGGMP21 Edition)

## Overview

This document describes the implementation of Zcash support in alkanes-rs using:
1. **ord-dogecoin inscription scheme** for scriptSig-based envelopes
2. **CGGMP21 multisig** (not FROST/Schnorr) for ECDSA threshold signatures
3. **Automatic z-address rejection** with transparent address fallback

## Key Design Decisions

### 1. CGGMP21 Instead of FROST

**Why CGGMP21?**
- Zcash uses **ECDSA signatures** (not Schnorr)
- P2PKH addresses require ECDSA
- FROST produces Schnorr signatures (incompatible)
- **CGGMP21** provides threshold ECDSA (perfect for Zcash)

**Implementation:**
- Located in: `./reference/subfrost-cggmp21/`
- Subfrost command: `cggmp21 aggregate-unwrap` (not `frost aggregate-unwrap`)
- FROST commands with `-p zcash` will error

### 2. Z-Address Handling with Fallback

**Problem:** Users might accidentally point to z-addresses (shielded, untrackable)

**Solution - Fallback Chain:**
1. **Primary**: Use `pointer` if it targets t-address ✅
2. **Fallback 1**: Use `refund_pointer` if it targets t-address ✅
3. **Fallback 2**: Find first t-address output (like default output in runes) ✅
4. **Last Resort**: Burn funds if no t-address found ⚠️

```rust
fn resolve_output_address(tx: &Transaction, pointer: Option<u32>, refund_pointer: Option<u32>) -> Option<u32> {
    // Try pointer first
    if let Some(p) = pointer {
        if p < tx.output.len() as u32 && is_t_address(&tx.output[p as usize].script_pubkey) {
            return Some(p);
        }
    }
    
    // Try refund_pointer
    if let Some(rp) = refund_pointer {
        if rp < tx.output.len() as u32 && is_t_address(&tx.output[rp as usize].script_pubkey) {
            return Some(rp);
        }
    }
    
    // Find first t-address (default output logic)
    for (i, output) in tx.output.iter().enumerate() {
        if !output.script_pubkey.is_op_return() && is_t_address(&output.script_pubkey) {
            return Some(i as u32);
        }
    }
    
    // No t-address found - funds will be burned
    None
}
```

### 3. frZEC AlkaneId

**frZEC (Synthetic Zcash)**:
```rust
AlkaneId {
    block: 42,  // CGGMP21 wrapped assets (not 32 for FROST)
    tx: 0       // frZEC is first CGGMP21 asset
}
```

**Rationale:**
- Block 32 = FROST-wrapped assets (frBTC uses [32, 0])
- Block 42 = CGGMP21-wrapped assets (frZEC uses [42, 0])
- Clear separation between signature schemes

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    ZCASH ECOSYSTEM                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Shielded Pool (z-addresses)                                   │
│  - Private balances                                            │
│  - Hidden transaction amounts                                  │
│  - NOT tracked by alkanes                                      │
│                           │                                     │
│                           ▼ WRAP                                │
│  ┌──────────────────────────────────────────┐                 │
│  │  WRAP TX (z-addr → t-addr)               │                 │
│  │  Input:  z-address (shielded)            │                 │
│  │  Output 0: OP_RETURN (runestone)         │                 │
│  │  Output 1: t-address (CGGMP21 signer)    │ ← TRACKED       │
│  │  Output 2: t-address (user change)       │                 │
│  └──────────────────────────────────────────┘                 │
│                           │                                     │
│                           ▼ MINT frZEC                          │
│  ┌──────────────────────────────────────────┐                 │
│  │  frZEC CONTRACT [42, 0]                  │                 │
│  │  - Sees ZEC payment to CGGMP21 address   │                 │
│  │  - Mints frZEC tokens                    │                 │
│  │  - Applies 0.1% premium                  │                 │
│  │  - Transfers to user's t-address         │                 │
│  └──────────────────────────────────────────┘                 │
│                           │                                     │
│                           ▼ USE frZEC                           │
│  ┌──────────────────────────────────────────┐                 │
│  │  ALKANES DeFi (Transparent)              │                 │
│  │  - Trade frZEC on AMMs                   │                 │
│  │  - Lend/borrow frZEC                     │                 │
│  │  - Smart contract interactions           │                 │
│  │  - All fully visible on-chain            │                 │
│  └──────────────────────────────────────────┘                 │
│                           │                                     │
│                           ▼ UNWRAP                              │
│  ┌──────────────────────────────────────────┐                 │
│  │  UNWRAP TX (frZEC → payment request)     │                 │
│  │  Input:  t-address with frZEC            │                 │
│  │  Output 0: OP_RETURN (burn message)      │                 │
│  │  Output 1: t-address (tracking)          │                 │
│  │  Pointer: t-address OR z-address         │                 │
│  │  ┌──────────────────────────────────┐    │                 │
│  │  │ Z-ADDRESS FALLBACK LOGIC:        │    │                 │
│  │  │ 1. Try pointer (if t-addr)       │    │                 │
│  │  │ 2. Try refund_pointer (if t-addr)│    │                 │
│  │  │ 3. Find first t-addr output      │    │                 │
│  │  │ 4. Burn if no t-addr found       │    │                 │
│  │  └──────────────────────────────────┘    │                 │
│  └──────────────────────────────────────────┘                 │
│                           │                                     │
│                           ▼ FULFILL                             │
│  ┌──────────────────────────────────────────┐                 │
│  │  CGGMP21 UNWRAP AGGREGATOR               │                 │
│  │  - Polls for pending payments            │                 │
│  │  - Aggregates multiple unwraps           │                 │
│  │  - CGGMP21 threshold ECDSA signing       │                 │
│  │  - Pays to resolved t-address            │                 │
│  └──────────────────────────────────────────┘                 │
│                           │                                     │
│                           ▼ RECEIVE                             │
│  User receives ZEC on t-address                                │
│  Optional: t-addr → z-addr (re-shield for privacy)             │
│                           │                                     │
│                           ▼                                     │
│  Shielded Pool (z-addresses)                                   │
│  - User regains full privacy                                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation

### 1. Z-Address Detection

```rust
// In alkanes-rs: src/zcash.rs (new file)

#[cfg(feature = "zcash")]
pub fn is_t_address(script_pubkey: &Script) -> bool {
    // Transparent addresses are P2PKH or P2SH
    script_pubkey.is_p2pkh() || script_pubkey.is_p2sh()
}

#[cfg(feature = "zcash")]
pub fn is_z_address(script_pubkey: &Script) -> bool {
    // Z-addresses are not standard Bitcoin script types
    // In practice, we can't detect z-addresses in outputs
    // But we can detect NON t-addresses
    !is_t_address(script_pubkey) && !script_pubkey.is_op_return()
}

#[cfg(feature = "zcash")]
pub fn find_default_t_address_output(tx: &Transaction) -> Option<u32> {
    // Like default_output() in protorune, but for t-addresses
    for (i, output) in tx.output.iter().enumerate() {
        if !output.script_pubkey.is_op_return() && is_t_address(&output.script_pubkey) {
            return Some(i as u32);
        }
    }
    None
}
```

### 2. Pointer Resolution with Fallback

```rust
// In alkanes-rs: crates/protorune/src/lib.rs or src/zcash.rs

#[cfg(feature = "zcash")]
pub fn resolve_pointer_with_fallback(
    tx: &Transaction,
    pointer: Option<u32>,
    refund_pointer: Option<u32>,
) -> Option<u32> {
    // 1. Try primary pointer
    if let Some(p) = pointer {
        if (p as usize) < tx.output.len() {
            if is_t_address(&tx.output[p as usize].script_pubkey) {
                return Some(p);
            }
        }
    }
    
    // 2. Try refund_pointer
    if let Some(rp) = refund_pointer {
        if (rp as usize) < tx.output.len() {
            if is_t_address(&tx.output[rp as usize].script_pubkey) {
                return Some(rp);
            }
        }
    }
    
    // 3. Find first t-address output (default output logic)
    find_default_t_address_output(tx)
    
    // 4. Return None - funds will be burned
}
```

### 3. Integration into Protorune Indexer

```rust
// In alkanes-rs: crates/protorune/src/lib.rs

impl Protorune {
    pub fn index_runestone<T: MessageContext>(
        atomic: &mut AtomicPointer,
        tx: &Transaction,
        runestone: &Runestone,
        height: u64,
        index: u32,
        block: &Block,
        runestone_output_index: u32,
    ) -> Result<()> {
        // ... existing code ...
        
        #[cfg(feature = "zcash")]
        let unallocated_to = {
            // Get refund_pointer from runestone if available
            let refund_pointer = runestone.refund; // Assuming this field exists
            
            match resolve_pointer_with_fallback(tx, runestone.pointer, refund_pointer) {
                Some(resolved) => resolved,
                None => {
                    // No t-address found - log warning and burn
                    log::warn!(
                        "Transaction {} has no t-address outputs. Funds will be burned.",
                        tx.compute_txid()
                    );
                    // Return a sentinel value or handle burn logic
                    return Ok(()); // Skip this transaction or burn funds
                }
            }
        };
        
        #[cfg(not(feature = "zcash"))]
        let unallocated_to = match runestone.pointer {
            Some(v) => v,
            None => default_output(tx),
        };
        
        // ... rest of indexing logic ...
    }
}
```

### 4. frZEC Contract AlkaneId

```rust
// In subfrost-alkanes/alkanes/fr-zec/src/lib.rs

pub const FRZEC_ALKANE_ID: AlkaneId = AlkaneId {
    block: 42,  // CGGMP21 wrapped assets
    tx: 0       // frZEC is first
};

impl SyntheticZcash {
    fn initialize(&self) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);
        
        // Set auth token with CGGMP21 block number
        self.set_auth_token(FRZEC_ALKANE_ID)?;
        
        self.set_name_and_symbol_str("frZEC".to_string(), "frZEC".to_string());
        
        Ok(response)
    }
}
```

### 5. CGGMP21 Integration in Subfrost

```rust
// In subfrost-cli: src/main.rs or commands

#[derive(Subcommand)]
enum Commands {
    Frost {
        #[command(subcommand)]
        command: FrostSubcommands,
    },
    Cggmp21 {
        #[command(subcommand)]
        command: Cggmp21Subcommands,
    },
}

#[derive(Subcommand)]
enum Cggmp21Subcommands {
    AggregateUnwrap {
        // Same args as FROST version
    },
    // ... other CGGMP21 commands
}

// Validation
impl Args {
    pub fn validate(&self) -> Result<()> {
        if self.provider.starts_with("zcash") {
            if let Commands::Frost { .. } = self.command {
                anyhow::bail!(
                    "FROST signing is not compatible with Zcash (uses Schnorr signatures). \
                     Use 'cggmp21' subcommand instead for ECDSA threshold signatures."
                );
            }
        }
        Ok(())
    }
}
```

## Build and Usage

### Build alkanes-rs with Zcash

```bash
cargo build --release --features zcash
```

### Deploy frZEC

```bash
cd reference/subfrost-alkanes/alkanes/fr-zec
cargo build --target wasm32-unknown-unknown --release
gzip -9 -c target/wasm32-unknown-unknown/release/fr_zec.wasm > fr_zec.wasm.gz
```

### Run Subfrost with CGGMP21

```bash
# Generate CGGMP21 keys (not FROST keys)
subfrost cggmp21 keygen \
  --threshold 2 \
  --parties 3 \
  --output-dir ./cggmp21-keys/

# Aggregate unwrap with Zcash
subfrost cggmp21 aggregate-unwrap \
  -p zcash \
  --bitcoin-rpc-url http://localhost:8232 \
  --auth zcashrpc:password \
  --metashrew-rpc-url http://localhost:8080 \
  --sandshrew-rpc-url http://localhost:3000 \
  --esplora-url https://zcash.blockexplorer.com/api \
  --cggmp21-keys ./cggmp21-keys/ \
  --passphrase "your-passphrase" \
  --unwrap-premium 0.001

# FROST commands will error with -p zcash
subfrost frost aggregate-unwrap -p zcash ...
# Error: FROST signing is not compatible with Zcash (uses Schnorr signatures).
#        Use 'cggmp21' subcommand instead for ECDSA threshold signatures.
```

## Z-Address Handling Examples

### Example 1: Normal Flow (All t-addresses)

```
Transaction:
  Input: frZEC from t-address
  Output 0: OP_RETURN (unwrap message)
  Output 1: t-address (tracking for signer)
  Pointer: 2
  Output 2: t-address (destination) ✅

Result: Funds sent to Output 2 (pointer target)
```

### Example 2: Pointer to Z-Address, Refund to T-Address

```
Transaction:
  Input: frZEC from t-address
  Output 0: OP_RETURN (unwrap message)
  Output 1: t-address (tracking for signer)
  Pointer: 2
  Output 2: z-address (shielded) ❌
  RefundPointer: 3
  Output 3: t-address (refund) ✅

Result: Funds sent to Output 3 (refund_pointer target)
Warning: "Pointer targets z-address, falling back to refund_pointer"
```

### Example 3: Both Pointer and Refund are Z-Addresses

```
Transaction:
  Input: frZEC from t-address
  Output 0: OP_RETURN (unwrap message)
  Output 1: t-address (tracking for signer)
  Pointer: 2
  Output 2: z-address (shielded) ❌
  RefundPointer: 3
  Output 3: z-address (also shielded) ❌
  Output 4: t-address (user change) ✅

Result: Funds sent to Output 4 (first t-address found)
Warning: "Pointer and refund_pointer both target z-addresses, using first t-address output"
```

### Example 4: No T-Addresses At All

```
Transaction:
  Input: frZEC from t-address
  Output 0: OP_RETURN (unwrap message)
  Output 1: z-address ❌
  Pointer: 1
  Output 2: z-address ❌

Result: Funds BURNED ⚠️
Error: "No transparent addresses found in transaction. Funds burned."
```

## Key Differences from Bitcoin Version

| Feature | Bitcoin (frBTC) | Zcash (frZEC) |
|---------|----------------|---------------|
| **Multisig** | FROST (Schnorr) | CGGMP21 (ECDSA) |
| **AlkaneId** | [32, 0] | [42, 0] |
| **Address Type** | P2TR (Taproot) | P2PKH (t1) |
| **Signature** | Schnorr in witness | ECDSA in scriptSig |
| **Inscription** | Witness tapscript | ScriptSig (ord-dogecoin) |
| **OP_RETURN** | "BIN" or OP_13 | "Z" |
| **Pointer Handling** | Direct | Fallback chain for z-addrs |
| **Burn Scenario** | Rare | Possible if no t-addrs |

## Security Considerations

1. **Z-Address Rejection**: Prevents loss of funds to untrackable addresses
2. **Fallback Chain**: Provides safety net for user errors
3. **Burn Warning**: Loud warnings before burning funds
4. **CGGMP21 Threshold**: Decentralized signing (no single point of failure)
5. **Transparent Only**: Clear boundary between tracked/untracked funds

## Testing

```rust
#[cfg(all(test, feature = "zcash"))]
mod zcash_tests {
    #[test]
    fn test_t_address_detection() {
        // Test P2PKH and P2SH recognition
    }
    
    #[test]
    fn test_pointer_fallback_chain() {
        // Test: pointer → refund_pointer → first t-addr → burn
    }
    
    #[test]
    fn test_all_z_addresses_burns_funds() {
        // Verify burn behavior when no t-addresses exist
    }
    
    #[test]
    fn test_frzec_alkane_id() {
        assert_eq!(FRZEC_ALKANE_ID, AlkaneId { block: 42, tx: 0 });
    }
}
```

## References

- CGGMP21 Implementation: `./reference/subfrost-cggmp21/`
- ord-dogecoin: `./reference/ord-dogecoin/`
- frBTC (FROST version): `./reference/subfrost-alkanes/alkanes/fr-btc/`
- Zcash Protocol: https://zips.z.cash/protocol/protocol.pdf
- CGGMP21 Paper: https://eprint.iacr.org/2021/060

## Summary

Zcash support in alkanes-rs uses:
1. **CGGMP21** for ECDSA threshold signatures (not FROST)
2. **ScriptSig envelopes** following ord-dogecoin pattern
3. **Automatic fallback** for z-address pointers (prevents fund loss)
4. **AlkaneId [42, 0]** for frZEC (distinguishes from FROST assets)
5. **Transparent-only** tracking with clear privacy boundaries
