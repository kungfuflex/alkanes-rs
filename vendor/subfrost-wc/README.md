# subfrost-wc (vendored)

This is a vendored copy of the canonical
[`subfrost-wallet-wc`](https://github.com/subfrost/subfrost) crate
that lives at `crates/subfrost-wallet-wc` in the `subfrost-mobile`
repo.

## Why vendored, not promoted to `crates/`?

The single source of truth is the mobile-side crate. The mobile
binary needs the crate to be a no-deps embedded build (the `default`
feature set), so the production APK only links the `crypto`,
`pairing`, and `wire` modules. The optional `relay-client` feature
that adds tokio + tokio-tungstenite + reqwest exists for the
*dapp side* — currently only `alkanes-cli` (in this workspace).

Keeping the crate vendored (rather than git-pinning the subfrost
repo) lets us:

  1. Add the dapp-side `signer.rs` + `relay.rs` modules on top
     without forcing them into the mobile build,
  2. Rename the crate to `subfrost-wc` so the alkanes-rs workspace
     doesn't collide with the mobile crate when both are
     sibling-built (e.g. when a developer has `~/subfrost-mobile`
     and `~/alkanes-rs` open in the same cargo workspace).
  3. Iterate the dapp-side modules without round-tripping every
     bump through the mobile release cadence.

## Sync contract

The three files MUST stay byte-identical between this vendor copy
and `subfrost-mobile/crates/subfrost-wallet-wc/src/`:

  * `crypto.rs`  — X25519 ECDH + HKDF-SHA256 + ChaCha20-Poly1305
  * `wire.rs`    — the `Plaintext` enum (4 shipped clients depend on
                  the exact snake_case JSON layout)
  * `pairing.rs` — `subfrost://wc/<topic>?key=...` URI parser

`signer.rs` and `relay.rs` are vendor-only (dapp-side). The
`tests/wire_round_trip.rs` integration test in this crate pins the
wire contract for the dapp side; the corresponding mobile-side test
lives at `subfrost-mobile/crates/subfrost-wallet-integ-tests/tests/wc_headless_e2e.rs`.

If you change `crypto.rs` / `wire.rs` / `pairing.rs` here, you MUST
also update the mobile crate in lockstep, then rebuild both
workspaces. The build does not enforce the byte-identity (no
cargo crate can reach across repo roots); the sync is by
convention + the dual test harnesses catching shape drift.

## Features

  * `default = []` — only `crypto`, `pairing`, `wire` (no deps).
    Use this for embedded callers (mobile FFI, wasm bindings, the
    extension's `subfrost-wallet-web-sys` umbrella).
  * `relay-client` — adds `signer` + `relay`, plus tokio +
    tokio-tungstenite + reqwest + futures-util + async-trait + log.
    Use this for native dapp-side callers (alkanes-cli's
    `wc_signer.rs` adapter).

## Related docs

  * `crates/alkanes-cli/src/wc_signer.rs` — the dapp-side adapter
    that bridges this crate's `WalletConnectSigner` into the
    `alkanes_cli_common::traits::RemoteSigner` trait.
  * `subfrost-mobile/.claude/projects/.../reference_wc_push_semantics.md` —
    full description of the FCM-wake path (`/v1/pair-register` +
    `/v1/pair-wake`) that lets the mobile listen-side go online
    on-demand when a dapp wants to send a sign request.
