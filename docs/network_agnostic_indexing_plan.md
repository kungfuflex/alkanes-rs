# Network-Agnostic Script-Based Indexing Refactoring Plan

## Current Architecture Issues

### Problem 1: Network-Specific Address Encoding
The current implementation uses `to_address_str()` which:
- Requires network configuration (p2pkh_prefix, p2sh_prefix, bech32_prefix)
- Only supports P2PKH, P2SH, P2WPKH, P2WSH, P2TR (via `Payload::from_script`)
- **Excludes P2PK** and other script types
- Stores network-encoded address strings (e.g., "bc1q...", "1...", "3...")

### Problem 2: Limited Script Support
`Payload::from_script()` in metashrew-support/src/address.rs only recognizes:
```rust
- script.is_p2pkh() -> P2PKH
- script.is_p2sh() -> P2SH  
- script.is_witness_program() -> P2WPKH, P2WSH, P2TR
// Returns error for P2PK and other types
```

### Problem 3: Network Lock-in
- OUTPOINT_SPENDABLE_BY stores: `outpoint -> network_address_string`
- OUTPOINT_SPENDABLE_BY_ADDRESS stores: `network_address_string -> [outpoints]`
- View functions must know the network to query by address

## Proposed Solution: Script-Based Indexing

### Core Principle
**Store and index by script_pubkey, not by network-encoded addresses**

### New Architecture

#### 1. Index Storage Schema

```
OUTPOINT_SPENDABLE_BY: outpoint -> script_pubkey_bytes
OUTPOINT_SPENDABLE_BY_SCRIPT: script_pubkey_bytes -> [outpoints]  
SCRIPT_TYPE_INDEX: script_type -> [outpoints]  (optional optimization)
```

#### 2. Script Type Classification

Extend support to ALL script types:
- **P2PK** (Pay to Public Key): `<pubkey> OP_CHECKSIG`
- **P2PKH** (Pay to Public Key Hash): Standard
- **P2MS** (Pay to Multi-Sig): `M <pubkey1> ... <pubkeyN> N OP_CHECKMULTISIG`
- **P2SH** (Pay to Script Hash): Standard
- **P2WPKH** (Witness v0 PKH): Standard
- **P2WSH** (Witness v0 SH): Standard
- **P2TR** (Witness v1 Taproot): Standard
- **NULL_DATA** (OP_RETURN): Data storage
- **UNKNOWN/NON_STANDARD**: Everything else

#### 3. View Function Interface

Support multiple query modes:

```rust
pub struct SpendableQuery {
    // Option 1: Query by raw script_pubkey (hex string)
    pub script_pubkey_hex: Option<String>,
    
    // Option 2: Query by address string (any network)
    pub address: Option<String>,
    
    // Option 3: Query by public key (hex)
    pub public_key_hex: Option<String>,
}
```

Query resolution:
1. If `script_pubkey_hex` provided -> use directly
2. If `address` provided -> parse with network detection -> extract script_pubkey
3. If `public_key_hex` provided -> derive all possible scripts (P2PK, P2PKH, P2WPKH, etc.)

### Implementation Steps

#### Step 1: Extend metashrew_support::address

```rust
// File: crates/metashrew-support/src/address.rs

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptType {
    P2PK,
    P2PKH,
    P2MS { required: u8, total: u8 },
    P2SH,
    P2WPKH,
    P2WSH,
    P2TR,
    NullData,
    Unknown,
}

impl ScriptType {
    pub fn from_script(script: &Script) -> Self {
        if script.is_p2pk() {
            ScriptType::P2PK
        } else if script.is_p2pkh() {
            ScriptType::P2PKH
        } else if script.is_p2sh() {
            ScriptType::P2SH
        } else if let Some(wit_prog) = script.witness_program() {
            match wit_prog.version() {
                WitnessVersion::V0 => {
                    if wit_prog.program().len() == 20 {
                        ScriptType::P2WPKH
                    } else {
                        ScriptType::P2WSH
                    }
                }
                WitnessVersion::V1 => ScriptType::P2TR,
                _ => ScriptType::Unknown,
            }
        } else if script.is_op_return() {
            ScriptType::NullData
        } else if let Some((req, total)) = Self::is_p2ms(script) {
            ScriptType::P2MS { required: req, total }
        } else {
            ScriptType::Unknown
        }
    }
    
    fn is_p2ms(script: &Script) -> Option<(u8, u8)> {
        // Parse M-of-N multisig pattern
        // Returns Some((M, N)) if valid multisig
        todo!("Implement multisig detection")
    }
}

pub struct ScriptPubkeyInfo {
    pub script: ScriptBuf,
    pub script_type: ScriptType,
    pub is_spendable: bool,
}

impl ScriptPubkeyInfo {
    pub fn from_script(script: &Script) -> Self {
        let script_type = ScriptType::from_script(script);
        let is_spendable = !matches!(script_type, ScriptType::NullData | ScriptType::Unknown);
        
        Self {
            script: script.to_owned(),
            script_type,
            is_spendable,
        }
    }
}
```

#### Step 2: Add Network Detection and Address Parsing

```rust
// File: crates/metashrew-support/src/address.rs

#[derive(Debug, Clone, Copy)]
pub enum DetectedNetwork {
    BitcoinMainnet,
    BitcoinTestnet,
    ZcashMainnet,
    ZcashTestnet,
    DogecoinMainnet,
    Unknown(u8), // Stores the prefix byte
}

pub fn detect_network_from_address(address: &str) -> Result<(DetectedNetwork, ScriptBuf)> {
    // Try base58 decoding
    if let Ok((version, payload)) = base58::decode_check(address) {
        let network = match version {
            0x00 => DetectedNetwork::BitcoinMainnet,  // BTC P2PKH
            0x05 => DetectedNetwork::BitcoinMainnet,  // BTC P2SH
            0x6f => DetectedNetwork::BitcoinTestnet,  // BTC Testnet P2PKH
            0xc4 => DetectedNetwork::BitcoinTestnet,  // BTC Testnet P2SH
            0x1c => DetectedNetwork::ZcashMainnet,    // ZEC t1 (P2PKH)
            0x1d => DetectedNetwork::ZcashMainnet,    // ZEC t3 (P2SH)
            0x1e => DetectedNetwork::DogecoinMainnet, // DOGE P2PKH
            v => DetectedNetwork::Unknown(v),
        };
        
        let script = if payload.len() == 20 {
            if version == 0x05 || version == 0xc4 || version == 0x1d {
                ScriptBuf::new_p2sh(&ScriptHash::from_slice(&payload)?)
            } else {
                ScriptBuf::new_p2pkh(&PubkeyHash::from_slice(&payload)?)
            }
        } else {
            return Err(anyhow!("Invalid payload length"));
        };
        
        return Ok((network, script));
    }
    
    // Try bech32 decoding
    if let Ok((hrp, version, program)) = bech32::segwit::decode(address) {
        let network = match hrp.as_str() {
            "bc" => DetectedNetwork::BitcoinMainnet,
            "tb" => DetectedNetwork::BitcoinTestnet,
            "bcrt" => DetectedNetwork::BitcoinTestnet,
            _ => DetectedNetwork::Unknown(0xff),
        };
        
        let witness_version = WitnessVersion::try_from(version)?;
        let witness_program = WitnessProgram::new(witness_version, &program)?;
        let script = ScriptBuf::new_witness_program(&witness_program);
        
        return Ok((network, script));
    }
    
    Err(anyhow!("Failed to decode address"))
}
```

#### Step 3: Refactor Protorune Indexing

```rust
// File: crates/protorune/src/lib.rs

pub fn index_spendables_script_based(
    txdata: &Vec<Transaction>
) -> Result<BTreeSet<Vec<u8>>> {
    #[cfg(feature = "cache")]
    let mut updated_scripts: BTreeSet<Vec<u8>> = BTreeSet::new();
    
    #[cfg(not(feature = "cache"))]
    let updated_scripts: BTreeSet<Vec<u8>> = BTreeSet::new();
    
    for (txindex, transaction) in txdata.iter().enumerate() {
        let tx_id = transaction.compute_txid();
        
        // Mark inputs as spent (remove from spendable)
        for input in transaction.inputs().iter() {
            let outpoint_bytes = consensus_encode(&input.previous_output)?;
            
            // Get the script_pubkey this outpoint was locked to
            let script_bytes = tables::OUTPOINT_SPENDABLE_BY
                .select(&outpoint_bytes)
                .get();
            
            if !script_bytes.is_empty() {
                #[cfg(feature = "cache")]
                updated_scripts.insert(script_bytes.to_vec());
                
                // Remove from script -> outpoint index
                let pos: u32 = tables::OUTPOINT_SPENDABLE_BY_SCRIPT
                    .select(&script_bytes)
                    .select(&outpoint_bytes)
                    .get_value();
                
                tables::OUTPOINT_SPENDABLE_BY_SCRIPT
                    .select(&script_bytes)
                    .delete_value(pos);
                
                // Nullify the outpoint -> script mapping
                tables::OUTPOINT_SPENDABLE_BY
                    .select(&outpoint_bytes)
                    .nullify();
            }
        }
        
        // Index new outputs
        for (vout, output) in transaction.outputs().iter().enumerate() {
            let outpoint = OutPoint {
                txid: tx_id,
                vout: vout as u32,
            };
            let outpoint_bytes = consensus_encode(&outpoint)?;
            
            let script_pubkey = &output.script_pubkey;
            let script_bytes = script_pubkey.as_bytes();
            
            // Skip OP_RETURN and other non-spendable scripts
            let script_info = ScriptPubkeyInfo::from_script(script_pubkey);
            if !script_info.is_spendable {
                continue;
            }
            
            #[cfg(feature = "cache")]
            updated_scripts.insert(script_bytes.to_vec());
            
            // Store: outpoint -> script_pubkey
            tables::OUTPOINT_SPENDABLE_BY
                .select(&outpoint_bytes)
                .set(Arc::new(script_bytes.to_vec()));
            
            // Store: script_pubkey -> [outpoints]
            tables::OUTPOINT_SPENDABLE_BY_SCRIPT
                .select(script_bytes)
                .append_ll(Arc::new(outpoint_bytes.clone()));
            
            let pos = tables::OUTPOINT_SPENDABLE_BY_SCRIPT
                .select(script_bytes)
                .length() - 1;
            
            // Store reverse lookup: outpoint -> position in script's list
            tables::OUTPOINT_SPENDABLE_BY_SCRIPT
                .select(&outpoint_bytes)
                .set_value(pos);
        }
    }
    
    Ok(updated_scripts)
}
```

#### Step 4: Update View Functions

```rust
// File: crates/protorune/src/view.rs

pub fn spendables_by_query(query: &SpendableQuery) -> Result<Vec<OutpointResponse>> {
    let script_pubkey = if let Some(ref hex) = query.script_pubkey_hex {
        // Direct script_pubkey query
        hex::decode(hex)?
    } else if let Some(ref address) = query.address {
        // Parse address to get script_pubkey (network-agnostic)
        let (_network, script) = detect_network_from_address(address)?;
        script.as_bytes().to_vec()
    } else if let Some(ref pubkey_hex) = query.public_key_hex {
        // Generate script_pubkey from public key
        // Could generate P2PK, P2PKH, P2WPKH depending on query.script_type
        todo!("Implement pubkey -> script derivation")
    } else {
        return Err(anyhow!("No query parameter provided"));
    };
    
    // Query by script_pubkey
    let outpoints = tables::OUTPOINT_SPENDABLE_BY_SCRIPT
        .select(&script_pubkey)
        .map_ll(|ptr, _| {
            let mut cursor = Cursor::new(ptr.get().as_ref());
            let outpoint: OutPoint = consensus_decode(&mut cursor)?;
            Ok(outpoint_to_response(&outpoint))
        })
        .collect::<Result<Vec<_>>>()?;
    
    Ok(outpoints)
}
```

### Migration Strategy

#### Phase 1: Add New Tables (Parallel Mode)
- Deploy with both old and new indexing
- Write to both OUTPOINT_SPENDABLE_BY (address) and OUTPOINT_SPENDABLE_BY_SCRIPT
- Old queries use old tables
- New queries use new tables

#### Phase 2: Backfill Historical Data
- Scan blockchain from genesis
- For each output:
  - Read address from OUTPOINT_SPENDABLE_BY
  - Parse to script_pubkey
  - Write to OUTPOINT_SPENDABLE_BY_SCRIPT

#### Phase 3: Switch to Script-Only Mode
- Update all indexing to use script-based approach
- Deprecate address-based tables
- Update all view functions

### Benefits

1. **Network Agnostic**: Same indexing works for BTC, ZEC, DOGE, etc.
2. **Complete Coverage**: Indexes P2PK, multisig, and all script types
3. **Flexible Queries**: Query by script, address, or public key
4. **Future Proof**: Supports new address formats without code changes
5. **No Configuration**: No need to set network parameters for indexing

### Compatibility

Old address-based queries can be supported via wrapper:
```rust
pub fn spendables_by_address_legacy(address: &str) -> Result<Vec<OutpointResponse>> {
    spendables_by_query(&SpendableQuery {
        address: Some(address.to_string()),
        script_pubkey_hex: None,
        public_key_hex: None,
    })
}
```
