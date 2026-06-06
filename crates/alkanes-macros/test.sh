#!/bin/bash
# Test script for alkanes-macros that runs tests on the host target
# This is needed because the default target is wasm32-unknown-unknown,
# but integration tests need to run natively

# Get the host target
HOST_TARGET=$(rustc -vV | sed -n 's|host: ||p')

# Run tests with the host target
cargo test --target "$HOST_TARGET" "$@"

