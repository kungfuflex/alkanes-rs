# Marketplace canonical producer commitment v1

`alkanes-contract-indexer` publishes Marketplace-readable state through
`"MarketplaceCanonicalCommit"`. The legacy pipeline still performs some transforms in separate
database transactions, so the commit row is the atomic visibility boundary: a consumer must never
treat a `TraceAlkane` or `TraceBalanceUtxo` row above the latest committed height as canonical.

Startup refuses an `indexer_position` that does not exactly equal the latest canonical commitment.
That makes adoption intentionally fail closed: a database created by an older producer requires a
full replay instead of silently blessing legacy partial state. It also rejects authority or spend
staging rows outside the one immediately retryable height, and rejects every spent mutation newer
than the committed tip.

## Publication fields

Each immutable row contains:

- `height`, `block_hash`, and `previous_block_hash` (lowercase 64-character Core hashes)
- `reorg_epoch`, incremented once for each atomically removed fork suffix
- `producer_revision`, injected at build time from the exact Git revision (or the explicit
  `ALKANES_PRODUCER_REVISION` build variable)
- `schema_revision = marketplace-canonical-commit-v1`
- count and SHA-256 digest for `TraceAlkane` rows created at the height
- count and SHA-256 digest for `TraceBalanceUtxo` rows created or spent at the height
- count and SHA-256 root for the complete active (`spent = false`) UTXO inventory
- `manifest_hash`, binding every field above

The same serializable transaction inserts the commitment and the matching `ProcessedBlocks` row,
then advances the single `indexer_position` row. There is no update-on-conflict path for a committed
height. An exact retry is idempotent; a different hash or manifest is an error.

The SQL consumer contract is:

```sql
"MarketplaceCanonicalCommit" (
  height BIGINT PRIMARY KEY,
  block_hash TEXT UNIQUE NOT NULL,
  previous_block_hash TEXT NOT NULL,
  reorg_epoch BIGINT NOT NULL,
  producer_revision TEXT NOT NULL,
  schema_revision TEXT NOT NULL,
  trace_alkane_row_count BIGINT NOT NULL,
  trace_alkane_row_hash TEXT NOT NULL,
  trace_balance_utxo_row_count BIGINT NOT NULL,
  trace_balance_utxo_row_hash TEXT NOT NULL,
  active_inventory_count BIGINT NOT NULL,
  active_inventory_root TEXT NOT NULL,
  manifest_hash TEXT UNIQUE NOT NULL,
  committed_at TIMESTAMPTZ NOT NULL
)

"MarketplaceCanonicalState" (
  id SMALLINT PRIMARY KEY CHECK (id = 1),
  reorg_epoch BIGINT NOT NULL
)
```

Hashes are lowercase 64-character hexadecimal strings. `producer_revision` is a lowercase 40- or
64-character Git object ID, and the v1 database constraint pins `schema_revision` to
`marketplace-canonical-commit-v1`.

The marker's `reorg_epoch` is the epoch in which that height was published. Consumers must also
read the singleton state's current epoch in the same database snapshot. Immediately after rollback
and before the replacement child is published, the state epoch is intentionally newer than the
immutable ancestor marker's epoch.

## Digest encoding

The algorithm is `alkanes-marketplace-canonical-manifest-v1`. Every domain and field is encoded as
an unsigned 64-bit big-endian byte length followed by exact UTF-8 bytes. A row begins with byte
`0x01` and an unsigned 64-bit big-endian field count. Rows use PostgreSQL's canonical decimal text
for integers/numerics and are ordered by their stable primary-key components before SHA-256.

Per-height `TraceAlkane` fields are:

1. `alkane_block`
2. `alkane_tx`
3. `created_at_block`
4. `created_at_tx`
5. `created_at_height`

Per-height `TraceBalanceUtxo` commitment entries are immutable logical events. A row created and
spent in the same block contributes both a `create` and a `spend` entry. This keeps every historical
height recomputable even though the live row's spent status changes. Active-inventory entries use
`event_kind = active`; therefore `trace_balance_utxo_row_count` counts logical event entries, while
`active_inventory_count` counts physical active rows. Fields are:

1. `event_kind` (`create`, `spend`, or `active`)
2. `outpoint_txid`
3. `outpoint_vout`
4. `address`
5. `alkane_block`
6. `alkane_tx`
7. `amount`
8. `block_height`
9. `spent`
10. `spent_at_height` (empty string when SQL `NULL`)

For a `create` entry, `spent` is the literal `false` and `spent_at_height` is empty. For a `spend`
entry, `spent` is `true`. Active entries at height `H` satisfy `block_height <= H AND
(spent_at_height IS NULL OR spent_at_height > H)` and encode `spent = false` with an empty
`spent_at_height`. This makes historical inventory roots recomputable from the current table.

The domains are respectively
`alkanes-marketplace-trace-alkane-height-v1`,
`alkanes-marketplace-trace-balance-utxo-height-v1`, and
`alkanes-marketplace-active-inventory-v1`.

`manifest_hash` uses domain `alkanes-marketplace-canonical-manifest-v1` and exactly one row with
these fields in order: `height`, `block_hash`, `previous_block_hash`, `reorg_epoch`,
`producer_revision`, `schema_revision`, `trace_alkane_row_count`, `trace_alkane_row_hash`,
`trace_balance_utxo_row_count`, `trace_balance_utxo_row_hash`, `active_inventory_count`, and
`active_inventory_root`. The published row count is not appended to a row digest; it is separately
bound by the manifest. An empty row set hashes only its length-prefixed domain.

Encoding test vector: domain `domain` with rows `["b", "2"]` and `["a", "1"]` (supplied in
either order) hashes to
`abc7d197a4f756fbd71d96257424da42d666904f82088e8629847910966fcc3b`.

## Failure, retry, and reorg rules

The full block interval is serialized with a PostgreSQL advisory lock. Registry loading, create-row
persistence, trace balance processing, and UTXO extraction errors are propagated; they cannot
advance progress. Before retrying an uncommitted height, its authoritative rows are deleted and
aggregate balance/holder tables are rebuilt from the remaining active `TraceBalanceUtxo` inventory.
The transaction list comes directly from Bitcoin Core and must contain only unique canonical txids.
Every fetched transaction body must exactly cover that list, report the same confirmed Core block
height/hash/timestamp, and provide structurally valid inputs, outputs, values, and scripts. The Core
height/hash link is checked again immediately before publication so a mid-transform reorg aborts the
attempt.

Protostone decoding and trace conversion are also fail closed. A missing trace payload, missing
event variant, unknown call/status enum, undecodable protostone message, or transfer missing its
Alkane ID/value aborts the block. Such data is never normalized to an empty trace or zero balance.

Every non-coinbase Bitcoin input is first written to the private
`MarketplaceBalanceSpendStage`. Staging never changes consumer-visible inventory. Publication
marks matching `TraceBalanceUtxo` rows spent, records `spent_at_height`, rebuilds aggregates, hashes
the resulting active inventory, inserts the marker and progress rows, and clears staging in one
transaction. A deferred trigger refuses any spent mutation without the matching marker. This also
makes same-block create-and-spend rows deterministic.

Every monitoring pass compares the committed tip hash with Bitcoin Core, including when no higher
block exists. On a same-height or deep fork, the producer scans committed hashes backwards until it
finds a Core-matching ancestor. One serializable transaction then:

1. re-verifies the selected ancestor's stored hash;
2. increments `reorg_epoch`;
3. deletes `TraceAlkane` and `TraceBalanceUtxo` rows created above it and reverses spends above it;
4. rebuilds aggregate balance/holder tables;
5. deletes the `ProcessedBlocks` and commitment suffix; and
6. rewinds or clears `indexer_position`.

Database triggers reject updates/deletes of commitment and `ProcessedBlocks` rows, reject invalid
`indexer_position` advances, and reject late mutations of `TraceAlkane`/`TraceBalanceUtxo` at or
below a committed height. A deferred marker trigger requires the matching `ProcessedBlocks` and
`indexer_position` rows and current state epoch before commit. The singleton epoch can only advance
by exactly one inside hash-verified rollback. Only canonical spend publication and hash-verified
rollback use narrowly scoped local bypass settings.

## Consumer rule

Marketplace authorities should read the latest commitment, require the expected producer and schema
revisions, independently verify its Core hash/checkpoint, and include the complete commitment fields
in readiness evidence. Read registry rows with `created_at_height <= height` and UTXO rows with
`block_height <= height`; active inventory at `H` uses `(spent_at_height IS NULL OR
spent_at_height > H)`. Recompute the latest per-height digests and active root before reporting
readiness. Startup performs this same
latest-state verification and also requires `ProcessedBlocks` to mirror the full marker chain.

The commit row publishes only the registry and canonical UTXO inventory. Legacy raw trace,
AMM/candle/storage, and auxiliary protocol tables are not covered by this v1 Marketplace
commitment; consumers must not infer their completeness from this marker.
