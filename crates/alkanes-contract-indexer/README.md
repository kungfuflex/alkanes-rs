## Alkanes Contract Indexer

A Rust service that monitors new blocks via Metashrew, fans out concurrent jobs to decode and index Alkanes-related data, and writes results to Postgres. It leverages the deezel toolkit for all Alkanes/Bitcoin RPC interactions.

### Highlights
- **Background polling**: Reliable loop that queries Metashrew and derives a canonical tip height (`metashrew_height - 1`), with exponential backoff and reorg awareness.
- **Pools/state refresh on new tip**: When a higher tip is detected, the service first refreshes pools and inserts new `PoolState` snapshots only if values changed.
- **Block-processing pipeline**: For each new block height the service:
  - resolves the block hash via Bitcoin RPC
  - fetches ordered txids via JSON-RPC `esplora_block::txids`
  - fetches transaction info concurrently in batches of 25 via `esplora_tx` (strict: if any per-tx fetch fails after retries, the whole block processing fails and is retried)
  - filters transactions for OP_RETURN outputs and logs the count
  - decodes Runestone/Protostone for OP_RETURN transactions and calls `alkanes_trace` per decoded protostone using 10-way batched parallelism
  - persists results in Postgres in a single transaction per block: upserts `AlkaneTransaction`, replaces `DecodedProtostone` and flattened per-event `TraceEvent` rows for affected `transactionId`s
- **Postgres writes**: Batch, transactional, and indexable by `transactionId` across `AlkaneTransaction`, `DecodedProtostone`, `TraceEvent`, and Subfrost event tables. Large batches are automatically chunked to stay within SQL bind parameter limits. Deletes for replacement use CTE+unnest for better plans on large arrays.
 - **Pool* success flag**: `PoolSwap`, `PoolMint`, and `PoolBurn` now record both successful and failed attempts. Successful rows have computed amounts and `successful=true`; failed attempts are recorded with zero amounts and `successful=false`. `PoolCreation` remains success-only (but includes a `successful` column defaulting to true for consistency).
 - **Processed blocks tracking**: After each `process_block_sequential` completes, the service upserts into `ProcessedBlocks` with `(blockHeight, blockHash, timestamp, isProcessing=false)`. The timestamp comes from the first transaction's `status.block_time` in the block if available, otherwise `Utc::now()`.
 - **Publishing policy**: Redis publish for "block processed" runs only for realtime tip blocks. Catch-up blocks do not publish, avoiding noisy downstream updates.

### Repository Structure
- `src/main.rs`: Program entrypoint; initializes config, DB, provider; runs background poller until Ctrl-C.
- `src/config.rs`: Loads configuration from environment variables.
- `src/db.rs`: Postgres pool initialization and re-exports DB submodules.
- `src/db/blocks.rs`: Helpers for `ProcessedBlocks` (table ensure and upsert after successful block processing).
- `src/db/pools.rs`: All SQL for `Pool` (read existing, batch insert, resolve IDs for pairs).
- `src/db/pool_state.rs`: All SQL for `PoolState` (fetch latest per pool, batch insert snapshots).
- `src/helpers/pools.rs`: Uses deezel's `AmmManager` helpers to simulate via Sandshrew; fetches pool IDs via `get_all_pools_via_raw_simulate` and then fetches each pool's details concurrently (10 in-flight) via `get_pool_details_via_raw_simulate` (no local decoders).
- `src/helpers/block.rs`: Block utilities: `canonical_tip_height`, `get_block_hash`, `get_block_txids`, `get_transactions_info` (batched concurrent fetch), and `tx_has_op_return`.
- `src/helpers/protostone.rs`: Runestone/Protostone decode + trace orchestration.
- `src/helpers/poolswap.rs`: PoolSwap indexer that reads `TraceEvent` JSON, `DecodedProtostone` (for pointer destinations), and `Pool` metadata to derive swaps and write `PoolSwap` rows.
- `src/helpers/poolcreate.rs`: Pool creation (initial liquidity) indexer that detects opcode `0x0` delegatecalls and writes `PoolCreation` rows.
- `src/helpers/poolmint.rs`: Pool mint (add_liquidity) indexer that detects opcode `0x1` delegatecalls and writes `PoolMint` rows (now also writes failed attempts with `successful=false`).
- `src/helpers/poolburn.rs`: Pool burn (remove_liquidity) indexer that detects opcode `0x2` delegatecalls and writes `PoolBurn` rows (now also writes failed attempts with `successful=false`).
- `src/helpers/subfrost.rs`: Subfrost wrap/unwrap indexers. Wraps detect opcode `0x4d` (77) invokes on alkaneAddress 32:0 and write `SubfrostWrap` rows. Unwraps detect opcode `0x4e` (78) invokes on alkaneAddress 32:0 and write `SubfrostUnwrap` rows. Both resolve `address` from `DecodedProtostone.pointer_destination.address` when available.
- `src/provider.rs`: Builds a `deezel_common::provider::ConcreteProvider` for RPC calls.
- `src/pipeline.rs`: Orchestrates per-tip work; now delegates decoding to helpers and DB writes to `src/db/*` modules. Includes Subfrost wrap and unwrap indexing.
- `src/poller.rs`: `BlockPoller` that polls `metashrew_height`, detects new heights, and invokes the pipeline.
- `src/db/transactions.rs`: Batch upsert/replace for `AlkaneTransaction`, `TraceEvent`, and `DecodedProtostone` keyed by `transactionId`, plus helper to read decoded protostones by `(transactionId, vout)`.
- `reference/deezel/`: Vendored reference copy of deezel source for exploration only (do not import from here at build time).

### Dependencies
- Rust toolchain (stable)
- Postgres (local or remote)
- deezel (via git dependency)

We depend on deezel’s common crate for provider and RPC traits. Upstream reference: [`Sprimage/deezel`](https://github.com/Sprimage/deezel).

### Environment Variables
Create a `.env` file at the repo root (you can copy from `.example.env`) or export variables in your shell.

```env
DATABASE_URL=postgres://user:pass@localhost:5432/alkanes_indexer

# Where Metashrew/Sandshrew JSON-RPC is available. Defaults to http://localhost:18888
SANDSHREW_RPC_URL=http://localhost:18888

# Optional: direct Bitcoin Core RPC (if different from Sandshrew)
#BITCOIN_RPC_URL=http://user:pass@127.0.0.1:8332

# Optional: Esplora base URL (if applicable)
#ESPLORA_URL=http://localhost:3002

# Network identity used by provider constructor (default: regtest)
NETWORK=regtest
# Optional override for notification key naming (defaults to NETWORK or 'mainnet')
#NETWORK_ENV=regtest

# Poll interval for metashrew height (ms); default 2000
POLL_INTERVAL_MS=2000

# Optional: start height for historical catch-up.
# - If set: a catch-up coordinator will process sequentially from max(START_HEIGHT, last_progress+1)
#   up to the current tip. The coordinator starts only after the poller has initialized tip
#   and refreshed pools. Catch-up processing does NOT publish realtime notifications.
# - If unset: no catch-up is performed; the poller immediately processes the current tip
#   on startup (publishing) and then continues with subsequent blocks.
#START_HEIGHT=800000

# Required: Factory contract ID for AMM pools discovery
# These must be the numeric string IDs (lo parts) expected by Metashrew
FACTORY_BLOCK_ID=0
FACTORY_TX_ID=0

# RPC resiliency (defaults shown)
# Global in-process concurrency cap for outbound RPCs
RPC_MAX_CONCURRENCY=64
# Max retry attempts per RPC
RPC_MAX_RETRIES=5
# Exponential backoff base and cap (ms)
RPC_BASE_BACKOFF_MS=200
RPC_MAX_BACKOFF_MS=5000
# Per-call timeout (ms)
RPC_TIMEOUT_MS=20000
# Circuit breaker cooldown (ms) before half-open probe
RPC_CIRCUIT_COOLDOWN_MS=5000
# Redis (optional; used to publish last processed pools height)
# Defaults to redis://127.0.0.1/
#REDIS_URL=redis://localhost:6379/
```

Notes:
- The service builds a deezel `ConcreteProvider`. Pool discovery calls pass `SANDSHREW_RPC_URL` directly to deezel's `AmmManager` helpers.
- Block tx discovery uses JSON-RPC methods (`esplora_block::txids`, `esplora_tx`) through the provider, preferring `SANDSHREW_RPC_URL` from the environment.
- `BITCOIN_RPC_URL` and `ESPLORA_URL` are optional; leave unset for Sandshrew-only routing.

### Resiliency and backpressure
- All JSON-RPC calls from the indexer use a resilient wrapper (`helpers/rpc.rs`) that applies:
  - per-call timeout, retries with exponential backoff + jitter, and a global concurrency limiter
  - a simple circuit breaker that opens on repeated failures and half-opens after `RPC_CIRCUIT_COOLDOWN_MS`
- Functions updated to use this wrapper include:
  - `helpers/block.rs::canonical_tip_height` (Metashrew height), `get_block_txids`, `get_transactions_info`
  - `helpers/protostone.rs::trace_call` (alkanes_trace)
- The poller only advances its `last_height` after a successful `process_block_sequential`; on failure it pauses advancing so the same height is retried on the next tick. Tip-height fetch already uses exponential backoff.

## Update deezel-common to latest
```bash
cargo update -p deezel-common
```

### Build
```bash
cargo build
```

### Build (Release)
```bash
cargo build --release
```

### Important: SQLx compile-time checks (fresh machine builds)
This project now avoids `sqlx::query!` in favor of runtime-checked `sqlx::query`, so a live database is NOT required to build on a fresh machine.

If you fork this repo and add `sqlx::query!` calls, SQLx will validate those macros at compile time by connecting to your `DATABASE_URL` and checking the queried tables/columns. On a fresh database without the schema, builds can fail with errors like:

```
error: error returned from database: relation "DecodedProtostone" does not exist
```

If you use `sqlx::query!`, choose one workflow:

- Initialize the database (typical flow)
  1. Ensure `DATABASE_URL` is set (see Environment Variables below).
  2. Build and push the schema:
     ```bash
     cargo run --bin dbctl -- push
     ```
  3. Build the binaries (optional, `cargo run` will build as needed):
     ```bash
     cargo build --release
     ```

- Use SQLx offline mode (no DB needed at build time)
  1. On a machine with a live DB and schema, generate prepare data and commit it:
     ```bash
     cargo install sqlx-cli
     export DATABASE_URL=postgres://user:pass@host/db
     cargo run --bin dbctl -- push
     cargo sqlx prepare -- --all-targets
     # commit the generated sqlx-data.json
     ```
  2. On fresh machines, build offline:
     ```bash
     SQLX_OFFLINE=true cargo build --release
     ```

Notes:
- If your Rust toolchain is too old for `edition = "2024"`, the compiler will error before SQLx runs; use a recent stable toolchain.
- You can replace specific `sqlx::query!` usages with `sqlx::query` to avoid compile-time checking, but you lose compile-time row-shape validation.

### Database Schema Management (CLI)
We provide a small CLI to manage the database schema.

```bash
# Push or update schema to DATABASE_URL
cargo run --bin dbctl -- push

# Drop all tables and recreate schema
cargo run --bin dbctl -- reset

# Drop all tables only (no re-push)
cargo run --bin dbctl -- drop

# Apply versioned migrations (non-destructive updates)
cargo run --bin dbctl -- migrate

# Reset progress so the next run starts at a specific height
# H > 0: sets kv_store.last_processed_height = H-1 (next run starts at H)
# H = 0: clears the progress key (next run starts at 0 only if START_HEIGHT is unset)
cargo run --bin dbctl -- reset-progress --height 917000

# Purge all indexed data for a specific blockHeight (safe order, single transaction)
cargo run --bin dbctl -- purge-block --height 917522
```

The schema mirrors the previous Prisma-based design (types mapped to Postgres) with performance-focused modifications:
- strings as `text`/`uuid`, JSON as `jsonb`, datetimes as `timestamptz`.
- tables include: `"AlkaneTransaction"`, `"TraceEvent"`, `"DecodedProtostone"`, `"ClockIn"`, `"ProcessedBlocks"`,
  `"ClockInBlockSummary"`, `"ClockInSummary"`, `"CorpData"`, `"Profile"`, `"Pool"`, `"PoolState"`,
  `"PoolCreation"`, `"PoolSwap"`, `"PoolBurn"`, `"PoolMint"`, `"CuratedPools"`, `"SubfrostWrap"`, and `kv_store`.
- `AlkaneTransaction`: primary key is now `transactionId` (surrogate id removed). Indexes: composite btree on (`blockHeight`,`transactionIndex`) and BRIN on `blockHeight`. JSONB `transactionData` stored EXTERNAL.
- `TraceEvent`: now includes `blockHeight`; `id` is `uuid`. Indexes: btree on `transactionId`, btree on (`blockHeight`,`eventType`), BRIN on `blockHeight`. Fillfactor/autovacuum tuned; JSONB `data` stored EXTERNAL.
- `DecodedProtostone`: composite primary key (`transactionId`,`vout`,`protostoneIndex`), includes `blockHeight`. BRIN on `blockHeight`. Fillfactor/autovacuum tuned; JSONB `decoded` stored EXTERNAL.

### Schema naming and compatibility
- Table and column names preserve the original Prisma casing by using quoted identifiers (e.g., `"AlkaneTransaction"`, `"blockHeight"`).
- UUID fields use Postgres `gen_random_uuid()`; the service enables `pgcrypto` automatically if available.
- Foreign keys and indexes match the original relationships and composite unique constraints where provided.

### Run
```bash
# With INFO logs
RUST_LOG=info cargo run

# With more verbose logs
RUST_LOG=debug cargo run
```

Run the compiled release binary with INFO logs:

```bash
RUST_LOG=info ./target/release/alkanes-contract-indexer
```

The service will:
1) Connect to Postgres
2) Construct a deezel provider
3) Start the `BlockPoller` loop which:
   - reads canonical tip height via `metashrew_height - 1`
   - detects new heights (filling gaps)
   - on first observation (no previous height): triggers `Pipeline::fetch_pools_for_tip(provider, tip)` once
   - after a successful pools refresh, writes Redis key `indexer-${NETWORK_ENV || NETWORK || 'mainnet'}-pools-lastblock` with value `<tip>`
   - on first observation AND no `START_HEIGHT`: also processes the current tip immediately (publishing enabled)
   - on height increase: first triggers `Pipeline::fetch_pools_for_tip(provider, tip)`
   - then processes each new block via `Pipeline::process_block_sequential`
   - after a block finishes indexing, record a row in `ProcessedBlocks` with the block's hash and timestamp
   - on no height change: skips pools/state refresh and block processing
4) If `START_HEIGHT` is set, start the catch-up coordinator which:
   - waits for the poller to initialize tip (and perform the initial pools/state refresh) before starting
   - reads canonical tip height and computes `[next..=tip]` from `START_HEIGHT` and the last stored progress from DB
   - sequentially processes `[next..=tip]` via `Pipeline::process_block_sequential` (publishing disabled for these blocks)
   - persists `last_processed_height` in `kv_store`
   - after catch-up, the poller continues processing subsequent new blocks as they arrive

### Standalone: Swap/Creation/Mint/Burn indexing for a specific block

You can run the full pipeline for a specific height (including swaps, creations, mints, and burns):

```bash
cargo run --bin swaps -- --height 840000
```

This will:
- Fetch OP_RETURN txs → decode protostones → trace via `alkanes_trace` → upsert `AlkaneTransaction`, `DecodedProtostone`, and per-event `TraceEvent` rows (each `invoke`/`return` is a separate row with `vout`).
- Batch index PoolSwap rows using router-aware logic:
  - Filter to `invoke` events with `data.type == "delegatecall"` and opcode `inputs[0] == 0x3`.
  - Use `invoke.alkaneAddressBlock/Tx` (already decimalized) as the pool ID; prefetch token pairs for all referenced pools in the block.
  - Ensure `context.incomingAlkanes` contains one of the pool tokens.
  - Extract desired output amounts from `inputs[1]` and/or `inputs[2]` (hex) when present.
  - Find the next `return` with the same `vout`, preferring one that returns the opposite token and whose amount matches `inputs[1]`/`inputs[2]`.
  - Compute totals for token0/token1 on invoke/return and infer sell/buy direction.
  - Persist into `PoolSwap` with a single batch write (also chunked under param limits). Rows are always written for candidate invokes: if a matching `return` is not found or computed amounts are zero, the row is written with `soldAmount=0`, `boughtAmount=0`, and `successful=false`. `sellerAddress` is resolved by reading `pointer_destination.address` from the matched protostone's decoded object for the transaction's `vout`.

- Decode PoolCreation rows:
  - Filter to `invoke` events with `data.type == "delegatecall"` and opcode `inputs[0] == 0x0`.
  - Identify token0/token1 from `invoke.context.incomingAlkanes` (excluding the poolId/LP token).
  - Choose the matching `return` on the same `vout` where LP out > LP in, preferring minimal token outs.
  - Compute net token0/token1 contributions and LP supply; persist rows when all nets > 0. `creatorAddress` is resolved from decoded protostone at the `vout` when available. `PoolCreation` remains success-only; failed attempts are not recorded in this table.

- Decode PoolMint (add_liquidity) rows:
  - Filter to `invoke` events with `data.type == "delegatecall"` and opcode `inputs[0] == 0x1`.
  - Use `Pool` to resolve token0/token1 for the poolId (`alkaneAddressBlock/Tx`).
  - Choose the matching `return` on the same `vout` where the LP token (poolId) appears in `response.alkanes` with amount strictly greater than any incoming LP (net minted). On multiple candidates, prefer minimal token outs (latest on tie).
  - Compute net amounts: token0Amount = in - out, token1Amount = in - out, lpTokenAmount = lp_out - lp_in. Rows are always written for candidate invokes: if a matching `return` is not found or computed nets are zero, the row is written with zero amounts and `successful=false`. Otherwise amounts are populated and `successful=true`. `minterAddress` is resolved from decoded protostone at the `vout` when available.

- Decode PoolBurn (remove_liquidity) rows:
  - Filter to `invoke` events with `data.type == "delegatecall"` and opcode `inputs[0] == 0x2`.
  - Use `Pool` to resolve token0/token1 for the poolId (`alkaneAddressBlock/Tx`).
  - Choose the matching `return` on the same `vout` where the user receives more token0 and token1 (net positive out) and where the LP token amount in `response.alkanes` is strictly less than any LP in `invoke.context.incomingAlkanes` (net burned). If multiple such returns exist, prefer the one with the smallest LP remaining (tie-breaker: the latest such return).
  - Compute net amounts: token0Amount = out - in, token1Amount = out - in, lpTokenAmount = lp_in - lp_out. Rows are always written for candidate invokes: if a matching `return` is not found or computed nets are zero, the row is written with zero amounts and `successful=false`. Otherwise amounts are populated and `successful=true`. `burnerAddress` is resolved from decoded protostone at the `vout` when available.

Notes on ID/value normalization:
- Token IDs and values in trace JSON may appear as u128 `{hi,lo}` objects, hex strings (e.g., `"0x2"`), decimal strings, or numbers.
- The decoder normalizes all of these to u128 before comparison or summation.

Notes:
- `sellerAddress` is derived from `DecodedProtostone.pointer_destination.address` by matching the `vout` of the swap's `invoke` event.
- Requires `Pool` table to be populated with the pools referenced by trace events.

Subfrost wraps/unwraps:
- Wraps: `amount` is taken from the successful `return.response.alkanes` sum for the Subfrost token id (32:0), matched on the same `vout` following an opcode `0x4d` invoke. `address` on `SubfrostWrap` is resolved from `DecodedProtostone.pointer_destination.address` matched by `vout` when available (otherwise null).
- Unwraps: `amount` is the net of Subfrost token id (32:0) between the `invoke.data.context.incomingAlkanes` (incoming) and `return.response.alkanes` (outgoing), matched on the same `vout` following an opcode `0x4e` invoke with a successful return. `address` on `SubfrostUnwrap` is resolved from `DecodedProtostone.pointer_destination.address` matched by `vout` when available (otherwise null).

### Standalone: Force re-process a block

To re-run the full pipeline for a single height (even if it already exists in `ProcessedBlocks`):

```bash
cargo run --bin reprocess -- --height 840000
```

This executes `Pipeline::process_block_sequential` for the given height and will:
- Re-fetch txids and txs, decode/trace, and write `AlkaneTransaction`, `DecodedProtostone`, and `TraceEvent`.
- Rebuild PoolSwap/PoolCreation/PoolMint/PoolBurn rows for the block via the existing replace-* functions (they delete by txids then insert), ensuring a clean reindex for that block.
- Upsert `ProcessedBlocks` again for the height with the current block hash and timestamp.

### Metashrew height off-by-one
- Metashrew's `get_metashrew_height()` reports the next height (tip + 1). The indexer normalizes this by subtracting 1 to obtain the canonical chain tip.
- Implementation: `helpers/block.rs` provides `canonical_tip_height(provider)` used by both the poller and catch-up coordinator.

Shutdown with Ctrl-C.

### Current Status
- Poller, pipeline pools fetch, and coordinator are implemented. Per-block processing:
  - resolves block hash → fetches txids via `esplora_block::txids` → fetches tx info concurrently (25 in-flight) via `esplora_tx`
  - filters for OP_RETURN transactions and logs the count
  - decodes Runestone/Protostone for OP_RETURN transactions via `deezel_common::runestone_enhanced::format_runestone_with_decoded_messages`
  - computes shadow vouts as `start = tx.outputs.len() + 1; vout = start + protostone_index`
  - calls `alkanes_trace` with little-endian txid and the computed shadow vout
  - aggregates structured results per tx (`TxDecodeTraceResult`) and writes DB changes transactionally per block

### Technical Details

#### Decode and Trace Flow
The decode/trace flow lives in `src/helpers/protostone.rs` and is invoked from `Pipeline::process_block_sequential`.

1. `process_block_sequential` fetches tx infos and filters for OP_RETURN transactions.
2. It logs the OP_RETURN count and invokes `decode_and_trace_for_block(provider, &op_return_txs, 32, 16)`.
3. `decode_and_trace_for_block` returns a `Vec<TxDecodeTraceResult>` and processes OP_RETURN txs using 10-way batched parallelism. Strictness: if any tx hex fetch or `alkanes_trace` call fails after retries, the function returns an error to fail the block so it is retried rather than silently dropping the tx.
   - Split OP_RETURN transactions into up to 10 batches (ceil-divided), each batch processed concurrently.
   - For each tx in a batch: fetch tx hex (Esplora first, fallback to Bitcoin Core) with timeout/backoff; deserialize to `bitcoin::Transaction`.
   - Decode runestone/protostones using `format_runestone_with_decoded_messages` from deezel.
   - Compute shadow vouts per protostone: `start = tx.output.len() + 1; vout = start + i`.
   - Reverse txid to little-endian and call `alkanes_trace` per protostone.
4. The pipeline batches and writes:
   - Upsert `"AlkaneTransaction"` rows `(blockHeight, transactionId, transactionIndex, hasTrace, traceSucceed, transactionData)`.
   - Replace `"DecodedProtostone"` rows for affected txids with `(transactionId, vout, protostoneIndex, blockHeight, decoded)`.
   - Replace `"TraceEvent"` rows for affected txids with `(transactionId, blockHeight, vout, eventType, data, alkaneAddressBlock, alkaneAddressTx)`; one row per event (`invoke`, `return`, etc.). For `invoke`, `alkaneAddress*` is derived from `context.myself` and converted from hex (e.g. `0x2`) to decimal string (e.g. `2`).
   - Deletes use `with ids as (select unnest($1::text[]) as txid) delete ... using ids` for efficient plans. Writes occur in a single SQL transaction and are chunked to respect Postgres parameter limits.

The code is instrumented with INFO logs at each step, plus per-block timing in `process_block_sequential`.

#### Concurrency Model
The helper currently uses 10 concurrent batches to balance throughput and RPC load. The function signature includes knobs (`_max_decode_in_flight`, `_max_trace_concurrency`) reserved for future fine-grained control, but the default behavior uses fixed 10 batches for simplicity and stability. Per-batch summaries report size, decoded protostones, trace_ok/trace_err, skipped, and elapsed_ms.

#### RPC Endpoints Used
- `esplora_block::txids` (JSON-RPC) for ordered txids by block hash
- `esplora_tx` (JSON-RPC) to fetch tx metadata for filtering and basic fields
- `EsploraProvider::get_tx_hex` or `BitcoinRpcProvider::get_transaction_hex` to obtain raw tx hex reliably
- `alkanes_trace` (JSON-RPC) for protostone tracing

#### Shadow Vouts
Shadow vouts are computed as an offset past the concrete outputs of the transaction:
`start = tx.output.len() + 1; end = start + protostones.len() - 1`.
For the i-th protostone (0-based), the vout is `start + i`.

#### Endianness
`alkanes_trace` expects a little-endian txid hex string. The indexer reverses the bytes from the standard big-endian representation before invoking the RPC.
- A minimal `kv_store` table is auto-created for progress tracking. The pool discovery and snapshotting flow (via deezel's `AmmManager`):
  - calls `get_all_pools_via_raw_simulate(&url, factory_block, factory_tx)` using `SANDSHREW_RPC_URL` to obtain pool IDs
  - fetches each pool's details with bounded parallelism (10 in-flight) via `get_pool_details_via_raw_simulate(&url, pool_block, pool_tx)`; failures are skipped and logged upstream
  - batch upserts `Pool` and inserts new `PoolState` snapshots on change

### Ignored transactions
- Some transactions can be temporarily excluded from decode/trace if they consistently return no trace or otherwise block batch processing.
- The ignore list is implemented as a constant array in `src/helpers/protostone.rs` named `IGNORED_TRACE_TXIDS` (big-endian txid strings). Any txid in this list is filtered out before any decode/trace work and a log line is emitted at INFO: "skipping txid from ignore list".
- To add or remove entries, edit `IGNORED_TRACE_TXIDS` and rebuild. This is intended as an operational safety valve; prefer fixing upstream issues where possible.
- Additionally, the decoder treats a specific upstream non-standard error from `alkanes_trace` as a non-fatal skip per protostone:
  - When the RPC error contains both `Non-standard error object received` and `Cannot read properties of undefined`, the protostone is skipped and the batch continues. This avoids failing the entire block due to this upstream client error (commonly originating in `alkanes/lib/base-rpc.js:addHexPrefix`).

### Pool discovery implementation details
- We rely on deezel-common's `alkanes::amm::AmmManager` helpers, which accept a Sandshrew/Metashrew URL parameter.
- The indexer reads `SANDSHREW_RPC_URL` from the environment and passes it to these helpers; we do not mutate process env at runtime.
- Local hex decoding utilities have been removed from `src/helpers/pools.rs` to avoid drift from upstream decode logic.

### Troubleshooting
- Verify `DATABASE_URL` is reachable.
- Ensure `SANDSHREW_RPC_URL` points to a running endpoint that supports `metashrew_height`.
- Increase `POLL_INTERVAL_MS` if your environment is resource-constrained.
- Enable debug logs to inspect simulate responses:
- If you see `cannot insert multiple commands into a prepared statement` at startup:
  - This indicates a multi-statement SQL was sent as a single prepared statement. The code has been updated to split the DDL into separate queries in `db::blocks::ensure_processed_blocks_table`.

  - `RUST_LOG=alkanes_contract_indexer=debug,deezel_common=debug cargo run`
- If you see `Other error: Failed to decode get_all_pools result`:
  - Confirm `SANDSHREW_RPC_URL` is correct (no localhost fallback).
  - Ensure the factory IDs (`FACTORY_BLOCK_ID`, `FACTORY_TX_ID`) are correct for your network.
  - Upstream can sometimes return placeholder IDs like `{"block":"0","tx":"0"}`; these will cause per-pool detail simulate to fail with `unexpected end-of-file` and are skipped upstream when present.

### Standalone: Transaction Inspector

Use the inspector to debug a specific `transactionId` and see why it is/isn't decoded as a pool swap.

Build:

```bash
cargo build --bin inspect
```

Run:

```bash
RUST_LOG=info ./target/debug/inspect <txid> [--verbose-json]
```

What it prints:
- `AlkaneTransaction` metadata (blockHeight, transactionIndex, hasTrace, traceSucceed)
- Stored `DecodedProtostone` and `TraceEvent` rows (pretty JSON with `--verbose-json`)
- Existing `PoolSwap` rows for the tx (if any)
- A simulated pool swap decoding pass with detailed logs:
  - candidate `delegatecall` invokes with opcode 3
  - pool ID from `alkaneAddressBlock/Tx`
  - incoming alkanes token presence
  - strict return matching (opposite token and amount match)
  - computed sold/bought amounts or reasons for skipping

### References
- deezel toolkit (used for RPC/provider): [`Sprimage/deezel`](https://github.com/Sprimage/deezel)
