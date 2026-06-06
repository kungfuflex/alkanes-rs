# Security Audit — Recycle Bin (`8:dead`), v2.2.0-rc.6

Scope: `crates/alkanes-std-recycle`, `src/recycle.rs`, the `8:*` dispatch in
`src/vm/utils.rs`, and `src/precompiled::precompiled_life_wasm`.
Threat owner ask (ksyao2002): *"make sure people can't just mint arbitrary
alkanes using that recycling bin."*

## Trust model

- **Capture** is native indexer code (no attacker-controlled execution). It is
  the **only** writer of `8:dead`'s inventory and `/recycle/<spk>` ledger.
- **Claim** is the `8:dead` WASM, attacker-callable with any cellpack/tx shape.
- The runtime's `checked_debit_with_minting` will **mint** a deficit if a contract
  emits more of an alkane than it holds in inventory. So the entire safety of the
  feature reduces to: *the claim WASM must never emit more than was genuinely
  stranded to the rightful EOA.*

## Findings

### A. Arbitrary mint via over-emit — MITIGATED (primary threat)
The claim only pushes transfers read from `/recycle/<spk>`, and **clamps each to
the live inventory** (`self.balance(myself, id)`), erroring on shortfall rather
than emitting. Inventory is credited only by capture, only by the exact stranded
amount. So a claim can never out-emit inventory → `checked_debit_with_minting`
never mints. ✅ Residual: depends on capture crediting inventory == ledger; test
#4 asserts this and the clamp is the backstop.

### B. Cross-recipient theft — MITIGATED
The ledger is partitioned by `script_pubkey`. Claim reads only
`/recycle/<default_output(claim_tx).spk>`. To claim entry X you must place X as
your first non-OP_RETURN output — but the released balances are routed to the
protostone pointer you set to that same output, i.e. **to X itself**. An attacker
naming a victim's spk only returns the victim's assets to the victim (benign).
✅ No path lets A receive B's balances.

### C. Double-claim / replay — MITIGATED
Claim zeroes `/recycle/<spk>` after emitting. A second claim reads empty and
errors. Atomic within the message's storage commit. ✅ Test #5.

### D. Non-EOA / contract claim — MITIGATED
Both capture and claim require `default_output` to be `is_eoa` (p2tr/p2wpkh/
p2pkh). Script-path/bare outputs are never credited (capture) and rejected
(claim), preventing contract-mediated re-entrancy on the recovery path and
matching flex's EOA-only rule. ✅ Test #3. ⚠️ Confirm bitcoin 0.32 `is_p2tr/
is_p2wpkh/is_p2pkh` cover the intended key-path set; bare/unknown witness
versions fall through to "burned" (safe default).

### E. Capture↔claim key divergence — MITIGATED, MUST-TEST
If the native ledger key ≠ the WASM's `StoragePointer` key, claim reads empty and
funds are unrecoverable (no loss/mint, but a silent failure). Both use the shared
`KeyValuePointer` keyword/select; capture nests under `/alkanes/<8:dead>/storage/`
as `pipe_storagemap_to` does. ✅ **Test #7 runs the real WASM against
capture-written state — this is the gating test; do not merge without it green.**

### F. Determinism / reindex — MITIGATED
Capture is pure function of block + prior state, runs every block from genesis,
clears each input it sweeps (idempotent on replay). No `Date/rand`. ✅ Note: this
is a **consensus change** — all indexers must adopt rc.6 together or balances
diverge on the recycle surface (flex: coordinate v10 upgrade by ~955000 + snapshot).

### G. Spam / DoS amplification — ACCEPTABLE
Capture adds O(inputs) storage writes per block; only fires on inputs that
actually carried alkanes. No VM invocation on capture (flex's requirement) → no
fuel/exec amplification. Bin growth is bounded by real stranded volume. ✅
⚠️ Benchmark from-genesis reindex cost (flex expects still <11d).

### H. Inventory list growth / unbounded `inventory` append — LOW
`credit_inventory` appends `rune` to `/alkanes/<8:dead>/inventory/` on every
credit without dedup; the list can grow with duplicates. Functionally harmless
(balances are keyed maps) but bloats the inventory index. ⚠️ Recommend dedup
(append only when `balance_pointer` was previously zero, matching the existing
`balance_pointer` auto-append guard) before merge.

### I. `default_output` all-OP_RETURN tx — MITIGATED
`default_output` returns `None`; capture leaves the balance burned + clears ghost;
claim errors. No funds created. ✅

### J. Reorg safety — REVIEW
Capture writes via `AtomicPointer::commit` per input. Confirm rollback on reorg
unwinds recycle credits the same way protorune balance writes are unwound (the
indexer's reorg path must replay capture). ⚠️ Add a reorg test before mainnet.

## Required before merge
1. Test #7 (key-parity, real WASM) green — gating.
2. Test #4 (anti-mint clamp) green.
3. Finding H dedup fix.
4. Finding J reorg replay confirmation.
5. Embed the built wasm; re-run full `cargo test`.

## Verdict
Design is sound against the primary "arbitrary mint" threat: claim emits only
clamped, partitioned, single-use, EOA-gated, capture-credited balances. No path
found to mint or steal. Remaining items (H, J) are correctness/operational, not
value-creation, but must be closed before a consensus rollout.
