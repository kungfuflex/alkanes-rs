# BuildInfo — a parameterized, reproducible-build format for alkanes

`BuildInfo` is a single self-contained JSON artifact that fully specifies how to
reproduce **one alkane's on-chain wasm from source, byte-for-byte, in a controlled
sandbox**. It captures every environment axis we found matters for a reproducible
alkane build, so `explorer.subfrost.io` (or any third party) can verify an alkane
from just its `alkaneid` — or reconstruct the build locally to get a not-yet-deployed
alkane verified on-chain.

- Canonical Rust type: [`schema.rs`](schema.rs) (`pub struct BuildInfo`, `SCHEMA_VERSION = "1.2.0"`)
- JSON Schema: [`build-info.schema.json`](build-info.schema.json)
- Golden fixtures: [`fixtures/`](fixtures/) — `4:797` (Linux, byte-exact), `2:0` (macOS-reconstructed), `4:9200` (not-yet-deployed)
- CLI: `alkanes-cli build-info` (+ `reverse` / `build` / `verify`)

## Spec lineage (version history)

`schema_version` is semver. **Compatibility is MAJOR-based**: a consumer accepts any
artifact of the same major (so a `1.0.0` fixture round-trips fine under a `1.1.x`
reader — the new fields are optional and defaulted). Bump the **minor** for additive
optional fields; bump the **major** only to change an existing field's shape or
meaning. The JSON Schema (`build-info.schema.json`) validates `schema_version`
against `^1\.` for this reason. Canonical lineage lives in the `SCHEMA_VERSION`
doc-comment in [`schema.rs`](schema.rs).

| Version | Change |
|---|---|
| **1.0.0** | Initial spec. The axes required for byte-exact single-alkane reproduction: `toolchain` (rustc + the darwin host-triple copy trick), `c_toolchain` (clang vendor/version + `apt`/`llvm-release`/`homebrew-bottle` assembly), `environment` (HOME + path-remap), `registry` (`committed-lock` vs crates.io-index `time-machine`), one `git_deps[]` source-id spelling (`bare`\|`rev`), the full resolved lockfile, and the `result` verdict. |
| **1.1.0** | Reproduction reach extended — **all additions optional/backward-compatible**: `git_deps[].redirect_to` (unify a crate pulled via different mirror URLs onto one source); `git_deps[].pin_transitive` (force the rev across **all** lockfile entries incl. transitive, via `cargo update --precise`); `c_toolchain.wasm_cflags` (e.g. `-nostdlibinc` so a stock **apt clang** cross-compiles `secp256k1-sys`→wasm, not only a self-contained Homebrew clang); `toolchain.rustc_version` may be a **channel** (`nightly-YYYY-MM-DD`/`beta`), not only `x.y.z`; `registry.archive_commit` is optional (auto-derived from the deploy `timestamp` by selecting the `snapshot-YYYY-MM-DD` archive branch containing it); and a **minimal recipe** (`identity` + `source` only) is valid — the verifier auto-reverses the rest from the on-chain bytecode fingerprint, client fields overriding. |
| **1.2.0** | rustc provenance modeled — optional/backward-compatible: `toolchain.rustc_source` (`RustcSource`, parallel to `c_toolchain.source` for clang). Absent ⇒ rustup installs the channel/semver (the common case). `method = "git-build"` ⇒ **compile rustc from a `rust-lang/rust` commit** (`git_commit` + `channel` + `download_ci_llvm`) — the only way to reproduce a contract built off an **unpublished** toolchain, e.g. a stable branch-point like `9fc6b431` ("Prepare 1.85.0") whose `/rustc/<hash>/` sysroot no rustup channel ships. Makes the builder-verifier able to reproduce **any** rustc, released or not. |

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
| Rustc source (1.2.0) | `toolchain.rustc_source.method` = `rustup` \| `git-build` | The `/rustc/<hash>/` sysroot path is a rustc-commit fingerprint. When that commit is an **unpublished** toolchain (a stable branch-point, a reverted nightly) no rustup channel installs it, so `git-build` compiles rustc from `rust-lang/rust@<commit>` with the matching `channel` + CI LLVM (`download_ci_llvm`) — reproducing what release channels can't. |
| C host-dependence | `c_toolchain.c_object_host_dependent` | The one thing that doesn't fully reconstruct: macOS vs Linux Homebrew clang emit a slightly different secp256k1 static footprint (memory-base shift). Pure-Rust alkanes → byte-exact; C-linking alkanes → `verified` (~99.997%). |
| Environment | `environment.home`, `.remap_path_prefix` | Reversed from embedded paths; registry/git/toolchain paths hang off `HOME`. Empty `remap_path_prefix` ⇒ paths are raw and `HOME` must be reconstructed. |
| Registry | `registry.mode` = `committed-lock` \| `time-machine` | Committed `Cargo.lock` ⇒ real crates.io + `--locked`. No lock ⇒ freeze the crates.io-index tree at the build date and serve it **AS** `index.crates.io` so the registry src-dir hash (`index.crates.io-<hash>`, cargo-version dependent) is canonical. |
| Git deps | `git_deps[].source_form` = `bare` \| `rev` | A bare git source-id (`?rev=X#` → `#`, then `--locked`) changes `-C metadata` and hence monomorphization ordering. Reproduce the exact spelling the origin lock used. |
| Git mirror unify (1.1.0) | `git_deps[].redirect_to`, `.pin_transitive` | Same crate pulled via two mirror URLs (e.g. sandshrew↔kungfuflex `metashrew`) → cargo builds two incompatible copies. `redirect_to` collapses them onto one source; `pin_transitive` forces the rev across transitive lockfile entries an in-tree edit can't reach. |
| Wasm C sysroot (1.1.0) | `c_toolchain.wasm_cflags` | apt clang leaks host glibc `/usr/include` on `wasm32-unknown-unknown` → `secp256k1-sys` fails. `-nostdlibinc` drops system libc includes so apt clang-13/14/15 (not just Homebrew clang) compile it — matching a Linux origin at its actual clang version. |
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

### Auto-reversal: a minimal recipe is enough

The live explorer verifier (`POST /api/v1/{key}/verify`) now **reverses the recipe from
the on-chain bytecode itself** before it builds. It reads the embedded panic paths and
`producers` record and auto-fills the git-dep revs, `HOME`, host triple, clang major,
rustc channel, and — from the deploy tx's block time — the crates.io freeze date. So a
submission of just `{ alkane, repo_url, commit, package, subdir }` reproduces most
alkanes; a full `BuildInfo` is the *explicit* form, and any field it carries overrides
the auto-reversed value. This means the fields below are all **optional over the wire**.

The flat verify request accepts two convenience fields that project onto the richer
`BuildInfo` shape above:

| Verify field | BuildInfo equivalent | Meaning |
|---|---|---|
| `git_pins` (newline `url rev [bare\|rev]`) | `git_deps[]` (`url` / `rev` / `source_form`) | Pin **every** git dep at once (a contract often pins both `alkanes-rs` and `metashrew`), incl. transitive deps reconciled through the lockfile, and mirrored-repo URLs unified onto one source. |
| `git_date` (`YYYY-MM-DD` / RFC3339) | `registry.timestamp` + `registry.archive_commit` (time-machine) | Date-freezes the crates.io index **and** any still-unpinned git dep to that date. Defaults to the deploy tx block time, so a no-committed-lock contract freezes automatically. |

The C toolchain also gained a Linux path for `secp256k1-sys`: apt `clang-13/14/15` can
now build it to wasm via `-nostdlibinc` + the crate's `wasm/wasm-sysroot`, so a
Linux-origin C-linking contract is matched at its actual apt clang version instead of
being forced onto assembled Homebrew clang (`c_toolchain.source.method = "apt"`).

> Note: this README's route table above is a design sketch; the shipped explorer routes
> are `POST /api/v1/{key}/verify` and the admin `POST /api/v1/{key}/attest` (see
> `explorer.subfrost.io/docs/api`).

## Reproduction cheatsheet (the hard-won bits)

- **macOS on Linux**: COPY the linux 1.86.0 toolchain to `.../toolchains/1.86.0-aarch64-apple-darwin`, set `HOME=/Users/<u>`, clone the repo to the origin build dir.
- **Homebrew clang on Linux**: assemble from GHCR bottle blobs `llvm`+`z3`+`libedit` (x86_64_linux), `patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2` + `--set-rpath`. Reports `Homebrew clang version 20.1.7`.
- **Registry time-machine**: depth-1 fetch the `crates.io-index-archive` commit frozen at the build date, serve over local HTTPS **as** `index.crates.io` (hosts + self-signed cert + `CARGO_HTTP_CAINFO`) so the src-dir hash is canonical (cargo 1.82 → `6f17d22bba15001f`, cargo 1.86 → `1949cf8c6b5b557f`).
- **Bare git source-id**: resolve WITH the rev, rewrite the lockfile `?rev=X#` → `#`, build `--locked`.
- **secp256k1 C-object**: the residual — C-linking alkanes reach `verified` (~99.997%) on a cross-host reconstruction, not full `reproducible`.
