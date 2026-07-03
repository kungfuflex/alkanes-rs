# Alkanes indexer security audit

Scope: the alkanes-rs indexer (`src/`, `crates/protorune*`, `crates/alkanes-support`)
and the metashrew `AtomicPointer` state layer it depends on. Focus areas requested:
arbitrary token minting, indexer panics (wedge vs. graceful revert), state rollback on
revert, and explicit invariants.

Method: full read of the consensus hot path (message → protostone → reconcile →
balance-sheet → VM extcall) plus targeted analysis of the block-parse, view/simulate,
and diesel-precompile surfaces. Findings that could be driven in the wasm test harness
were validated with a PoC.

---

## 1. Token inflation / arbitrary mint — GATE IS SOUND (post inflation fix)

The only intended supply growth is a contract minting **itself** (`AlkaneId(b,t)` may
increase the balance of token `(b,t)` without bound). I traced every balance-writing
site and confirmed this boundary holds:

- The real conservation gate is `checked_debit_with_minting` (`src/utils.rs:85`), which
  permits an underflow (a "mint") **only** when `transfer.id == from`. It backs both
  `debit_balances` (`src/message.rs:131`, `from = myself`) and `transfer_from`
  (extcall incoming `from = myself`; `Saveable::save` return `from = submyself`). At
  every call site `from` is the entity actually spending, so a contract can over-emit
  only its own id.
- `debit_mintable`/`reconcile` (`crates/protorune/src/balance_sheet.rs:101,124`) do **not**
  enforce conservation for alkanes (every alkane is `mintable_in_protocol`); they clamp
  instead of erroring. This is fine only because `debit_balances` runs ahead of them and
  bounds `response.alkanes` to `myself`'s real inventory. Confirmed, not a bug.
- The `pipe` all-or-nothing fix + `balances_snapshot` restore in `process_message`
  (`crates/protorune/src/protostone.rs:140,190`) close the "two stores, one rollback"
  duplication class. `src/tests/inflation_poc.rs` asserts conservation across single-tx,
  cross-tx, and cascade variants.
- Identity is unforgeable: `sequence` is monotonic, CREATERESERVED refuses to overwrite,
  `myself` is set by the runtime, not the contract (`src/vm/utils.rs:93`).
- The `to == AlkaneId(0,0)` early-return in `transfer_from` (`src/utils.rs:127`) skips
  **both** debit and credit, and is unreachable with a token-holding `from` (the incoming
  path reverts on the empty binary; `caller` is never `(0,0)` on the save path). Dead
  defensive code, not exploitable.
- DIESEL mint (`(2,0)` opcode 77) is a self-mint bounded by contract-internal per-block/
  per-tx storage keys. The native fast path is **off by default** (`#[cfg(feature =
  "fastpath")]`); its comments record an un-root-caused ~25-DIESEL divergence vs. the wasm
  path — do not ship `fastpath` until that is reproduced in CI (`src/vm/utils.rs:282`).

**No arbitrary-mint bug was found.** The self-mint case cannot be pushed past a contract
minting its own token.

---

## 2. FIXED + PoC — unchecked add in `transfer_from` (panic-wedge / balance-wrap)

`src/utils.rs:138` credited the recipient with a **raw `+`**, while `credit_balances`
right above it uses `checked_add`:

```rust
to_pointer.set_value::<u128>(to_pointer.get_value::<u128>() + transfer.value);
```

`transfer_from` is the credit path for every extcall (incoming, and the `Saveable::save`
return). A contract `C` can forward `C:MAX` (self-minted, allowed) twice in one tx into a
contract `D` that hoards its incoming; the second credit computes `MAX + MAX`. In a build
with overflow-checks (any debug/test build) this **panics inside `index_block`** — and
because metashrew retries a panicking block forever, that **wedges the whole indexer** on
a cheap crafted tx. In the release profile (`[profile.release]`, overflow-checks off) it
silently **wraps**, corrupting the balance ledger (a consensus break).

Fix: credit with `checked_add`, returning `anyhow!("balance overflow during
transfer_from")` so the condition becomes a graceful per-tx revert.

PoC: `src/tests/transfer_overflow_poc.rs::test_transfer_from_overflow_wedge`. Pre-fix it
panics at `src/utils.rs:138` (verified); post-fix the attack tx reverts cleanly, `D`
holds no `C`, and `C:MAX` is refunded intact (conservation). Passes.

---

## 3. Open panic / DoS findings (documented, fixes recommended — not yet applied)

The metashrew state layer makes any panic on the indexing hot path fatal: `commit()` on
an empty checkpoint stack `panic!`s, `set()` on an empty stack panics, and every typed
read (`get_value`) does `try_into().expect("incorrect length")` on any stored blob whose
length is non-zero and not exactly the type width. A panic aborts the block and metashrew
re-runs it identically → permanent wedge. So each item below is a wedge primitive.

| Sev | Where | Issue | Fix |
|-----|-------|-------|-----|
| HIGH | `crates/alkanes-support/src/storage.rs:21-24` | `StorageMap::parse` reads an attacker `u32` key/value length and calls `consume_exact` which does `vec![0u8; n]` **before** checking bytes remain — a contract can return a response claiming ~4 GB, aborting the block (OOM). Reachable by any deployed contract. | Bound both lengths against remaining cursor bytes before allocating. |
| HIGH | `src/lib.rs:434-441` | `AuxpowBlock::parse(...).unwrap()`, `consensus_decode::<Block>(...).unwrap()`, and `index_block(...).unwrap()` at the metashrew entry point turn any deterministic parse/index `Err` into a permanent wedge. | Propagate the error / log-and-skip instead of `.unwrap()`. |
| HIGH | `src/view.rs:1016-1067` | The `simulate_protostones` view path enables four global flags (`SKIP_PROTOSTONE_PERSISTENCE`, trace/final-balance/touched-storage collectors) with **no `Drop` guard**; an early-return (`seed_input_balances(...)?`) or any panic between enable and disable leaks `SKIP_PROTOSTONE_PERSISTENCE = true` into the next `index_block`, which then **skips `save_balances` + `clear_balances`** → index corruption. | Wrap the flags in an RAII guard that disables on unwind/early-return. |
| HIGH | `src/view.rs:955-968,814` | `simulateprotostones` allocates `max(pointer)+1` dust `TxOut`s from an attacker `u32` pointer → multi-GB OOM on one RPC call. | Clamp `num_dust_outputs`. |
| MED | `crates/protorune/src/protoburn.rs:204` | `(cycle+1) % (self.max as i32)` with `self.max == 0` (a runestone with zero protoburns but burn-cycle-driving edicts) is a `% 0` panic. Currently `#[cfg(test)]`-gated in `index_protostones`, so latent in production but LIVE in the test harness and the moment protoburn is enabled. | Guard `max == 0` before the modulo. |
| MED | `src/view.rs` (`parcel_from_protobuf:59-75`, `getstorageat:407`, `getinventory:379`, `alkanes_id_to_outpoint:368-372`) and `crates/protorune/src/view.rs:196,249,271,306` | `.unwrap()` on missing protobuf fields / malformed hex aborts the RPC query (robustness/DoS of the view surface). | Replace with `?`/`ok_or`. |
| MED | `src/unwrap.rs:34` | `bytes[0..16].try_into().unwrap()` on a `/premium` value guarded only by `is_empty()` → panic on a 1–15-byte blob. | Length-check before slicing. |
| LOW | `src/vm/host_functions.rs:43`, `_get_number_diesel_mints` | `DIESEL_MINTS_CACHE` is a global cleared only at `index_block` start; simulate paths populate it from attacker block bytes and never clear it → stale cross-simulation results (not consensus). | Clear at simulate entry/exit. |

Verified safe (previously risky, now hardened): balance-sheet arithmetic
(`checked_*` + `?`), protostone `decipher` (`take_n` converts length to an error), varint/
consensus decoders (return `Err`), `Cellpack::try_from` (`v.len() < 2` guard),
`process_message` output-pointer bounds checks, and the `simulate_block` sandbox
(never commits to the real store).

---

## 4. Rollback correctness

- `process_message` binds the two ledgers: on any message failure it restores the
  in-memory `balances_by_output` snapshot **and** `atomic.rollback()`s, then refunds from
  the clean snapshot (`crates/protorune/src/protostone.rs:165-226`). Correct.
- A child extcall revert (post-`V220_FORK_HEIGHT`) rolls back only the child checkpoint,
  restores tokens sent to the child, and returns a negative value without aborting the
  parent (`src/vm/host_functions.rs:860-899`). The precompile path is correctly excluded
  from rollback to avoid a checkpoint-stack underflow (`:544-551`).
- `derive()` **shares** the checkpoint stack (Arc), so isolation is by checkpoint/commit/
  rollback discipline only — any mismatch corrupts the shared stack (this is why an extra
  rollback or a stray commit is a latent wedge). The current call sites are balanced.

---

## Note: pre-existing test failures (baseline, unrelated to this audit)

On a clean tree, 5 tests already fail: `genesis_upgrade::test_new_genesis_contract_non_eoa`,
`fr_btc::test_set_signer_no_auth`, `fuel::test_infinite_extcall_loop`,
`upgradeable::test_beacon_proxy`, `upgradeable::test_upgradeability`. The
`transfer_from` fix does not change this set (86 pass incl. the new PoC, same 5 fail).
