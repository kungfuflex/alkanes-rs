# Helpers: Technical Documentation

This directory groups helper modules used by the indexer pipeline. These modules isolate RPC access, decoding logic, and domain-specific processing so they can be extended or optimized independently.

## block.rs
- canonical_tip_height(provider): Returns metashrew_height - 1 to correct Metashrew's off-by-one. It uses the resilient provider wrapper (timeout, retry/backoff, circuit breaker, global semaphore) to tolerate transient RPC failures.
- get_block_hash(provider, height): Thin wrapper over Bitcoin RPC to get the block hash by height.
- get_block_txids(provider, block_hash): Calls JSON-RPC esplora_block::txids on the configured Sandshrew/Metashrew endpoint (from SANDSHREW_RPC_URL or provider default) via the resilient JSON-RPC wrapper.
- get_transactions_info(provider, txids, batch_size): Concurrent fan-out using Futures streams to fetch esplora_tx for each txid, each request using the resilient JSON-RPC wrapper. Returns a Vec<serde_json::Value> (preserving inputs order after collection is not guaranteed; callers that require order should re-map).
- tx_has_op_return(tx_json): Utility to detect OP_RETURN outputs based on scriptpubkey_type, scriptpubkey_asm, or hex prefix 6a.

Timestamp source for `ProcessedBlocks`:
- The pipeline derives the block timestamp from the first transaction in the block that has `status.block_time` when present; otherwise it falls back to `Utc::now()`. This timestamp is used when upserting the `ProcessedBlocks` row after successful processing.

Tips:
- Adjust batch_size based on your RPC capacity. The helper already applies backpressure via .buffer_unordered(batch_size).

## pools.rs
- Uses `deezel_common::alkanes::amm::AmmManager` to discover pools and fetch per-pool details via upstream raw simulate APIs.
- `fetch_all_pools_with_details(provider, factory_block, factory_tx)`: Two-step concurrent flow that respects `SANDSHREW_RPC_URL` from the environment.
  1. Calls `AmmManager::get_all_pools_via_raw_simulate(&url, factory_block, factory_tx)` to obtain pool IDs.
  2. Fetches each pool's details with bounded parallelism (10 in-flight) via `AmmManager::get_pool_details_via_raw_simulate(&url, pool_block, pool_tx)` and collects results.
- fetch_and_upsert_pools_for_tip(provider, pool, factory_block, factory_tx, tip_height): E2E helper to fetch pools, insert any new pools, then insert PoolState snapshots only when values changed since the last snapshot.

Tips:
- Database writes are batched in a transaction for consistency. Use the same pattern if you add new upserts.
 - Pool detail RPC fetches are performed concurrently with a fixed concurrency of 10 to balance throughput and upstream load.

## protostone.rs
Implements the Runestone/Protostone decode + trace flow with 10-way batched parallelism for OP_RETURN transactions and returns structured results for DB writes.

- decode_and_trace_for_block(provider, txs, _, _): Returns `Vec<TxDecodeTraceResult>`; processes only OP_RETURN transactions in up to 10 concurrent batches:
  1. Fetch raw tx hex using EsploraProvider::get_tx_hex(txid); fallback to BitcoinRpcProvider::get_transaction_hex(txid) with timeout/backoff retries and INFO/WARN logs.
  2. Deserialize hex into bitcoin::Transaction.
  3. Decode runestone/protostones via deezel_common::runestone_enhanced::format_runestone_with_decoded_messages.
  4. Compute shadow vouts: start = tx.output.len() + 1; vout = start + i for i-th protostone.
  5. Reverse txid to little-endian and call alkanes_trace per protostone using the resilient JSON-RPC wrapper; collect `decoded_protostones` and `trace_events`.
     - The trace result is flattened into per-event rows: each `invoke`/`return` is recorded separately with the same shadow `vout` so downstream consumers can match them.

Types:
- `TxDecodeTraceResult { transaction_id, transaction_json, decoded_protostones, trace_events, has_trace, trace_succeed }`
- `DecodedProtostoneItem { vout, protostone_index, decoded }`
- `TraceEventItem { vout, event_type, data, alkane_address_block, alkane_address_tx }`
  - For `invoke` events, `alkane_address_block/tx` is taken from `data.context.myself.{block,tx}` and converted from hex (e.g. `0x2`) to decimal string (e.g. `2`) for consistency with DB `Pool` identifiers.

Concurrency model:
- OP_RETURN transactions are split into ceil(total/10) sized chunks; each chunk is processed concurrently. This yields significant end-to-end speedups while keeping RPC pressure bounded.
- The signature includes `_max_decode_in_flight` and `_max_trace_concurrency` for future fine-grained controls if needed.

Extension points:
- Swap format_runestone_with_decoded_messages with a different decoder if protocol evolves.
- If you need strict ordering, carry index metadata through TraceJob and re-order at write time.

Operational considerations:
- SANDSHREW_RPC_URL is used as the default JSON-RPC endpoint. EsploraProvider will also use a direct HTTP ESPLORA_URL if compiled with native-deps.
- alkanes_trace expects little-endian txid hex; the helper converts the standard big-endian string before calling.
- Logs now include per-batch summaries (size, decoded, trace_ok/trace_err, skipped, elapsed_ms) and overall totals with elapsed time.

Error-based skipping:
- If `alkanes_trace` returns a non-standard upstream error whose message includes both `Non-standard error object received` and `Cannot read properties of undefined`, that protostone is skipped (WARN log) and the batch continues without failing the block.

Ignored transactions:
- A constant `IGNORED_TRACE_TXIDS` in this module contains big-endian txid strings that will be skipped before any decode/trace work.
- This is a safety valve for pathological txs that repeatedly fail to trace and would otherwise fail a whole block. Update the list in `src/helpers/protostone.rs` and rebuild.

## notify.rs
Writes a Redis key after pools are refreshed for a tip so dependent services can react. Publishing for processed blocks is restricted to realtime blocks only; catch-up processing does not publish.

- Key name: `indexer-${NETWORK_ENV || 'mainnet'}-pools-lastblock`
- Value: decimal block height (string)
- Env:
  - `REDIS_URL` (optional; default `redis://127.0.0.1/`)
  - `NETWORK_ENV` (optional; key naming; defaults to `mainnet` when unset)

Implementation details:
- `notify_pools_processed(height: u64)`: best-effort async write using `redis` crate with multiplexed Tokio connection. Failures are logged at WARN and are non-fatal.
- Invoked by `Pipeline::fetch_pools_for_tip` only when pool fetch/upsert succeeds.

## poolswap.rs
Indexes AMM pool swap events from stored trace events and writes structured rows into `PoolSwap`.

- Flow per block:
  1. Collect `invoke` events with `data.type == "delegatecall"` and opcode `inputs[0] == 0x3` (swap opcode).
  2. Extract unique `(poolBlockId, poolTxId)` pairs from those events and batch-fetch their token pairs from `Pool` using `get_pool_tokens_for_pairs`.
  3. For each candidate invoke event:
     - Ensure `context.incomingAlkanes` contains one of the pool tokens.
     - Parse optional desired amounts from `inputs[1]` and `inputs[2]` (hex) when present.
     - Find the next `return` with the same `vout`, preferring one that returns the opposite token with amount matching `inputs[1]`/`inputs[2]`.
  4. Compute totals for token0 and token1:
     - Incoming = sum of `context.incomingAlkanes` for that token on the invoke event.
     - Outgoing = sum of `response.alkanes` for that token on the return event.
  5. Infer trade direction:
     - If token0_out == 0 and token1_in == 0 → selling token0 for token1
     - Else → selling token1 for token0
  6. Persist a row for every candidate invoke. If a matching `return` is not found or computed amounts are zero, write `soldAmount=0`, `boughtAmount=0`, and `successful=false`. Otherwise write computed amounts and `successful=true`.
  7. Perform a single `replace_pool_swaps` call which deletes existing rows for the block’s txids and inserts the new set.

- Performance:
  - Token pairs fetched once per block for all pools seen in traces (no per-event DB lookups).
  - Writes are batched in one transaction for the entire block’s swaps, with automatic chunking of INSERTs to stay under Postgres/sqlx parameter limits.

- Data sources and shapes used:
  - `TraceEvent` rows produced by `protostone.rs` with fields: `eventType`, `vout`, `alkaneAddressBlock`, `alkaneAddressTx`, and JSON `data` containing `context.inputs`, `context.incomingAlkanes`, `response.alkanes`, etc.
  - `Pool` table provides canonical token pairs per pool (`token0BlockId/token0TxId`, `token1BlockId/token1TxId`).

- Normalization details:
  - Token `id.block/tx` and `value` fields may arrive as u128 `{hi,lo}` objects, hex strings (e.g., `"0x2"`), decimal strings, or numbers. The decoder normalizes each to u128 before comparing or summing to avoid false negatives.

- Integration points:
  - Called from `Pipeline::process_block_sequential` after protostones and trace events are written.
  - Also exposed via a standalone CLI (`swaps`) to process a specific block.

- sellerAddress resolution:
  - The indexer preloads decoded protostones for all txids in the block via a DB helper.
  - For each swap, it matches on the `invoke` event's `vout` and reads `pointer_destination.address` from the decoded protostone JSON.
  - The resulting address is stored as `sellerAddress` on the `PoolSwap` row.

## poolcreate.rs
Indexes AMM pool creation (initial liquidity) events from stored trace events and writes structured rows into `PoolCreation`.

- Detection logic per candidate trace event:
  1. Event must be `invoke` with `data.type == "delegatecall"` and opcode `inputs[0] == 0x0` (create opcode) on the pool’s own alkane address (`alkaneAddressBlock/alkaneAddressTx`).
  2. Events are normalized to the same order as the inspector: by `vout` ascending with `invoke` before `return` for matching.
  3. Determine the two non-LP token IDs from `context.incomingAlkanes` on the invoke. If fewer than two unique non-LP ids are present, skip.
  4. Select the matching `return` on the same `vout` where the LP token amount in `response.alkanes` is strictly greater than any LP amount in `incomingAlkanes` (net LP minted). If multiple such returns exist, prefer the one with the smallest amounts returned of token0 and token1 (tie-breaker: the latest such return).
  5. Compute net amounts (all values are u128, stored as strings):
     - token0_amount = sum(invoke.incomingAlkanes[token0]) - sum(return.response.alkanes[token0])
     - token1_amount = sum(invoke.incomingAlkanes[token1]) - sum(return.response.alkanes[token1])
     - token_supply  = sum(return.response.alkanes[lpId]) - sum(invoke.incomingAlkanes[lpId])
  6. Persist only if all three nets are > 0. `PoolCreation` remains success-only; failed attempts are not recorded.

- Implementation notes:
  - LP token id equals the pool id (`alkaneAddressBlock/alkaneAddressTx`).
  - The indexer preloads decoded protostones and, when available at the same `vout`, uses `pointer_destination.address` as `creatorAddress`.
  - Event-order normalization eliminates mismatches between the inspector and the indexer.

- Integration points:
  - `index_pool_creations_for_block` decodes candidates for a block and returns fully shaped rows.
  - `Pipeline::process_block_sequential` calls it after writing `TraceEvent`/`DecodedProtostone`, then persists via `replace_pool_creations` in a single transaction for the block.

## poolmint.rs
Indexes AMM pool mint (add_liquidity) events from stored trace events and writes structured rows into `PoolMint`.

- Detection logic per candidate trace event:
  1. Event must be `invoke` with `data.type == "delegatecall"` and opcode `inputs[0] == 0x1` (mint opcode) on the pool’s own alkane address (`alkaneAddressBlock/alkaneAddressTx`).
  2. Events are normalized to the same order as the inspector: by `vout` ascending with `invoke` before `return` for matching.
  3. Use `Pool` metadata to resolve the pool’s `token0` and `token1` ids.
  4. Select the matching `return` on the same `vout` where the LP token amount in `response.alkanes` is strictly greater than any LP amount in `invoke.context.incomingAlkanes` (net LP minted). If multiple such returns exist, prefer the one with the smallest returned amounts of token0 and token1 (tie-breaker: the latest such return).
  5. Compute net amounts (stored as strings):
     - token0Amount = sum(invoke.incomingAlkanes[token0]) - sum(return.response.alkanes[token0])
     - token1Amount = sum(invoke.incomingAlkanes[token1]) - sum(return.response.alkanes[token1])
     - lpTokenAmount = sum(return.response.alkanes[poolId]) - sum(invoke.incomingAlkanes[poolId])
  6. Persist a row for every candidate invoke. If a matching `return` is not found or computed nets are zero, write zero amounts and `successful=false`. Otherwise write computed amounts and `successful=true`.

- Implementation notes:
  - LP token id equals the pool id (`alkaneAddressBlock/alkaneAddressTx`). The chosen return is guaranteed to include the LP token with amount strictly greater than incoming LP (if any).
  - The indexer preloads decoded protostones and, when available at the same `vout`, uses `pointer_destination.address` as `minterAddress`.

- Integration points:
  - `index_pool_mints_for_block` decodes candidates for a block and writes via `replace_pool_mints` in a single transaction for the block.

## poolburn.rs
Indexes AMM pool burn (remove_liquidity) events from stored trace events and writes structured rows into `PoolBurn`.

- Detection logic per candidate trace event:
  1. Event must be `invoke` with `data.type == "delegatecall"` and opcode `inputs[0] == 0x2` (burn opcode) on the pool’s own alkane address (`alkaneAddressBlock/alkaneAddressTx`).
  2. Events are normalized to the same order as the inspector: by `vout` ascending with `invoke` before `return` for matching.
  3. Use `Pool` metadata to resolve the pool’s `token0` and `token1` ids.
  4. Select the matching `return` on the same `vout` where token0 and token1 are net positive out to the caller and, if there is any incoming LP on the invoke, the LP token amount in `response.alkanes` is strictly less than the incoming LP (net LP burned). If multiple such returns exist, prefer the one with the smallest LP remaining (tie-breaker: the latest such return).
  5. Compute net amounts (stored as strings):
     - token0Amount = sum(return.response.alkanes[token0]) - sum(invoke.incomingAlkanes[token0])
     - token1Amount = sum(return.response.alkanes[token1]) - sum(invoke.incomingAlkanes[token1])
     - lpTokenAmount = sum(invoke.incomingAlkanes[poolId]) - sum(return.response.alkanes[poolId])
  6. Persist a row for every candidate invoke. If a matching `return` is not found or computed nets are zero, write zero amounts and `successful=false`. Otherwise write computed amounts and `successful=true`.

- Implementation notes:
  - LP token id equals the pool id (`alkaneAddressBlock/alkaneAddressTx`).
  - The indexer preloads decoded protostones and, when available at the same `vout`, uses `pointer_destination.address` as `burnerAddress`.

- Integration points:
  - `index_pool_burns_for_block` decodes candidates for a block and writes via `replace_pool_burns` in a single transaction for the block.

## subfrost.rs
Indexes Subfrost wrap (mint) and unwrap (redeem) events from stored trace events and writes structured rows into `SubfrostWrap` and `SubfrostUnwrap`.

- Wrap detection per candidate trace event:
  1. `invoke` with opcode `inputs[0] == 0x4d` (77) on the Subfrost contract (`alkaneAddressBlock/Tx == 32:0`).
  2. Events are normalized: by `vout` ascending with `invoke` before `return` for matching.
  3. Select the matching `return` on the same `vout` where `status` is success and `response.alkanes` contains the Subfrost token id `(block=0x20, tx=0)`; sum that amount as wrapped units.
  4. Persist for every candidate invoke. If no matching `return` or computed amount is zero, write `amount="0"` and `successful=false`; otherwise `successful=true` with computed amount.
  5. Resolve `address` via decoded protostone at that `vout` (`pointer_destination.address`) when available.

- Unwrap detection per candidate trace event:
  1. `invoke` with opcode `inputs[0] == 0x4e` (78) on the Subfrost contract (`alkaneAddressBlock/Tx == 32:0`).
  2. Events are normalized: by `vout` ascending with `invoke` before `return` for matching.
  3. Compute incoming Subfrost amount from `invoke.data.context.incomingAlkanes` for token `(block=0x20, tx=0)`.
  4. Select the matching `return` on the same `vout` with success status and compute outgoing Subfrost from `return.data.response.alkanes` for the same token.
  5. Net amount = `incoming - outgoing` (stored as string). Persist for every candidate invoke; if no matching return, write `amount="0"` and `successful=false`.
  6. Resolve `address` via decoded protostone at that `vout` (`pointer_destination.address`) when available.

- Integration points:
  - `index_subfrost_wraps_for_block` and `index_subfrost_unwraps_for_block` preload decoded protostones and write via `replace_subfrost_wraps` / `replace_subfrost_unwraps` respectively in single transactions for the block.

## inspect.rs (CLI)
Standalone inspector to analyze a single `transactionId`:

```bash
cargo build --bin inspect
RUST_LOG=info ./target/debug/inspect <txid> [--verbose-json]
```

It prints:
- `AlkaneTransaction` metadata
- Stored `DecodedProtostone` and `TraceEvent` rows (`--verbose-json` prints pretty JSON)
- Existing `PoolSwap` rows
- A simulated pool swap decoding run with detailed logs covering delegatecall/opcode check, pool ID extraction, incoming token presence, strict return matching, and computed swap amounts.
- A simulated pool creation decoding run mirroring `poolcreate.rs`: it orders events by `vout` and event type, checks `delegatecall` with opcode `0x0`, identifies token0/token1 from invoke `incomingAlkanes`, selects the proper `return` (net LP minted and minimal token outs), and prints net token contributions and LP supply.
  
Additionally, pool mint (add_liquidity) is decoded similarly to pool creation, but keyed by opcode `0x1` and requiring the selected `return` to have LP minted (LP out > LP in). The inspector logs the chosen return and net amounts.

## Coding Guidelines
- Error handling: Prefer early returns and clear anyhow::Context messages so upstream callers get actionable logs.
- Logging: Use INFO for high-signal steps (fetch, decode, trace) and DEBUG for verbose payloads. Avoid spamming at INFO in hot loops unless debugging.
- Concurrency & resiliency: Use the shared resilient wrappers in `helpers/rpc.rs` for outbound RPCs. Bound concurrency and use the global semaphore to apply backpressure to upstream services. Tune via `RPC_MAX_CONCURRENCY`, `RPC_MAX_RETRIES`, `RPC_*BACKOFF*`, `RPC_TIMEOUT_MS`, and `RPC_CIRCUIT_COOLDOWN_MS`.
