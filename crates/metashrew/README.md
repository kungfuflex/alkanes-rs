# Metashrew Crate for ALKANES-RS

This crate provides Metashrew bindings for the ALKANES-RS project. It has been updated to use the `metashrew-lib` crate, which provides convenient macros and primitives for building Metashrew indexer programs.

## Overview

The `metashrew` crate serves as a compatibility layer between the ALKANES-RS project and the `metashrew-lib` crate. It maintains the same API as the original implementation but uses the new `metashrew-lib` crate under the hood.

## Features

- **Backward Compatibility**: Maintains the same API as the original implementation
- **Modern Implementation**: Uses the new `metashrew-lib` crate under the hood
- **Improved Macros**: Provides access to the new macros from `metashrew-lib`
- **Type Safety**: Leverages the type-safe view functions from `metashrew-lib`

## Usage

### Original API

You can continue to use the original API:

```rust
use metashrew::{flush, input, println};

// Get input data
let data = input();

// Process data
// ...

// Flush changes
flush();
```

### New API via metashrew-lib

You can also use the new API from `metashrew-lib`:

```rust
use metashrew::lib::{metashrew_indexer, indexer::{Indexer, KeyValueStore}};

// Define your indexer
struct MyIndexer {
    store: KeyValueStore,
}

impl Indexer for MyIndexer {
    fn index_block(&mut self, height: u32, block: &[u8]) -> anyhow::Result<()> {
        // Process the block and update the store
        Ok(())
    }
    
    fn flush(&self) -> anyhow::Result<Vec<(Vec<u8>, Vec<u8>)>> {
        Ok(self.store.pairs())
    }
}

// Define the Metashrew indexer program
metashrew_indexer! {
    struct MyIndexerProgram {
        indexer: MyIndexer,
        views: {
            // View functions
        }
    }
}
```

## Migration Guide

If you're migrating from the original implementation to the new `metashrew-lib` crate, here are some tips:

1. **Use the Macros**: The new macros (`metashrew_indexer!`, `metashrew_view!`, `metashrew_proto_program!`) make it easier to define indexers and view functions.

2. **Implement the Indexer Trait**: Instead of directly implementing the `_start` function, implement the `Indexer` trait.

3. **Use Type-Safe View Functions**: The new macros support type-safe view functions using Serde or Protocol Buffers.

4. **Use the Host Functions**: The new host functions provide a safer and more convenient API for interacting with the Metashrew runtime.

## License

MIT