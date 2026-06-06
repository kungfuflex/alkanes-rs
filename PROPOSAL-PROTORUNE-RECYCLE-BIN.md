# Recycle Bin (`8:dead`) — design + implementation (v3.0.0-alpha.2)

**Status:** Implemented on `kungfuflex/v3.0.0-alpha.2` (off `kungfuflex/v3.0.0`);
part of the v3.0.0 upgrade. Pending green build + audit sign-off.
**Source:** FROST Batallion 6, 2026-06-06 (flex final say; ksyao2002 + mork1e).
**Centerpiece:** stranded alkanes become claimable instead of lost — no height
fork, recovered from genesis on reindex.

---

## 0. Problem

A Bitcoin transaction with **no protostone** (no OP_RETURN / Runestone) that
spends a UTXO carrying alkanes does not move those alkanes — `index_protostones`
never runs, so the protocol-tag balance is left at the now-spent input outpoint
and is unspendable ("garbage collected"). This is *intended* anti-spam behavior
(flex wants non-alkanes wallets to discard spam alkanes rather than spread them
to every output like runes), but it was **missing its failsafe**: a way for the
rightful owner to recover a balance lost this way. Real incident: 22 FIRE Bond
NFTs + ~0.15 DIESEL/frBTC LP stranded by a codex-built consolidation tx
(`bda33f9…`, block 952581).

## 1. Design (flex)

- **`8:*` is a new reserved namespace** for indexer-embedded precompiled "life
  WASMs". (`1:*` stays the witness-payload deploy space.) The recycle bin is
  **`AlkaneId(8, 0xdead)`**.
- **Capture is native, at the indexer scope — no wasmi.** Invoking the VM for
  every accidental burn would be wasteful; capture is plain indexer code that
  writes to `8:dead`'s storage + inventory.
- **No height gate.** "Our code already handles it." Capture runs from genesis,
  so a reindex (resync espo, distribute snapshot, get people on v10) recovers all
  historical strandings — including the back-catalogue.
- **Claim is the WASM**, dispatched only when someone actually claims:
  cellpack **`8:dead:3`** (`3` = `Claim`, the fire `Claim`-opcode convention).
- **Release-to-pointer, EOA-only.** The recipient is the first non-OP_RETURN
  output (`default_output`) of the claim tx, which must be an EOA (p2tr/p2wpkh/
  p2pkh); the protostone pointer is set to that output.
- **View opcodes** so people can see their bin balance.

## 2. Mechanism

### Capture — `src/recycle.rs::capture_block` (called from `src/indexer.rs`)

Runs once per block after `Protorune::index_block`. For every input still
holding a protocol-tag balance (i.e. stranded — a protostone spend would have
cleared it in `index_protostones`):

1. credit `8:dead`'s **inventory** (`/alkanes/<rune>/balances/<8:dead>` += amount,
   register `rune` in `/alkanes/<8:dead>/inventory/`);
2. append to the **ledger** at `8:dead` storage `/recycle/<script_pubkey>`, keyed
   by `default_output(tx).script_pubkey` (the EOA that would have received it);
3. clear the stranded input balance.

Non-EOA recipient → left burned (intended spam GC), ghost cleared.

### Claim — `crates/alkanes-std-recycle` (the `8:dead` WASM), opcode `3`

Reads `/recycle/<default_output(tx).spk>` (EOA-only), emits the recorded balances
**from its own inventory** to the response (routed to the protostone pointer),
and zeroes the ledger entry. View opcode `10` (`GetRecycleBalance`) returns the
same ledger without mutating; `99/100` name/symbol; `0` no-op initialize.

### Dispatch — `src/vm/utils.rs::run_special_cellpacks`

New branch: `target.block == 8` → load the embedded binary via
`precompiled::precompiled_life_wasm` (`8:dead` → `alkanes_std_recycle_build`).

## 3. Safety invariant (ksyao: "can't mint arbitrary alkanes")

The claim WASM emits from `8:dead`'s **inventory**, which is credited **only** by
the native capture, and only by exactly what it strands. The contract additionally
**clamps each transfer to the live inventory balance** (`self.balance(myself,id)`)
and errors on shortfall, so even a ledger/inventory desync can never trigger the
runtime's self-mint rule (`checked_debit_with_minting`). Claims are made
single-use by zeroing the ledger entry. Net: a claim can only ever return what
was actually stranded to its rightful EOA — no minting, no double-claim, no
cross-recipient theft (the ledger is partitioned by `script_pubkey`).

## 4. Key-parity (capture ↔ claim)

Capture (native `IndexPointer`) and claim (`StoragePointer`) both implement
`KeyValuePointer`, so `/recycle/<spk>` resolves to byte-identical keys; capture
nests it under `/alkanes/<8:dead>/storage/` exactly as `pipe_storagemap_to` does
for the WASM. `default_output`, `is_eoa`, and the ledger codec are duplicated
verbatim on both sides. **Validated end-to-end by an integration test that runs
the real WASM against a capture-populated state** (see §5).

## 5. Tests (`src/tests/recycle.rs`, `cargo test`)

1. capture: no-protostone spend of an alkane UTXO → bin keyed by `default_output`
   EOA spk; inventory + ledger credited; input cleared.
2. claim: `8:dead:3` releases the exact balances to the pointer; ledger zeroed.
3. EOA-only: non-EOA `default_output` left burned (capture) / claim rejected.
4. anti-mint: claim never emits > inventory; desync errors, never mints.
5. double-claim: second claim returns nothing (ledger zeroed).
6. partition: claimant A cannot claim B's bin (distinct script_pubkeys).
7. key-parity: capture-written ledger is read identically by the WASM.

## 6. Files

| File | Role |
|---|---|
| `crates/alkanes-std-recycle/` | the `8:dead` claim WASM (opcodes 0/3/10/99/100) |
| `src/recycle.rs` | native capture (`capture_block`) |
| `src/indexer.rs` | calls `capture_block` after `index_block` |
| `src/vm/utils.rs` | `8:*` precompiled dispatch in `run_special_cellpacks` |
| `src/precompiled/{mod,alkanes_std_recycle_build}.rs` | embedded binary + registry |
| `src/tests/recycle.rs` | coverage |

## 7. Open items

- Embed the built `alkanes_std_recycle.wasm` into `alkanes_std_recycle_build.rs`.
- Decide `GetRecycleBalance` query ergonomics for the subfrost app (view by spk).
- Benchmark capture cost on a full from-genesis reindex (flex: still <11d, fine).
