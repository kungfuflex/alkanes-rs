## Database: Schema and Write Paths

This document describes the current database schema for hot tables and the write paths used by the indexer. It focuses on correctness, scalability, and predictable performance at tens of millions to billions of rows.

### Tables and Keys

- AlkaneTransaction
  - Primary key: `transactionId` (text)
  - Columns: `blockHeight` int, `transactionIndex` int, flags `hasTrace`/`traceSucceed`, `transactionData` jsonb, timestamps
  - Indexes:
    - btree: (`blockHeight`, `transactionIndex`) for per-block ordering/queries
    - BRIN: `blockHeight` for range scans
  - Storage: `transactionData` set to STORAGE EXTERNAL

- TraceEvent
  - Primary key: `id` uuid
  - Columns: `transactionId` text (FK -> AlkaneTransaction.transactionId), `blockHeight` int, `vout` int, `eventType` text, `data` jsonb, `alkaneAddressBlock` text, `alkaneAddressTx` text, timestamps
  - Indexes:
    - btree: `transactionId`
    - btree: (`blockHeight`, `eventType`)
    - BRIN: `blockHeight`
  - Storage/Tuning: `data` STORAGE EXTERNAL; table `fillfactor=80`, `autovacuum_vacuum_scale_factor=0.01`, `autovacuum_vacuum_threshold=5000`, `autovacuum_analyze_scale_factor=0.02`

- DecodedProtostone
  - Primary key: (`transactionId`, `vout`, `protostoneIndex`)
  - Columns: `blockHeight` int, `decoded` jsonb, timestamps
  - Indexes:
    - BRIN: `blockHeight`
  - Storage/Tuning: `decoded` STORAGE EXTERNAL; table `fillfactor=80`, `autovacuum_vacuum_scale_factor=0.01`, `autovacuum_vacuum_threshold=5000`, `autovacuum_analyze_scale_factor=0.02`

### AMM Tables (Pools, States, Swaps, Mints, Burns, Creations)

- ProcessedBlocks
  - Unique keys: `blockHeight` (unique), `blockHash` (unique)
  - Columns: `timestamp` timestamptz, `isProcessing` boolean, `createdAt` timestamptz
  - Indexes:
    - btree: `blockHash`
  - Usage: read latest processed block height for cache scoping and API consistency.

- Pool
  - Primary key: `id` (text)
  - Unique: (`poolBlockId`,`poolTxId`)
  - Columns: factory ids (`factoryBlockId`,`factoryTxId`), pool ids (`poolBlockId`,`poolTxId`), token ids (`token0BlockId`,`token0TxId`,`token1BlockId`,`token1TxId`), `poolName`, timestamps
  - Indexes:
    - btree: (`factoryBlockId`,`factoryTxId`)
    - btree: (`token0BlockId`,`token0TxId`,`token1BlockId`,`token1TxId`)  [pair orientation A]
    - btree: (`token1BlockId`,`token1TxId`,`token0BlockId`,`token0TxId`)  [pair orientation B]
  - Notes: dual token-pair indexes support symmetric pair searches (e.g., token vs BUSD) for price-related endpoints.

- PoolState
  - Primary key: `id` (text)
  - Unique: (`poolId`,`blockHeight`)
  - Columns: `poolId` (FK -> Pool.id), `blockHeight` int, `token0Amount` text, `token1Amount` text, `tokenSupply` text, `createdAt` timestamptz
  - Indexes:
    - btree: `poolId`
    - btree: `blockHeight`
  - Optional (recommended when pool histories are hot): composite btree (`poolId`,`blockHeight` DESC) to accelerate descending history fetches and “latest per pool” if using raw SQL DISTINCT ON.

- PoolCreation
  - Primary key: `id` (text)
  - Unique: (`poolBlockId`,`poolTxId`)
  - Columns: tx refs (`transactionId`,`blockHeight`,`transactionIndex`), pool ids, token ids, `token0Amount` text, `token1Amount` text, `tokenSupply` text, `creatorAddress` text, `successful` boolean default true, `timestamp` timestamptz, timestamps
  - Indexes:
    - btree: `transactionId`
    - btree: `blockHeight`
    - btree: (`poolBlockId`,`poolTxId`)
    - btree: (`blockHeight`,`transactionIndex`)
    - btree: (`successful`,`blockHeight`,`transactionIndex`)
    - btree: (`poolBlockId`,`poolTxId`,`timestamp`)          [pool history]
    - btree: (`creatorAddress`,`timestamp`)                   [address history]
    - btree: (`creatorAddress`,`poolBlockId`,`poolTxId`,`timestamp`)  [address+pool history]
    - BRIN: `timestamp`                                       [coarse pruning on time]

- PoolSwap
  - Primary key: `id` (text)
  - Columns: tx refs (`transactionId`,`blockHeight`,`transactionIndex`), pool ids, token ids for sold/bought, `soldAmount` double precision, `boughtAmount` double precision, `sellerAddress` text, `successful` boolean default true, `timestamp` timestamptz, timestamps
  - Indexes:
    - btree: `transactionId`
    - btree: `blockHeight`
    - btree: (`poolBlockId`,`poolTxId`)
    - btree: (`blockHeight`,`transactionIndex`)
    - btree: (`successful`,`blockHeight`,`transactionIndex`)
    - btree: (`poolBlockId`,`poolTxId`,`timestamp`)           [pool history]
    - btree: (`soldTokenBlockId`,`soldTokenTxId`,`timestamp`) [token history]
    - btree: (`boughtTokenBlockId`,`boughtTokenTxId`,`timestamp`) [token history]
    - btree: (`sellerAddress`,`timestamp`)                     [address history]
    - btree: (`sellerAddress`,`poolBlockId`,`poolTxId`,`timestamp`) [address+pool]
    - btree: (`sellerAddress`,`soldTokenBlockId`,`soldTokenTxId`,`timestamp`)   [address+token]
    - btree: (`sellerAddress`,`boughtTokenBlockId`,`boughtTokenTxId`,`timestamp`) [address+token]
    - BRIN: `timestamp`
  - Notes: amounts use double precision for speed; if exact sums are required long-term, consider `numeric(38,18)` and Prisma Decimal mapping.

- PoolMint
  - Primary key: `id` (text)
  - Columns: tx refs, pool ids, `lpTokenAmount` text, token ids and amounts (text), `minterAddress` text, `successful` boolean default true, `timestamp` timestamptz, timestamps
  - Indexes:
    - btree: `transactionId`
    - btree: `blockHeight`
    - btree: (`poolBlockId`,`poolTxId`)
    - btree: (`blockHeight`,`transactionIndex`)
    - btree: (`successful`,`blockHeight`,`transactionIndex`)
    - btree: (`poolBlockId`,`poolTxId`,`timestamp`)           [pool history]
    - btree: (`minterAddress`,`timestamp`)                    [address history]
    - btree: (`minterAddress`,`poolBlockId`,`poolTxId`,`timestamp`) [address+pool]
    - BRIN: `timestamp`

- PoolBurn
  - Primary key: `id` (text)
  - Columns: tx refs, pool ids, `lpTokenAmount` text, token ids and amounts (text), `burnerAddress` text, `successful` boolean default true, `timestamp` timestamptz, timestamps
  - Indexes:
    - btree: `transactionId`
    - btree: `blockHeight`
    - btree: (`poolBlockId`,`poolTxId`)
    - btree: (`blockHeight`,`transactionIndex`)
    - btree: (`successful`,`blockHeight`,`transactionIndex`)
    - btree: (`poolBlockId`,`poolTxId`,`timestamp`)           [pool history]
    - btree: (`burnerAddress`,`timestamp`)                    [address history]
    - btree: (`burnerAddress`,`poolBlockId`,`poolTxId`,`timestamp`) [address+pool]
    - BRIN: `timestamp`

- SubfrostWrap
  - Primary key: `id` (text)
  - Columns: tx refs (`transactionId`,`blockHeight`,`transactionIndex`), `address` (optional), `amount` text, `successful` boolean default true, `timestamp` timestamptz, timestamps
  - Indexes:
    - btree: `transactionId`
    - btree: `blockHeight`
    - btree: (`address`,`timestamp`)
    - btree: (`blockHeight`,`transactionIndex`)
    - btree: (`successful`,`blockHeight`,`transactionIndex`)
    - BRIN: `timestamp`

- SubfrostUnwrap
  - Primary key: `id` (text)
  - Columns: tx refs (`transactionId`,`blockHeight`,`transactionIndex`), `address` (optional), `amount` text, `successful` boolean default true, `timestamp` timestamptz, timestamps
  - Indexes:
    - btree: `transactionId`
    - btree: `blockHeight`
    - btree: (`address`,`timestamp`)
    - btree: (`blockHeight`,`transactionIndex`)
    - btree: (`successful`,`blockHeight`,`transactionIndex`)
    - BRIN: `timestamp`

- CuratedPools
  - Primary key: `id` (text)
  - Columns: `factoryId` text unique, `poolIds` text[]

- kv_store
  - Primary key: `key` text
  - Columns: `value` text
  - Usage: lightweight progress and configuration storage.

- Profile, CorpData, ClockIn, ClockInSummary, ClockInBlockSummary
  - Non-AMM tables used by other product surfaces; kept here for completeness. See inline schema for indexes.

#### Success flags and writers
- `PoolSwap`, `PoolMint`, and `PoolBurn` include `successful boolean not null default true`.
- Composite btree indexes on (`successful`,`blockHeight`,`transactionIndex`) coexist with time- and entity-oriented indexes for optional success filtering.
- Writers (`replace_pool_swaps`, `replace_pool_mints`, `replace_pool_burns`) accept a trailing `successful: bool` and replace rows per transaction id.
- `PoolCreation` includes `successful` primarily for consistency; decode logic only inserts valid creations.

### Write Paths (batching and replacements)

- Upsert AlkaneTransaction
  - Function: `db::transactions::upsert_alkane_transactions`
  - Batches rows and uses `ON CONFLICT (transactionId) DO UPDATE` with a no-op guard via `IS DISTINCT FROM` to avoid unnecessary updates/WAL.
  - Batch size clamped to keep each INSERT under parameter limits and ~1s execution.

- Replace TraceEvent
  - Function: `db::transactions::replace_trace_events`
  - Shape per row: `(transactionId, blockHeight, vout, eventType, data, alkaneAddressBlock, alkaneAddressTx)`
  - Deletes existing rows for txids using CTE + `unnest($1::text[])` for better plans on large arrays, then inserts in chunks.

- Replace DecodedProtostone
 - Replace PoolSwap/PoolMint/PoolBurn
  - Functions: `db::transactions::{replace_pool_swaps, replace_pool_mints, replace_pool_burns}`
  - Behavior: delete existing rows for the provided txids, then insert provided rows in chunks under parameter limits.
  - Shape per row:
    - PoolSwap: `(transactionId, blockHeight, transactionIndex, poolBlockId, poolTxId, soldTokenBlockId, soldTokenTxId, boughtTokenBlockId, boughtTokenTxId, soldAmount double, boughtAmount double, sellerAddress, successful, timestamp)`
    - PoolMint: `(transactionId, blockHeight, transactionIndex, poolBlockId, poolTxId, lpTokenAmount text, token0BlockId, token0TxId, token1BlockId, token1TxId, token0Amount text, token1Amount text, minterAddress, successful, timestamp)`
    - PoolBurn: `(transactionId, blockHeight, transactionIndex, poolBlockId, poolTxId, lpTokenAmount text, token0BlockId, token0TxId, token1BlockId, token1TxId, token0Amount text, token1Amount text, burnerAddress, successful, timestamp)`
    - SubfrostWrap: `(transactionId, blockHeight, transactionIndex, address, amount text, successful, timestamp)`
    - SubfrostUnwrap: `(transactionId, blockHeight, transactionIndex, address, amount text, successful, timestamp)`
  - Deletes use `delete from "SubfrostWrap" where "transactionId" = any($1)` and `delete from "SubfrostUnwrap" where "transactionId" = any($1)`.

  - Function: `db::transactions::replace_decoded_protostones`
Additional Subfrost writers:
- `db::transactions::replace_subfrost_wraps`: deletes by txids then inserts wrap rows in chunks.
- `db::transactions::replace_subfrost_unwraps`: deletes by txids then inserts unwrap rows in chunks.
  - Shape per row: `(transactionId, vout, protostoneIndex, blockHeight, decoded)`
  - Deletes existing rows for txids using CTE + `unnest`; inserts with `ON CONFLICT ... DO UPDATE` guarded by `IS DISTINCT FROM` on `decoded`.

### Performance Considerations

- BRIN on `blockHeight` localizes range-scoped reads and maintenance even for very large tables.
- Redundant indexes were avoided; composite and PKs cover most access. This reduces write amplification and index churn.
- JSONB moved to EXTERNAL storage to reduce heap page churn and improve HOT update chances for metadata columns.
- Autovacuum settings are more aggressive on high-churn tables to keep bloat in check; tune per deployment if needed.
- Batch sizes for INSERTs are clamped to avoid slow statements; deletes use `unnest` to avoid bad planner choices with huge `= ANY($1)` arrays.

### Operational Notes

- Schema management via `cargo run --bin dbctl -- push|reset|drop`.
- The indexer writes the three hot tables in a single transaction per block, minimizing partial states.
- If you need to reprocess a block: run `cargo run --bin reprocess -- --height <H>`; it will recompute and fully replace rows for that height’s txids.
 - Reset or control progress:
   - `cargo run --bin dbctl -- reset-progress --height H` sets `kv_store.last_processed_height` to `H-1` (H=0 clears the key).
 - Purge all indexed data for a specific blockHeight (safe order):
   - `cargo run --bin dbctl -- purge-block --height H`


