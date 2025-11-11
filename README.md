# alkanes-rs

![Tests](https://img.shields.io/github/actions/workflow/status/AssemblyScript/assemblyscript/test.yml?branch=main&label=test&logo=github)
![Publish](https://img.shields.io/github/actions/workflow/status/AssemblyScript/assemblyscript/publish.yml?branch=main&label=publish&logo=github)

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

Bindings to the METASHREW environment are available in `crates/metashrew`.

Sources needed to build both metashrew and protorunes meant to be shared with builds of individual alkanes or the generic alkanes-runtime bindings are factored out into `crates/metashrew-support` and `crates/protorune-support` such that they can be imported into an alkane build without the metashrew import definitions leaking in and generating import statements for the METASHREW environment.

In this way, all crates with a `-support` suffix can be imported into any Rust project since they do not depend on a specific environment or `wasm-bindgen`.

This design is permissive enough for this monorepo to host `alkanes-runtime`, which is a complete set of bindings for building alkane smart contracts to a WASM format, suitable for deployment within the witness envelope of a Bitcoin transaction.

Boilerplate for various alkanes are included and prefixed with `alkanes-std-` and placed in the `alkanes/` directory. Pre-built WASM files for all alkanes are committed to the repository in `crates/alkanes/src/tests/std/wasm/` and are used by the test suite.

## Building

ALKANES indexer is built with the command:

```sh
cargo build --release
```

This will build the `alkanes.wasm` indexer binary at `target/wasm32-unknown-unknown/release/alkanes.wasm`.

### Building Standard Alkanes

The standard alkanes (prefixed with `alkanes-std-`) have pre-built WASM files committed to the repository. To rebuild them (only needed if you modify the alkane source code), run:

```sh
./scripts/build-std.sh
```

This script will:
- Build all alkanes in `alkanes/` to WASM
- Generate network-specific builds (bellscoin, luckycoin, mainnet, fractal, regtest, testnet) for alkanes that require them
- Place the WASM files in `crates/alkanes/src/tests/std/wasm/`
- Regenerate `crates/alkanes/src/tests/std/mod.rs` with the appropriate module declarations

The WASM files are platform-independent and are committed to the repository so developers don't need to rebuild them unless modifying the alkane source code

## Indexing

Refer to the METASHREW documentation for descriptions of the indexer stack used for ALKANES.

[https://github.com/sandshrewmetaprotocols/metashrew](https://github.com/sandshrewmetaprotocols/metashrew)

A sample command may look like:

```sh
~/metashrew/target/release/rockshrew-mono --daemon-rpc-url http://localhost:8332 --auth bitcoinrpc:bitcoinrpc --db-path ~/.metashrew --indexer ~/alkanes-rs/target/wasm32-unknown-unknown/release/alkanes.wasm --start-block 880000 --host 0.0.0.0 --port 8080 --cors '*'
```

### Testing

#### Prerequisites

If you encounter issues with `wasm-bindgen-test-runner`, install the correct version:

```sh
cargo install -f wasm-bindgen-cli --version 0.2.100
```

#### Running ALKANES Tests

The alkanes crate tests run in WebAssembly using `wasm-bindgen-test`. There are two ways to run them:

**Option 1: From the alkanes package directory (recommended)**

```sh
cd crates/alkanes
cargo test --lib
```

The package-level `.cargo/config.toml` automatically sets the target to `wasm32-unknown-unknown`.

**Option 2: From the workspace root with explicit target**

```sh
cargo test -p alkanes --target wasm32-unknown-unknown --lib
```

#### Running Other Tests

To test a specific crate (non-WASM tests):

```sh
cargo test -p [CRATE]
```

Example:

```sh
cargo test --features test-utils -p protorune
```

#### Unit Testing (Native Rust)

Some crates have unit tests that run on native Rust (not WASM). For these, you may need to specify your target architecture:

- Macbook Intel x86: `x86_64-apple-darwin`
- Macbook Apple Silicon: `aarch64-apple-darwin`
- Ubuntu 20.04 LTS: `x86_64-unknown-linux-gnu`

```sh
cargo test -p protorune --target TARGET
```

### Authors

- flex
- v16
- butenprks
- clothic
- m3

## Quick Usage Examples

### Wallet Operations

```bash
# Create a new wallet
alkanes wallet create

# Get addresses
alkanes wallet addresses

# Check balance
alkanes wallet balance

# Send Bitcoin
alkanes wallet send bc1p... 10000 --fee-rate 600 -y
```

### Alkanes Operations

```bash
# Wrap BTC to frBTC
alkanes alkanes wrap-btc 100000 --from "p2tr:0" --mine -y

# Execute an alkanes contract
alkanes alkanes execute \
  --inputs "B:10000" \
  --to "bc1p..." \
  --protostones "[32,0,77]" \
  -y

# Get balance
alkanes alkanes getbalance --address "bc1p..."

# Inspect a contract
alkanes alkanes inspect <outpoint> --disasm
```

### BRC20-Prog Operations

```bash
# Deploy a smart contract
alkanes brc20-prog deploy-contract ./out/MyContract.sol/MyContract.json \
  --from "p2tr:0" --mine -y

# Call a contract function
alkanes brc20-prog transact \
  --address 0x1234... \
  --signature "transfer(address,uint256)" \
  --calldata "0x5678...,1000" \
  --from "p2tr:0" -y

# Wrap BTC and execute
alkanes brc20-prog wrap-btc 100000 \
  --target 0xABCD... \
  --signature "deposit()" \
  --calldata "" \
  --from "p2tr:0" -y
```

## Documentation

Comprehensive documentation is available in the [`docs/`](./docs) directory:

### Core Documentation
- **[Documentation Index](./docs/README.md)** - Complete documentation structure
- **[Getting Started](./docs/quickstart.md)** - Quick start guide
- **[CLI Usage](./docs/cli-usage.md)** - Complete CLI reference

### Protocol Features
- **[BRC20-Prog Guide](./docs/cli/brc20-prog.md)** - BRC20 programmable contracts
- **[Wrap-BTC Feature](./docs/features/wrap-btc.md)** - Wrapping BTC to frBTC
- **[External Signing](./docs/features/external-signing.md)** - Address-only mode and external key signing
- **[Transaction Broadcasting](./docs/features/transaction-broadcasting.md)** - All broadcast options (Slipstream, Rebar, etc.)
- **[Rebar Shield](./docs/features/rebar-shield.md)** - Private relay with MEV protection

### Development
- **[Architecture](./docs/architecture/overview.md)** - System design and components
- **[Crates Reference](./docs/crates/)** - Detailed crate documentation
- **[Developer Guide](./docs/dev/building-alkanes.md)** - Building alkane contracts
- **[Examples](./docs/examples/)** - Usage examples and patterns
- **[Helper Scripts](./scripts/README.md)** - Transaction building and broadcasting scripts

For detailed API documentation and protocol specifications, see:
- [Alkanes Wiki](https://github.com/kungfuflex/alkanes-rs/wiki) - Protocol specification
- [Protorune Spec](https://github.com/kungfuflex/protorune/wiki) - Protorune protocol
- [Metashrew](https://github.com/sandshrewmetaprotocols/metashrew) - Indexer stack

### License

MIT
