# BuildInfo — a parameterized, reproducible-build format for alkanes

`BuildInfo` is a single self-contained JSON artifact that fully specifies how to
reproduce **one alkane's on-chain wasm from source, byte-for-byte, in a controlled
sandbox**. It captures every environment axis we found matters for a reproducible
alkane build, so `explorer.subfrost.io` (or any third party) can verify an alkane
from just its `alkaneid` — or reconstruct the build locally to get a not-yet-deployed
alkane verified on-chain.

- Canonical Rust type: [`schema.rs`](schema.rs) (`pub struct BuildInfo`, `SCHEMA_VERSION = "1.0.0"`)
- JSON Schema: [`build-info.schema.json`](build-info.schema.json)
- Golden fixtures: [`fixtures/`](fixtures/) — `4:797` (Linux, byte-exact), `2:0` (macOS-reconstructed), `4:9200` (not-yet-deployed)
- CLI: `alkanes-cli build-info` (+ `reverse` / `build` / `verify`)

## Why every field exists

Reproducing a wasm byte-for-byte means reproducing everything the compiler bakes into
it. The wasm embeds panic paths (`HOME`, the registry src-dir hash, git-checkout
hashes, the toolchain triple) and a `producers` section (rustc + clang vendor/version).
Each axis below is one thing we had to pin to reach byte-exact.

| Axis | Field | Why it matters |
|---|---|---|
| Identity | `identity.target_source` = `onchain` \| `local-file` | On-chain (jsonrpc `getbytecode`) or a local `.wasm` (offline; also not-yet-deployed alkanes). |
| Source | `source.kind` = `git` \| `inline` | A git ref, or fully-stringified inline sources for a self-contained artifact. |
| Toolchain | `toolchain.host_triple` | **The macOS trick.** wasm32 rust-std is host-independent; only the toolchain PATH leaks into panic strings. COPY (not symlink — rustc canonicalizes) the Linux toolchain to `$RUSTUP_HOME/toolchains/1.86.0-aarch64-apple-darwin` and a Linux box reproduces a macOS build byte-for-byte. |
| C toolchain | `c_toolchain.source.method` = `apt` \| `llvm-release` \| `homebrew-bottle` | clang vendor+version is verbatim in `producers`. Homebrew clang is assembled on Linux from GHCR bottle blobs (content-addressed, persist untagged) + patchelf. |
| C host-dependence | `c_toolchain.c_object_host_dependent` | The one thing that doesn't fully reconstruct: macOS vs Linux Homebrew clang emit a slightly different secp256k1 static footprint (memory-base shift). Pure-Rust alkanes → byte-exact; C-linking alkanes → `verified` (~99.997%). |
| Environment | `environment.home`, `.remap_path_prefix` | Reversed from embedded paths; registry/git/toolchain paths hang off `HOME`. Empty `remap_path_prefix` ⇒ paths are raw and `HOME` must be reconstructed. |
| Registry | `registry.mode` = `committed-lock` \| `time-machine` | Committed `Cargo.lock` ⇒ real crates.io + `--locked`. No lock ⇒ freeze the crates.io-index tree at the build date and serve it **AS** `index.crates.io` so the registry src-dir hash (`index.crates.io-<hash>`, cargo-version dependent) is canonical. |
| Git deps | `git_deps[].source_form` = `bare` \| `rev` | A bare git source-id (`?rev=X#` → `#`, then `--locked`) changes `-C metadata` and hence monomorphization ordering. Reproduce the exact spelling the origin lock used. |
| Resolved deps | `resolved_deps`, `cargo_lock_b64` | The full lockfile (base64) is ground truth; `resolved_deps` is the parsed graph for browsing. |
| Result | `result.verdict` | `reproducible` (byte-exact) \| `verified` \| `code_match` \| `partial` \| `mismatch`. |

## CLI

`--at` accepts EITHER an alkane id (`\d+:\d+`, fetched via `--jsonrpc-url`) OR a local
`.wasm` path. **Offline is the default** — jsonrpc is only needed to fetch an on-chain
target.

```bash
# 1. Reverse the build environment from a target (offline for a .wasm; jsonrpc for an id)
alkanes-cli build-info reverse --at ./free_mint.wasm --emit reversed.json
alkanes-cli -p mainnet --jsonrpc-url https://mainnet.subfrost.io/v4/jsonrpc \
  build-info reverse --at 2:0

# 2. Full pipeline: reverse + reconstruct from source + diff → BuildInfo JSON
alkanes-cli -p mainnet --jsonrpc-url https://mainnet.subfrost.io/v4/jsonrpc \
  build-info ./path/to/source --at 2:0 \
  --repo https://github.com/kungfuflex/alkanes-rs \
  --package alkanes-std-genesis-alkane-upgraded-eoa \
  --emit build-info.json

# 3. Build a wasm from a BuildInfo JSON in a docker sandbox
alkanes-cli build-info build build-info.json --out built.wasm

# 4. Diff a candidate against the target → verdict
alkanes-cli build-info verify --at 2:0 built.wasm
```

The `build`/`build-info` steps run the reproduction formula
([`formula::generate_build_script`](formula.rs)) in a docker sandbox
(`--add-host index.crates.io:127.0.0.1` for the time-machine). The CLI **is** the
controlled-env runner, so an operator or a Claude skill can drive each sub-step.

### Worst case: reconstruct locally to get verified on-chain

For a not-yet-deployed alkane (`identity.target_source = "local-file"`, e.g. `4:9200`
from a PR artifact), point `--at` at the built `.wasm`, produce the `BuildInfo`, and
carry it to deployment so the explorer can promote it to `verified` against
`getbytecode` once it lands on-chain.

## Explorer `/api/v1` contract (sketch)

The verifier pipeline that lives in `~/subvh` (explorer.subfrost.io) consumes/produces
this exact JSON. Subfrost API keys gate the write paths.

| Route | Auth | Body / Params | Returns |
|---|---|---|---|
| `GET /api/v1/build-info/:alkane_id` | public | — | Stored `BuildInfo` + `result.verdict`, or `404`. |
| `POST /api/v1/build-info/:alkane_id/reverse` | api-key | — | `Reversed` readout (server fetches `getbytecode`). |
| `POST /api/v1/build-info/:alkane_id/verify` | api-key | `BuildInfo` (or `{source, toolchain, …}` hints) | Runs the sandbox build+diff; persists + returns `MatchResult`. |
| `POST /api/v1/build-info/verify-file` | api-key | multipart: `wasm` + `BuildInfo` | Verify a `local-file` target (not-yet-deployed). |
| `GET /api/v1/build-info/:alkane_id/formula` | api-key | — | The rendered bash reproduction script (transparency / local replay). |

`MatchResult.verdict` maps 1:1 to the explorer's verification badge. The verifier runs
the same [`formula`](formula.rs) the CLI emits, so a local `alkanes-cli build-info`
result and the explorer's result are the same computation — a Claude skill can produce
a `BuildInfo` locally and POST it, or reproduce what the explorer did.

## Reproduction cheatsheet (the hard-won bits)

- **macOS on Linux**: COPY the linux 1.86.0 toolchain to `.../toolchains/1.86.0-aarch64-apple-darwin`, set `HOME=/Users/<u>`, clone the repo to the origin build dir.
- **Homebrew clang on Linux**: assemble from GHCR bottle blobs `llvm`+`z3`+`libedit` (x86_64_linux), `patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2` + `--set-rpath`. Reports `Homebrew clang version 20.1.7`.
- **Registry time-machine**: depth-1 fetch the `crates.io-index-archive` commit frozen at the build date, serve over local HTTPS **as** `index.crates.io` (hosts + self-signed cert + `CARGO_HTTP_CAINFO`) so the src-dir hash is canonical (cargo 1.82 → `6f17d22bba15001f`, cargo 1.86 → `1949cf8c6b5b557f`).
- **Bare git source-id**: resolve WITH the rev, rewrite the lockfile `?rev=X#` → `#`, build `--locked`.
- **secp256k1 C-object**: the residual — C-linking alkanes reach `verified` (~99.997%) on a cross-host reconstruction, not full `reproducible`.
