# Publishing Guide for alkanes-rs

This document explains how the alkanes-rs automated publishing pipeline works and how to consume the published packages.

## Overview

The alkanes-rs repository uses automated publishing to Google Cloud Artifact Registry for both:
- **TypeScript SDK** (`@alkanes/ts-sdk`) - npm package
- **Rust crates** (31 workspace crates) - cargo registry

Every push to the `develop` branch automatically triggers publishing workflows that build and publish packages.

## Repository URLs

- **npm**: https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
- **cargo**: sparse+https://us-central1-cargo.pkg.dev/distributable-octet-pipeline/cargo-packages/
- **Web Console**: https://console.cloud.google.com/artifacts?project=distributable-octet-pipeline

## Quick Start (For Consumers)

The repositories are public - no authentication required!

### Install TypeScript SDK (npm)

**Option 1: Simple registry setup (Recommended)**
```bash
# One-time configuration
npm config set @alkanes:registry https://pkg.alkanes.build/

# Install
npm install @alkanes/ts-sdk
```

**Option 2: Direct package.json**
```json
{
  "dependencies": {
    "@alkanes/ts-sdk": "npm:@alkanes/ts-sdk@*"
  }
}
```

Then add to `.npmrc` in your project:
```
@alkanes:registry=https://pkg.alkanes.build/
```

No authentication needed! Just install and use.

### Install Rust Crates (cargo)

**Option 1: Git dependencies (Simplest)**
```toml
[dependencies]
alkanes = { git = "https://github.com/kungfuflex/alkanes-rs", branch = "develop" }
```

**Option 2: Public registry**
```toml
# Add to ~/.cargo/config.toml (one-time)
[registries.alkanes]
index = "sparse+https://us-central1-cargo.pkg.dev/distributable-octet-pipeline/cargo-packages/"

# Then in your Cargo.toml
[dependencies]
alkanes = { version = "10.0.0", registry = "alkanes" }
```

No authentication needed!

---

## For Developers (Publishing)

### Automatic Publishing

Publishing happens automatically when you push to the `develop` branch:

```bash
git checkout develop
git pull origin develop

# Make your changes
git add .
git commit -m "Your changes"
git push origin develop
```

The following workflows will trigger:

1. **`.github/workflows/publish-npm.yml`** - Publishes TypeScript SDK
   - Triggers when: `ts-sdk/`, `crates/alkanes-web-sys/`, or `prod_wasms/` changes
   - Version format: `0.1.0-dev.TIMESTAMP.SHA` (e.g., `0.1.0-dev.20251215143022.2e5c0ea`)
   - Build process:
     1. Build WASM from `crates/alkanes-web-sys`
     2. Vendor production WASMs from `prod_wasms/`
     3. Build TypeScript SDK
     4. Publish to Artifact Registry
   - Tags: `latest-dev` always points to latest version

2. **`.github/workflows/publish-cargo.yml`** - Publishes all Rust crates
   - Triggers when: `crates/`, `Cargo.toml`, or `Cargo.lock` changes
   - Version: Workspace version `10.0.0` with metadata `+dev.TIMESTAMP.SHA`
   - Publishes 31 crates in dependency order
   - Skips already-published versions

### Monitoring Workflow Runs

Check the status of workflow runs:
- GitHub Actions: https://github.com/kungfuflex/alkanes-rs/actions
- Workflow status badges show in PR checks
- Each run creates a summary with installation instructions

### Version Management

**TypeScript SDK versions** are auto-generated:
- Format: `MAJOR.MINOR.PATCH-dev.TIMESTAMP.SHA`
- Example: `0.1.0-dev.20251215143022.2e5c0ea`
- Each push creates a unique version
- `latest-dev` tag always points to the most recent version

**Rust crate versions** use the workspace version:
- Base version: `10.0.0` (defined in root `Cargo.toml`)
- Metadata: `+dev.TIMESTAMP.SHA` (informational only)
- To publish a new version, update the version in `Cargo.toml`

### Manual Publishing (Advanced)

If you need to publish manually:

**npm (TypeScript SDK)**:
```bash
cd ts-sdk

# Authenticate with GCP
gcloud auth login
gcloud auth print-access-token > /tmp/token.txt

# Configure npm
cat > .npmrc <<EOF
@alkanes:registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
//us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/:_authToken=$(cat /tmp/token.txt)
EOF

# Build
npm ci
cd ../crates/alkanes-web-sys
wasm-pack build --target bundler --out-dir ../../ts-sdk/build/wasm
cd ../../ts-sdk
npm run build:vendor
npm run build:ts

# Publish
npm publish --registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
```

**cargo (Rust crates)**:
```bash
# Configure Cargo (already done in .cargo/config.toml)
# Add credentials
mkdir -p ~/.cargo
cat > ~/.cargo/credentials.toml <<EOF
[registries.alkanes]
token = "$(gcloud auth print-access-token)"
EOF

# Publish using manual script
bash .github/scripts/publish-crates.sh
```

## For Consumers (Installation)

> **Note**: The repositories are PUBLIC. No authentication required for downloads!

### Using the TypeScript SDK

#### 1. Configure npm (one-time)

```bash
npm config set @alkanes:registry https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
```

Or add to your project's `.npmrc`:
```
@alkanes:registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
```

#### 2. Install

```bash
# Install latest dev version
npm install @alkanes/ts-sdk@latest-dev

# Or install specific version
npm install @alkanes/ts-sdk@0.1.0-dev.20251215143022.2e5c0ea

# Or just "latest"
npm install @alkanes/ts-sdk
```

#### 4. Use in your code

```typescript
import { AlkanesClient } from '@alkanes/ts-sdk';

const client = new AlkanesClient({
  network: 'mainnet',
  // ... configuration
});

// Use the client
await client.getBalance('bc1q...');
```

### Using Rust Crates

**Option 1: Git dependencies (Recommended - Simplest)**

Just use git dependencies - no configuration needed:

```toml
[dependencies]
alkanes = { git = "https://github.com/kungfuflex/alkanes-rs", branch = "develop" }
alkanes-runtime = { git = "https://github.com/kungfuflex/alkanes-rs", branch = "develop" }

# Or use a specific tag/version:
# alkanes = { git = "https://github.com/kungfuflex/alkanes-rs", tag = "v10.0.0" }
```

**Option 2: Public registry**

#### 1. Configure Cargo (one-time)

Add to `~/.cargo/config.toml`:

```toml
[registries.alkanes]
index = "sparse+https://us-central1-cargo.pkg.dev/distributable-octet-pipeline/cargo-packages/"

[net]
git-fetch-with-cli = true
```

No authentication needed - the registry is public!

#### 2. Use in Cargo.toml

```toml
[dependencies]
alkanes = { version = "10.0.0", registry = "alkanes" }
alkanes-runtime = { version = "10.0.0", registry = "alkanes" }
alkanes-cli-common = { version = "10.0.0", registry = "alkanes" }
protorune = { version = "10.0.0", registry = "alkanes" }

# Or use git for latest develop
# alkanes = { git = "https://github.com/kungfuflex/alkanes-rs", branch = "develop" }
```

#### 4. Build

```bash
cargo build
```

Cargo will fetch the crates from the Artifact Registry.

### Available Rust Crates

The workspace contains 31 crates organized in dependency tiers:

**Tier 1 (Base):**
- metashrew-core
- metashrew-support
- ordinals
- alkanes-macros
- alkanes-pretty-print-macro
- alkanes-build

**Tier 2 (Core Runtime):**
- metashrew-runtime
- metashrew-minimal
- metashrew-sync
- protorune-support
- rockshrew-diff

**Tier 3 (Extended Runtime):**
- protorune
- alkanes-support
- rockshrew-runtime
- rockshrew-mono
- memshrew-runtime
- memshrew-p2p

**Tier 4 (Alkanes Core):**
- alkanes-runtime
- alkanes-asc
- alkanes-cli-common
- alkanes

**Tier 5 (Bindings & Tools):**
- alkanes-web-sys
- alkanes-web-leptos
- alkanes-ffi
- alkanes-jni
- alkanes-cli-sys
- alkanes-cli
- alkanes-jsonrpc
- alkanes-data-api
- alkanes-contract-indexer
- alkanes-trace-transform

## CI/CD Integration

> **Note**: Repositories are public - no authentication needed!

### GitHub Actions (npm)

```yaml
- name: Configure npm
  run: |
    echo "@alkanes:registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/" > .npmrc

- name: Install dependencies
  run: npm install
```

No secrets needed!

### GitHub Actions (cargo)

**Option 1: Git dependencies (simplest)**
```yaml
# No configuration needed - just use git dependencies in Cargo.toml
- name: Build
  run: cargo build
```

**Option 2: Registry**
```yaml
- name: Configure Cargo
  run: |
    mkdir -p ~/.cargo
    cat >> ~/.cargo/config.toml <<EOF
    [registries.alkanes]
    index = "sparse+https://us-central1-cargo.pkg.dev/distributable-octet-pipeline/cargo-packages/"
    EOF

- name: Build
  run: cargo build
```

### Docker

```dockerfile
# For npm
RUN echo "@alkanes:registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/" > .npmrc
RUN npm install

# For cargo (Option 1: git dependencies)
# No configuration needed - just build
RUN cargo build

# For cargo (Option 2: registry)
RUN mkdir -p /root/.cargo && \
    echo '[registries.alkanes]\nindex = "sparse+https://us-central1-cargo.pkg.dev/distributable-octet-pipeline/cargo-packages/"' > /root/.cargo/config.toml
RUN cargo build
```

Simple build:
```bash
docker build .
```

## Troubleshooting

### npm: Registry not configured

**Cause**: Registry not set for @alkanes scope

**Solution**:
```bash
npm config set @alkanes:registry https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
```

Or add to `.npmrc`:
```
@alkanes:registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
```

### cargo: Git dependencies preferred

**Recommendation**: For simplest installation, use git dependencies:

```toml
[dependencies]
alkanes = { git = "https://github.com/kungfuflex/alkanes-rs", branch = "develop" }
```

### Package not found

**Cause**: Package not yet published or wrong registry

**Solution**:
- Check workflow runs: https://github.com/kungfuflex/alkanes-rs/actions
- Verify registry URL in .npmrc or Cargo.toml
- Check package exists: https://console.cloud.google.com/artifacts?project=distributable-octet-pipeline

### Workflow failed to publish

**Cause**: Various (authentication, build errors, network issues)

**Solution**:
1. Check workflow logs in GitHub Actions
2. Look for error messages in the publish step
3. Verify GCP credentials are configured correctly
4. Check the TROUBLESHOOTING.md guide

## Package Information

### TypeScript SDK Package Details

- **Name**: `@alkanes/ts-sdk`
- **Size**: ~7-8 MB (includes WASM + contract WASMs)
- **Formats**: CommonJS and ES modules
- **Exports**:
  - Main SDK: `@alkanes/ts-sdk`
  - WASM module: `@alkanes/ts-sdk/wasm`
- **Browser compatible**: Yes
- **Node.js compatible**: Yes

### Rust Crates Details

- **Workspace version**: 10.0.0
- **Total crates**: 31
- **Repository**: Google Cloud Artifact Registry
- **License**: MIT
- **Documentation**: https://github.com/kungfuflex/alkanes-rs

## Support

- **Issues**: https://github.com/kungfuflex/alkanes-rs/issues
- **Documentation**: See README.md and inline docs
- **Troubleshooting**: See TROUBLESHOOTING.md

## Additional Resources

- **GCP Setup Guide**: `.github/docs/GCP_SETUP.md`
- **Workflow Files**:
  - npm: `.github/workflows/publish-npm.yml`
  - cargo: `.github/workflows/publish-cargo.yml`
- **Scripts**:
  - Cloudflare DNS: `.github/scripts/setup-cloudflare-dns.sh`
  - Manual cargo publish: `.github/scripts/publish-crates.sh`
