# Zcash Implementation Plan - CGGMP21 Edition

## Overview

This plan covers the complete implementation of Zcash support using:
- **CGGMP21** for ECDSA threshold signatures (not FROST/Schnorr)
- **AlkaneId [42, 0]** for frZEC (distinct from FROST's [32, 0])
- **Z-address fallback logic** (pointer → refund_pointer → first t-addr → burn)

## Status: IN PROGRESS

### ✅ Completed

#### alkanes-rs
1. Feature flag `zcash = []` added to Cargo.toml
2. Protocol identifier `b"ZAK"` for envelopes
3. ScriptSig envelope extraction (`from_scriptsig()`)
4. Runestone OP_RETURN with `b"Z"` identifier
5. Network configuration (t1/t3 addresses)
6. Z-address detection utilities (`src/zcash.rs`)
7. Pointer fallback logic (`resolve_pointer_with_fallback`)
8. Documentation (`docs/zcash.md`)

### 🚧 In Progress

#### alkanes-rs
- [ ] Integrate z-address fallback into protorune indexer
- [ ] Add transparent-only transaction filtering
- [ ] Wire up zcash module in indexer

#### subfrost
- [ ] Provider Zcash detection
- [ ] CGGMP21 command validation
- [ ] Error on `frost aggregate-unwrap` with `-p zcash`

#### subfrost-alkanes
- [ ] Create fr-zec contract directory
- [ ] Adapt fr-btc to fr-zec
- [ ] Update AlkaneId to [42, 0]

## Detailed Task List

### Phase 1: alkanes-rs Core (Priority: HIGH)

#### Task 1.1: Integrate Z-Address Fallback
**File:** `crates/protorune/src/lib.rs`
**Location:** `Protorune::index_runestone()`

```rust
// Around line 195 where unallocated_to is set
#[cfg(feature = "zcash")]
let unallocated_to = {
    use crate::zcash::{resolve_pointer_with_fallback, require_t_address_output};
    
    // Require at least one t-address output
    if let Err(e) = require_t_address_output(tx) {
        log::error!("{}", e);
        return Ok(()); // Skip this transaction
    }
    
    // TODO: Extract refund_pointer from runestone if available
    // For now, use None as refund_pointer
    let refund_pointer = None; // runestone.refund_pointer?
    
    match resolve_pointer_with_fallback(tx, runestone.pointer, refund_pointer) {
        Some(resolved) => resolved,
        None => {
            log::error!(
                "Transaction {} has no t-address outputs. Skipping to prevent burn.",
                tx.compute_txid()
            );
            return Ok(()); // Skip transaction
        }
    }
};

#[cfg(not(feature = "zcash"))]
let unallocated_to = match runestone.pointer {
    Some(v) => v,
    None => default_output(tx),
};
```

**Estimated:** 30 minutes

#### Task 1.2: Add Refund Pointer Support (if not exists)
**File:** `crates/ordinals/src/runestone.rs`

Check if `Runestone` struct has `refund_pointer` field. If not, add it:

```rust
pub struct Runestone {
    pub edicts: Vec<Edict>,
    pub etching: Option<Etching>,
    pub mint: Option<RuneId>,
    pub pointer: Option<u32>,
    pub refund_pointer: Option<u32>, // ADD THIS
    pub protocol: Option<Vec<u128>>,
}
```

Update encoding/decoding logic to handle refund_pointer.

**Estimated:** 1 hour (if needs implementation)

#### Task 1.3: Wire Up Zcash Module
**File:** `src/lib.rs` ✅ DONE

**File:** `crates/protorune/Cargo.toml`

Add zcash feature dependency if needed:
```toml
[features]
zcash = []
```

**Estimated:** 15 minutes

#### Task 1.4: Testing
**File:** `src/zcash.rs` - Tests already included ✅

Run:
```bash
cargo test --features zcash
```

**Estimated:** 30 minutes

### Phase 2: subfrost-alkanes (Priority: HIGH)

#### Task 2.1: Create fr-zec Directory Structure

```bash
mkdir -p reference/subfrost-alkanes/alkanes/fr-zec/src
```

**Estimated:** 2 minutes

#### Task 2.2: Copy and Adapt fr-btc

```bash
# Copy files
cp reference/subfrost-alkanes/alkanes/fr-btc/Cargo.toml reference/subfrost-alkanes/alkanes/fr-zec/
cp reference/subfrost-alkanes/alkanes/fr-btc/src/lib.rs reference/subfrost-alkanes/alkanes/fr-zec/src/
```

**Estimated:** 2 minutes

#### Task 2.3: Update fr-zec/Cargo.toml

```toml
[package]
name = "fr-zec"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
# Same as fr-btc
alkanes-runtime = { path = "../../../crates/alkanes-runtime" }
alkanes-support = { path = "../../../crates/alkanes-support" }
alkanes-std-factory-support = { path = "../../../crates/alkanes-std-factory-support" }
anyhow = "1.0"
bitcoin = "0.32"
# ... rest same as fr-btc

[features]
zcash = []
```

**Estimated:** 10 minutes

#### Task 2.4: Update fr-zec/src/lib.rs

Key changes:
1. Change AlkaneId to [42, 0]
2. Update signer address generation (P2PKH instead of P2TR)
3. Add z-address validation
4. Update name/symbol to "frZEC"

```rust
// Change AlkaneId
pub const FRZEC_ALKANE_ID: AlkaneId = AlkaneId {
    block: 42,  // CGGMP21 wrapped assets (not 32 for FROST)
    tx: 0       // frZEC
};

// Update DEFAULT_SIGNER_PUBKEY to compressed ECDSA format (33 bytes)
pub const DEFAULT_SIGNER_PUBKEY: [u8; 33] = [
    0x03, // Compressed pubkey prefix
    // ... 32 bytes from CGGMP21 ceremony ...
    // TODO: Replace with actual CGGMP21-generated Zcash pubkey
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
];

impl SyntheticZcash {
    fn get_signer_script(&self) -> ScriptBuf {
        let signer_pubkey_bytes = self.signer();
        
        // ZCASH: Use P2PKH, not P2TR
        let signer_pubkey = PublicKey::from_slice(&signer_pubkey_bytes)
            .expect("Invalid compressed pubkey");
        
        // Generate P2PKH script (t1 address format)
        ScriptBuf::new_p2pkh(&signer_pubkey.pubkey_hash())
    }
    
    // Add z-address validation
    fn validate_pointer_address(&self, output: &TxOut) -> Result<()> {
        if !output.script_pubkey.is_p2pkh() && !output.script_pubkey.is_p2sh() {
            return Err(anyhow!(
                "Pointer must target transparent address (t-addr). \
                 Shielded addresses (z-addr) are not supported by alkanes."
            ));
        }
        Ok(())
    }
    
    fn initialize(&self) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);
        
        // Use CGGMP21 AlkaneId
        self.set_auth_token(FRZEC_ALKANE_ID)?;
        
        // Set name and symbol
        self.set_name_and_symbol_str("frZEC".to_string(), "frZEC".to_string());
        
        Ok(response)
    }
}
```

**Estimated:** 2 hours

#### Task 2.5: Add to Workspace
**File:** `reference/subfrost-alkanes/Cargo.toml`

```toml
[workspace]
members = [
    "alkanes/fr-btc",
    "alkanes/fr-zec",  # ADD THIS
    # ... other members
]
```

**Estimated:** 5 minutes

#### Task 2.6: Build Script Integration
**File:** `reference/subfrost-alkanes/build.rs`

Add fr-zec to build process if needed.

**Estimated:** 15 minutes

### Phase 3: subfrost CGGMP21 Integration (Priority: MEDIUM)

#### Task 3.1: Provider Zcash Detection
**File:** `reference/subfrost/crates/subfrost-common/src/provider.rs`

```rust
impl ConcreteProvider {
    pub fn is_zcash(&self) -> bool {
        self.provider_name.starts_with("zcash")
    }
    
    pub fn get_rpc_port(&self) -> u16 {
        if self.is_zcash() {
            8232 // Zcash RPC port
        } else {
            8332 // Bitcoin RPC port
        }
    }
    
    pub fn get_network(&self) -> bitcoin::Network {
        match self.provider_name.as_str() {
            "zcash" => bitcoin::Network::Bitcoin,
            "zcash-testnet" => bitcoin::Network::Testnet,
            "zcash-regtest" => bitcoin::Network::Regtest,
            _ => bitcoin::Network::Bitcoin, // Default
        }
    }
}
```

**Estimated:** 30 minutes

#### Task 3.2: Command Validation
**File:** `reference/subfrost/crates/subfrost-common/src/commands.rs`

```rust
impl Args {
    pub fn validate(&self) -> Result<()> {
        // Prevent FROST with Zcash
        if self.provider.starts_with("zcash") {
            if let Commands::Frost { .. } = &self.command {
                anyhow::bail!(
                    "❌ FROST signing is not compatible with Zcash.\n\
                     \n\
                     Zcash uses ECDSA signatures (P2PKH), not Schnorr (P2TR).\n\
                     \n\
                     ✅ Use 'cggmp21' subcommand instead:\n\
                     \n\
                     subfrost cggmp21 aggregate-unwrap -p zcash ...\n\
                     \n\
                     FROST (Schnorr) → Bitcoin (P2TR/Taproot)\n\
                     CGGMP21 (ECDSA) → Zcash (P2PKH/transparent)\n"
                );
            }
        }
        
        Ok(())
    }
}
```

**Estimated:** 30 minutes

#### Task 3.3: CGGMP21 Subcommand Structure
**File:** `reference/subfrost/crates/subfrost-cli/src/main.rs`

Verify CGGMP21 subcommand exists:
```rust
#[derive(Subcommand)]
enum Commands {
    Frost {
        #[command(subcommand)]
        command: FrostSubcommands,
    },
    Cggmp21 {  // Should already exist in cggmp21 branch
        #[command(subcommand)]
        command: Cggmp21Subcommands,
    },
}
```

If not exists, refer to `./reference/subfrost-cggmp21/` for implementation.

**Estimated:** Check only - 15 minutes

#### Task 3.4: Address Generation for Zcash
**File:** `reference/subfrost/crates/subfrost-cli/src/unwrap.rs` or cggmp21 equivalent

```rust
// In CGGMP21 aggregate unwrap handler
let address = if args.provider.starts_with("zcash") {
    // CGGMP21 pubkey → P2PKH (t1)
    let pubkey_bytes = cggmp21_public_key.to_bytes(); // Compressed ECDSA
    let pubkey = PublicKey::from_slice(&pubkey_bytes)?;
    bitcoin::Address::p2pkh(&pubkey, network)
} else {
    // Bitcoin: Still use P2PKH or P2WPKH with CGGMP21
    let pubkey_bytes = cggmp21_public_key.to_bytes();
    let pubkey = PublicKey::from_slice(&pubkey_bytes)?;
    bitcoin::Address::p2pkh(&pubkey, network)
};
```

**Estimated:** 1 hour (depends on cggmp21 branch state)

### Phase 4: Testing & Documentation (Priority: MEDIUM)

#### Task 4.1: alkanes-rs Tests

```bash
# Unit tests
cargo test --features zcash

# Specific module tests
cargo test --features zcash zcash::
cargo test --features zcash --package alkanes-support
cargo test --features zcash --package ordinals
```

**Estimated:** 1 hour

#### Task 4.2: fr-zec Tests

```bash
cd reference/subfrost-alkanes/alkanes/fr-zec
cargo test --features zcash
cargo build --target wasm32-unknown-unknown --release
```

**Estimated:** 1 hour

#### Task 4.3: Integration Tests

Create test script:
```bash
#!/bin/bash
# test-zcash-integration.sh

# 1. Start Zcash regtest
zcashd -regtest -daemon

# 2. Build alkanes with Zcash
cd alkanes-rs
cargo build --release --features zcash

# 3. Start metashrew indexer
# ...

# 4. Deploy frZEC
# ...

# 5. Test wrap/unwrap flow
# ...
```

**Estimated:** 2-3 hours

#### Task 4.4: Update Documentation

Files to update:
- [x] `docs/zcash.md` - DONE
- [ ] `reference/subfrost/docs/zcash.md` - Needs CGGMP21 updates
- [ ] `reference/subfrost-alkanes/docs/zcash.md` - Needs AlkaneId [42,0] updates
- [ ] `README.md` - Add Zcash feature mention

**Estimated:** 1 hour

## Build Commands Reference

### alkanes-rs
```bash
# Build with Zcash support
cargo build --release --features zcash

# Test
cargo test --features zcash

# Build WASM
cargo build --target wasm32-unknown-unknown --release --features zcash
```

### fr-zec
```bash
cd reference/subfrost-alkanes/alkanes/fr-zec

# Build
cargo build --target wasm32-unknown-unknown --release

# Compress
gzip -9 -c target/wasm32-unknown-unknown/release/fr_zec.wasm > fr_zec.wasm.gz

# Check size
ls -lh fr_zec.wasm.gz  # Should be ~150KB or less
```

### subfrost
```bash
cd reference/subfrost

# Build with CGGMP21 support (from cggmp21 branch)
cargo build --release

# Use with Zcash
./target/release/subfrost cggmp21 aggregate-unwrap \
  -p zcash \
  --bitcoin-rpc-url http://localhost:8232 \
  --auth zcashrpc:password \
  --metashrew-rpc-url http://localhost:8080 \
  --sandshrew-rpc-url http://localhost:3000 \
  --esplora-url https://zcash.blockexplorer.com/api \
  --cggmp21-keys ./cggmp21-keys/ \
  --passphrase "your-passphrase" \
  --unwrap-premium 0.001

# This should error (good!)
./target/release/subfrost frost aggregate-unwrap -p zcash ...
# Error: FROST signing is not compatible with Zcash...
```

## Time Estimates

- Phase 1 (alkanes-rs): **3-4 hours**
- Phase 2 (fr-zec): **3-4 hours**
- Phase 3 (subfrost): **2-3 hours** (depends on cggmp21 branch)
- Phase 4 (testing/docs): **3-4 hours**

**Total: 11-15 hours** (assuming cggmp21 branch is mostly ready)

## Critical Path

1. ✅ Z-address utilities (DONE)
2. **Integrate fallback into protorune** (blocks all testing)
3. **Create fr-zec contract** (blocks wrap/unwrap testing)
4. **Subfrost CGGMP21 validation** (blocks unwrap fulfillment)
5. Testing & refinement

## Next Steps

1. **NOW**: Implement Task 1.1 (integrate z-address fallback)
2. **THEN**: Implement Task 2 (create fr-zec)
3. **THEN**: Test end-to-end with regtest
4. **FINALLY**: Refine and document

## Success Criteria

- [x] alkanes-rs compiles with `--features zcash`
- [ ] fr-zec contract deploys successfully
- [ ] Z-address pointer triggers fallback (not error)
- [ ] CGGMP21 unwrap aggregation works with `-p zcash`
- [ ] FROST commands error with `-p zcash`
- [ ] Full wrap → use → unwrap flow completes
- [ ] All tests pass

## Files Changed

### alkanes-rs
- [x] `Cargo.toml` - Added zcash feature
- [x] `crates/alkanes-support/src/envelope.rs` - ScriptSig extraction
- [x] `crates/ordinals/src/runestone.rs` - Zcash protocol ID
- [x] `src/indexer.rs` - Zcash network config
- [x] `src/lib.rs` - Module declaration
- [x] `src/zcash.rs` - NEW FILE - Z-address utilities
- [ ] `crates/protorune/src/lib.rs` - Integrate fallback
- [ ] `crates/ordinals/src/runestone.rs` - Refund pointer (if needed)
- [x] `docs/zcash.md` - Updated documentation

### subfrost-alkanes
- [ ] `alkanes/fr-zec/` - NEW DIRECTORY
- [ ] `alkanes/fr-zec/Cargo.toml` - NEW FILE
- [ ] `alkanes/fr-zec/src/lib.rs` - NEW FILE
- [ ] `Cargo.toml` - Add fr-zec to workspace

### subfrost
- [ ] `crates/subfrost-common/src/provider.rs` - Zcash detection
- [ ] `crates/subfrost-common/src/commands.rs` - FROST validation
- [ ] Refer to `reference/subfrost-cggmp21/` for CGGMP21 implementation

## Notes

- AlkaneId [42, 0] clearly distinguishes frZEC (CGGMP21) from frBTC (FROST)
- Z-address fallback prevents accidental fund loss
- CGGMP21 provides proper ECDSA threshold signatures for Zcash
- subfrost-cggmp21 branch already has most CGGMP21 infrastructure

## Questions/Decisions

1. ✅ **Signature scheme**: CGGMP21 (ECDSA), not FROST (Schnorr)
2. ✅ **AlkaneId**: [42, 0] for frZEC (not [32, 0])
3. ✅ **Z-address handling**: Automatic fallback chain
4. ✅ **Burn behavior**: Last resort if no t-address
5. ✅ **Inscription method**: ScriptSig (ord-dogecoin style)
