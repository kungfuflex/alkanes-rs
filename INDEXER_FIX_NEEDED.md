# 🔍 Indexer Fix Required for Futures

## Problem Summary

The `generatefuture` RPC correctly creates protostones with cellpack [32, 0, 77] in the coinbase, but the indexer doesn't deploy individual future contracts at [31:N].

## Current Behavior

### ✅ What Works:
1. Bitcoin Core `generatefuture` RPC creates blocks with protostones
2. Coinbase has 3 outputs (payment + witness + protostone)
3. Protostone correctly formatted: `6a5d090200000101a080b402`
4. Master ftrBTC contract at [31, 0] is initialized

### ❌ What's Missing:
1. Futures at [31:N] have 0 bytes bytecode
2. No contracts deployed when protostone is processed
3. Can't claim or trade futures that don't exist

## Root Cause Analysis

### Current Indexer Code

**File:** `crates/alkanes/src/network.rs`

```rust
pub fn setup_ftrbtc(block: &Block) -> Result<()> {
    // ftrBTC uses alkane ID [31, 0] - reserved for futures master contract
    let ftr_btc_id = AlkaneId { block: 31, tx: 0 };
    
    let mut ptr =
        IndexPointer::from_keyword("/alkanes/").select(&ftr_btc_id.into());
    if ptr.get().len() == 0 {
        ptr.set(Arc::new(compress(ftr_btc_build::get_bytes())?));
    } else {
        return Ok(());
    }
    Ok(())
}
```

**Problem:** This function only initializes [31, 0] once at genesis. It doesn't:
- Check for protostones with cellpack [32, 0, 77]
- Deploy contracts at [31:N] where N is block height
- Process the protostone message to create futures

### Expected Behavior

When a protostone with cellpack [32, 0, 77] appears in the coinbase:
1. Indexer should detect the protostone
2. Extract block height N from context
3. Deploy future contract at [31:N]
4. Set bytecode for [31:N] to the ftrBTC contract code
5. Initialize storage for the future

## Solution Needed

### Option 1: Modify `setup_ftrbtc`

Add logic to check every block for future-generating protostones:

```rust
pub fn setup_ftrbtc(block: &Block, height: u32) -> Result<()> {
    // Initialize master contract at [31, 0] (only once)
    let ftr_btc_id = AlkaneId { block: 31, tx: 0 };
    let mut ptr = IndexPointer::from_keyword("/alkanes/").select(&ftr_btc_id.into());
    if ptr.get().len() == 0 {
        ptr.set(Arc::new(compress(ftr_btc_build::get_bytes())?));
    }
    
    // NEW: Check coinbase for future-generating protostone
    if let Some(coinbase) = block.txdata.first() {
        if let Some(Artifact::Runestone(ref runestone)) = Runestone::decipher(coinbase) {
            let protostones = Protostone::from_runestone(runestone)?;
            
            for protostone in protostones {
                // Check if this is a future-generating protostone
                // Cellpack [32, 0, 77] should be in the message
                let calldata: Vec<u8> = protostone.message
                    .iter()
                    .flat_map(|v| v.to_be_bytes())
                    .collect();
                    
                if !calldata.is_empty() {
                    let varint_list = decode_varint_list(&mut Cursor::new(calldata))?;
                    
                    // Check for cellpack [32, 0, 77]
                    if varint_list.len() >= 2 && 
                       varint_list[0] == 32 && 
                       varint_list[1] == 0 &&
                       varint_list.get(2) == Some(&77) {
                        
                        // Deploy future contract at [31:height]
                        let future_id = AlkaneId { block: 31, tx: height };
                        let mut future_ptr = IndexPointer::from_keyword("/alkanes/")
                            .select(&future_id.into());
                        
                        if future_ptr.get().len() == 0 {
                            // Deploy the same ftrBTC contract code
                            future_ptr.set(Arc::new(compress(ftr_btc_build::get_bytes())?));
                            
                            // TODO: Initialize storage for this specific future
                            // - Set strike price based on height
                            // - Set expiry block
                            // - Set initial supply
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}
```

### Option 2: Add Dedicated Future Deployment Handler

Create a new function that specifically handles future deployment:

```rust
pub fn deploy_futures_from_protostones(block: &Block, height: u32) -> Result<()> {
    // Only check coinbase transaction
    let coinbase = match block.txdata.first() {
        Some(tx) => tx,
        None => return Ok(()),
    };
    
    // Parse runestone from coinbase
    let runestone = match Runestone::decipher(coinbase) {
        Some(Artifact::Runestone(r)) => r,
        _ => return Ok(()),
    };
    
    // Extract protostones
    let protostones = Protostone::from_runestone(&runestone)?;
    
    for protostone in protostones {
        // Only process protostones with protocol_tag = 1 (alkanes)
        if protostone.protocol_tag != 1 {
            continue;
        }
        
        // Decode the message (cellpack)
        let calldata: Vec<u8> = protostone.message
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
            
        if calldata.is_empty() {
            continue;
        }
        
        let varint_list = decode_varint_list(&mut Cursor::new(calldata))?;
        
        // Check for cellpack [32, 0, 77] - future generation cellpack
        if varint_list.len() >= 3 &&
           varint_list[0] == 32 &&
           varint_list[1] == 0 &&
           varint_list[2] == 77 {
            
            println!("Deploying future at block {}", height);
            
            // Deploy future contract at [31:height]
            let future_id = AlkaneId { block: 31, tx: height };
            let mut future_ptr = IndexPointer::from_keyword("/alkanes/")
                .select(&future_id.into());
            
            // Set the bytecode (same as master ftrBTC contract)
            future_ptr.set(Arc::new(compress(ftr_btc_build::get_bytes())?));
            
            // Initialize the future contract by calling its init function
            let mut atomic = AtomicPointer::default();
            let parcel = MessageContextParcel {
                atomic: atomic.derive(&IndexPointer::default()),
                runes: vec![],
                transaction: coinbase.clone(),
                block: block.clone(),
                height: height as u128,
                pointer: 0,
                refund_pointer: 0,
                calldata: vec![0], // Call init (opcode 0)
                sheets: Box::new(BalanceSheet::default()),
                txindex: 0,
                vout: 0,
                runtime_balances: Box::new(BalanceSheet::default()),
            };
            
            match simulate_parcel(&parcel, u64::MAX) {
                Ok((response, _gas)) => {
                    pipe_storagemap_to(
                        &response.storage,
                        &mut atomic.derive(&IndexPointer::from_keyword("/alkanes/")
                            .select(&future_id.into())),
                    );
                    atomic.commit();
                    println!("Future {} deployed successfully", height);
                }
                Err(e) => {
                    println!("Error deploying future: {:?}", e);
                }
            }
        }
    }
    
    Ok(())
}
```

Then call it in `index_block`:

```rust
pub fn index_block<B: BlockLike>(block: &B, height: u32) -> Result<()> {
    // ... existing setup code ...
    
    setup_diesel(&bitcoin_block)?;
    setup_frsigil(&bitcoin_block)?;
    setup_frbtc(&bitcoin_block)?;
    setup_ftrbtc(&bitcoin_block)?;
    deploy_futures_from_protostones(&bitcoin_block, height)?;  // NEW!
    check_and_upgrade_precompiled(height)?;
    
    // ... rest of indexing ...
}
```

## Files to Modify

1. **`crates/alkanes/src/network.rs`**
   - Modify `setup_ftrbtc` OR
   - Add `deploy_futures_from_protostones`

2. **`crates/alkanes/src/indexer.rs`**
   - Call the new function in `index_block`

## Testing Steps

After implementing the fix:

### 1. Rebuild WASM
```bash
cd ~/alkanes-rs
./build-wasm.sh
```

### 2. Restart Services
```bash
docker-compose down
docker volume rm alkanes-rs_metashrew-data
docker-compose up -d
```

### 3. Generate a Future
```bash
./target/release/alkanes-cli -p regtest bitcoind generatefuture
```

### 4. Verify Future Has Bytecode
```bash
sleep 5
BLOCK=$(./target/release/alkanes-cli -p regtest bitcoind getblockcount)
./target/release/alkanes-cli -p regtest alkanes inspect 31:$BLOCK
```

**Expected:** `Bytecode Length: > 0 bytes` ✅

### 5. Test in UI
```bash
cd ~/subfrost-app
yarn dev
# Open http://localhost:3000/futures
# Click "Generate Future"
# See REAL futures with data!
```

## Additional Considerations

### 1. Future Contract Initialization

The future contract may need specific initialization:
- Strike price (based on current BTC price or height)
- Expiry block (N + some offset)
- Initial token supply
- Underlying asset reference (frBTC)

### 2. Storage Layout

Each future at [31:N] needs:
- Its own storage space
- Balance tracking
- Claim status
- Expiry information

### 3. Claiming Mechanism

When cellpack [31, 0, 14] is sent:
- Should claim ALL pending futures
- Transfer them to the sender
- Mark them as claimed

This might need additional indexer logic to:
- Track unclaimed futures
- Process claim transactions
- Update balances

## Summary

**Current Status:** Protostones are created ✅, but futures aren't deployed ❌

**Fix Needed:** Add protostone detection and future deployment to indexer

**Complexity:** Medium - requires understanding of:
- Protostone parsing
- Contract deployment
- Storage initialization

**Estimated Effort:** 2-4 hours for someone familiar with the codebase

**Impact:** HIGH - Enables complete futures functionality

---

## Quick Reference

### Protostone Format
```
Hex: 6a5d090200000101a080b402

Decoded:
- 6a = OP_RETURN
- 5d09 = PUSHDATA2, 9 bytes  
- 02 00 = Pointer field (tag=2, value=0)
- 00 = Protocol field (tag=0)
- 01 01 a0 80 b4 02 = Cellpack [32, 0, 77] as varints
```

### Cellpack [32, 0, 77] Means:
- Target: [32, 0] (frBTC signer contract)
- Inputs: [77] (future generation opcode)

### Expected Result:
- Deploy contract at [31:height]
- Set bytecode from `ftr_btc_build::get_bytes()`
- Initialize with strike/expiry data
- Make claimable via [31, 0, 14]

---

**This document provides the complete roadmap for fixing futures deployment in the indexer.**
