# Deterministic WASM Builds for Alkane Contracts

This document explains the comprehensive approach to building reproducible WASM binaries for the `free-mint` alkane contract. These techniques enable bytecode verification similar to Etherscan's source verification.

## The Problem

Rust's compiler (`rustc`) targeting `wasm32-unknown-unknown` produces **non-deterministic** bytecode. The same source code compiled on different systems (macOS vs Linux, or even different directories) produces different binaries, making verification impossible.

**Why this matters for Alkanes/Espo:**
- Users can't verify that deployed bytecode matches published source code
- Security audits can't confirm the running code
- Reproducible builds are essential for trustless verification

## Our Multi-Layered Solution

We've implemented **all** known techniques for deterministic builds, based on battle-tested approaches from CosmWasm:

### 1. Strict Compiler Settings (Cargo.toml)

```toml
[profile.release]
codegen-units = 1          # CRITICAL: Single compilation unit
lto = true                 # Link-time optimization
opt-level = 3              # Maximum optimization
panic = "abort"            # Smaller binaries
incremental = false        # No incremental compilation
strip = "symbols"          # Strip symbols
```

**Key:** `codegen-units = 1` prevents parallel compilation, which is a major source of non-determinism.

### 2. Compiler Flags (.cargo/config.toml)

```toml
[target.wasm32-unknown-unknown]
rustflags = [
    "-C", "codegen-units=1",
    "-C", "lto=fat",
    "-C", "embed-bitcode=no",
    "--remap-path-prefix", "$HOME=~",
]
```

These flags ensure:
- Single compilation unit (redundant with Cargo.toml for safety)
- Full LTO across all dependencies
- No embedded LLVM bitcode
- Normalized file paths (partial - has known wasm issues)

### 3. Pinned Rust Version (rust-toolchain.toml)

```toml
[toolchain]
channel = "1.90.0"
targets = ["wasm32-unknown-unknown"]
```

Different Rust versions produce different output. This file ensures everyone uses the exact same compiler.

### 4. Docker-Based Reproducible Builds

The most reliable approach: containerized builds with a locked environment.

**Files:**
- `Dockerfile.builder` - Defines the build environment
- `scripts/docker-build.sh` - Build script (runs inside container)
- `scripts/build-reproducible.sh` - Convenience wrapper

**Why Docker:**
- ✅ Identical OS, libraries, and toolchain for everyone
- ✅ Eliminates path-based non-determinism
- ✅ Isolates from host system differences
- ✅ Version-controlled build environment

### 5. Post-Processing with wasm-opt

After compilation, we run `wasm-opt -Os` which:
- Further optimizes for size
- Normalizes the output format
- Adds an extra layer of determinism

### 6. Verification Scripts

`scripts/verify-build.sh` - Compare checksums against published builds

## Usage Guide

### Quick Start: Reproducible Build

```bash
# One command to build deterministically
./scripts/build-reproducible.sh
```

This will:
1. Build the Docker image (if not exists)
2. Compile the contract in the container
3. Optimize with wasm-opt
4. Generate checksums in `artifacts/`

**Output:**
```
artifacts/
├── free-mint.wasm        # Optimized WASM binary
├── checksums.txt         # SHA256 hash
└── build-info.json       # Build metadata
```

### Verifying a Build

```bash
# Get the published SHA256 (from GitHub release, docs, etc.)
PUBLISHED_HASH="abc123..."

# Build locally
./scripts/build-reproducible.sh

# Verify match
./scripts/verify-build.sh $PUBLISHED_HASH
```

If the hashes match: ✅ **Verified!** The bytecode matches the source.

### Local Development Builds

For faster iteration during development:

```bash
cargo build --release
```

This uses the deterministic settings from `Cargo.toml` and `.cargo/config.toml`, but may still vary across systems. **Use Docker builds for verification.**

## How to Publish Verifiable Builds

1. **Tag the release:**
   ```bash
   git tag -a v1.0.0 -m "Release v1.0.0"
   git push origin v1.0.0
   ```

2. **Build with Docker:**
   ```bash
   ./scripts/build-reproducible.sh
   ```

3. **Document the checksum:**
   ```bash
   cat artifacts/checksums.txt
   # abc123... free-mint.wasm
   ```

4. **Publish:**
   - Upload `artifacts/free-mint.wasm` to GitHub releases
   - Document the SHA256 in release notes
   - Include git commit hash
   - Include build metadata from `artifacts/build-info.json`

5. **Users verify:**
   ```bash
   git checkout v1.0.0
   ./scripts/build-reproducible.sh
   ./scripts/verify-build.sh abc123...
   ```

## Architecture-Specific Builds

**Important:** ARM64 and x86_64 produce different WASM artifacts!

We provide architecture-specific Docker images:
- `alkane-builder:latest` (x86_64)
- `alkane-builder:arm64` (Apple Silicon, ARM servers)

For maximum compatibility, **publish both** with clear labeling:
- `free-mint-x86_64.wasm` (SHA256: ...)
- `free-mint-arm64.wasm` (SHA256: ...)

Users must match their architecture when verifying.

## Technical Background

### Why is rustc Non-Deterministic?

Several factors cause different output:
1. **Parallel codegen** - Different ordering of compilation units
2. **File paths** - Embedded in debug info and metadata
3. **Timestamps** - Some build metadata includes time
4. **LLVM versions** - Different Rust versions use different LLVM
5. **Platform differences** - OS-specific code generation

### What We've Addressed

| Issue | Solution | Effectiveness |
|-------|----------|---------------|
| Parallel codegen | `codegen-units=1` | ✅ Complete |
| File paths | `--remap-path-prefix` | ⚠️ Partial (wasm bugs) |
| LLVM versions | `rust-toolchain.toml` | ✅ Complete |
| Platform differences | Docker container | ✅ Complete |
| Optimization variance | `opt-level=3`, `lto=true` | ✅ Complete |
| Symbol variations | `strip=symbols` | ✅ Complete |

### Known Limitations

1. **ARM64 vs x86_64** - Different architectures produce different output (fundamental limitation)
2. **Path remapping** - Has known bugs with wasm32-unknown-unknown (Rust issue [#80776](https://github.com/rust-lang/rust/issues/80776))
3. **Incremental cache** - Must be disabled (`incremental=false`)

## Comparison with Other Projects

### CosmWasm
- Uses `cosmwasm/rust-optimizer` Docker images
- Achieves byte-for-byte reproducibility on same architecture
- Powers hundreds of millions in TVL
- **We adapted their approach**

### Substrate/Polkadot
- Similar challenges with WASM runtime
- Uses `srtool` (Substrate Runtime Tooling)
- Also Docker-based for determinism

### Ethereum Smart Contracts
- Solidity compiler has better determinism
- Still uses verification services (Etherscan, Sourcify)

## Frequently Asked Questions

**Q: Why can't I just use `cargo build --release`?**
A: You can for development, but different systems will produce different bytecode. Docker ensures identical environment.

**Q: Do I need Docker for local development?**
A: No. Use regular `cargo build` for development. Only use Docker when you need to verify or publish canonical builds.

**Q: What if the checksums don't match?**
A: Check:
1. Same git commit?
2. Using Docker build?
3. Same architecture (ARM64 vs x86_64)?
4. Clean build (`docker system prune` if needed)?

**Q: How much overhead does this add?**
A: Compile time is ~20-30% longer due to `codegen-units=1` and `lto=true`. Docker adds ~10s startup overhead. Worth it for verifiability.

**Q: Can I verify old deployments?**
A: Yes! If you have:
1. Git commit hash
2. Published SHA256
3. Same Rust version (from that time)

Then checkout the commit and run the Docker build.

## References

- [CosmWasm rust-optimizer](https://github.com/CosmWasm/optimizer)
- [CosmWasm verification](https://github.com/CosmWasm/cosmwasm-verify)
- [Rust RFC 3127: trim-paths](https://rust-lang.github.io/rfcs/3127-trim-paths.html)
- [Rust Issue #117597: Non-deterministic wasm32 output](https://github.com/rust-lang/rust/issues/117597)
- [Rust Issue #128675: codegen-units non-determinism](https://github.com/rust-lang/rust/issues/128675)

## Future Improvements

1. **wasm-in-wasm compiler** - Compile rustc itself to WASM for perfect reproducibility
2. **CI automation** - Automated verification on PRs
3. **Public verification service** - Like Etherscan for Alkanes
4. **Multi-arch support** - Better ARM64 vs x86_64 handling

## Contributing

If you find ways to improve determinism, please:
1. Test thoroughly (multiple systems)
2. Document the change
3. Compare checksums before/after
4. Submit a PR with verification results

---

**Summary:** This setup provides the most comprehensive approach to deterministic WASM builds currently possible in Rust. While not 100% perfect due to compiler limitations, it achieves byte-for-byte reproducibility within the same architecture using Docker, enabling trustless source verification for Alkane contracts.
