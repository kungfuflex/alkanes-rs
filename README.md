# alkanes-rs

![Tests](https://img.shields.io/github/actions/workflow/status/kungfuflex/alkanes-rs/rust.yml?branch=main&label=tests&logo=github)
![Publish](https://img.shields.io/github/actions/workflow/status/kungfuflex/alkanes-rs/publish-npm.yml?branch=main&label=publish&logo=github)

**The ALKANES specification is hosted at** 👉🏻👉🏼👉🏽👉🏾👉🏿 [https://github.com/kungfuflex/alkanes-rs/wiki](https://github.com/kungfuflex/alkanes-rs/wiki)

This repository hosts Rust sources for the ALKANES metaprotocol. The indexer for ALKANES can be built as the top level crate in the monorepo, with builds targeting wasm32-unknown-unknown, usable within the METASHREW indexer stack.

ALKANES is a metaprotocol designed to support an incarnation of DeFi as we have traditionally seen it, but designed specifically for the Bitcoin consensus model and supporting structures.
The ALKANES genesis block is 880000. Builders are encouraged to test on regtest using the docker-compose environment at [https://github.com/kungfuflex/alkanes](https://github.com/kungfuflex/alkanes)

A signet RPC will be available on https://signet.sandshrew.io

Join ALKANES / metashrew discussion on the SANDSHREW サンド Discord.

#### NOTE: ALKANES does not have a network token

Protocol fees are accepted in terms of Bitcoin and compute is metered with the wasmi fuel implementation, for protection against DoS.

## Software Topology

This repository is a pure Rust implementation, built entirely for a WASM target and even tested within the WASM test runner `wasm-bindgen-test-runner`.

The top level crate in the monorepo contains sources for the ALKANES indexer, built for the METASHREW environment.

ALKANES is designed and implemented as a subprotocol of runes, one which is protorunes compatible. In order to encapsulate the behavior of protorunes for a Rust build system, a Rust implementation of protorunes generics is contained in the monorepo in `crates/protorune`.

For information on protorunes, refer to the specification hosted at:

[https://github.com/kungfuflex/protorune/wiki](https://github.com/kungfuflex/protorune/wiki)

The indexer stack used to synchronize the state of the metaprotocol and offer an RPC to consume its data and features is METASHREW. METASHREW is started with a WASM binary of the indexer program, produced with a normal build of this repository as `alkanes.wasm`.

Bindings to the METASHREW environment are consumed from the pinned [`sandshrewmetaprotocols/metashrew`](https://github.com/sandshrewmetaprotocols/metashrew) dependency; the environment-agnostic pieces live in `crates/metashrew-support`.

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

Refer to the METASHREW documentation for descriptions of the indexer stack used for ALKANES.

[https://github.com/sandshrewmetaprotocols/metashrew](https://github.com/sandshrewmetaprotocols/metashrew)

### Running against mainnet

To index ALKANES on mainnet you must run matching, current versions of both components:

- **alkanes-rs `v2.2.1-rc.5`** (latest) — built to `alkanes.wasm` and loaded via `--indexer`. See [releases](https://github.com/kungfuflex/alkanes-rs/releases).
- **metashrew `v9.0.5-rc.13`** — the [`kungfuflex/metashrew`](https://github.com/kungfuflex/metashrew/releases/tag/v9.0.5-rc.13) indexer stack (`rockshrew-mono`).

Running mismatched versions can produce divergent state. Live mainnet system health — indexer height, sync status, and RPC availability — can be checked at **[https://mainnet.subfrost.io](https://mainnet.subfrost.io)**.

A sample command may look like:

```sh
~/metashrew/target/release/rockshrew-mono --daemon-rpc-url http://localhost:8332 --auth bitcoinrpc:bitcoinrpc --db-path ~/.metashrew --indexer ~/alkanes-rs/target/wasm32-unknown-unknown/release/alkanes.wasm --start-block 880000 --host 0.0.0.0 --port 8080 --cors '*'
```

### Testing

To run all tests in the monorepo

```
# this might be necessary if running into: could not execute process `wasm-bindgen-test-runner...
cargo install -f wasm-bindgen-cli --version 0.2.100
```

```
cargo test --all
```

To test the alkanes indexer end-to-end, it is only required to run:

```
cargo test
```

To run tests for a specific crate

```
cargo test -p [CRATE]
```

example:

```
cargo test --features test-utils -p protorune
```

This will provide a stub environment to test a METASHREW indexer program, and it will test the alkanes standard library smart contracts in simulated blocks.

Features are provided within the Cargo.toml at the root of the monorepo to declare alkanes which should be built with `cargo build` or `cargo test`.

### Unit testing

- These are written inside the library rust code
- Do not compile to wasm, instead unit test the native rust. Therefore, you need to find the correct target for your local machine to properly run these tests. Below are some common targets for some architectures:
  - Macbook intel x86: x86_64-apple-darwin
  - Macbook Apple silicon: aarch64-apple-darwin
  - Ubuntu 20.04 LTS: x86_64-unknown-linux-gnu

```
cargo test -p protorune --target TARGET
```

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
