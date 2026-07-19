# alkanes-rs

![Tests](https://img.shields.io/github/actions/workflow/status/kungfuflex/alkanes-rs/rust.yml?branch=main&label=tests&logo=github)
![Publish](https://img.shields.io/github/actions/workflow/status/kungfuflex/alkanes-rs/publish-npm.yml?branch=main&label=publish&logo=github)

**The ALKANES specification is hosted at** 👉🏻👉🏼👉🏽👉🏾👉🏿 [https://github.com/kungfuflex/alkanes-rs/wiki](https://github.com/kungfuflex/alkanes-rs/wiki)

This repository hosts Rust sources for the ALKANES metaprotocol. The indexer for ALKANES can be built as the top level crate in the monorepo, with builds targeting wasm32-unknown-unknown, usable within the METASHREW indexer stack.

ALKANES is a metaprotocol designed to support an incarnation of DeFi as we have traditionally seen it, but designed specifically for the Bitcoin consensus model and supporting structures.
The ALKANES genesis block is 880000. Builders can end-to-end test their own alkanes smart contracts against the **exact mainnet indexer code path** — no real funds and no live regtest required — using this repository's test harness (see [Testing alkanes end-to-end](#testing-alkanes-end-to-end) below).

Public SUBFROST RPC endpoints: mainnet `https://mainnet.subfrost.io/v4/jsonrpc`, signet `https://signet.subfrost.io/v4/jsonrpc`.

#### NOTE: ALKANES does not have a network token

Protocol fees are accepted in terms of Bitcoin and compute is metered with the wasmi fuel implementation, for protection against DoS.

## Software Topology

This repository is a pure Rust implementation, built entirely for a WASM target and even tested within the WASM test runner `wasm-bindgen-test-runner`.

The top level crate in the monorepo contains sources for the ALKANES indexer, built for the METASHREW environment.

ALKANES is designed and implemented as a subprotocol of runes, one which is protorunes compatible. In order to encapsulate the behavior of protorunes for a Rust build system, a Rust implementation of protorunes generics is contained in the monorepo in `crates/protorune`.

For information on protorunes, refer to the specification hosted at:

[https://github.com/kungfuflex/protorune/wiki](https://github.com/kungfuflex/protorune/wiki)

The indexer stack used to synchronize the state of the metaprotocol and offer an RPC to consume its data and features is METASHREW. METASHREW is started with a WASM binary of the indexer program, produced with a normal build of this repository as `alkanes.wasm`.

Bindings to the METASHREW environment are consumed from the pinned [`kungfuflex/metashrew`](https://github.com/kungfuflex/metashrew) dependency (the canonical METASHREW repository); the environment-agnostic pieces live in `crates/metashrew-support`.

Sources needed to build both metashrew and protorunes meant to be shared with builds of individual alkanes or the generic alkanes-runtime bindings are factored out into `crates/metashrew-support` and `crates/protorune-support` such that they can be imported into an alkane build without the metashrew import definitions leaking in and generating import statements for the METASHREW environment.

In this way, all crates with a `-support` suffix can be imported into any Rust project since they do not depend on a specific environment or `wasm-bindgen`.

This design is permissive enough for this monorepo to host `alkanes-runtime`, which is a complete set of bindings for building alkane smart contracts to a WASM format, suitable for deployment within the witness envelope of a Bitcoin transaction.

Boilerplate for various alkanes are included and prefixed with `alkanes-std-` and placed in the `crates/` directory. The build system is designed such that the WASM builds of each crate with this prefix is made available to the test suite as a Rust source file.

## Building

The production ALKANES indexer wasm is built with the command:

```sh
cargo build --release --target wasm32-unknown-unknown --features mainnet -p alkanes --locked
```

This is the exact, reproducible build used for the shipped `alkanes.wasm` (see `scripts/Dockerfile.wasm` / `scripts/build.sh`, which pin the toolchain and set `SOURCE_DATE_EPOCH`). Replace `mainnet` with your network of choice — constants are defined for luckycoin, regtest, mainnet, dogecoin, bellscoin, and fractal; for other networks or test networks, use the regtest feature.

The `alkanes.wasm` file is produced at `target/wasm32-unknown-unknown/release/alkanes.wasm`, and a WASM for every crate prefixed with `alkanes-std-` is made available to the test suite.

> Note: this repository is a mixed workspace — the top-level `alkanes` indexer crate targets `wasm32-unknown-unknown`, while the CLI, SDKs, and tooling crates build natively. The default cargo target is native, so pass `--target wasm32-unknown-unknown -p alkanes` explicitly when building the indexer.

## Indexing

Refer to the METASHREW documentation for descriptions of the indexer stack used for ALKANES. The canonical METASHREW repository is:

[https://github.com/kungfuflex/metashrew](https://github.com/kungfuflex/metashrew)

### Running against mainnet

To index ALKANES on mainnet you must run matching, current versions of both components:

- **alkanes-rs `v2.2.1-rc.3`** (latest) — built to `alkanes.wasm` and loaded via `--indexer`. See [`v2.2.1-rc.3`](https://github.com/kungfuflex/alkanes-rs/releases/tag/v2.2.1-rc.3).
- **metashrew `v9.0.5-rc.13`** — the [`kungfuflex/metashrew`](https://github.com/kungfuflex/metashrew/releases/tag/v9.0.5-rc.13) indexer stack (`rockshrew-mono`).

Running mismatched versions can produce divergent state. Live mainnet system health — indexer height, sync status, and RPC availability — can be checked at **[https://mainnet.subfrost.io](https://mainnet.subfrost.io)**.

A sample command may look like:

```sh
~/metashrew/target/release/rockshrew-mono --daemon-rpc-url http://localhost:8332 --auth bitcoinrpc:bitcoinrpc --db-path ~/.metashrew --indexer ~/alkanes-rs/target/wasm32-unknown-unknown/release/alkanes.wasm --start-block 880000 --host 0.0.0.0 --port 8080 --cors '*'
```

## Testing alkanes end-to-end

The most useful thing this repository gives contract authors is a **test harness that runs your alkane through the exact same indexer code path that runs on mainnet** — the real `wasmi` execution, the real protorune/alkanes state transitions, the real view functions — all in-memory. You build a Bitcoin block, drop in a transaction whose protostone deploys and calls your contract, index it, then call view functions to assert on the result. **No real funds and no live regtest node.**

An alkanes test project compiles to and runs on `wasm32-unknown-unknown` under **`wasm-bindgen-test-runner`** (the same WASM runner the indexer itself is tested with). One-time setup:

```sh
rustup target add wasm32-unknown-unknown
# The runner's version MUST match the pinned wasm-bindgen (0.2.100), or you get a
# "schema version" mismatch:
cargo install -f wasm-bindgen-cli --version 0.2.100
```

with a `.cargo/config.toml` that points the runner at wasm:

```toml
[build]
target = "wasm32-unknown-unknown"

[target.wasm32-unknown-unknown]
runner = "wasm-bindgen-test-runner"
```

### An alkane is opcodes, like a contract interface

An alkane exposes its interface as **opcodes** — numeric selectors, directly analogous to method selectors on an EVM contract. You declare them with a `MessageDispatch` enum: `#[opcode(N)]` is the selector, and `#[returns(T)]` marks a **view** method (one that only reads and returns data — the analogue of a `view`/`pure` function you'd call with `eth_call`):

```rust
use alkanes_runtime::{declare_alkane, message::MessageDispatch, runtime::AlkaneResponder};
use alkanes_support::response::CallResponse;
use anyhow::Result;

#[derive(Default)]
struct MyToken(());

#[derive(MessageDispatch)]
enum MyTokenMessage {
    #[opcode(0)]
    Initialize { token_units: u128 },     // state-changing: mints the initial supply

    #[opcode(99)]
    #[returns(String)]
    GetName,                              // VIEW: read-only, returns the name
}

impl AlkaneResponder for MyToken {}

impl MyToken {
    fn initialize(&self, token_units: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        // ... persist name, mint `token_units` into response.alkanes ...
        Ok(response)
    }
    fn get_name(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        response.data = b"MyToken".to_vec();   // read-only: just fill response.data
        Ok(response)
    }
}

declare_alkane! {
    impl AlkaneResponder for MyToken { type Message = MyTokenMessage; }
}
```

`declare_alkane!` emits the wasm entrypoints (`__execute`, `__meta`); the crate is `crate-type = ["cdylib", "rlib"]`. A `build.rs` compiles your contract to wasm and generates a `get_bytes()` your tests can deploy — copy the pattern from [free-mint's `build.rs`](https://github.com/kungfuflex/free-mint/blob/master/build.rs). (Stock contracts are also available prebuilt via `alkanes::precompiled::*::get_bytes()`, used below.)

### Deploy, index, and call a view — the same code path as mainnet

Add the harness as dev-dependencies. The **`test-utils` feature is required** — it's what makes `alkanes::tests::helpers` public — and your metashrew source must match the one alkanes-rs uses (`kungfuflex/metashrew`, tag `v9.0.5-rc.8`) so the workspace resolves to a single metashrew:

```toml
[dev-dependencies]
alkanes        = { git = "https://github.com/kungfuflex/alkanes-rs", features = ["test-utils"] }
protorune      = { git = "https://github.com/kungfuflex/alkanes-rs", features = ["test-utils"] }
alkanes-support = { git = "https://github.com/kungfuflex/alkanes-rs" }
metashrew-core = { git = "https://github.com/kungfuflex/metashrew", tag = "v9.0.5-rc.8", features = ["test-utils"] }
wasm-bindgen-test = "0.3"
anyhow = "1"
```

The test below deploys the stock **owned-token** contract in a block, indexes it, and reads its `GetName` view opcode with `call_view` — the **`eth_call` analogue**: it executes a view opcode against current state and returns its bytes, **without persisting any state change**. (This exact test passes against `main`.)

```rust
use alkanes::indexer::index_block;
use alkanes::precompiled::{alkanes_std_auth_token_build, alkanes_std_owned_token_build};
use alkanes::tests::helpers::{self as alkane_helpers, clear};
use alkanes::view;
use alkanes_support::cellpack::Cellpack;
use alkanes_support::constants::AUTH_TOKEN_FACTORY_ID;
use alkanes_support::id::AlkaneId;
use alkanes_support::utils::string_to_u128_list;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn deploy_and_read_view() -> anyhow::Result<()> {
    clear();                                   // reset the in-memory indexer state

    // owned-token opcode 1 = InitializeWithNameSymbol(auth_units, token_units, name, symbol).
    // inputs[0] is the opcode; Strings are packed into u128s via string_to_u128_list.
    let mut init = vec![1u128, 1, 1000];
    init.extend(string_to_u128_list("MyToken".to_string()));
    init.extend(string_to_u128_list("MTK".to_string()));

    // Each binary is deployed in its own tx; each Cellpack is that tx's protostone.
    // {3, AUTH_TOKEN_FACTORY_ID} deploys the auth-token factory; {1,0} deploys + calls
    // owned-token, which lands at {2,1}.
    let block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        vec![
            alkanes_std_auth_token_build::get_bytes(),
            alkanes_std_owned_token_build::get_bytes(),
        ],
        vec![
            Cellpack { target: AlkaneId { block: 3, tx: AUTH_TOKEN_FACTORY_ID }, inputs: vec![100] },
            Cellpack { target: AlkaneId { block: 1, tx: 0 }, inputs: init },
        ],
    );

    // Run it through the REAL indexer — the exact code path used on mainnet.
    index_block(&block, 0)?;

    // Call the view opcode 99 (GetName) with call_view — no state change, like eth_call.
    let owned = AlkaneId { block: 2, tx: 1 };
    let name = view::call_view(&owned, &vec![99u128], 100_000 /* fuel */)?;
    assert_eq!(String::from_utf8(name)?, "MyToken");
    Ok(())
}
```

Run it with `cargo test`. You've now exercised a contract's `wasmi` execution and a view call in the precise environment it will see live on mainnet — no funds, no regtest.

To read the **persisted execution trace** of a real indexed call (rather than a stateless view), use `view::trace(&outpoint)`. For a full worked contract — a custom alkane with its own `build.rs`, a state-changing mint, and trace assertions — see the [free-mint](https://github.com/kungfuflex/free-mint) and [oyl-amm](https://github.com/kungfuflex/oyl-amm) crates, which test against this harness the same way.

**Notes**

- Tests use `#[wasm_bindgen_test]` (not `#[test]`), because the suite compiles to and runs on wasm — the same target as the indexer.
- The `test-utils` feature is mandatory on the `alkanes` / `protorune` / `metashrew-core` dev-deps; it's what exposes `alkanes::tests::helpers`.
- Deploy stock contracts with `alkanes::precompiled::*::get_bytes()`; deploy your own with a `build.rs`-generated `get_bytes()`.

## Using alkanes-cli

`alkanes-cli` is the command-line client for the ALKANES protocol, Bitcoin, ordinals/runes, and the SUBFROST APIs. Build it from this repo:

```sh
cargo build --release -p alkanes-cli
# binary at target/release/alkanes-cli
```

Point it at any SUBFROST endpoint with `-p <network> --jsonrpc-url <url>`. The public mainnet JSON-RPC is free (rate-limited); for higher limits, put your API key in the path (`/v4/<API_KEY>`):

```sh
# current indexed block height
alkanes-cli -p mainnet --jsonrpc-url https://mainnet.subfrost.io/v4/jsonrpc metashrew height
# -> 958629

# inspect a deployed alkane's bytecode + code hash
alkanes-cli -p mainnet --jsonrpc-url https://mainnet.subfrost.io/v4/jsonrpc alkanes inspect 2:0 --codehash
# -> Bytecode Length: 262445 bytes
#    Code Hash: 3b16eaa2d72f4695cb47fc58ecbbd2909d9c403a703e7db5a688ab3612fdbe00
```

Command namespaces include `wallet`, `alkanes` (execute / simulate / trace / inspect / swap), `ord`, `esplora`, `dataapi`, `metashrew`, `runestone`, `protorunes`, `subfrost`, and `brc20-prog`. Full reference: [https://api.subfrost.io/docs/cli/overview](https://api.subfrost.io/docs/cli/overview)

## Building on Alkanes with @alkanes/ts-sdk

`@alkanes/ts-sdk` is the TypeScript library for building on ALKANES — encrypted keystores, HD wallets, transaction construction, and typed clients (`AlkanesRpcClient`, `AlkanesProvider`) for the protocol and SUBFROST data APIs. Install the immutable, content-addressed build from `pkg.alkanes.build`:

```sh
npm install "https://pkg.alkanes.build/dist/@alkanes/ts-sdk?v=0.1.6-669e7c0"
```

Create an encrypted keystore and a wallet:

```ts
import { createKeystore, createWallet } from '@alkanes/ts-sdk';

// ethers-style encrypted keystore (PBKDF2) + a BIP39 mnemonic
const { keystore, mnemonic } = await createKeystore('your-password', { network: 'mainnet' });
const wallet = await createWallet(keystore, 'your-password');
```

The typed clients read from the same endpoints as the CLI (`https://mainnet.subfrost.io/v4/jsonrpc`). Full reference: [https://api.subfrost.io/docs](https://api.subfrost.io/docs)

## Resources

**Start building today — no indexer to host.** SUBFROST serves live ALKANES + Bitcoin data over a public, rate-limited API; get an API key for higher limits.

- **API access & docs** — [https://api.subfrost.io](https://api.subfrost.io) (mainnet JSON-RPC: `https://mainnet.subfrost.io/v4/jsonrpc`)

**Explore & build**

- **espo** — open-source ALKANES indexer & explorer engine — [https://espo.sh](https://espo.sh)
- **Block explorer** — [https://explorer.subfrost.io](https://explorer.subfrost.io)
- **SUBFROST app** (swaps, frBTC, AMM) — [https://app.subfrost.io](https://app.subfrost.io)
- **Ecosystem** — [https://subfrost.io/ecosystem](https://subfrost.io/ecosystem)
- **Governance** (WIP) — [https://surtur.org](https://surtur.org)

**Wallets — ALKANES-enabled Bitcoin, everywhere**

- **Chrome extension** — [https://subfrost.io/download](https://subfrost.io/download)
- **Android** — [Google Play](https://play.google.com/store/apps/details?id=io.subfrost.android) (iOS coming soon — so ALKANES-enabled Bitcoin works on mobile)
- **espo** will soon release an **open-source wallet** for those running their own infrastructure.

**Stay updated**

- **Articles & updates** — [https://subfrost.io/articles](https://subfrost.io/articles)

## Acknowledgements

ALKANES is carried forward by a community of builders, indexers, contract authors, and researchers who believe finance can be open, permissionless, and native to Bitcoin. To everyone writing code, running infrastructure, shipping contracts, filing issues, and stress-testing the protocol in the open — thank you. This work moves because you move it, toward a vision of free finance for everyone. 🧡

### Authors

- flex
- v16
- butenprks
- clothic
- m3

### License

MIT
