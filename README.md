# alkanes-rs

![Tests](https://img.shields.io/github/actions/workflow/status/AssemblyScript/assemblyscript/test.yml?branch=main&label=test&logo=github)
![Publish](https://img.shields.io/github/actions/workflow/status/AssemblyScript/assemblyscript/publish.yml?branch=main&label=publish&logo=github)

**The ALKANES specification is hosted at** 👉🏻👉🏼👉🏽👉🏾👉🏿 [https://github.com/kungfuflex/alkanes-rs/wiki](https://github.com/kungfuflex/alkanes-rs/wiki)

This repository hosts Rust sources for the ALKANES metaprotocol. The indexer for ALKANES can be built as the top level crate in the monorepo, with builds targeting wasm32-unknown-unknown, usable within the METASHREW indexer stack.

ALKANES is a metaprotocol designed to support an incarnation of DeFi as we have traditionally seen it, but designed specifically for the Bitcoin consensus model and supporting structures.

#### NOTE: ALKANES does not have a network token

Protocol fees are accepted in terms of Bitcoin and compute is metered with the wasmi fuel implementation, for protection against DoS.

## Software Topology

This repository is a pure Rust implementation, built entirely for a WASM target and even tested withim the WASM test runner `wasm-bindgen-test-runner`.

The top level crate in the monorepo contains sources for the ALKANES indexer, built for the METASHREW environment.

ALKANES is designed and implemented as a subprotocol of runes, one which is protorunes compatible. In order to encapsulate the behavior of protorunes for a Rust build system, a Rust implementation of protorunes generics is contained in the monorepo in `crates/protorune`.

For information on protorunes, refer to the specification hosted at:

[https://github.com/kungfuflex/protorune/wiki](https://github.com/kungfuflex/protorune/wiki)

The indexer stack used to synchronize the state of the metaprotocol and offer an RPC to consume its data and features is METASHREW. METASHREW is started with a WASM binary of the indexer program, produced with a normal build of this repository as `alkanes.wasm`.

Bindings to the METASHREW environment are available in `crates/metashrew`.

Sources needed to build both metashrew and protorunes meant to be shared with builds of individual alkanes or the generic alkanes-runtime bindings are factored out into `crates/metashrew-support` and `crates/protorune-support` such that they can be imported into an alkane build without the metashrew import definitions leaking in and generating import statements for the METASHREW environment.

In this way, all crates with a `-support` suffix can be imported into any Rust project since they do not depend on a specific environment or `wasm-bindgen`.

This design is permissive enough for this monorepo to host `alkanes-runtime`, which is a complete set of bindings for building alkane smart contracts to a WASM format, suitable for deployment within the witness envelope of a Bitcoin transaction.

Boilerplate for various alkanes are included and prefixed with `alkanes-std-` and placed in the `crates/` directory. The build system is designed such that the WASM builds of each crate with this prefix is made available to the test suite as a Rust source file.

## Building

ALKANES is built with the command:

```sh
cargo build --release
```

An `alkanes.wasm` file will be built, as well as a WASM for every crate prefixed with `alkanes-std-`, which will be built to `target/alkanes/wasm32-unknown-unknown/release`

## Indexing

Refer to the METASHREW documentation for descriptions of the indexer stack used for ALKANES.

[https://github.com/sandshrewmetaprotocols/metashrew](https://github.com/sandshrewmetaprotocols/metashrew)

A sample command may look like:

```sh
~/metashrew/target/release/metashrew-keydb --redis redis+unix:///home/ubuntu/keydb/keydb.sock --rpc-url http://localhost:8332 --auth bitcoinrpc:bitcoinrpc --indexer ~/alkanes/target/release/alkanes.wasm
```

### Testing

To test the indexer end-to-end, it is only required to run:

```
cargo test
```

This will provide a stub environment to test a METASHREW indexer program, and it will test the alkanes standard library smart contracts in simulated blocks.

Features are provided within the Cargo.toml at the root of the monorepo to declare alkanes which should be built with `cargo build` or `cargo test`.

### Authors

- flex
- v16
- butenprks
- clothic
- m3

### License

MIT
