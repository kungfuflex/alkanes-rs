# Quick Start: Deterministic Builds & Verification

> **TL;DR:** Build WASM contracts that anyone can verify match the source code.

## For Developers: Publishing a Verifiable Release

```bash
# 1. Tag your release
git tag -a v1.0.0 -m "Release v1.0.0"

# 2. Build deterministically
./scripts/build-reproducible.sh

# 3. Get the checksum
cat artifacts/checksums.txt
# Output: abc123... free-mint.wasm

# 4. Publish to GitHub releases with:
#    - The WASM file (artifacts/free-mint.wasm)
#    - The SHA256 checksum
#    - The git commit hash
#    - Build info (artifacts/build-info.json)
```

## For Users: Verifying a Published Build

```bash
# 1. Clone and checkout the tagged release
git clone <repo-url>
cd free-mint
git checkout v1.0.0

# 2. Build it yourself
./scripts/build-reproducible.sh

# 3. Compare with published checksum
./scripts/verify-build.sh <published-sha256>

# If verification succeeds, the bytecode matches the source! ✅
```

## What You Need

### Required
- Docker (for reproducible builds)
- Git

### Optional (for local dev)
- Rust 1.90.0 (pinned in rust-toolchain.toml)
- wasm32-unknown-unknown target

## Quick Commands

| Task | Command |
|------|---------|
| **Reproducible build** | `./scripts/build-reproducible.sh` |
| **Verify checksum** | `./scripts/verify-build.sh <sha256>` |
| **Local dev build** | `cargo build --release` |
| **Run tests** | `cargo test` |
| **Clean artifacts** | `rm -rf artifacts/` |

## What Gets Created

```
artifacts/
├── free-mint.wasm      # Your verified WASM binary
├── checksums.txt       # SHA256: abc123... free-mint.wasm
└── build-info.json     # Build metadata (Rust version, timestamp, etc.)
```

## Troubleshooting

**"Docker not found"**
- Install Docker: https://docs.docker.com/get-docker/

**"Checksums don't match"**
- Verify you're on the same git commit
- Check you're using the Docker build (not `cargo build`)
- Ensure same architecture (ARM64 vs x86_64)
- Try `docker system prune` and rebuild

**"Build is slow"**
- First build downloads dependencies (slow)
- Subsequent builds use Docker cache (faster)
- Development builds: use `cargo build` (fast but not reproducible)

## Why This Matters

Without reproducible builds:
- ❌ Can't verify deployed bytecode matches source
- ❌ Security audits can't confirm running code
- ❌ Users must trust the developer

With reproducible builds:
- ✅ Anyone can verify bytecode = source
- ✅ Trustless verification (like Etherscan)
- ✅ Security through transparency

## Architecture Notes

⚠️ **Important:** ARM64 (Apple Silicon) and x86_64 (Intel) produce **different** WASM binaries.

When publishing, specify architecture:
```
Releases:
├── free-mint-x86_64.wasm  (SHA256: abc...)
└── free-mint-arm64.wasm   (SHA256: def...)
```

Users must verify against their architecture's build.

## Full Documentation

See [DETERMINISTIC_BUILDS.md](./DETERMINISTIC_BUILDS.md) for:
- Technical deep-dive
- How it works under the hood
- Comparison with CosmWasm, Substrate
- Known limitations and workarounds
- FAQs

## Example Verification Flow

```bash
$ git checkout v1.0.0
Switched to tag 'v1.0.0'

$ ./scripts/build-reproducible.sh
==========================================
  Building with Docker (Reproducible)
==========================================

Starting reproducible build...
[... build output ...]
✓ Build Complete!

$ cat artifacts/checksums.txt
a1b2c3d4e5f6... free-mint.wasm

$ ./scripts/verify-build.sh a1b2c3d4e5f6...
==========================================
  WASM Build Verification
==========================================

Actual SHA256:   a1b2c3d4e5f6...
Expected SHA256: a1b2c3d4e5f6...

✓ VERIFICATION SUCCESSFUL!

The WASM binary matches the expected checksum.
This build is reproducible and verified.
```

---

**Got questions?** See [DETERMINISTIC_BUILDS.md](./DETERMINISTIC_BUILDS.md) or open an issue.
