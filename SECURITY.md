# Security policy

alkanes-rs is the consensus code of the ALKANES metaprotocol: the indexer that
every alkanes node runs, the standard contracts it deploys into chain state, and
the SDK wallets build transactions with. A bug here can move or destroy funds or
split nodes onto different state, so please report it privately first.

## Reporting a vulnerability

**Do not open a public issue, pull request or discussion for a security problem.**

Report it through GitHub's private vulnerability reporting:

1. Go to the repository's **Security** tab.
2. Click **Report a vulnerability**
   (direct link: <https://github.com/kungfuflex/alkanes-rs/security/advisories/new>).
3. Describe the issue, the affected version or commit, and how to reproduce it.
   A failing test, a transaction or block that triggers it, or a proof of concept
   helps most.

Only the maintainers can read the report. We will discuss it with you in the
private advisory, and you can attach files and a proposed patch there.

## What to expect

- We acknowledge a report within **3 business days**.
- We aim to give a first assessment (accepted or not, and severity) within
  **10 business days**.
- Consensus fixes need every indexer operator to upgrade, usually at an
  activation height. We coordinate the release and the height with you and
  publish the advisory once enough of the network runs the fix.
- We credit reporters in the advisory unless you ask us not to.

Please give us a reasonable time to fix and deploy before any public disclosure,
and do not test against mainnet nodes or funds you do not own. Use regtest or
the test suites in this repository.

## Scope

In scope:

- the indexer (`alkanes` crate, `src/`) and its consensus dependencies
  (`crates/protorune`, `crates/protorune-support`, `crates/alkanes-support`,
  `crates/ordinals`);
- the standard contracts under `crates/alkanes-std-*` and the precompiled
  contract wasms in `src/precompiled/`;
- the runtime and macros contracts are built with (`crates/alkanes-runtime`,
  `crates/alkanes-macros`);
- the TypeScript SDK (`ts-sdk/`, `@alkanes/ts-sdk`) and the crates compiled
  into it (`crates/alkanes-web-sys`, `crates/alkanes-cli-common`);
- this repository's CI and release workflows (`.github/workflows/`).

The metashrew runtime that hosts the indexer (`rockshrew-mono`,
`metashrew-core`) lives in <https://github.com/kungfuflex/metashrew>; report
issues there the same way.

## Supported versions

Security fixes go to the latest release line (currently `v3.0.0-rc.*` on
`main`). Older releases are not patched; upgrade to the latest release.
