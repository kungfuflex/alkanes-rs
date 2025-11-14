# alkanes-macros

Proc macro crate for ALKANES.

## Running Tests

Due to the workspace default target being `wasm32-unknown-unknown`, integration tests need to be run with the host target. You can do this in two ways:

### Option 1: Use the test script (recommended)

```bash
./test.sh
```

### Option 2: Specify the target manually

```bash
# Get your host target
rustc -vV | grep host

# Run tests with that target (replace with your actual host target)
cargo test --target aarch64-apple-darwin  # for Apple Silicon
# or
cargo test --target x86_64-apple-darwin   # for Intel Mac
# or
cargo test --target x86_64-unknown-linux-gnu  # for Linux
```

## Note on Dependencies

The `.cargo/config.toml` in this directory (if present) only affects builds when running `cargo` commands from this directory. It does **not** affect other crates that depend on `alkanes-macros` because:

1. **Proc macros are always compiled for the host target** - When other crates depend on `alkanes-macros`, the proc macro itself is compiled for the machine running the build (host target), not the target being built for. This is because proc macros run at compile time, not runtime.

2. **Config is directory-scoped** - The `.cargo/config.toml` only applies when running cargo from this specific directory or its subdirectories.

So it's safe for other crates to depend on `alkanes-macros` even if they're building to `wasm32-unknown-unknown`.
