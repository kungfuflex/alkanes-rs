//! Fail-closed publication boundary for Marketplace-consumable Alkanes state.
//!
//! The legacy indexer writes several derived tables in independent transactions.  A row in
//! `MarketplaceCanonicalCommit` is therefore the visibility boundary: readers MUST ignore rows
//! above the latest committed height.  Before a retry, rows for the uncommitted height are
//! discarded and aggregates are rebuilt from the authoritative UTXO inventory.  Publication of
//! the commitment, `ProcessedBlocks`, and `indexer_position` happens in one transaction.

use anyhow::{Context, Result, anyhow, bail};
use bitcoin::hashes::{Hash, HashEngine, sha256};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::collections::HashSet;

pub const SCHEMA_REVISION: &str = "marketplace-canonical-commit-v1";
pub const PRODUCER_REVISION: &str = env!("ALKANES_PRODUCER_REVISION");
pub const SERIAL_ADVISORY_LOCK_KEY: i64 = 0x414c_4b4d_5043_4331;
pub const ZERO_BLOCK_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

const CREATE_COMMIT_TABLES: &str = r#"
CREATE TABLE IF NOT EXISTS "MarketplaceCanonicalState" (
    id SMALLINT PRIMARY KEY CHECK (id = 1),
    reorg_epoch BIGINT NOT NULL CHECK (reorg_epoch >= 0)
)
"#;

const CREATE_COMMIT_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS "MarketplaceCanonicalCommit" (
    height BIGINT PRIMARY KEY CHECK (height >= 0),
    block_hash TEXT NOT NULL UNIQUE CHECK (block_hash ~ '^[0-9a-f]{64}$'),
    previous_block_hash TEXT NOT NULL CHECK (previous_block_hash ~ '^[0-9a-f]{64}$'),
    reorg_epoch BIGINT NOT NULL CHECK (reorg_epoch >= 0),
    producer_revision TEXT NOT NULL
        CONSTRAINT marketplace_commit_producer_revision_check
        CHECK (producer_revision ~ '^[0-9a-f]{40}([0-9a-f]{24})?$'),
    schema_revision TEXT NOT NULL
        CONSTRAINT marketplace_commit_schema_revision_check
        CHECK (schema_revision = 'marketplace-canonical-commit-v1'),
    trace_alkane_row_count BIGINT NOT NULL CHECK (trace_alkane_row_count >= 0),
    trace_alkane_row_hash TEXT NOT NULL CHECK (trace_alkane_row_hash ~ '^[0-9a-f]{64}$'),
    trace_balance_utxo_row_count BIGINT NOT NULL CHECK (trace_balance_utxo_row_count >= 0),
    trace_balance_utxo_row_hash TEXT NOT NULL CHECK (trace_balance_utxo_row_hash ~ '^[0-9a-f]{64}$'),
    active_inventory_count BIGINT NOT NULL CHECK (active_inventory_count >= 0),
    active_inventory_root TEXT NOT NULL CHECK (active_inventory_root ~ '^[0-9a-f]{64}$'),
    manifest_hash TEXT NOT NULL UNIQUE CHECK (manifest_hash ~ '^[0-9a-f]{64}$'),
    committed_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp()
)
"#;

const CREATE_SPEND_STAGE_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS "MarketplaceBalanceSpendStage" (
    spend_height INTEGER NOT NULL CHECK (spend_height >= 0),
    outpoint_txid TEXT NOT NULL CHECK (outpoint_txid ~ '^[0-9a-f]{64}$'),
    outpoint_vout INTEGER NOT NULL CHECK (outpoint_vout >= 0),
    PRIMARY KEY (spend_height, outpoint_txid, outpoint_vout)
)
"#;

const ADD_SPENT_HEIGHT_COLUMN: &str = r#"
ALTER TABLE "TraceBalanceUtxo"
ADD COLUMN IF NOT EXISTS spent_at_height INTEGER
"#;

const ADD_SPENT_HEIGHT_CONSTRAINT: &str = r#"
DO $constraints$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = '"TraceBalanceUtxo"'::regclass
          AND conname = 'trace_balance_utxo_spent_height_check'
    ) THEN
        ALTER TABLE "TraceBalanceUtxo"
        ADD CONSTRAINT trace_balance_utxo_spent_height_check
        CHECK (
            (NOT spent AND spent_at_height IS NULL) OR
            (spent AND spent_at_height IS NOT NULL AND spent_at_height >= block_height)
        );
    END IF;
END
$constraints$
"#;

const CREATE_COMMIT_GUARD_FUNCTION: &str = r#"
CREATE OR REPLACE FUNCTION marketplace_guard_commit_immutable()
RETURNS trigger LANGUAGE plpgsql AS $guard$
BEGIN
    IF current_setting('alkanes.marketplace_rollback', true) IS DISTINCT FROM 'on' THEN
        RAISE EXCEPTION 'MarketplaceCanonicalCommit rows are immutable; use hash-verified rollback';
    END IF;
    RETURN OLD;
END
$guard$
"#;

const CREATE_STATE_GUARD_FUNCTION: &str = r#"
CREATE OR REPLACE FUNCTION marketplace_guard_canonical_state()
RETURNS trigger LANGUAGE plpgsql AS $guard$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.id <> 1 OR NEW.reorg_epoch <> 0 THEN
            RAISE EXCEPTION 'MarketplaceCanonicalState must begin at epoch zero';
        END IF;
        RETURN NEW;
    END IF;
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'MarketplaceCanonicalState cannot be deleted';
    END IF;
    IF current_setting('alkanes.marketplace_rollback', true) IS DISTINCT FROM 'on'
       OR NEW.id <> OLD.id
       OR NEW.reorg_epoch <> OLD.reorg_epoch + 1 THEN
        RAISE EXCEPTION 'MarketplaceCanonicalState epoch can only advance by one during rollback';
    END IF;
    RETURN NEW;
END
$guard$
"#;

const CREATE_TRACE_ALKANE_GUARD_FUNCTION: &str = r#"
CREATE OR REPLACE FUNCTION marketplace_guard_trace_alkane_committed()
RETURNS trigger LANGUAGE plpgsql AS $guard$
DECLARE
    affected_height INTEGER;
BEGIN
    IF current_setting('alkanes.marketplace_rollback', true) = 'on' THEN
        IF TG_OP = 'DELETE' THEN
            RETURN OLD;
        END IF;
        RETURN NEW;
    END IF;
    IF TG_OP = 'UPDATE' AND
       (OLD.created_at_height IS NULL OR NEW.created_at_height IS NULL) THEN
        RAISE EXCEPTION 'cannot mutate TraceAlkane with an unknown creation height';
    END IF;
    affected_height := CASE
        WHEN TG_OP = 'INSERT' THEN NEW.created_at_height
        WHEN TG_OP = 'DELETE' THEN OLD.created_at_height
        ELSE LEAST(OLD.created_at_height, NEW.created_at_height)
    END;
    IF affected_height IS NULL OR EXISTS (
        SELECT 1 FROM "MarketplaceCanonicalCommit" WHERE height >= affected_height
    ) THEN
        RAISE EXCEPTION 'cannot mutate TraceAlkane at or below a committed Marketplace height';
    END IF;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END
$guard$
"#;

const CREATE_SPEND_COMMIT_GUARD_FUNCTION: &str = r#"
CREATE OR REPLACE FUNCTION marketplace_require_spend_commitment()
RETURNS trigger LANGUAGE plpgsql AS $guard$
BEGIN
    IF NEW.spent AND OLD.spent_at_height IS DISTINCT FROM NEW.spent_at_height
       AND NOT EXISTS (
           SELECT 1 FROM "MarketplaceCanonicalCommit"
           WHERE height = NEW.spent_at_height
       ) THEN
        RAISE EXCEPTION 'spent TraceBalanceUtxo mutation requires its canonical commitment';
    END IF;
    RETURN NEW;
END
$guard$
"#;

const CREATE_TRACE_UTXO_GUARD_FUNCTION: &str = r#"
CREATE OR REPLACE FUNCTION marketplace_guard_trace_balance_utxo_committed()
RETURNS trigger LANGUAGE plpgsql AS $guard$
DECLARE
    affected_height INTEGER;
    publish_height INTEGER;
    publish_height_text TEXT;
BEGIN
    IF current_setting('alkanes.marketplace_rollback', true) = 'on' THEN
        IF TG_OP = 'DELETE' THEN
            RETURN OLD;
        END IF;
        RETURN NEW;
    END IF;
    publish_height_text := current_setting('alkanes.marketplace_publish_height', true);
    IF TG_OP = 'UPDATE' AND publish_height_text IS NOT NULL THEN
        publish_height := publish_height_text::INTEGER;
        IF NOT OLD.spent AND NEW.spent
           AND OLD.spent_at_height IS NULL
           AND NEW.spent_at_height = publish_height
           AND publish_height >= OLD.block_height
           AND OLD.outpoint_txid IS NOT DISTINCT FROM NEW.outpoint_txid
           AND OLD.outpoint_vout IS NOT DISTINCT FROM NEW.outpoint_vout
           AND OLD.address IS NOT DISTINCT FROM NEW.address
           AND OLD.alkane_block IS NOT DISTINCT FROM NEW.alkane_block
           AND OLD.alkane_tx IS NOT DISTINCT FROM NEW.alkane_tx
           AND OLD.amount IS NOT DISTINCT FROM NEW.amount
           AND OLD.block_height IS NOT DISTINCT FROM NEW.block_height
           AND OLD.created_at IS NOT DISTINCT FROM NEW.created_at
           AND NOT EXISTS (
               SELECT 1 FROM "MarketplaceCanonicalCommit" WHERE height >= publish_height
           ) THEN
            RETURN NEW;
        END IF;
    END IF;
    affected_height := CASE
        WHEN TG_OP = 'INSERT' THEN NEW.block_height
        WHEN TG_OP = 'DELETE' THEN OLD.block_height
        ELSE LEAST(OLD.block_height, NEW.block_height)
    END;
    IF EXISTS (
        SELECT 1 FROM "MarketplaceCanonicalCommit" WHERE height >= affected_height
    ) THEN
        RAISE EXCEPTION 'cannot mutate TraceBalanceUtxo at or below a committed Marketplace height';
    END IF;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END
$guard$
"#;

const CREATE_PROCESSED_BLOCK_GUARD_FUNCTION: &str = r#"
CREATE OR REPLACE FUNCTION marketplace_guard_processed_block_committed()
RETURNS trigger LANGUAGE plpgsql AS $guard$
BEGIN
    IF current_setting('alkanes.marketplace_rollback', true) = 'on' THEN
        IF TG_OP = 'DELETE' THEN
            RETURN OLD;
        END IF;
        RETURN NEW;
    END IF;
    IF TG_OP <> 'INSERT' THEN
        RAISE EXCEPTION 'committed ProcessedBlocks rows are immutable';
    END IF;
    IF NEW."isProcessing" OR NOT EXISTS (
        SELECT 1 FROM "MarketplaceCanonicalCommit"
        WHERE height = NEW."blockHeight" AND block_hash = NEW."blockHash"
    ) THEN
        RAISE EXCEPTION 'ProcessedBlocks publication requires an exact canonical commitment';
    END IF;
    RETURN NEW;
END
$guard$
"#;

const CREATE_POSITION_GUARD_FUNCTION: &str = r#"
CREATE OR REPLACE FUNCTION marketplace_guard_indexer_position()
RETURNS trigger LANGUAGE plpgsql AS $guard$
DECLARE
    committed_previous_hash TEXT;
BEGIN
    IF current_setting('alkanes.marketplace_rollback', true) = 'on' THEN
        IF TG_OP = 'DELETE' THEN
            RETURN OLD;
        END IF;
        RETURN NEW;
    END IF;
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'indexer_position can only be deleted by hash-verified rollback';
    END IF;
    SELECT previous_block_hash INTO committed_previous_hash
    FROM "MarketplaceCanonicalCommit"
    WHERE height = NEW.height AND block_hash = NEW.block_hash;
    IF committed_previous_hash IS NULL THEN
        RAISE EXCEPTION 'indexer_position must reference an exact canonical commitment';
    END IF;
    IF TG_OP = 'INSERT' AND NEW.height <> 0 THEN
        RAISE EXCEPTION 'first indexer_position must publish height zero';
    END IF;
    IF TG_OP = 'UPDATE' AND
       (NEW.height <> OLD.height + 1 OR committed_previous_hash <> OLD.block_hash) THEN
        RAISE EXCEPTION 'indexer_position must advance by one hash-linked commitment';
    END IF;
    RETURN NEW;
END
$guard$
"#;

const CREATE_COMMIT_COMPLETENESS_FUNCTION: &str = r#"
CREATE OR REPLACE FUNCTION marketplace_require_atomic_progress()
RETURNS trigger LANGUAGE plpgsql AS $guard$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM "ProcessedBlocks"
        WHERE "blockHeight" = NEW.height AND "blockHash" = NEW.block_hash
          AND NOT "isProcessing"
    ) OR NOT EXISTS (
        SELECT 1 FROM indexer_position
        WHERE id = 1 AND height = NEW.height AND block_hash = NEW.block_hash
    ) OR NOT EXISTS (
        SELECT 1 FROM "MarketplaceCanonicalState"
        WHERE id = 1 AND reorg_epoch = NEW.reorg_epoch
    ) THEN
        RAISE EXCEPTION 'canonical commitment must publish matching state and progress atomically';
    END IF;
    RETURN NEW;
END
$guard$
"#;

const ADD_COMMIT_REVISION_CONSTRAINTS: &str = r#"
DO $constraints$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = '"MarketplaceCanonicalCommit"'::regclass
          AND conname = 'marketplace_commit_producer_revision_check'
    ) THEN
        ALTER TABLE "MarketplaceCanonicalCommit"
        ADD CONSTRAINT marketplace_commit_producer_revision_check
        CHECK (producer_revision ~ '^[0-9a-f]{40}([0-9a-f]{24})?$');
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = '"MarketplaceCanonicalCommit"'::regclass
          AND conname = 'marketplace_commit_schema_revision_check'
    ) THEN
        ALTER TABLE "MarketplaceCanonicalCommit"
        ADD CONSTRAINT marketplace_commit_schema_revision_check
        CHECK (schema_revision = 'marketplace-canonical-commit-v1');
    END IF;
END
$constraints$
"#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowCommitment {
    pub count: u64,
    pub hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalCommit {
    pub height: u64,
    pub block_hash: String,
    pub previous_block_hash: String,
    pub reorg_epoch: u64,
    pub producer_revision: String,
    pub schema_revision: String,
    pub trace_alkane: RowCommitment,
    pub trace_balance_utxo: RowCommitment,
    pub active_inventory: RowCommitment,
    pub manifest_hash: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrepareOutcome {
    Ready,
    AlreadyCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishOutcome {
    Published,
    AlreadyCommitted,
}

struct RowDigest {
    engine: sha256::HashEngine,
    count: u64,
}

impl RowDigest {
    fn new(domain: &str) -> Self {
        let mut engine = sha256::Hash::engine();
        put_field(&mut engine, domain.as_bytes());
        Self { engine, count: 0 }
    }

    fn push(&mut self, fields: &[String]) {
        self.engine.input(&[0x01]);
        self.engine.input(&(fields.len() as u64).to_be_bytes());
        for field in fields {
            put_field(&mut self.engine, field.as_bytes());
        }
        self.count += 1;
    }

    fn finish(self) -> RowCommitment {
        RowCommitment {
            count: self.count,
            hash: sha256::Hash::from_engine(self.engine).to_string(),
        }
    }
}

fn put_field(engine: &mut sha256::HashEngine, bytes: &[u8]) {
    engine.input(&(bytes.len() as u64).to_be_bytes());
    engine.input(bytes);
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn validate_block_link(height: u64, block_hash: &str, previous_block_hash: &str) -> Result<()> {
    if !valid_hash(block_hash) {
        bail!("block hash must be exactly 64 lowercase hexadecimal characters");
    }
    if !valid_hash(previous_block_hash) {
        bail!("previous block hash must be exactly 64 lowercase hexadecimal characters");
    }
    if height == 0 && previous_block_hash != ZERO_BLOCK_HASH {
        bail!("height zero must use the all-zero previous block hash");
    }
    Ok(())
}

pub fn manifest_hash(commit: &CanonicalCommit) -> String {
    let fields = [
        commit.height.to_string(),
        commit.block_hash.clone(),
        commit.previous_block_hash.clone(),
        commit.reorg_epoch.to_string(),
        commit.producer_revision.clone(),
        commit.schema_revision.clone(),
        commit.trace_alkane.count.to_string(),
        commit.trace_alkane.hash.clone(),
        commit.trace_balance_utxo.count.to_string(),
        commit.trace_balance_utxo.hash.clone(),
        commit.active_inventory.count.to_string(),
        commit.active_inventory.hash.clone(),
    ];
    let mut digest = RowDigest::new("alkanes-marketplace-canonical-manifest-v1");
    digest.push(&fields);
    digest.finish().hash
}

fn valid_producer_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn validate_commit_record(commit: &CanonicalCommit) -> Result<()> {
    validate_block_link(
        commit.height,
        &commit.block_hash,
        &commit.previous_block_hash,
    )?;
    if !valid_producer_revision(&commit.producer_revision) {
        bail!("Marketplace producer revision is not a pinned Git object id");
    }
    if commit.schema_revision != SCHEMA_REVISION {
        bail!("unsupported Marketplace schema revision");
    }
    for (label, hash) in [
        ("TraceAlkane row hash", &commit.trace_alkane.hash),
        ("TraceBalanceUtxo row hash", &commit.trace_balance_utxo.hash),
        ("active inventory root", &commit.active_inventory.hash),
        ("manifest hash", &commit.manifest_hash),
    ] {
        if !valid_hash(hash) {
            bail!("{label} is not canonical lowercase hexadecimal");
        }
    }
    if manifest_hash(commit) != commit.manifest_hash {
        bail!("Marketplace manifest hash does not bind its stored fields");
    }
    Ok(())
}

fn validate_commit_chain_descending(commits: &[CanonicalCommit]) -> Result<()> {
    let Some(genesis) = commits.last() else {
        return Ok(());
    };
    if genesis.height != 0 {
        bail!("Marketplace commitment chain does not begin at height zero");
    }
    let mut previous: Option<&CanonicalCommit> = None;
    for commit in commits.iter().rev() {
        validate_commit_record(commit)?;
        if let Some(parent) = previous {
            if commit.height != parent.height + 1 {
                bail!("Marketplace commitment chain contains a height gap");
            }
            if commit.previous_block_hash != parent.block_hash {
                bail!("Marketplace commitment chain contains a broken hash link");
            }
            if commit.reorg_epoch < parent.reorg_epoch {
                bail!("Marketplace reorg epoch moved backwards");
            }
        }
        previous = Some(commit);
    }
    Ok(())
}

/// Pure helper used by tests and external contract verifiers. Rows are sorted lexicographically
/// before hashing, so database execution order cannot affect the commitment.
pub fn deterministic_rows_digest(domain: &str, mut rows: Vec<Vec<String>>) -> RowCommitment {
    rows.sort();
    let mut digest = RowDigest::new(domain);
    for row in rows {
        digest.push(&row);
    }
    digest.finish()
}

pub async fn ensure_schema(pool: &PgPool) -> Result<()> {
    let mut tx = pool.begin().await?;
    acquire_serial_lock(&mut tx).await?;
    sqlx::query(CREATE_COMMIT_TABLES).execute(&mut *tx).await?;
    sqlx::query(
        r#"INSERT INTO "MarketplaceCanonicalState" (id, reorg_epoch)
           VALUES (1, 0) ON CONFLICT (id) DO NOTHING"#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(CREATE_COMMIT_TABLE).execute(&mut *tx).await?;
    sqlx::query(CREATE_SPEND_STAGE_TABLE)
        .execute(&mut *tx)
        .await?;
    sqlx::query(ADD_SPENT_HEIGHT_COLUMN)
        .execute(&mut *tx)
        .await?;
    sqlx::query(ADD_SPENT_HEIGHT_CONSTRAINT)
        .execute(&mut *tx)
        .await?;
    sqlx::query(ADD_COMMIT_REVISION_CONSTRAINTS)
        .execute(&mut *tx)
        .await?;

    sqlx::query(CREATE_COMMIT_GUARD_FUNCTION)
        .execute(&mut *tx)
        .await?;
    sqlx::query(CREATE_STATE_GUARD_FUNCTION)
        .execute(&mut *tx)
        .await?;
    sqlx::query(CREATE_TRACE_ALKANE_GUARD_FUNCTION)
        .execute(&mut *tx)
        .await?;
    sqlx::query(CREATE_TRACE_UTXO_GUARD_FUNCTION)
        .execute(&mut *tx)
        .await?;
    sqlx::query(CREATE_SPEND_COMMIT_GUARD_FUNCTION)
        .execute(&mut *tx)
        .await?;
    sqlx::query(CREATE_PROCESSED_BLOCK_GUARD_FUNCTION)
        .execute(&mut *tx)
        .await?;
    sqlx::query(CREATE_POSITION_GUARD_FUNCTION)
        .execute(&mut *tx)
        .await?;
    sqlx::query(CREATE_COMMIT_COMPLETENESS_FUNCTION)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        r#"DROP TRIGGER IF EXISTS marketplace_commit_immutable ON "MarketplaceCanonicalCommit""#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"CREATE TRIGGER marketplace_commit_immutable
           BEFORE UPDATE OR DELETE ON "MarketplaceCanonicalCommit"
           FOR EACH ROW EXECUTE FUNCTION marketplace_guard_commit_immutable()"#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"DROP TRIGGER IF EXISTS marketplace_canonical_state_guard
           ON "MarketplaceCanonicalState""#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"CREATE TRIGGER marketplace_canonical_state_guard
           BEFORE INSERT OR UPDATE OR DELETE ON "MarketplaceCanonicalState"
           FOR EACH ROW EXECUTE FUNCTION marketplace_guard_canonical_state()"#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(r#"DROP TRIGGER IF EXISTS marketplace_trace_alkane_immutable ON "TraceAlkane""#)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"CREATE TRIGGER marketplace_trace_alkane_immutable
           BEFORE INSERT OR UPDATE OR DELETE ON "TraceAlkane"
           FOR EACH ROW EXECUTE FUNCTION marketplace_guard_trace_alkane_committed()"#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"DROP TRIGGER IF EXISTS marketplace_trace_balance_utxo_immutable ON "TraceBalanceUtxo""#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"CREATE TRIGGER marketplace_trace_balance_utxo_immutable
           BEFORE INSERT OR UPDATE OR DELETE ON "TraceBalanceUtxo"
           FOR EACH ROW EXECUTE FUNCTION marketplace_guard_trace_balance_utxo_committed()"#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"DROP TRIGGER IF EXISTS marketplace_spend_requires_commitment ON "TraceBalanceUtxo""#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"CREATE CONSTRAINT TRIGGER marketplace_spend_requires_commitment
           AFTER UPDATE ON "TraceBalanceUtxo"
           DEFERRABLE INITIALLY DEFERRED
           FOR EACH ROW EXECUTE FUNCTION marketplace_require_spend_commitment()"#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"DROP TRIGGER IF EXISTS marketplace_processed_block_immutable ON "ProcessedBlocks""#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"CREATE TRIGGER marketplace_processed_block_immutable
           BEFORE INSERT OR UPDATE OR DELETE ON "ProcessedBlocks"
           FOR EACH ROW EXECUTE FUNCTION marketplace_guard_processed_block_committed()"#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query("DROP TRIGGER IF EXISTS marketplace_position_guard ON indexer_position")
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"CREATE TRIGGER marketplace_position_guard
           BEFORE INSERT OR UPDATE OR DELETE ON indexer_position
           FOR EACH ROW EXECUTE FUNCTION marketplace_guard_indexer_position()"#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"DROP TRIGGER IF EXISTS marketplace_commit_atomic_progress
           ON "MarketplaceCanonicalCommit""#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"CREATE CONSTRAINT TRIGGER marketplace_commit_atomic_progress
           AFTER INSERT ON "MarketplaceCanonicalCommit"
           DEFERRABLE INITIALLY DEFERRED
           FOR EACH ROW EXECUTE FUNCTION marketplace_require_atomic_progress()"#,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Refuse to silently bless a legacy progress row. A canonical deployment must replay so every
/// advertised height has a manifest and a verified predecessor.
pub async fn verify_bootstrap_state(pool: &PgPool) -> Result<()> {
    let mut serial_guard = pool.begin().await?;
    acquire_serial_lock(&mut serial_guard).await?;
    // Validate every immutable record, not only the tip: hashes, manifest binding, revision pins,
    // chain continuity, and epoch monotonicity are all part of the startup trust boundary.
    let commits = commits_descending(pool).await?;
    let latest = commits.first();
    let state_epoch: i64 =
        sqlx::query_scalar(r#"SELECT reorg_epoch FROM "MarketplaceCanonicalState" WHERE id = 1"#)
            .fetch_one(pool)
            .await?;
    if state_epoch < 0
        || latest
            .map(|commit| state_epoch < commit.reorg_epoch as i64)
            .unwrap_or(false)
    {
        bail!("Marketplace canonical state has an invalid reorg epoch");
    }

    let committed_tip = latest
        .map(|commit| i64::try_from(commit.height))
        .transpose()?
        .unwrap_or(-1);
    let retryable_height = committed_tip
        .checked_add(1)
        .context("Marketplace committed tip has no representable successor")?;
    let invalid_registry_suffix: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
               SELECT 1 FROM "TraceAlkane"
               WHERE created_at_height IS NULL
                  OR created_at_height < 0
                  OR created_at_height > $1
           )"#,
    )
    .bind(retryable_height)
    .fetch_one(pool)
    .await?;
    if invalid_registry_suffix {
        bail!("TraceAlkane contains rows outside the committed or retryable height range");
    }
    let invalid_inventory_suffix: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
               SELECT 1 FROM "TraceBalanceUtxo"
               WHERE block_height < 0
                  OR block_height > $1
                  OR spent_at_height > $2
           )"#,
    )
    .bind(retryable_height)
    .bind(committed_tip)
    .fetch_one(pool)
    .await?;
    if invalid_inventory_suffix {
        bail!("TraceBalanceUtxo contains a non-retryable row or an uncommitted spend mutation");
    }
    let invalid_staging_suffix: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
               SELECT 1 FROM "MarketplaceBalanceSpendStage"
               WHERE spend_height <> $1
           )"#,
    )
    .bind(retryable_height)
    .fetch_one(pool)
    .await?;
    if invalid_staging_suffix {
        bail!("spend staging contains a row outside the single retryable height");
    }

    let progress_mismatch: bool = sqlx::query_scalar(
        r#"SELECT EXISTS (
               SELECT 1
               FROM "MarketplaceCanonicalCommit" AS commit
               FULL OUTER JOIN "ProcessedBlocks" AS processed
                 ON processed."blockHeight"::BIGINT = commit.height
               WHERE commit.height IS NULL
                  OR processed."blockHeight" IS NULL
                  OR processed."blockHash" <> commit.block_hash
                  OR processed."isProcessing"
           )"#,
    )
    .fetch_one(pool)
    .await?;
    if progress_mismatch {
        bail!("ProcessedBlocks does not exactly mirror Marketplace canonical commitments");
    }

    if let Some(commit) = latest {
        let committed_staging_row: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
                   SELECT 1 FROM "MarketplaceBalanceSpendStage"
                   WHERE spend_height <= $1
               )"#,
        )
        .bind(i64::try_from(commit.height)?)
        .fetch_one(pool)
        .await?;
        if committed_staging_row {
            bail!("spend staging contains a row at or below the committed tip");
        }

        let mut verify_tx = pool.begin().await?;
        sqlx::query(r#"LOCK TABLE "TraceAlkane", "TraceBalanceUtxo" IN SHARE MODE"#)
            .execute(&mut *verify_tx)
            .await?;
        let height = i64::try_from(commit.height)?;
        let trace_alkane = trace_alkane_commitment(&mut verify_tx, height).await?;
        let trace_balance = trace_balance_utxo_commitment(
            &mut verify_tx,
            Some(height),
            height,
            "alkanes-marketplace-trace-balance-utxo-height-v1",
        )
        .await?;
        let active_inventory = trace_balance_utxo_commitment(
            &mut verify_tx,
            None,
            height,
            "alkanes-marketplace-active-inventory-v1",
        )
        .await?;
        if trace_alkane != commit.trace_alkane
            || trace_balance != commit.trace_balance_utxo
            || active_inventory != commit.active_inventory
        {
            bail!("latest Marketplace commitment does not match authoritative rows");
        }
        verify_tx.commit().await?;
    }

    let position: Option<(i64, String)> =
        sqlx::query_as("SELECT height, block_hash FROM indexer_position WHERE id = 1")
            .fetch_optional(pool)
            .await?;
    let result: Result<()> = match (position, latest) {
        (None, None) => Ok(()),
        (Some((height, hash)), Some(commit))
            if height >= 0 && commit.height == height as u64 && commit.block_hash == hash =>
        {
            Ok(())
        }
        (None, Some(_)) => bail!("canonical commitment exists without indexer_position"),
        (Some(_), None) => bail!(
            "legacy indexer_position has no Marketplace canonical commitment; full replay is required"
        ),
        (Some(_), Some(_)) => bail!(
            "indexer_position does not exactly match the latest Marketplace canonical commitment"
        ),
    };
    result?;
    serial_guard.commit().await?;
    Ok(())
}

pub async fn acquire_serial_lock(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(SERIAL_ADVISORY_LOCK_KEY)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Stage Bitcoin inputs without changing consumer-visible inventory. The matching UTXO rows are
/// marked spent only inside `publish_height`, alongside the commitment and progress records.
pub async fn stage_spent_outpoints(
    pool: &PgPool,
    height: u64,
    outpoints: &[(String, i32)],
) -> Result<()> {
    if outpoints.is_empty() {
        return Ok(());
    }
    let db_height = i32::try_from(height).context("spend height exceeds PostgreSQL INTEGER")?;
    let mut seen = HashSet::with_capacity(outpoints.len());
    let mut txids = Vec::with_capacity(outpoints.len());
    let mut vouts = Vec::with_capacity(outpoints.len());
    for (txid, vout) in outpoints {
        if !valid_hash(txid) {
            bail!("spent outpoint txid is not canonical lowercase hexadecimal");
        }
        if *vout < 0 {
            bail!("spent outpoint vout cannot be negative");
        }
        if !seen.insert((txid.as_str(), *vout)) {
            bail!("a block cannot spend the same outpoint twice");
        }
        txids.push(txid.clone());
        vouts.push(*vout);
    }

    let mut tx = pool.begin().await?;
    sqlx::query(r#"LOCK TABLE "MarketplaceBalanceSpendStage" IN ROW EXCLUSIVE MODE"#)
        .execute(&mut *tx)
        .await?;
    let committed: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
               SELECT 1 FROM "MarketplaceCanonicalCommit" WHERE height >= $1
           )"#,
    )
    .bind(i64::from(db_height))
    .fetch_one(&mut *tx)
    .await?;
    if committed {
        bail!("cannot stage spends at or below a committed Marketplace height");
    }
    sqlx::query(
        r#"INSERT INTO "MarketplaceBalanceSpendStage"
              (spend_height, outpoint_txid, outpoint_vout)
           SELECT $1, staged.txid, staged.vout
           FROM UNNEST($2::TEXT[], $3::INTEGER[]) AS staged(txid, vout)"#,
    )
    .bind(db_height)
    .bind(txids)
    .bind(vouts)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn latest_commit(pool: &PgPool) -> Result<Option<CanonicalCommit>> {
    let row = sqlx::query(
        r#"SELECT height, block_hash, previous_block_hash, reorg_epoch,
                  producer_revision, schema_revision,
                  trace_alkane_row_count, trace_alkane_row_hash,
                  trace_balance_utxo_row_count, trace_balance_utxo_row_hash,
                  active_inventory_count, active_inventory_root, manifest_hash
           FROM "MarketplaceCanonicalCommit" ORDER BY height DESC LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await?;
    row.map(commit_from_row).transpose()
}

pub async fn commit_at_height(pool: &PgPool, height: u64) -> Result<Option<CanonicalCommit>> {
    let row = sqlx::query(
        r#"SELECT height, block_hash, previous_block_hash, reorg_epoch,
                  producer_revision, schema_revision,
                  trace_alkane_row_count, trace_alkane_row_hash,
                  trace_balance_utxo_row_count, trace_balance_utxo_row_hash,
                  active_inventory_count, active_inventory_root, manifest_hash
           FROM "MarketplaceCanonicalCommit" WHERE height = $1"#,
    )
    .bind(i64::try_from(height).context("height exceeds PostgreSQL BIGINT")?)
    .fetch_optional(pool)
    .await?;
    row.map(commit_from_row).transpose()
}

pub async fn commits_descending(pool: &PgPool) -> Result<Vec<CanonicalCommit>> {
    let rows = sqlx::query(
        r#"SELECT height, block_hash, previous_block_hash, reorg_epoch,
                  producer_revision, schema_revision,
                  trace_alkane_row_count, trace_alkane_row_hash,
                  trace_balance_utxo_row_count, trace_balance_utxo_row_hash,
                  active_inventory_count, active_inventory_root, manifest_hash
           FROM "MarketplaceCanonicalCommit" ORDER BY height DESC"#,
    )
    .fetch_all(pool)
    .await?;
    let commits: Vec<_> = rows
        .into_iter()
        .map(commit_from_row)
        .collect::<Result<_>>()?;
    validate_commit_chain_descending(&commits)?;
    Ok(commits)
}

fn commit_from_row(row: sqlx::postgres::PgRow) -> Result<CanonicalCommit> {
    let height: i64 = row.try_get("height")?;
    let epoch: i64 = row.try_get("reorg_epoch")?;
    let alkane_count: i64 = row.try_get("trace_alkane_row_count")?;
    let utxo_count: i64 = row.try_get("trace_balance_utxo_row_count")?;
    let inventory_count: i64 = row.try_get("active_inventory_count")?;
    if height < 0 || epoch < 0 || alkane_count < 0 || utxo_count < 0 || inventory_count < 0 {
        bail!("negative value in Marketplace canonical commitment");
    }
    let commit = CanonicalCommit {
        height: height as u64,
        block_hash: row.try_get("block_hash")?,
        previous_block_hash: row.try_get("previous_block_hash")?,
        reorg_epoch: epoch as u64,
        producer_revision: row.try_get("producer_revision")?,
        schema_revision: row.try_get("schema_revision")?,
        trace_alkane: RowCommitment {
            count: alkane_count as u64,
            hash: row.try_get("trace_alkane_row_hash")?,
        },
        trace_balance_utxo: RowCommitment {
            count: utxo_count as u64,
            hash: row.try_get("trace_balance_utxo_row_hash")?,
        },
        active_inventory: RowCommitment {
            count: inventory_count as u64,
            hash: row.try_get("active_inventory_root")?,
        },
        manifest_hash: row.try_get("manifest_hash")?,
    };
    validate_commit_record(&commit)?;
    Ok(commit)
}

pub async fn prepare_height(
    pool: &PgPool,
    height: u64,
    block_hash: &str,
    previous_block_hash: &str,
) -> Result<PrepareOutcome> {
    validate_block_link(height, block_hash, previous_block_hash)?;
    let latest = latest_commit(pool).await?;
    if let Some(existing) = commit_at_height(pool, height).await? {
        if existing.block_hash == block_hash && existing.previous_block_hash == previous_block_hash
        {
            if latest.as_ref().map(|commit| commit.height) == Some(height) {
                return Ok(PrepareOutcome::AlreadyCommitted);
            }
            bail!("refusing to reprocess immutable committed height below the canonical tip");
        }
        bail!("canonical commitment conflict at height {height}; reconcile the fork first");
    }

    match (height, latest.as_ref()) {
        (0, None) => {}
        (0, Some(_)) => bail!("height zero cannot be appended to a non-empty commitment chain"),
        (_, Some(parent))
            if parent.height + 1 == height && parent.block_hash == previous_block_hash => {}
        (_, Some(parent)) if parent.height + 1 != height => bail!(
            "non-contiguous canonical append: latest height is {}, requested height is {height}",
            parent.height
        ),
        (_, Some(_)) => bail!("previous block hash does not match the committed parent"),
        (_, None) => bail!("first Marketplace canonical commitment must be height zero"),
    }

    reset_uncommitted_height(pool, height).await?;
    Ok(PrepareOutcome::Ready)
}

async fn rebuild_balance_aggregates(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    for table in [
        r#"TraceAlkaneBalance"#,
        r#"TraceBalanceAggregate"#,
        r#"TraceHolder"#,
        r#"TraceHolderCount"#,
    ] {
        let statement = format!(r#"DELETE FROM "{table}""#);
        sqlx::query(&statement).execute(&mut **tx).await?;
    }

    sqlx::query(
        r#"INSERT INTO "TraceAlkaneBalance"
              (address, alkane_block, alkane_tx, balance, last_updated_block,
               last_updated_tx, last_updated_timestamp)
           SELECT address, alkane_block, alkane_tx, SUM(amount), MAX(block_height),
                  MAX(outpoint_txid), transaction_timestamp()
           FROM "TraceBalanceUtxo" WHERE NOT spent
           GROUP BY address, alkane_block, alkane_tx"#,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO "TraceBalanceAggregate"
              (address, alkane_block, alkane_tx, total_amount, updated_at)
           SELECT address, alkane_block, alkane_tx, SUM(amount), transaction_timestamp()
           FROM "TraceBalanceUtxo" WHERE NOT spent
           GROUP BY address, alkane_block, alkane_tx"#,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO "TraceHolder"
              (alkane_block, alkane_tx, address, total_amount, updated_at)
           SELECT alkane_block, alkane_tx, address, SUM(amount), transaction_timestamp()
           FROM "TraceBalanceUtxo" WHERE NOT spent
           GROUP BY alkane_block, alkane_tx, address
           HAVING SUM(amount) > 0"#,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"INSERT INTO "TraceHolderCount" (alkane_block, alkane_tx, count, updated_at)
           SELECT alkane_block, alkane_tx, COUNT(*), transaction_timestamp()
           FROM "TraceHolder" GROUP BY alkane_block, alkane_tx"#,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn reset_uncommitted_height(pool: &PgPool, height: u64) -> Result<()> {
    let height = i64::try_from(height).context("height exceeds PostgreSQL BIGINT")?;
    let mut tx = pool.begin().await?;
    sqlx::query(
        r#"LOCK TABLE "MarketplaceBalanceSpendStage", "TraceAlkane", "TraceBalanceUtxo"
           IN SHARE ROW EXCLUSIVE MODE"#,
    )
    .execute(&mut *tx)
    .await?;
    let committed: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM "MarketplaceCanonicalCommit" WHERE height >= $1)"#,
    )
    .bind(height)
    .fetch_one(&mut *tx)
    .await?;
    if committed {
        bail!("cannot reset rows at or below a committed height");
    }
    sqlx::query(r#"DELETE FROM "MarketplaceBalanceSpendStage" WHERE spend_height = $1"#)
        .bind(height)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM "TraceBalanceUtxo" WHERE block_height = $1"#)
        .bind(height)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM "TraceAlkane" WHERE created_at_height = $1"#)
        .bind(height)
        .execute(&mut *tx)
        .await?;
    rebuild_balance_aggregates(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

async fn trace_alkane_commitment(
    tx: &mut Transaction<'_, Postgres>,
    height: i64,
) -> Result<RowCommitment> {
    let rows = sqlx::query(
        r#"SELECT alkane_block::TEXT AS alkane_block, alkane_tx::TEXT AS alkane_tx,
                  created_at_block::TEXT AS created_at_block, created_at_tx,
                  created_at_height::TEXT AS created_at_height
           FROM "TraceAlkane" WHERE created_at_height = $1
           ORDER BY alkane_block, alkane_tx, created_at_tx"#,
    )
    .bind(height)
    .fetch_all(&mut **tx)
    .await?;
    let mut digest = RowDigest::new("alkanes-marketplace-trace-alkane-height-v1");
    for row in rows {
        digest.push(&[
            row.try_get("alkane_block")?,
            row.try_get("alkane_tx")?,
            row.try_get("created_at_block")?,
            row.try_get("created_at_tx")?,
            row.try_get("created_at_height")?,
        ]);
    }
    Ok(digest.finish())
}

async fn trace_balance_utxo_commitment(
    tx: &mut Transaction<'_, Postgres>,
    height: Option<i64>,
    as_of_height: i64,
    domain: &str,
) -> Result<RowCommitment> {
    let (statement, filter_height) = match height {
        Some(height) => (
            r#"SELECT event_kind, outpoint_txid, outpoint_vout, address, alkane_block,
                      alkane_tx, amount, block_height, spent, spent_at_height
               FROM (
                   SELECT 'create'::TEXT AS event_kind, outpoint_txid,
                          outpoint_vout::TEXT AS outpoint_vout, address,
                          alkane_block::TEXT AS alkane_block, alkane_tx::TEXT AS alkane_tx,
                          amount::TEXT AS amount, block_height::TEXT AS block_height,
                          'false'::TEXT AS spent, ''::TEXT AS spent_at_height
                   FROM "TraceBalanceUtxo" WHERE block_height = $1
                   UNION ALL
                   SELECT 'spend'::TEXT AS event_kind, outpoint_txid,
                          outpoint_vout::TEXT AS outpoint_vout, address,
                          alkane_block::TEXT AS alkane_block, alkane_tx::TEXT AS alkane_tx,
                          amount::TEXT AS amount, block_height::TEXT AS block_height,
                          'true'::TEXT AS spent, spent_at_height::TEXT AS spent_at_height
                   FROM "TraceBalanceUtxo" WHERE spent_at_height = $1
               ) AS events
               ORDER BY event_kind, outpoint_txid, outpoint_vout, alkane_block, alkane_tx"#,
            height,
        ),
        None => (
            r#"SELECT 'active'::TEXT AS event_kind, outpoint_txid,
                      outpoint_vout::TEXT AS outpoint_vout, address,
                      alkane_block::TEXT AS alkane_block, alkane_tx::TEXT AS alkane_tx,
                      amount::TEXT AS amount, block_height::TEXT AS block_height,
                      'false'::TEXT AS spent, ''::TEXT AS spent_at_height
               FROM "TraceBalanceUtxo"
               WHERE block_height <= $1
                 AND (spent_at_height IS NULL OR spent_at_height > $1)
               ORDER BY outpoint_txid, outpoint_vout, alkane_block, alkane_tx"#,
            as_of_height,
        ),
    };
    let rows = sqlx::query(statement)
        .bind(filter_height)
        .fetch_all(&mut **tx)
        .await?;
    let mut digest = RowDigest::new(domain);
    for row in rows {
        digest.push(&[
            row.try_get("event_kind")?,
            row.try_get("outpoint_txid")?,
            row.try_get("outpoint_vout")?,
            row.try_get("address")?,
            row.try_get("alkane_block")?,
            row.try_get("alkane_tx")?,
            row.try_get("amount")?,
            row.try_get("block_height")?,
            row.try_get("spent")?,
            row.try_get("spent_at_height")?,
        ]);
    }
    Ok(digest.finish())
}

pub async fn publish_height(
    pool: &PgPool,
    height: u64,
    block_hash: &str,
    previous_block_hash: &str,
    block_timestamp: DateTime<Utc>,
) -> Result<PublishOutcome> {
    validate_block_link(height, block_hash, previous_block_hash)?;
    let db_height = i64::try_from(height).context("height exceeds PostgreSQL BIGINT")?;
    let db_height_i32 = i32::try_from(height).context("height exceeds PostgreSQL INTEGER")?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"LOCK TABLE "MarketplaceBalanceSpendStage" IN SHARE ROW EXCLUSIVE MODE"#)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"LOCK TABLE "TraceAlkane", "TraceBalanceUtxo" IN SHARE MODE"#)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"LOCK TABLE "MarketplaceCanonicalCommit", "MarketplaceCanonicalState",
                  "ProcessedBlocks", indexer_position
           IN SHARE ROW EXCLUSIVE MODE"#,
    )
    .execute(&mut *tx)
    .await?;

    let existing = sqlx::query(
        r#"SELECT manifest_hash, block_hash, previous_block_hash
           FROM "MarketplaceCanonicalCommit" WHERE height = $1"#,
    )
    .bind(db_height)
    .fetch_optional(&mut *tx)
    .await?;

    let parent: Option<(i64, String)> = sqlx::query_as(
        r#"SELECT height, block_hash FROM "MarketplaceCanonicalCommit"
           ORDER BY height DESC LIMIT 1"#,
    )
    .fetch_optional(&mut *tx)
    .await?;
    match (existing.as_ref(), height, parent.as_ref()) {
        (Some(_), _, Some((tip_height, tip_hash)))
            if *tip_height == db_height && tip_hash == block_hash => {}
        (Some(_), _, _) => bail!("existing Marketplace commitment is not the canonical tip"),
        (None, 0, None) => {}
        (None, 0, Some(_)) => bail!("cannot publish height zero to a non-empty chain"),
        (None, _, Some((parent_height, parent_hash)))
            if *parent_height == db_height - 1 && parent_hash == previous_block_hash => {}
        (None, _, Some(_)) => bail!("canonical parent changed before publication"),
        (None, _, None) => bail!("first Marketplace canonical commitment must be height zero"),
    }

    let epoch: i64 = sqlx::query_scalar(
        r#"SELECT reorg_epoch FROM "MarketplaceCanonicalState" WHERE id = 1 FOR UPDATE"#,
    )
    .fetch_one(&mut *tx)
    .await?;
    if epoch < 0 {
        bail!("negative Marketplace reorg epoch");
    }
    let staged_spends: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
               SELECT 1 FROM "MarketplaceBalanceSpendStage" WHERE spend_height = $1
           )"#,
    )
    .bind(db_height_i32)
    .fetch_one(&mut *tx)
    .await?;
    if existing.is_some() && staged_spends {
        bail!("an already committed height cannot retain staged spends");
    }
    if existing.is_none() && staged_spends {
        let already_spent: bool = sqlx::query_scalar(
            r#"SELECT EXISTS (
                   SELECT 1
                   FROM "MarketplaceBalanceSpendStage" AS staged
                   JOIN "TraceBalanceUtxo" AS inventory
                     ON inventory.outpoint_txid = staged.outpoint_txid
                    AND inventory.outpoint_vout = staged.outpoint_vout
                   WHERE staged.spend_height = $1 AND inventory.spent
               )"#,
        )
        .bind(db_height_i32)
        .fetch_one(&mut *tx)
        .await?;
        if already_spent {
            bail!("canonical block attempts to spend an already-spent Marketplace outpoint");
        }
        sqlx::query_scalar::<_, String>(
            "SELECT set_config('alkanes.marketplace_publish_height', $1, true)",
        )
        .bind(db_height_i32.to_string())
        .fetch_one(&mut *tx)
        .await?;
        let updated = sqlx::query(
            r#"UPDATE "TraceBalanceUtxo" AS inventory
               SET spent = TRUE, spent_at_height = $1
               FROM "MarketplaceBalanceSpendStage" AS staged
               WHERE staged.spend_height = $1
                 AND inventory.outpoint_txid = staged.outpoint_txid
                 AND inventory.outpoint_vout = staged.outpoint_vout
                 AND NOT inventory.spent"#,
        )
        .bind(db_height_i32)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() > 0 {
            rebuild_balance_aggregates(&mut tx).await?;
        }
    }
    let trace_alkane = trace_alkane_commitment(&mut tx, db_height).await?;
    let trace_balance_utxo = trace_balance_utxo_commitment(
        &mut tx,
        Some(db_height),
        db_height,
        "alkanes-marketplace-trace-balance-utxo-height-v1",
    )
    .await?;
    let active_inventory = trace_balance_utxo_commitment(
        &mut tx,
        None,
        db_height,
        "alkanes-marketplace-active-inventory-v1",
    )
    .await?;
    let mut candidate = CanonicalCommit {
        height,
        block_hash: block_hash.to_owned(),
        previous_block_hash: previous_block_hash.to_owned(),
        reorg_epoch: epoch as u64,
        producer_revision: PRODUCER_REVISION.to_owned(),
        schema_revision: SCHEMA_REVISION.to_owned(),
        trace_alkane,
        trace_balance_utxo,
        active_inventory,
        manifest_hash: String::new(),
    };
    candidate.manifest_hash = manifest_hash(&candidate);

    if let Some(row) = existing {
        let existing_manifest: String = row.try_get("manifest_hash")?;
        let existing_hash: String = row.try_get("block_hash")?;
        let existing_previous: String = row.try_get("previous_block_hash")?;
        if existing_manifest == candidate.manifest_hash
            && existing_hash == candidate.block_hash
            && existing_previous == candidate.previous_block_hash
        {
            tx.commit().await?;
            return Ok(PublishOutcome::AlreadyCommitted);
        }
        bail!("conflicting immutable Marketplace commitment at height {height}");
    }

    let position: Option<(i64, String)> =
        sqlx::query_as("SELECT height, block_hash FROM indexer_position WHERE id = 1 FOR UPDATE")
            .fetch_optional(&mut *tx)
            .await?;
    match (height, position) {
        (0, None) => {}
        (_, Some((position_height, position_hash)))
            if position_height == db_height - 1 && position_hash == previous_block_hash => {}
        _ => bail!("indexer_position is not the exact parent of the height being published"),
    }

    sqlx::query(
        r#"INSERT INTO "MarketplaceCanonicalCommit"
              (height, block_hash, previous_block_hash, reorg_epoch,
               producer_revision, schema_revision,
               trace_alkane_row_count, trace_alkane_row_hash,
               trace_balance_utxo_row_count, trace_balance_utxo_row_hash,
               active_inventory_count, active_inventory_root, manifest_hash)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)"#,
    )
    .bind(db_height)
    .bind(&candidate.block_hash)
    .bind(&candidate.previous_block_hash)
    .bind(epoch)
    .bind(&candidate.producer_revision)
    .bind(&candidate.schema_revision)
    .bind(i64::try_from(candidate.trace_alkane.count)?)
    .bind(&candidate.trace_alkane.hash)
    .bind(i64::try_from(candidate.trace_balance_utxo.count)?)
    .bind(&candidate.trace_balance_utxo.hash)
    .bind(i64::try_from(candidate.active_inventory.count)?)
    .bind(&candidate.active_inventory.hash)
    .bind(&candidate.manifest_hash)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"INSERT INTO "ProcessedBlocks"
              ("blockHeight", "blockHash", "timestamp", "isProcessing")
           VALUES ($1, $2, $3, false)"#,
    )
    .bind(db_height_i32)
    .bind(block_hash)
    .bind(block_timestamp)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO indexer_position (id, height, block_hash) VALUES (1, $1, $2)\n         ON CONFLICT (id) DO UPDATE SET height = EXCLUDED.height, block_hash = EXCLUDED.block_hash",
    )
    .bind(db_height)
    .bind(block_hash)
    .execute(&mut *tx)
    .await?;
    sqlx::query(r#"DELETE FROM "MarketplaceBalanceSpendStage" WHERE spend_height = $1"#)
        .bind(db_height_i32)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(PublishOutcome::Published)
}

/// Delete a committed suffix only after the caller has independently matched the ancestor hash
/// against Bitcoin Core. The row immutability triggers accept deletion only in this transaction.
pub async fn rollback_to_common_ancestor(
    pool: &PgPool,
    ancestor: Option<(u64, String)>,
) -> Result<u64> {
    if let Some((_, ref hash)) = ancestor {
        if !valid_hash(hash) {
            bail!("common-ancestor hash is not canonical lowercase hexadecimal");
        }
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL alkanes.marketplace_rollback = 'on'")
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"LOCK TABLE "MarketplaceBalanceSpendStage", "TraceAlkane", "TraceBalanceUtxo",
                  "MarketplaceCanonicalCommit", "MarketplaceCanonicalState",
                  "ProcessedBlocks", indexer_position
           IN ACCESS EXCLUSIVE MODE"#,
    )
    .execute(&mut *tx)
    .await?;

    let ancestor_height = match ancestor {
        Some((height, expected_hash)) => {
            let actual: Option<String> = sqlx::query_scalar(
                r#"SELECT block_hash FROM "MarketplaceCanonicalCommit" WHERE height = $1"#,
            )
            .bind(i64::try_from(height)?)
            .fetch_optional(&mut *tx)
            .await?;
            if actual.as_deref() != Some(expected_hash.as_str()) {
                bail!("hash-verified common ancestor changed before rollback");
            }
            i64::try_from(height)?
        }
        None => -1,
    };

    let next_epoch: i64 = sqlx::query_scalar(
        r#"UPDATE "MarketplaceCanonicalState" SET reorg_epoch = reorg_epoch + 1
           WHERE id = 1 RETURNING reorg_epoch"#,
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(r#"DELETE FROM "TraceBalanceUtxo" WHERE block_height > $1"#)
        .bind(ancestor_height)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"UPDATE "TraceBalanceUtxo"
           SET spent = FALSE, spent_at_height = NULL
           WHERE spent_at_height > $1"#,
    )
    .bind(ancestor_height)
    .execute(&mut *tx)
    .await?;
    sqlx::query(r#"DELETE FROM "MarketplaceBalanceSpendStage" WHERE spend_height > $1"#)
        .bind(ancestor_height)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM "TraceAlkane" WHERE created_at_height > $1"#)
        .bind(ancestor_height)
        .execute(&mut *tx)
        .await?;
    rebuild_balance_aggregates(&mut tx).await?;
    sqlx::query(r#"DELETE FROM "ProcessedBlocks" WHERE "blockHeight" > $1"#)
        .bind(ancestor_height)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM "MarketplaceCanonicalCommit" WHERE height > $1"#)
        .bind(ancestor_height)
        .execute(&mut *tx)
        .await?;

    if ancestor_height >= 0 {
        let ancestor_commit = commit_from_row(
            sqlx::query(
                r#"SELECT height, block_hash, previous_block_hash, reorg_epoch,
                          producer_revision, schema_revision,
                          trace_alkane_row_count, trace_alkane_row_hash,
                          trace_balance_utxo_row_count, trace_balance_utxo_row_hash,
                          active_inventory_count, active_inventory_root, manifest_hash
                   FROM "MarketplaceCanonicalCommit" WHERE height = $1"#,
            )
            .bind(ancestor_height)
            .fetch_one(&mut *tx)
            .await?,
        )?;
        let trace_alkane = trace_alkane_commitment(&mut tx, ancestor_height).await?;
        let trace_balance = trace_balance_utxo_commitment(
            &mut tx,
            Some(ancestor_height),
            ancestor_height,
            "alkanes-marketplace-trace-balance-utxo-height-v1",
        )
        .await?;
        let active_inventory = trace_balance_utxo_commitment(
            &mut tx,
            None,
            ancestor_height,
            "alkanes-marketplace-active-inventory-v1",
        )
        .await?;
        if trace_alkane != ancestor_commit.trace_alkane
            || trace_balance != ancestor_commit.trace_balance_utxo
            || active_inventory != ancestor_commit.active_inventory
        {
            bail!("rollback result does not match the hash-committed ancestor state");
        }
        sqlx::query(
            "INSERT INTO indexer_position (id, height, block_hash) VALUES (1, $1, $2)\n             ON CONFLICT (id) DO UPDATE SET height = EXCLUDED.height, block_hash = EXCLUDED.block_hash",
        )
        .bind(ancestor_height)
        .bind(ancestor_commit.block_hash)
        .execute(&mut *tx)
        .await?;
    } else {
        let authority_rows: i64 = sqlx::query_scalar(
            r#"SELECT
                   (SELECT COUNT(*) FROM "TraceAlkane") +
                   (SELECT COUNT(*) FROM "TraceBalanceUtxo")"#,
        )
        .fetch_one(&mut *tx)
        .await?;
        if authority_rows != 0 {
            bail!("full rollback left Marketplace authority rows behind");
        }
        sqlx::query("DELETE FROM indexer_position WHERE id = 1")
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    u64::try_from(next_epoch).map_err(|_| anyhow!("negative reorg epoch after rollback"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(byte: char) -> String {
        std::iter::repeat(byte).take(64).collect()
    }

    fn candidate(height: u64, block: char, previous: char) -> CanonicalCommit {
        let rows = deterministic_rows_digest(
            "alkanes-marketplace-test-v1",
            vec![vec!["b".into(), "2".into()], vec!["a".into(), "1".into()]],
        );
        let mut commit = CanonicalCommit {
            height,
            block_hash: hash(block),
            previous_block_hash: if height == 0 {
                ZERO_BLOCK_HASH.into()
            } else {
                hash(previous)
            },
            reorg_epoch: 0,
            producer_revision: std::iter::repeat('1').take(40).collect(),
            schema_revision: SCHEMA_REVISION.into(),
            trace_alkane: rows.clone(),
            trace_balance_utxo: rows.clone(),
            active_inventory: rows,
            manifest_hash: String::new(),
        };
        commit.manifest_hash = manifest_hash(&commit);
        commit
    }

    #[test]
    fn ordered_digest_is_deterministic_and_field_bound() {
        let forward = vec![
            vec!["outpoint-b".into(), "2".into()],
            vec!["outpoint-a".into(), "11".into()],
        ];
        let mut reverse = forward.clone();
        reverse.reverse();
        assert_eq!(
            deterministic_rows_digest("domain", forward),
            deterministic_rows_digest("domain", reverse)
        );
        assert_ne!(
            deterministic_rows_digest("domain", vec![vec!["ab".into(), "c".into()]]),
            deterministic_rows_digest("domain", vec![vec!["a".into(), "bc".into()]])
        );
        assert_eq!(
            deterministic_rows_digest(
                "domain",
                vec![vec!["b".into(), "2".into()], vec!["a".into(), "1".into()],],
            )
            .hash,
            "abc7d197a4f756fbd71d96257424da42d666904f82088e8629847910966fcc3b"
        );
    }

    #[test]
    fn utxo_digest_binds_spend_status_and_height() {
        let active = deterministic_rows_digest(
            "alkanes-marketplace-trace-balance-utxo-height-v1",
            vec![vec!["tx".into(), "0".into(), "false".into(), "".into()]],
        );
        let spent = deterministic_rows_digest(
            "alkanes-marketplace-trace-balance-utxo-height-v1",
            vec![vec!["tx".into(), "0".into(), "true".into(), "101".into()]],
        );
        let spent_later = deterministic_rows_digest(
            "alkanes-marketplace-trace-balance-utxo-height-v1",
            vec![vec!["tx".into(), "0".into(), "true".into(), "102".into()]],
        );
        assert_ne!(active.hash, spent.hash);
        assert_ne!(spent.hash, spent_later.hash);
    }

    #[test]
    fn manifest_changes_for_every_consensus_field() {
        let base = candidate(1, 'b', 'a');
        let mut changed = base.clone();
        changed.active_inventory.count += 1;
        assert_ne!(manifest_hash(&base), manifest_hash(&changed));
        changed = base.clone();
        changed.reorg_epoch += 1;
        assert_ne!(manifest_hash(&base), manifest_hash(&changed));
        changed = base.clone();
        changed.producer_revision.push('x');
        assert_ne!(manifest_hash(&base), manifest_hash(&changed));
    }

    #[test]
    fn invalid_hashes_and_genesis_link_fail_closed() {
        assert!(validate_block_link(1, &hash('a'), &hash('b')).is_ok());
        assert!(validate_block_link(1, &hash('A'), &hash('b')).is_err());
        assert!(validate_block_link(0, &hash('a'), &hash('b')).is_err());
        assert!(validate_block_link(0, &hash('a'), ZERO_BLOCK_HASH).is_ok());
    }

    #[test]
    fn exact_retry_is_idempotent_but_conflict_changes_manifest() {
        let first = candidate(1, 'b', 'a');
        let retry = candidate(1, 'b', 'a');
        assert_eq!(first.manifest_hash, retry.manifest_hash);
        let conflict = candidate(1, 'c', 'a');
        assert_ne!(first.manifest_hash, conflict.manifest_hash);
    }

    #[test]
    fn chain_validation_rejects_gap_broken_link_and_epoch_regression() {
        let genesis = candidate(0, 'a', '0');
        let one = candidate(1, 'b', 'a');
        let two = candidate(2, 'c', 'b');
        assert!(
            validate_commit_chain_descending(&[two.clone(), one.clone(), genesis.clone(),]).is_ok()
        );

        assert!(validate_commit_chain_descending(&[two.clone(), genesis.clone()]).is_err());
        let mut broken = two.clone();
        broken.previous_block_hash = hash('d');
        broken.manifest_hash = manifest_hash(&broken);
        assert!(validate_commit_chain_descending(&[broken, one.clone(), genesis.clone()]).is_err());
        let mut parent_epoch = one.clone();
        parent_epoch.reorg_epoch = 2;
        parent_epoch.manifest_hash = manifest_hash(&parent_epoch);
        assert!(validate_commit_chain_descending(&[two, parent_epoch, genesis]).is_err());
    }

    #[test]
    fn chain_validation_rejects_invalid_non_tip_revision_and_manifest() {
        let tip = candidate(1, 'b', 'a');
        let mut genesis = candidate(0, 'a', '0');
        genesis.producer_revision = "unpinned".into();
        genesis.manifest_hash = manifest_hash(&genesis);
        assert!(validate_commit_chain_descending(&[tip.clone(), genesis]).is_err());

        let mut genesis = candidate(0, 'a', '0');
        genesis.trace_alkane.count += 1;
        assert!(validate_commit_chain_descending(&[tip, genesis]).is_err());
    }

    #[test]
    fn transform_failure_has_no_publish_outcome() {
        fn stage(fail_after: Option<usize>) -> Result<CanonicalCommit> {
            let mut rows = Vec::new();
            for index in 0..3 {
                if fail_after == Some(index) {
                    bail!("injected transform failure");
                }
                rows.push(vec![index.to_string()]);
            }
            let mut commit = candidate(1, 'b', 'a');
            commit.trace_balance_utxo = deterministic_rows_digest("stage", rows);
            commit.manifest_hash = manifest_hash(&commit);
            Ok(commit)
        }
        assert!(stage(Some(1)).is_err());
        assert_eq!(stage(None).unwrap().trace_balance_utxo.count, 3);
    }

    #[test]
    fn same_height_and_deep_forks_choose_only_hash_verified_ancestor() {
        let chain = [
            candidate(0, 'a', '0'),
            candidate(1, 'b', 'a'),
            candidate(2, 'c', 'b'),
        ];
        let same_height_fork = [(0, hash('a')), (1, hash('b')), (2, hash('d'))];
        let same_ancestor = chain
            .iter()
            .rev()
            .find(|commit| {
                same_height_fork
                    .iter()
                    .any(|(height, block)| *height == commit.height && block == &commit.block_hash)
            })
            .unwrap();
        assert_eq!(same_ancestor.height, 1);

        let deep_fork = [(0, hash('a')), (1, hash('e')), (2, hash('f'))];
        let deep_ancestor = chain
            .iter()
            .rev()
            .find(|commit| {
                deep_fork
                    .iter()
                    .any(|(height, block)| *height == commit.height && block == &commit.block_hash)
            })
            .unwrap();
        assert_eq!(deep_ancestor.height, 0);
    }

    #[test]
    fn schema_guards_commits_and_authoritative_rows_against_late_mutation() {
        assert!(CREATE_COMMIT_GUARD_FUNCTION.contains("immutable"));
        assert!(CREATE_STATE_GUARD_FUNCTION.contains("advance by one"));
        assert!(CREATE_TRACE_ALKANE_GUARD_FUNCTION.contains("MarketplaceCanonicalCommit"));
        assert!(CREATE_TRACE_UTXO_GUARD_FUNCTION.contains("MarketplaceCanonicalCommit"));
        assert!(CREATE_COMMIT_TABLE.contains("previous_block_hash"));
        assert!(CREATE_COMMIT_TABLE.contains("active_inventory_root"));
        assert!(!CREATE_TRACE_ALKANE_GUARD_FUNCTION.contains("COALESCE(NEW, OLD)"));
        assert!(!CREATE_TRACE_UTXO_GUARD_FUNCTION.contains("COALESCE(NEW, OLD)"));
        assert!(CREATE_PROCESSED_BLOCK_GUARD_FUNCTION.contains("immutable"));
        assert!(CREATE_POSITION_GUARD_FUNCTION.contains("advance by one"));
        assert!(CREATE_COMMIT_COMPLETENESS_FUNCTION.contains("matching state and progress"));
        assert!(CREATE_SPEND_STAGE_TABLE.contains("MarketplaceBalanceSpendStage"));
        assert!(ADD_SPENT_HEIGHT_CONSTRAINT.contains("spent_at_height >= block_height"));
        assert!(CREATE_SPEND_COMMIT_GUARD_FUNCTION.contains("requires its canonical commitment"));
    }
}
