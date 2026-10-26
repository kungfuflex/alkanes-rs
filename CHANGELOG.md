# Changelog

Release notes for older versions are on the GitHub releases page
(https://github.com/kungfuflex/alkanes-rs/releases).

## v3.0.0-rc.4 (security)

A dependency-only security release for the CLI, RPC and host crates. **The indexer is
unchanged.** `alkanes.wasm` built with `--features mainnet` is byte-identical to rc.3
(sha256 `27b003d1234e3b278ac80699c3cfec74727aed4164c4813ae76e44852dc9694f`), so an rc.3
indexer and an rc.4 indexer are the same program. Indexer operators don't need to do
anything.

**Who should upgrade:** anyone who builds or runs `alkanes-cli`, `alkanes-cli-sys`,
`alkanes-jsonrpc`, `alkanes-ffi` / `alkanes-jni`, or any crate from this workspace that
makes HTTPS connections. Rebuild from the `v3.0.0-rc.4` tag with `--locked`.

- **TLS trust roots (CVE-2026-111943).** Certificate authorities that the browser root
  programs have distrusted were still trusted by the bundled roots, so HTTPS connections
  made by the CLI and RPC crates could accept certificates that should be rejected.
  - `webpki-roots` 1.0.9 → 1.0.11 (RUSTSEC-2026-0381)
  - `rustls-webpki` 0.103.13 → 0.103.16 (RUSTSEC-2026-0382)
  - `rustls` 0.23.42 → 0.23.46 (RUSTSEC-2026-0285, -0346, -0348, -0349)
  - `alkanes-cli-common`: the SQLite cache now builds sqlx with `runtime-tokio` instead of
    `runtime-tokio-rustls`. SQLite is a local file and never uses TLS, so the old setting only
    linked a second, older TLS stack (rustls 0.21 / webpki-roots 0.25, which has no fixed
    release) into the CLI. That stack is no longer in `alkanes-cli`, `alkanes-cli-sys`,
    `alkanes-ffi` or `alkanes-jni`.
- **Denial of service.**
  - `quinn-proto` 0.11.16 → 0.11.19 (RUSTSEC-2026-0338, -0339, -0340, -0341, -0342)
  - `h2` 0.4.15 → 0.4.20 (RUSTSEC-2026-0258)
- **Release:** workspace version 3.0.0-rc.3 → 3.0.0-rc.4 for the host, CLI and RPC crates.
  The five crates compiled into the indexer wasm (`alkanes`, `alkanes-support`, `ordinals`,
  `protorune`, `protorune-support`) now pin their own version and stay at **3.0.0-rc.3**.
  A crate's version is an input to the compiled wasm, so bumping them would change the
  indexer's bytes even with no code change. `Cargo.lock` changes only the packages above and
  the workspace versions. The precompiled contract wasms aren't rebuilt.

Known and not fixed in this release, because each needs a major-version upgrade rather than
a patch bump: `sqlx` 0.7 (used with Postgres by `alkanes-data-api`,
`alkanes-contract-indexer` and `alkanes-trace-transform`; it still carries the rustls 0.21
stack), `alloy-dyn-abi` 0.7, `time` 0.3.41 (the fix needs rustc 1.88; this workspace pins
1.86.0), `rsa` (no fix released) and `crypto-bigint` 0.5. None of them is in the indexer
wasm.
