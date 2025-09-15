The primary objective is to get the `alkanes-rs` test suite to pass by running `cargo test`.

The core issue is a series of compilation errors related to type mismatches between `metashrew_core::index_pointer::IndexPointer` and `metashrew_core::index_pointer::AtomicPointer`. The `WasmHost` struct in `src/lib.rs` needs to provide the functionality of both types: the transactional methods of `AtomicPointer` for the VM, and the `IndexPointer` for the `protorune_support::host::Host` trait.

Here's a summary of the key files and their roles:
- `src/lib.rs`: Defines the `WasmHost` struct and its trait implementations.
- `src/vm/utils.rs`: Contains utility functions that operate on pointers.
- `src/vm/host_functions.rs`: Contains the host functions called from within the Wasm VM, which make heavy use of the `WasmHost`.
- `src/utils.rs`: Contains additional utility functions.

The task is to refactor these files to correctly handle the pointer types and resolve all compilation errors. The final goal is to run `cargo test` and have all tests pass.