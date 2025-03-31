# Metashrew Library

A library for building Metashrew indexer programs with convenient macros and primitives.

## Overview

The `metashrew-lib` crate provides a high-level API for building Metashrew indexer programs. It simplifies the development of WASM modules for the Metashrew Bitcoin indexer framework by providing:

- Convenient macros for defining indexers and view functions
- Safe wrappers around the Metashrew host functions
- Protocol Buffer integration for type-safe communication
- Utilities for working with the Metashrew database
- Support for building native standalone binaries

## Features

- **Declarative Indexer Definition**: Define your indexer and view functions using simple macros
- **Type-Safe View Functions**: Use Serde or Protocol Buffers for type-safe view functions
- **Safe Host Function Wrappers**: Interact with the Metashrew host functions safely
- **Key-Value Store Abstraction**: Work with a simple key-value store abstraction
- **Protocol Buffer Integration**: Use Protocol Buffers for efficient serialization
- **Native Binary Support**: Build native standalone binaries for better performance

## Usage

### Basic Indexer

```rust
use anyhow::Result;
use metashrew_lib::{metashrew_indexer, indexer::{Indexer, KeyValueStore}};
use serde::{Deserialize, Serialize};

// Define your indexer
struct MyIndexer {
    store: KeyValueStore,
}

impl Default for MyIndexer {
    fn default() -> Self {
        Self {
            store: KeyValueStore::new(),
        }
    }
}

impl Indexer for MyIndexer {
    fn index_block(&mut self, height: u32, block: &[u8]) -> Result<()> {
        // Process the block and update the store
        self.store.set(b"last_height".to_vec(), height.to_le_bytes().to_vec());
        Ok(())
    }
    
    fn flush(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        Ok(self.store.pairs())
    }
}

// Define view function input/output types
#[derive(Deserialize)]
struct GetHeightInput {
    format: String,
}

#[derive(Serialize)]
struct GetHeightOutput {
    height: u32,
    formatted: String,
}

impl MyIndexer {
    // Define a view function
    fn get_height(&self, input: GetHeightInput) -> Result<GetHeightOutput> {
        let height = if let Some(height_bytes) = self.store.get(b"last_height") {
            u32::from_le_bytes([
                height_bytes[0], height_bytes[1], height_bytes[2], height_bytes[3],
            ])
        } else {
            0
        };
        
        let formatted = match input.format.as_str() {
            "hex" => format!("0x{:x}", height),
            _ => format!("{}", height),
        };
        
        Ok(GetHeightOutput { height, formatted })
    }
}

// Define the Metashrew indexer program
metashrew_indexer! {
    struct MyIndexerProgram {
        indexer: MyIndexer,
        views: {
            "get_height" => get_height(GetHeightInput) -> GetHeightOutput,
        }
    }
}
```

### Protocol Buffer Indexer

```rust
use anyhow::Result;
use metashrew_lib::{declare_indexer, indexer::{Indexer, KeyValueStore}};
use protobuf::Message;

// Define Protocol Buffer messages
#[derive(Clone, PartialEq, Message)]
pub struct GetBalanceRequest {
    #[prost(string, tag = "1")]
    pub address: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetBalanceResponse {
    #[prost(uint64, tag = "1")]
    pub balance: u64,
}

// Define your indexer
struct TokenIndexer {
    store: KeyValueStore,
}

impl Default for TokenIndexer {
    fn default() -> Self {
        Self {
            store: KeyValueStore::new(),
        }
    }
}

impl Indexer for TokenIndexer {
    fn index_block(&mut self, height: u32, block: &[u8]) -> Result<()> {
        // Process the block and update the store
        Ok(())
    }
    
    fn flush(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        Ok(self.store.pairs())
    }
}

impl TokenIndexer {
    // Define a view function
    fn get_balance(&self, request: GetBalanceRequest) -> Result<GetBalanceResponse> {
        let address = request.address;
        let balance_key = format!("balance:{}", address).into_bytes();
        
        let balance = if let Some(balance_bytes) = self.store.get(&balance_key) {
            u64::from_le_bytes([
                balance_bytes[0], balance_bytes[1], balance_bytes[2], balance_bytes[3],
                balance_bytes[4], balance_bytes[5], balance_bytes[6], balance_bytes[7],
            ])
        } else {
            0
        };
        
        let mut response = GetBalanceResponse::new();
        response.balance = balance;
        
        Ok(response)
    }
}

// Define the Metashrew indexer program with Protocol Buffer messages
declare_indexer! {
    struct TokenIndexerProgram {
        indexer: TokenIndexer,
        views: {
            "get_balance" => {
                fn get_balance(&self, request: GetBalanceRequest) -> Result<GetBalanceResponse> {
                    self.get_balance(request)
                }
            }
        }
    }
}
```

### Native Binary

You can also build a native standalone binary for better performance:

```rust
use anyhow::Result;
use metashrew_lib::{
    indexer::{Indexer, KeyValueStore, NativeIndexer, ProtoViewFunctionWrapper, ViewFunctionWrapper},
    native_binary,
    view::ProtoViewFunction,
};
use protobuf::Message;
use std::collections::HashMap;

// Define Protocol Buffer messages
#[derive(Clone, PartialEq, Message)]
pub struct GetBalanceRequest {
    #[prost(string, tag = "1")]
    pub address: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct GetBalanceResponse {
    #[prost(uint64, tag = "1")]
    pub balance: u64,
}

// Define your indexer
#[derive(Clone)]
struct TokenIndexer {
    store: KeyValueStore,
}

impl Default for TokenIndexer {
    fn default() -> Self {
        Self {
            store: KeyValueStore::new(),
        }
    }
}

impl Indexer for TokenIndexer {
    fn index_block(&mut self, height: u32, block: &[u8]) -> Result<()> {
        // Process the block and update the store
        Ok(())
    }
    
    fn flush(&self) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        Ok(self.store.pairs())
    }
}

impl TokenIndexer {
    // Define a view function
    fn get_balance(&self, request: GetBalanceRequest) -> Result<GetBalanceResponse> {
        let address = request.address;
        let balance_key = format!("balance:{}", address).into_bytes();
        
        let balance = if let Some(balance_bytes) = self.store.get(&balance_key) {
            u64::from_le_bytes([
                balance_bytes[0], balance_bytes[1], balance_bytes[2], balance_bytes[3],
                balance_bytes[4], balance_bytes[5], balance_bytes[6], balance_bytes[7],
            ])
        } else {
            0
        };
        
        let mut response = GetBalanceResponse::new();
        response.balance = balance;
        
        Ok(response)
    }
}

// Implement the ProtoViewFunction trait
impl ProtoViewFunction<GetBalanceRequest, GetBalanceResponse> for TokenIndexer {
    fn execute_proto(&self, request: GetBalanceRequest) -> Result<GetBalanceResponse> {
        self.get_balance(request)
    }
}

// Implement the NativeIndexer trait
impl NativeIndexer for TokenIndexer {
    fn view_functions(&self) -> HashMap<String, Box<dyn ViewFunctionWrapper>> {
        let mut map = HashMap::new();
        
        // Add the get_balance view function
        map.insert(
            "get_balance".to_string(),
            Box::new(ProtoViewFunctionWrapper::<Self, GetBalanceRequest, GetBalanceResponse>::new(
                self.clone(),
            )),
        );
        
        map
    }
}

// Define the native binary
native_binary! {
    indexer: TokenIndexer,
    name: "token-indexer",
    version: "0.1.0",
    about: "A simple token indexer",
}
```

## Building and Running

### WASM Module

To build your Metashrew indexer program as a WASM module:

```bash
cargo build --target wasm32-unknown-unknown --release
```

This will produce a WebAssembly module that can be loaded by Metashrew.

To run your indexer with Metashrew:

```bash
rockshrew-mono --daemon-rpc-url http://localhost:8332 \
  --auth bitcoinrpc:password \
  --indexer path/to/your_indexer.wasm \
  --db-path ~/.metashrew \
  --host 0.0.0.0 \
  --port 8080
```

### Native Binary

To build your Metashrew indexer program as a native binary:

```bash
cargo build --features native --release
```

This will produce a native binary that can be run directly:

```bash
./target/release/your-indexer --daemon-rpc-url http://localhost:8332 \
  --auth bitcoinrpc:password \
  --db-path ~/.metashrew \
  --host 0.0.0.0 \
  --port 8080
```

## API Reference

### Macros

- `metashrew_indexer!`: Define a Metashrew indexer program with Serde-based view functions
- `metashrew_view!`: Define a standalone Metashrew view function
- `declare_indexer!`: Define a Metashrew indexer program with Protocol Buffer-based view functions
- `native_binary!`: Define a native binary for a Metashrew indexer program

### Host Functions

- `load_input()`: Load the input data from the host
- `log(message)`: Log a message to the host
- `get(key)`: Get a value from the database
- `flush(pairs)`: Flush key-value pairs to the database

### Indexer

- `Indexer` trait: Implement this trait to define your indexer
- `NativeIndexer` trait: Implement this trait to define a native indexer
- `KeyValueStore`: A simple key-value store for indexer state

### View Functions

- `ViewFunction` trait: Implement this trait to define view functions
- `ProtoViewFunction` trait: Implement this trait to define view functions with Protocol Buffer messages
- `parse_view_input()`: Parse the input for a view function
- `return_view_result()`: Return the result of a view function

## License

MIT