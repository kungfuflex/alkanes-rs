# Quick Start: Simplified Package Distribution

This document provides a quick overview of the simplified package distribution setup.

## Overview

The alkanes-rs publishing pipeline uses **public** Google Cloud Artifact Registry repositories, allowing users to install packages without authentication.

## For Administrators

### Initial Setup (One-time)

1. **Set up GCP infrastructure** (follow `.github/docs/GCP_SETUP.md`):
   - Create GCP project `distributable-octet-pipeline`
   - Enable Artifact Registry
   - Set up Workload Identity Federation
   - Configure GitHub Secrets

2. **Make repositories public**:
   ```bash
   ./.github/scripts/make-registries-public.sh
   ```

3. **Set up Cloudflare DNS** (optional for custom domain):
   ```bash
   export CLOUDFLARE_API_TOKEN="your-token"
   ./.github/scripts/setup-cloudflare-dns.sh
   ```

4. **Done!** Push to `develop` branch to trigger publishing.

## For Users (Installing Packages)

### npm (TypeScript SDK)

**One-time configuration:**
```bash
npm config set @alkanes:registry https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
```

**Install:**
```bash
npm install @alkanes/ts-sdk
```

**Or add to `.npmrc` in your project:**
```
@alkanes:registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
```

Then:
```bash
npm install @alkanes/ts-sdk
```

### cargo (Rust Crates)

**Option 1: Git dependencies (Recommended - No config needed)**
```toml
[dependencies]
alkanes = { git = "https://github.com/kungfuflex/alkanes-rs", branch = "develop" }
```

**Option 2: Registry**

Add to `~/.cargo/config.toml` (one-time):
```toml
[registries.alkanes]
index = "sparse+https://us-central1-cargo.pkg.dev/distributable-octet-pipeline/cargo-packages/"
```

Then in `Cargo.toml`:
```toml
[dependencies]
alkanes = { version = "10.0.0", registry = "alkanes" }
```

## How It Works

```
Developer pushes to develop branch
          ↓
GitHub Actions workflows trigger
          ↓
    Authenticate with Workload Identity (secure, keyless)
          ↓
    Build packages (WASM + TypeScript / Rust crates)
          ↓
    Publish to public Artifact Registry
          ↓
Users install without authentication (public read access)
```

## Security Model

- **Publishing**: Restricted to authorized GitHub workflows via Workload Identity
- **Downloading**: Public (anyone can download)
- **No credentials needed**: Users never need to authenticate
- **Secure**: Service account only has write permissions, protected by Workload Identity

## Key Benefits

✅ **Simple for users**: No authentication configuration needed
✅ **Secure for publishing**: Keyless auth via Workload Identity
✅ **Automated**: Every push to `develop` publishes automatically
✅ **Versioned**: Every build gets a unique version with commit SHA
✅ **Cost-effective**: ~$5-20/month for Artifact Registry storage

## URLs

- npm packages: https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
- cargo packages: https://us-central1-cargo.pkg.dev/distributable-octet-pipeline/cargo-packages/
- Console: https://console.cloud.google.com/artifacts?project=distributable-octet-pipeline

## Troubleshooting

**Package not found?**
- Check workflow runs: https://github.com/kungfuflex/alkanes-rs/actions
- Verify registry is configured correctly

**Registry not working?**
- For npm: `npm config get @alkanes:registry`
- For cargo: Check `~/.cargo/config.toml`

**Need help?**
- Full guide: `PUBLISHING.md`
- Detailed troubleshooting: `TROUBLESHOOTING.md`
- GCP setup: `.github/docs/GCP_SETUP.md`

## Next Steps

1. Complete GCP setup (`.github/docs/GCP_SETUP.md`)
2. Run `make-registries-public.sh` to enable public access
3. Test by pushing to `develop` branch
4. Share installation instructions with users

## Documentation

- **For Users**: `PUBLISHING.md` (installation and usage)
- **For Admins**:
  - `.github/docs/GCP_SETUP.md` (infrastructure setup)
  - `.github/docs/REVERSE_PROXY_SETUP.md` (advanced reverse proxy options)
  - `TROUBLESHOOTING.md` (debugging guide)
