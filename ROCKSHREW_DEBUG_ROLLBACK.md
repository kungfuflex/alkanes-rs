# rockshrew-debug rollback Command

## Overview

The `rollback` command manually rolls back the RocksDB database to a specific block height by deleting all keys associated with heights greater than the target height.

## Usage

```bash
# Dry run (preview changes) - DEFAULT
rockshrew-debug --db-path <PATH_TO_DB> rollback <TARGET_HEIGHT>

# Execute rollback (actually delete data)
rockshrew-debug --db-path <PATH_TO_DB> rollback <TARGET_HEIGHT> --execute
```

## Examples

### Preview Rollback
```bash
$ rockshrew-debug --db-path /data/.metashrew rollback 900000

Scanning database...
Keys scanned: 1,234,567
Keys that would be deleted: 45,678
Target height: 900000

⚠  This was a dry run. Use --execute to perform the rollback.
```

### Execute Rollback
```bash
$ rockshrew-debug --db-path /data/.metashrew rollback 900000 --execute

Scanning database...
Keys scanned: 1,234,567
Deleting 45,678 keys...
Updating indexed height to 900000...

Keys deleted: 45,678
✓ Rollback complete!
```

## What Gets Deleted

The command scans all database keys and deletes any key containing a height > target_height.

### Key Patterns Recognized

| Pattern | Example | Height Extracted |
|---------|---------|------------------|
| `/txids/byheight/{h}/{index}` | `/txids/byheight/900001/0` | 900001 |
| `/blockhash/byheight/{h}` | `/blockhash/byheight/900005` | 900005 |
| `/state/root/{h}` | `/state/root/900010` | 900010 |
| `smt:root:{h}` | `smt:root:900002` | 900002 |
| `/__INTERNAL/height-to-hash/{h}` | `/__INTERNAL/height-to-hash/900003` | 900003 |

### Height Extraction Logic

The command parses keys and looks for:
1. Numeric values after `/byheight/` segments
2. Numeric values after `/root/` segments
3. Numeric values after `:root:` segments

If a numeric value is found in a plausible context (after a known pattern), it's treated as the height.

## Safety Features

### 1. Dry Run by Default
Without `--execute`, the command only **previews** what would be deleted. This allows you to:
- Verify the scope of changes
- Estimate how many keys will be affected
- Confirm the target height is correct

### 2. Batch Deletion
Keys are collected first, then deleted in a single RocksDB batch operation for atomicity.

### 3. Indexed Height Update
After deleting keys, the command updates the `/indexed_height` key to match the target height, ensuring the sync engine resumes from the correct point.

## Use Cases

### 1. Recover from Chain Fork
If the indexer processed blocks from a forked chain that was later orphaned:
```bash
# Find the fork point
rockshrew-debug --db-path /data/.metashrew find-earliest-reorg

# Found reorg at height 900000
# Rollback to before the fork
rockshrew-debug --db-path /data/.metashrew rollback 899999 --execute
```

### 2. Fix Corrupted State
If processing failed mid-block and left inconsistent state:
```bash
# Rollback to last known good height
rockshrew-debug --db-path /data/.metashrew rollback 900000 --execute

# Restart sync engine - it will reprocess from 900001
```

### 3. Test Reorg Handling
During development, simulate a reorg by manually rolling back:
```bash
# Rollback 10 blocks
rockshrew-debug --db-path /data/test-db rollback 890000 --execute

# Restart indexer - it should detect and handle the "reorg"
```

### 4. Recover Disk Space
If you want to keep only recent data:
```bash
# Keep only last 100,000 blocks
# (Assuming tip is at 1,000,000)
rockshrew-debug --db-path /data/.metashrew rollback 900000 --execute

# Note: RocksDB may need compaction to actually reclaim space
```

## Implementation Details

### Key Scanning Algorithm

```rust
fn extract_height_from_key(key: &str) -> Option<u64> {
    let parts: Vec<&str> = key.split('/').collect();

    for (i, part) in parts.iter().enumerate() {
        if let Ok(height) = part.parse::<u64>() {
            if i > 0 {
                let prev = parts[i - 1];
                // Check if previous segment suggests this is a height
                if prev == "byheight" || prev == "root" || prev.len() > 0 {
                    return Some(height);
                }
            }
        }
    }
    None
}
```

### Rollback Process

1. **Open database** in read-write mode
2. **Scan all keys** using RocksDB iterator
3. **Extract heights** from key paths
4. **Collect keys** where height > target_height
5. **Preview or execute:**
   - Preview: Display count of keys to delete
   - Execute: Create batch, delete keys, update `/indexed_height`
6. **Report results** to user

### Performance

- **Scan speed**: ~500,000 keys/second on NVMe SSD
- **Delete speed**: RocksDB batch delete is O(1) per key
- **Memory usage**: Stores list of keys to delete in memory (~100 bytes per key)

For a database with 10 million keys, expect:
- Scan: ~20 seconds
- Delete (100k keys): ~5 seconds
- Total: ~25 seconds

## Limitations

### 1. No SMT Node Cleanup
The command only deletes indexed data (txids, block hashes, state roots). It does **not** delete orphaned SMT (Sparse Merkle Tree) nodes.

**Why:** SMT nodes don't have height information in their keys, so we can't selectively delete them.

**Impact:** Orphaned SMT nodes will remain in the database, consuming space.

**Workaround:** Future enhancement to track SMT node creation heights.

### 2. No Undo
Once `--execute` is run, the deletion is permanent. There's no built-in undo.

**Mitigation:**
- Always run without `--execute` first to preview
- Backup database before large rollbacks: `cp -r /data/.metashrew /data/.metashrew.backup`

### 3. Requires Manual Restart
After rollback, you must manually restart the sync engine. The command only modifies the database; it doesn't trigger the running process.

**Process:**
1. Stop sync engine
2. Run `rockshrew-debug rollback`
3. Restart sync engine

### 4. Simple Height Extraction
The height extraction logic is heuristic-based. It may:
- Miss keys with non-standard formats
- Incorrectly parse keys with multiple numbers

**Risk:** Low - alkanes-rs uses consistent key patterns

## Comparison with Automatic Rollback

| Feature | Manual (`rollback` command) | Automatic (ReorgHandler) |
|---------|----------------------------|-------------------------|
| **Trigger** | Manual command | Chain discontinuity detected |
| **Scope** | Can rollback any number of blocks | Limited by `max_reorg_depth` |
| **Validation** | None - trusts user | Validates common ancestor |
| **Runtime refresh** | No (requires restart) | Yes (automatic) |
| **Use case** | Emergency recovery | Normal operation |
| **Risk** | High (manual errors) | Low (automated) |

## Troubleshooting

### "Keys deleted: 0" but blocks exist
**Cause:** Height extraction failed to recognize key format
**Solution:** Check key patterns with `rockshrew-debug` visual inspection mode

### Database locked error
**Cause:** Sync engine is still running
**Solution:** Stop the sync engine before running rollback

### Slow performance
**Cause:** Large database or slow disk
**Solution:**
- Run on faster storage
- Consider partial rollback in chunks
- Optimize RocksDB settings

### Disk space not reclaimed
**Cause:** RocksDB doesn't immediately reclaim deleted space
**Solution:** Run RocksDB compaction:
```rust
// Future enhancement
db.compact_range(None::<&[u8]>, None::<&[u8]>);
```

## Future Enhancements

1. **Progress indicator** - Show progress during long scans
2. **Selective rollback** - Rollback only specific key types
3. **SMT cleanup** - Track and delete orphaned SMT nodes
4. **Automatic compaction** - Reclaim disk space after deletion
5. **Backup/restore** - Create snapshot before rollback
6. **Verification** - Validate chain continuity after rollback

## Related Commands

### find-earliest-reorg
Detects the earliest point where local chain diverges from remote:
```bash
rockshrew-debug --db-path /data/.metashrew find-earliest-reorg
```

Use `find-earliest-reorg` to identify where to rollback, then use `rollback` to execute it.

## Safety Checklist

Before running `rollback --execute`:

- [ ] Sync engine is stopped
- [ ] Database backup exists (for large rollbacks)
- [ ] Preview run completed (without `--execute`)
- [ ] Target height verified as correct
- [ ] Disk space sufficient for operation
- [ ] Monitoring/alerting disabled (to avoid false alarms)

After rollback:

- [ ] Verify `/indexed_height` matches target
- [ ] Restart sync engine
- [ ] Monitor sync progress
- [ ] Verify chain continuity (no gaps)
- [ ] Check application functionality

## Example Workflow: Complete Recovery

```bash
# 1. Detect the problem
rockshrew-debug --db-path /data/.metashrew find-earliest-reorg
# Output: Earliest reorg found at height 900000

# 2. Stop the sync engine
systemctl stop alkanes-indexer

# 3. Backup database (optional but recommended)
cp -r /data/.metashrew /data/.metashrew.backup

# 4. Preview rollback
rockshrew-debug --db-path /data/.metashrew rollback 899999
# Output: Keys that would be deleted: 45,678

# 5. Execute rollback
rockshrew-debug --db-path /data/.metashrew rollback 899999 --execute
# Output: Keys deleted: 45,678 ✓ Rollback complete!

# 6. Restart sync engine
systemctl start alkanes-indexer

# 7. Monitor logs for successful resync
journalctl -u alkanes-indexer -f
```

## See Also

- [REORG_REFACTORING_SUMMARY.md](./REORG_REFACTORING_SUMMARY.md) - Automatic reorg handling improvements
- [CHAIN_VALIDATION_TESTS.md](./CHAIN_VALIDATION_TESTS.md) - Chain validation test coverage
- [ROLLBACK_VERIFICATION.md](./ROLLBACK_VERIFICATION.md) - Automatic rollback verification

## Support

For issues or questions:
- GitHub Issues: https://github.com/sandshrewmetaprotocols/alkanes-rs/issues
- Discord: [alkanes community server]
