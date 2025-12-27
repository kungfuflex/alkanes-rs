# Chain Validation and Continuity Tracking

## Overview

The sync engine now validates blockchain continuity as it processes blocks, ensuring that each new block properly connects to the previous block. This provides early detection of chain inconsistencies, reorgs, and indexing errors.

## How It Works

### Chain Continuity Validation

Before processing each block, the sync engine performs the following validation:

1. **Deserialize the block** to access its header
2. **Extract `prev_blockhash`** from the block header
3. **Retrieve stored hash** of the previous block from storage
4. **Compare hashes**: Ensure `block[N].header.prev_blockhash == stored_hash[N-1]`

### Implementation

Located in `crates/metashrew-sync/src/sync.rs`:

```rust
async fn validate_block_connects(&self, height: u32, block_data: &[u8]) -> SyncResult<bool> {
    // Genesis block has no previous block
    if height == 0 {
        return Ok(true);
    }

    // Decode the block to get prev_blockhash
    let block: bitcoin::Block = bitcoin::consensus::deserialize(block_data)?;
    let block_prev_hash: Vec<u8> = block.header.prev_blockhash.to_byte_array().to_vec();

    // Get the stored hash of the previous block
    let stored_prev_hash = self.storage.get_block_hash(height - 1).await?;

    match stored_prev_hash {
        Some(stored_hash) => {
            if stored_hash != block_prev_hash {
                error!(
                    "⚠ CHAIN DISCONTINUITY at height {}: Block's prev_blockhash {} does not match stored hash {} of block {}",
                    height,
                    hex::encode(&block_prev_hash),
                    hex::encode(&stored_hash),
                    height - 1
                );
                Ok(false)
            } else {
                debug!("✓ Block {} connects to previous block {}", height, height - 1);
                Ok(true)
            }
        }
        None => {
            warn!("No stored hash for block {} - unable to validate", height - 1);
            Ok(true) // Allow processing to continue
        }
    }
}
```

### Integration

The validation is called at the start of `process_block()`:

```rust
pub async fn process_block(&self, height: u32, block_data: Vec<u8>, block_hash: Vec<u8>) -> SyncResult<()> {
    // Validate that this block connects to the previous one
    if !self.validate_block_connects(height, &block_data).await? {
        return Err(SyncError::BlockProcessing {
            height,
            message: format!(
                "Block does not connect to previous block - possible reorg or chain inconsistency"
            ),
        });
    }

    // Continue with normal processing...
}
```

## Benefits

### 1. **Early Reorg Detection**
Instead of discovering reorgs reactively by comparing stored hashes with the node, we detect chain discontinuity immediately when processing a block that doesn't connect.

### 2. **Clear Error Messages**
When a discontinuity is detected, the logs show:
```
⚠ CHAIN DISCONTINUITY at height 925000: Block's prev_blockhash a1b2c3... does not match stored hash d4e5f6... of block 924999
```

This immediately tells you:
- At what height the issue occurred
- What hash was expected (from the previous block)
- What hash was found (in the new block's header)

### 3. **Prevents Processing Wrong Blocks**
If the indexer somehow receives a block from a different chain fork, it will reject it immediately rather than processing it and corrupting the index.

### 4. **Tracks Longest Chain**
By validating the chain progression (blockhash → next blockhash), the indexer ensures it's following a valid blockchain and not jumping between forks.

## Example Scenario

### Normal Operation
```
Block 999:  hash = 0xaabbcc...
Block 1000: prev_blockhash = 0xaabbcc...  ✓ Connects!
Block 1001: prev_blockhash = 0xddeeff... ✓ Connects to block 1000
```

### Detected Reorg
```
Indexed:
  Block 999: hash = 0xaabbcc...
  Block 1000: hash = 0xddeeff...

Node sends (after reorg):
  Block 1000: prev_blockhash = 0x112233... ✗ DISCONTINUITY!
                                           Expected 0xaabbcc but got 0x112233

Indexer rejects block and triggers reorg handling
```

## Complete Rollback Flow

When chain discontinuity is detected, the system automatically triggers rollback:

### 1. **Chain Validation Fails**
```
Block 1000 being processed:
  prev_blockhash = 0x112233...

Stored block 999:
  hash = 0xaabbcc...

✗ Mismatch detected! Error returned.
```

### 2. **Error Handling Detects Chain Issue**
```rust
// In pipeline error handler (sync.rs:579-602)
if error.contains("does not connect to previous block") {
    warn!("Chain discontinuity at height {}. Triggering reorg.", height);

    // Trigger reorg handler
    handle_reorg(height, node, storage, runtime, config).await
}
```

### 3. **Reorg Handler Finds Common Ancestor**
```rust
// In handle_reorg (sync.rs:788-849)
let mut check_height = current_height.saturating_sub(1);

// Scan backwards up to max_reorg_depth
while check_height > 0 && check_height >= current_height - max_reorg_depth {
    let local_hash = storage.get_block_hash(check_height).await?;
    let remote_hash = node.get_block_hash(check_height).await?;

    if local_hash == remote_hash {
        break; // Common ancestor found at check_height
    }

    check_height -= 1;
}
```

### 4. **Storage Rollback**
```rust
// Rollback to common ancestor
storage.rollback_to_height(rollback_height).await?;
```

The storage adapter deletes:
- Block hashes after rollback_height
- State roots after rollback_height
- Indexed height set back to rollback_height
- All indexer data after rollback_height (via runtime refresh)

### 5. **Runtime Memory Refresh**
```rust
// Clear runtime memory/caches
runtime.refresh_memory().await?;
```

### 6. **Resume from Rollback Point**
```rust
// Set current height to resume processing
current_height.store(rollback_height + 1, Ordering::SeqCst);
info!("Rolled back to {}. Resuming sync.", rollback_height);
```

### Complete Example

```
Initial state:
  ✓ Block 998: hash 0xaa...
  ✓ Block 999: hash 0xbb...
  ✓ Block 1000: hash 0xcc...

Reorg occurs on network:
  Block 999': hash 0xdd... (different chain)
  Block 1000': prev = 0xdd... (new chain)

Indexer tries to process block 1000':
  ✗ Validation fails: prev 0xdd... != stored 0xbb...

Automatic rollback triggered:
  → Scan backwards: Check 999
  → Local hash: 0xbb..., Remote hash: 0xdd... (mismatch)
  → Scan backwards: Check 998
  → Local hash: 0xaa..., Remote hash: 0xaa... (MATCH!)
  → Common ancestor found at 998

Rollback executed:
  → Delete blocks 999, 1000 from storage
  → Delete state roots 999, 1000
  → Refresh runtime memory
  → Set current height = 999

Resume processing:
  ✓ Process block 999' (new chain)
  ✓ Process block 1000' (new chain)
  ✓ Continue syncing on correct chain
```

## Complementary with Existing Reorg Detection

This validation works alongside the existing reorg detection mechanism:

1. **Forward Validation** (NEW): Validates each block connects to previous
   - Detects issues immediately during processing
   - Prevents processing invalid blocks
   - **Automatically triggers rollback via error handling**

2. **Backward Validation** (EXISTING): Compares stored hashes with node
   - Scans backwards to find common ancestor
   - Handles rollback and reprocessing
   - Called both proactively (near tip) and reactively (on validation failure)

Together, these provide comprehensive protection against chain inconsistencies.

## Logging Levels

- **DEBUG**: Successful chain continuity validation
- **WARN**: Missing previous block hash (allows processing to continue)
- **ERROR**: Chain discontinuity detected (blocks processing)

## Performance Impact

Minimal - the validation:
- Deserializes the block header (already needed for processing)
- Performs one storage lookup (previous block hash)
- Compares two 32-byte hashes

The overhead is negligible compared to full block processing.

## Future Enhancements

Potential improvements:

1. **Automatic Reorg Trigger**: When discontinuity is detected, automatically trigger reorg handling instead of just rejecting the block

2. **Chain Tip Tracking**: Store the current chain tip hash separately for quick validation

3. **Fork Detection**: Track alternative chain tips to detect and handle forks more intelligently

4. **Validation Stats**: Track how often validation fails to identify node instability or network issues
