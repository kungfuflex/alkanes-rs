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
- **zstd (RUSTSEC-2026-0391, -0398, -0403, -0410) and h2 0.3 (RUSTSEC-2026-0258).** Both
  came only through actix-web's default features in `alkanes-jsonrpc` and `alkanes-data-api`.
  The two now build actix-web without `compress-zstd` and `http2`, the same choice metashrew
  v9.1.0-rc.6 makes:
  - `compress-zstd`: nothing uses the Compress middleware, but with this feature actix-http
    decoded any request body sent with `Content-Encoding: zstd` through
    `zstd::stream::write::Decoder` from zstd 0.13, which has no fixed release. That is the path
    RUSTSEC-2026-0403 describes (a corrupt legacy frame exposes up to 32 KiB of uninitialized
    heap memory). Such bodies now reach the handler undecoded and fail to parse. gzip and
    brotli request bodies still work.
  - `http2`: both servers bind plain HTTP/1.1, and this feature linked h2 0.3.27, which has no
    fix for RUSTSEC-2026-0258, reachable through h2c prior knowledge. h2 0.4.20 (above) is now
    the only h2.
  - `Cargo.lock` removes `zstd` 0.13.3, `zstd-safe` 7.2.4, `zstd-sys` 2.0.16, `h2` 0.3.27 and
    `jobserver`, and changes nothing else. None of them is in the indexer wasm's dependency
    graph, and the wasm is rebuilt byte-identical (sha256 above).
- **tokio** 1.53.0 → 1.53.3 (RUSTSEC-2026-0386, `AsyncFd` I/O safety). It is used by the
  host crates and isn't in the indexer wasm.
- **ruint** 1.14.0 → 1.17.2 (RUSTSEC-2025-0137, unsound `reciprocal_mg10` used by `Uint`
  division). It comes in through `alloy-primitives` 0.7 in `alkanes-cli-common` (EVM ABI
  encoding) and the CLI, RPC and data-api crates, and isn't in the indexer wasm. 1.17.2 is
  the newest ruint that builds with rustc 1.86. `enum-ordinalize` is held at 4.3.2 for the
  same reason; it and the other packages this adds to `Cargo.lock` (`ark-ff` 0.5, `educe`)
  are optional ruint dependencies that no workspace crate enables, so none of them is
  compiled.
- **Release:** workspace version 3.0.0-rc.3 → 3.0.0-rc.4 for the host, CLI and RPC crates.
  The five crates compiled into the indexer wasm (`alkanes`, `alkanes-support`, `ordinals`,
  `protorune`, `protorune-support`) now pin their own version and stay at **3.0.0-rc.3**.
  A crate's version is an input to the compiled wasm, so bumping them would change the
  indexer's bytes even with no code change. `Cargo.lock` changes only the packages above and
  the workspace versions. The precompiled contract wasms aren't rebuilt.

Known and not fixed in this release, because each needs a major-version upgrade or a newer
toolchain rather than a patch bump: `sqlx` 0.7 (used with Postgres by `alkanes-data-api`,
`alkanes-contract-indexer` and `alkanes-trace-transform`; it still carries the rustls 0.21
stack), `alloy-dyn-abi` 0.7, `time` 0.3.41, `actix-http` 3.11.2 (GHSA-xhj4-vrgc-hr34, CL.TE
request smuggling in `alkanes-jsonrpc` and `alkanes-data-api`; run them behind a proxy that
normalizes `Transfer-Encoding` / `Content-Length`), `rsa` (no fix released) and
`crypto-bigint` 0.5, and `ruint` RUSTSEC-2026-0220 (wrong overflow flags from `Uint` shifts,
fixed in ruint 1.20.0). The `time` and `actix-http` fixes need rustc 1.88, ruint 1.20 needs
rustc 1.90, and this workspace pins 1.86.0. None of them is in the indexer wasm.
