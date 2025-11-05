# Zcash Implementation Status

## Completed

### alkanes-rs

1. ✅ **Feature Flag** (`Cargo.toml`)
   - Added `zcash = []` feature

2. ✅ **Protocol Identifier** (`crates/alkanes-support/src/envelope.rs`)
   - Added `#[cfg(feature = "zcash")]` for `PROTOCOL_ID = b"ZAK"`

3. ✅ **ScriptSig Envelope Extraction** (`crates/alkanes-support/src/envelope.rs`)
   - Added `from_scriptsig()` method for Zcash
   - Feature-gated `from_transaction()` to use scriptSig instead of witness

4. ✅ **Runestone OP_RETURN** (`crates/ordinals/src/runestone.rs`)
   - Added Zcash protocol ID `b"Z"`
   - Updated encipher/decipher logic for Zcash

5. ✅ **Network Configuration** (`src/indexer.rs`)
   - Added Zcash network params (t1/t3 addresses)

## Remaining Work

### alkanes-rs

#### High Priority

1. **Transparent Address Validation** (`src/indexer.rs` or new file `src/zcash.rs`)
   ```rust
   #[cfg(feature = "zcash")]
   pub fn is_transparent_only(tx: &Transaction) -> bool {
       // Check if transaction only uses transparent inputs/outputs
       // Reject transactions with shielded components
   }
   
   #[cfg(feature = "zcash")]
   pub fn validate_pointer_address(script_pubkey: &Script) -> Result<()> {
       // Ensure pointer targets P2PKH or P2SH (t-address)
       if !script_pubkey.is_p2pkh() && !script_pubkey.is_p2sh() {
           return Err(anyhow!("Pointer must target transparent address"));
       }
       Ok(())
   }
   ```
   Location: Add these to `src/indexer.rs` or create `src/zcash.rs`

2. **Block Parsing** (optional if metashrew handles this)
   - If needed, add Zcash block header parsing
   - Location: `src/block.rs` or use existing `AuxpowBlock` support

3. **Transaction Filtering in Indexer**
   ```rust
   pub fn index_block(block: &Block, height: u32) -> Result<()> {
       #[cfg(feature = "zcash")]
       {
           for tx in &block.txdata {
               if !is_transparent_only(tx) {
                   continue; // Skip shielded transactions
               }
               // Process transparent transaction
           }
       }
       // ... rest of indexing logic
   }
   ```
   Location: `src/indexer.rs::index_block()`

#### Testing

4. **Unit Tests**
   ```rust
   #[cfg(all(test, feature = "zcash"))]
   mod zcash_tests {
       #[test]
       fn test_scriptsig_envelope_extraction() { }
       
       #[test]
       fn test_transparent_address_validation() { }
       
       #[test]
       fn test_zcash_runestone_encoding() { }
   }
   ```
   Location: `crates/alkanes-support/src/envelope.rs`, `src/indexer.rs`

### subfrost

#### High Priority

1. **Provider Updates** (`crates/subfrost-common/src/provider.rs`)
   ```rust
   impl ConcreteProvider {
       pub fn is_zcash(&self) -> bool {
           self.provider_name.starts_with("zcash")
       }
       
       pub fn get_rpc_port(&self) -> u16 {
           if self.is_zcash() { 8232 } else { 8332 }
       }
   }
   ```

2. **Address Generation** (`crates/subfrost-cli/src/unwrap.rs`)
   ```rust
   let address = if args.provider.starts_with("zcash") {
       // FROST pubkey → P2PKH (t1)
       let pubkey = PublicKey::from_x_only_public_key(internal_xonly, Parity::Even);
       bitcoin::Address::p2pkh(&pubkey, network)
   } else {
       // Bitcoin: P2TR (Taproot)
       bitcoin::Address::p2tr_tweaked(tweaked_xonly.into(), network)
   };
   ```

3. **Transaction Building** (`crates/subfrost-cli/src/unwrap.rs`)
   ```rust
   let tx = Transaction {
       version: if provider.is_zcash() {
           bitcoin::transaction::Version(4) // Zcash Sapling
       } else {
           bitcoin::transaction::Version(2) // Bitcoin
       },
       // ... rest
   };
   ```

4. **PSBT Signing** (`crates/subfrost-cli/src/psbt.rs`)
   ```rust
   pub fn sign_psbt_input(
       secp: &Secp256k1<secp256k1::All>,
       key_packages: &BTreeMap<frost::Identifier, KeyPackage>,
       public_key_package: &PublicKeyPackage,
       psbt: &mut Psbt,
       input_index: usize,
       threshold: u16,
       is_zcash: bool, // New parameter
   ) -> Result<()> {
       let sighash = if is_zcash {
           // ECDSA sighash for P2PKH
           let mut cache = SighashCache::new(&psbt.unsigned_tx);
           cache.legacy_signature_hash(
               input_index,
               &tx_out.script_pubkey,
               EcdsaSighashType::All.to_u32(),
           )?.to_byte_array()
       } else {
           // Schnorr sighash for Taproot
           // ... existing code
       };
       
       // FROST signing (same for both)
       let group_signature = frost::aggregate(...)?;
       
       if is_zcash {
           // ECDSA signature → scriptSig
           let signature = bitcoin::ecdsa::Signature {
               sig: secp256k1::ecdsa::Signature::from_compact(&group_signature.serialize())?,
               hash_ty: EcdsaSighashType::All,
           };
           
           let script_sig = bitcoin::script::Builder::new()
               .push_slice(&signature.serialize())
               .push_slice(&pubkey)
               .into_script();
           
           psbt.inputs[input_index].final_script_sig = Some(script_sig);
       } else {
           // Bitcoin Taproot: witness
           psbt.inputs[input_index].tap_key_sig = Some(...);
       }
       
       Ok(())
   }
   ```

5. **CLI Arguments** (`crates/subfrost-common/src/commands.rs`)
   - Already supports `-p` flag
   - Just need to handle `zcash`, `zcash-testnet`, `zcash-regtest` values

#### Testing

6. **Integration Tests**
   ```rust
   #[tokio::test]
   async fn test_zcash_unwrap_flow() { }
   
   #[tokio::test]
   async fn test_frost_zcash_signing() { }
   ```

### subfrost-alkanes

#### High Priority

1. **Create fr-zec Directory**
   ```bash
   mkdir -p reference/subfrost-alkanes/alkanes/fr-zec/src
   ```

2. **Copy and Adapt fr-btc**
   ```bash
   cp reference/subfrost-alkanes/alkanes/fr-btc/Cargo.toml reference/subfrost-alkanes/alkanes/fr-zec/
   cp reference/subfrost-alkanes/alkanes/fr-btc/src/lib.rs reference/subfrost-alkanes/alkanes/fr-zec/src/
   ```

3. **Update fr-zec/Cargo.toml**
   ```toml
   [package]
   name = "fr-zec"
   version = "0.1.0"
   edition = "2021"
   
   [lib]
   crate-type = ["cdylib", "rlib"]
   
   [dependencies]
   # Same as fr-btc
   
   [features]
   zcash = []
   ```

4. **Update fr-zec/src/lib.rs**
   - Change `DEFAULT_SIGNER_PUBKEY` to ECDSA compressed format
   - Update `get_signer_script()` to use P2PKH
   - Add `validate_pointer_address()` for t-address enforcement
   - Update AlkaneId to `{ block: 32, tx: 1 }`
   - Change name/symbol to "frZEC"

5. **Add to Build Configuration**
   - Update `reference/subfrost-alkanes/Cargo.toml` workspace members
   - Add build script entry for fr-zec

## Build Commands

### alkanes-rs

```bash
# Build with Zcash support
cargo build --release --features zcash

# Test
cargo test --features zcash
```

### subfrost

```bash
# Build subfrost CLI
cd reference/subfrost
cargo build --release

# Use with Zcash
./target/release/subfrost aggregate-unwrap \
  -p zcash \
  --bitcoin-rpc-url http://localhost:8232 \
  --auth zcashrpc:password \
  --metashrew-rpc-url http://localhost:8080 \
  --sandshrew-rpc-url http://localhost:3000 \
  --esplora-url https://zcash.blockexplorer.com/api \
  --frost-files ./frost-keys/ \
  --passphrase "your-passphrase" \
  --unwrap-premium 0.001
```

### subfrost-alkanes

```bash
# Build fr-zec
cd reference/subfrost-alkanes/alkanes/fr-zec
cargo build --target wasm32-unknown-unknown --release --features zcash

# Compress
gzip -9 -c target/wasm32-unknown-unknown/release/fr_zec.wasm > fr_zec.wasm.gz
```

## Priority Order

1. **alkanes-rs remaining items** (needed for indexer)
2. **fr-zec contract** (needed for wrapping/unwrapping)
3. **subfrost updates** (needed for unwrap fulfillment)
4. **Testing and documentation**

## Next Steps

To complete the implementation:

1. Implement transparent address validation in alkanes-rs
2. Create fr-zec contract in subfrost-alkanes
3. Update subfrost provider and transaction building
4. Update subfrost PSBT signing for ECDSA
5. Test end-to-end flow

## Files to Create/Modify

### alkanes-rs
- `src/zcash.rs` (new) - Zcash-specific utilities
- `src/indexer.rs` - Add transaction filtering

### subfrost
- `crates/subfrost-common/src/provider.rs` - Zcash detection
- `crates/subfrost-cli/src/unwrap.rs` - Address generation, tx building
- `crates/subfrost-cli/src/psbt.rs` - ECDSA signing

### subfrost-alkanes
- `alkanes/fr-zec/` (new directory)
- `alkanes/fr-zec/Cargo.toml` (new)
- `alkanes/fr-zec/src/lib.rs` (new)
- `Cargo.toml` - Add fr-zec to workspace

## Documentation

All documentation is complete:
- ✅ `docs/zcash.md`
- ✅ `reference/subfrost/docs/zcash.md`
- ✅ `reference/subfrost-alkanes/docs/zcash.md`
