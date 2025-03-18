# ALKANES-RS Technical Context

## Technologies Used

### Programming Languages

- **Rust**: The primary language used throughout the project. Chosen for its performance, memory safety, and strong type system.
- **WebAssembly (WASM)**: The compilation target for both the indexer and smart contracts, enabling portable and sandboxed execution.

### Runtime Environments

- **METASHREW Indexer Stack**: The underlying infrastructure for processing blockchain data and maintaining state.
- **wasmi**: WebAssembly interpreter used for executing smart contracts in a controlled environment.

### Serialization Formats

- **Protocol Buffers (protobuf)**: Used for data serialization and RPC interfaces.
- **Bitcoin Serialization**: Used for parsing and handling Bitcoin blockchain data.

### Storage

- **Key-Value Storage**: Used for persistent state management, implemented through the METASHREW indexer stack.

### Testing Frameworks

- **wasm-bindgen-test-runner**: Used for testing WebAssembly code.
- **Rust's built-in testing framework**: Used for unit and integration testing.

## Development Setup

### Prerequisites

- **Rust Toolchain**: Required for building the project.
  ```
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- **wasm32-unknown-unknown Target**: Required for compiling to WebAssembly.
  ```
  rustup target add wasm32-unknown-unknown
  ```

- **wasm-bindgen-cli**: Required for testing WebAssembly code.
  ```
  cargo install -f wasm-bindgen-cli --version 0.2.99
  ```

### Building the Project

The project can be built with Cargo, specifying the desired network:

```sh
cargo build --release --features all,<network>
```

Where `<network>` is one of:
- `mainnet`: Bitcoin mainnet
- `testnet`: Bitcoin testnet
- `regtest`: Bitcoin regtest
- `dogecoin`: Dogecoin network
- `luckycoin`: Luckycoin network
- `bellscoin`: Bellscoin network
- `fractal`: Fractal network

The build produces:
- `alkanes.wasm`: Main indexer binary
- Standard library contract WASMs in `target/alkanes/wasm32-unknown-unknown/release/`

### Testing

The project includes several testing approaches:

1. **Unit Tests**: Testing individual components.
   ```
   cargo test -p <crate-name>
   ```

2. **Integration Tests**: Testing interactions between components.
   ```
   cargo test
   ```

3. **WASM Tests**: Testing the compiled WebAssembly.
   ```
   cargo test --all
   ```

### Running the Indexer

The ALKANES indexer can be run with the METASHREW indexer stack:

```sh
metashrew-keydb --redis <redis-url> --rpc-url <bitcoin-rpc> --auth <auth> --indexer <path-to-alkanes.wasm>
```

## View Functions and Data Access

### View Function Architecture

ALKANES-RS provides several view functions for querying the state of the system including but not limited to:

- **protorunes_by_address**: Returns all protorunes held by a specific address
  ```rust
  pub fn protorunes_by_address(input: &Vec<u8>) -> Result<WalletResponse>
  ```
  - Uses the `OUTPOINTS_FOR_ADDRESS` table to find outpoints associated with an address
  - Then uses the `OUTPOINT_TO_RUNES` table to get the balances for those outpoints
  - Takes a `ProtorunesWalletRequest` with wallet (address) and protocol_tag parameters
  - Returns a `WalletResponse` with outpoints and their balances

- **runes_by_address**: Returns all runes held by a specific address
  ```rust
  pub fn runes_by_address(input: &Vec<u8>) -> Result<WalletResponse>
  ```
  - Similar to `protorunes_by_address` but for all runes, not just protorunes
  - Uses the same tables but doesn't filter by protocol_tag

- **protorunes_by_outpoint**: Returns protorune balances for a specific outpoint
  ```rust
  pub fn protorunes_by_outpoint(input: &Vec<u8>) -> Result<OutpointResponse>
  ```
  - Directly queries the `OUTPOINT_TO_RUNES` table for a specific outpoint
  - Takes an `OutpointRequest` with outpoint and protocol_tag parameters
  - Returns an `OutpointResponse` with the balances for that outpoint

These functions are exposed through the WASM runtime and can be called via RPC.

#### View Function Dependencies and Testing

When testing view functions, it's important to understand their dependencies:

1. **Table Dependencies**:
   - `protorunes_by_address` depends on the `OUTPOINTS_FOR_ADDRESS` and `OUTPOINT_TO_RUNES` tables
   - `protorunes_by_outpoint` depends on the `OUTPOINT_TO_RUNES` table

2. **Testing Considerations**:
   - Ensure that the necessary tables are populated before calling view functions
   - Avoid double indexing as it can lead to inconsistent state between tables
   - Use the `clear()` function between tests to ensure a clean state

3. **Common Issues**:
   - Double indexing can cause token IDs to be assigned differently than expected
   - Incorrect token IDs in requests will result in empty responses

### Protobuf Message Encoding

The view functions use Protocol Buffers for input and output serialization. Key message types include:

- **ProtorunesWalletRequest**: Request for the `protorunes_by_address` function
  ```protobuf
  message ProtorunesWalletRequest {
    bytes wallet = 1;
    optional Uint128 protocol_tag = 2;
  }
  ```

- **WalletResponse**: Response for wallet-related queries
  ```protobuf
  message WalletResponse {
    repeated OutpointResponse outpoints = 1;
  }
  ```

- **OutpointResponse**: Information about an outpoint and its balances
  ```protobuf
  message OutpointResponse {
    optional BalanceSheet balances = 1;
    optional Outpoint outpoint = 2;
    optional Output output = 3;
    uint32 height = 4;
    uint32 txindex = 5;
    bytes address = 6;
  }
  ```

When using these messages in RPC calls, it's important to ensure proper encoding of nested message types like `Uint128`, which should be encoded as length-delimited fields rather than simple varints.

## Technical Constraints

### Bitcoin Compatibility

ALKANES must operate within the constraints of the Bitcoin protocol:
- Limited transaction size
- Limited script capabilities
- No native smart contract support
- Immutable transaction history

### WebAssembly Limitations

Smart contracts must operate within WebAssembly constraints:
- Limited memory model
- No direct system access
- Deterministic execution
- Limited floating-point precision

### Indexer Performance

The indexer must efficiently process blockchain data:
- Handle large blocks and transactions
- Maintain state consistency
- Provide responsive queries
- Scale with blockchain growth

### Cross-Network Support

The system must support multiple Bitcoin-based networks:
- Different address formats
- Different block structures (e.g., Auxpow for some networks)
- Different activation heights
- Network-specific parameters

## Dependencies

### Core Dependencies

- **bitcoin (0.32.4)**: Bitcoin data structures and utilities
  ```toml
  bitcoin = { version = "0.32.4", features = ["rand"] }
  ```

- **wasmi (0.37.2)**: WebAssembly interpreter
  ```toml
  wasmi = "0.37.2"
  ```

- **protobuf (3.7.1)**: Protocol Buffers implementation
  ```toml
  protobuf = "3.7.1"
  ```

- **anyhow (1.0.90)**: Error handling utilities
  ```toml
  anyhow = "1.0.90"
  ```

### Internal Dependencies

- **metashrew**: Indexer stack for blockchain data
  ```toml
  metashrew = { path = "crates/metashrew" }
  ```

- **protorune**: Implementation of the protorunes protocol
  ```toml
  protorune = { path = "crates/protorune" }
  ```

- **alkanes-support**: Core utilities for the ALKANES protocol
  ```toml
  alkanes-support = { path = "crates/alkanes-support" }
  ```

### Development Dependencies

- **wasm-bindgen-test (0.3.49)**: Testing framework for WebAssembly
  ```toml
  wasm-bindgen-test = "0.3.49"
  ```

- **protobuf-codegen (3.4.0)**: Code generation for Protocol Buffers
  ```toml
  protobuf-codegen = "3.4.0"
  ```

## Feature Flags

The project uses feature flags to control compilation:

### Network Features

- `mainnet`: Bitcoin mainnet support
- `testnet`: Bitcoin testnet support
- `regtest`: Bitcoin regtest support
- `dogecoin`: Dogecoin network support
- `luckycoin`: Luckycoin network support
- `bellscoin`: Bellscoin network support
- `fractal`: Fractal network support

### Component Features

- `proxy`: Include proxy contract functionality
- `owned_token`: Include owned token contract
- `auth_token`: Include authentication token contract
- `genesis_alkane`: Include genesis contract
- `amm_pool`: Include AMM pool contract
- `amm_factory`: Include AMM factory contract
- `orbital`: Include orbital functionality
- `minimal`: Include a minimal set of contracts
- `all`: Include all contracts

## Build System

The project uses Cargo as its build system, with custom build scripts for:

1. **Protocol Buffer Generation**: Generating Rust code from .proto files
   ```rust
   // build.rs
   protoc_rust::Codegen::new()
       .out_dir("src/proto")
       .inputs(&["proto/alkanes.proto"])
       .include("proto")
       .run()
       .expect("protoc");
   ```

2. **WASM Compilation**: Compiling smart contracts to WebAssembly
   ```rust
   // crates/alkanes-build/src/main.rs
   Command::new("cargo")
       .args(&["build", "--target", "wasm32-unknown-unknown", "--release"])
       .current_dir(path)
       .status()
       .expect("Failed to build WASM");
   ```

3. **Precompiled Contract Integration**: Embedding compiled contracts into the indexer
   ```rust
   // src/precompiled/mod.rs
   pub mod alkanes_std_auth_token_build;
   pub mod alkanes_std_genesis_alkane_build;
   // ...
   ```

## Deployment Considerations

### Resource Requirements

- **CPU**: Moderate to high, especially during initial synchronization
- **Memory**: Moderate, depending on the size of the state
- **Storage**: High, as the blockchain and state grow
- **Network**: Moderate, for blockchain synchronization

### Security Considerations

- **Fuel Metering**: Prevents DoS attacks through resource exhaustion
- **Sandboxed Execution**: Isolates contract execution from the host system
- **Input Validation**: Ensures only valid transactions are processed
- **Error Handling**: Gracefully handles invalid inputs and execution failures

### Monitoring and Maintenance

- **Logs**: The system produces logs for debugging and monitoring
- **State Backup**: Regular backups of the state are recommended
- **Version Compatibility**: Updates should maintain compatibility with existing contracts
- **Network Synchronization**: The indexer must stay synchronized with the blockchain