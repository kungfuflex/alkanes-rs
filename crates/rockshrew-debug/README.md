# rockshrew-debug

Debug utilities for Rockshrew indexer to detect and diagnose issues.

## Features

### Find Earliest Reorg

Scans backwards through the blockchain index to find blocks that were processed multiple times without proper rollback, indicating a missed reorg.

**How it works:**
- Scans `HEIGHT_TO_TRANSACTION_IDS` table for duplicate transaction IDs
- Works backwards from the current tip to a specified height
- Displays real-time progress with a terminal UI
- Shows scanning speed, ETA, and earliest reorg found

**Usage:**

```bash
rockshrew-debug --db-path /path/to/.metashrew find-earliest-reorg --exit-at 900000
```

**Example:**

```bash
# Scan from current tip down to block 900000
cargo run -p rockshrew-debug --release -- \
  --db-path /data/.metashrew \
  find-earliest-reorg \
  --exit-at 900000
```

**Interactive Controls:**
- Press `q` to quit

**Output:**
The TUI displays:
- **Current Height**: Block currently being scanned
- **Exit At**: Target height where scanning will stop
- **Blocks Scanned**: Number of blocks checked
- **Speed**: Blocks scanned per second
- **Elapsed**: Time since scan started
- **ETA**: Estimated time to completion
- **Earliest Reorg Found**: Details about the earliest detected reorg (height, duplicate count)

When complete, prints a summary:
```
=== Scan Complete ===
Blocks scanned: 50000

⚠ EARLIEST REORG DETECTED:
  Height: 925000
  Total TXIDs: 4500
  Unique TXIDs: 2250
  Duplicates: 2250
```

## Building

```bash
cargo build -p rockshrew-debug --release
```

The binary will be at `target/release/rockshrew-debug`.

## How Reorg Detection Works

When a blockchain reorg occurs but the indexer doesn't properly roll back:

1. Block at height N is processed → txids appended to `HEIGHT_TO_TRANSACTION_IDS[N]`
2. Reorg happens, but rollback fails
3. Block at height N (new chain) is processed → txids appended AGAIN
4. Result: `HEIGHT_TO_TRANSACTION_IDS[N]` now has duplicates

This tool detects these duplicates by:
- Reading all transaction IDs for each height
- Counting unique vs total txids
- Reporting heights where `unique_count < total_count`

The earliest reorg found indicates the first block that was processed incorrectly, helping you determine where to reindex from.
