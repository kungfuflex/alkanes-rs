# Troubleshooting Guide

This guide helps diagnose and fix common issues with the alkanes-rs publishing pipeline and package installation.

## Table of Contents

1. [Publishing Workflow Issues](#publishing-workflow-issues)
2. [npm Installation Issues](#npm-installation-issues)
3. [Cargo Installation Issues](#cargo-installation-issues)
4. [GCP Authentication Issues](#gcp-authentication-issues)
5. [Cloudflare DNS Issues](#cloudflare-dns-issues)
6. [Build Issues](#build-issues)
7. [Rollback Procedures](#rollback-procedures)

---

## Publishing Workflow Issues

### Workflow doesn't trigger

**Symptoms**: Push to develop, but no workflow runs

**Diagnosis**:
```bash
# Check if workflows are enabled
gh workflow list

# Check workflow syntax
actionlint .github/workflows/publish-npm.yml
actionlint .github/workflows/publish-cargo.yml
```

**Solutions**:
1. Ensure files changed match the `paths` filter:
   - npm workflow: `ts-sdk/`, `crates/alkanes-web-sys/`, `prod_wasms/`
   - cargo workflow: `crates/`, `Cargo.toml`, `Cargo.lock`

2. Check if workflows are disabled:
   ```bash
   gh workflow enable publish-npm.yml
   gh workflow enable publish-cargo.yml
   ```

3. Verify branch name is exactly `develop`:
   ```bash
   git branch --show-current
   ```

### Workflow fails with "Workload Identity authentication failed"

**Symptoms**:
```
Error: Failed to generate Google Cloud access token
Error: google-github-actions/auth failed with: retry function failed after 1 attempt(s)
```

**Diagnosis**:
```bash
# Verify GitHub secrets are set
gh secret list

# Required secrets:
# - GCP_WORKLOAD_IDENTITY_PROVIDER
# - GCP_SERVICE_ACCOUNT
# - GCP_PROJECT_ID
```

**Solutions**:

1. Verify secrets are correct:
   ```bash
   # Get the provider name from GCP
   gcloud iam workload-identity-pools providers describe "github-provider" \
     --location="global" \
     --workload-identity-pool="github-actions-pool" \
     --format="value(name)"

   # Should match GCP_WORKLOAD_IDENTITY_PROVIDER secret
   ```

2. Check service account binding:
   ```bash
   SA_EMAIL="github-actions-publisher@distributable-octet-pipeline.iam.gserviceaccount.com"
   gcloud iam service-accounts get-iam-policy "${SA_EMAIL}"

   # Look for workloadIdentityUser binding for kungfuflex/alkanes-rs
   ```

3. Verify workload identity pool configuration:
   ```bash
   gcloud iam workload-identity-pools describe github-actions-pool \
     --location=global
   ```

4. Re-add the workload identity binding:
   ```bash
   GITHUB_REPO="kungfuflex/alkanes-rs"
   PROJECT_NUMBER=$(gcloud projects describe distributable-octet-pipeline --format='value(projectNumber)')

   gcloud iam service-accounts add-iam-policy-binding "${SA_EMAIL}" \
     --role="roles/iam.workloadIdentityUser" \
     --member="principalSet://iam.googleapis.com/projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/github-actions-pool/attribute.repository/${GITHUB_REPO}"
   ```

### npm workflow fails at "Publish to Artifact Registry"

**Symptoms**:
```
npm ERR! 401 Unauthorized
npm ERR! need auth This command requires you to be logged in to https://us-central1-npm.pkg.dev/...
```

**Diagnosis**:
```bash
# Check token generation step in workflow logs
# Look for: "GCP access token length: XXXX"
# Token should be ~800+ characters
```

**Solutions**:

1. Verify service account has write permissions:
   ```bash
   gcloud artifacts repositories get-iam-policy npm-packages --location=us-central1
   # Should show github-actions-publisher with roles/artifactregistry.writer
   ```

2. Add permissions if missing:
   ```bash
   SA_EMAIL="github-actions-publisher@distributable-octet-pipeline.iam.gserviceaccount.com"
   gcloud artifacts repositories add-iam-policy-binding npm-packages \
     --location=us-central1 \
     --member="serviceAccount:${SA_EMAIL}" \
     --role="roles/artifactregistry.writer"
   ```

3. Check if repository exists:
   ```bash
   gcloud artifacts repositories describe npm-packages --location=us-central1
   ```

### cargo workflow fails with "crate already uploaded"

**Symptoms**:
```
error: crate version `10.0.0` is already uploaded
```

**Expected behavior**: This is normal! The workflow should skip already-published crates.

**Diagnosis**:
- Check if the workflow continues to next crates
- Look for "already published, skipping" messages

**Solutions**:
- This is not an error - the workflow will skip published crates
- To publish a new version, update the version in `Cargo.toml`
- The `--skip-published` flag in cargo-workspaces handles this automatically

### WASM build fails

**Symptoms**:
```
error: could not compile `alkanes-web-sys`
wasm-pack build failed
```

**Diagnosis**:
```bash
# Test locally
cd crates/alkanes-web-sys
wasm-pack build --target bundler
```

**Solutions**:

1. Check Rust version:
   ```bash
   rustc --version
   # Should be 1.83 or newer
   ```

2. Check wasm32 target is installed:
   ```bash
   rustup target list --installed | grep wasm32-unknown-unknown
   # If not, install:
   rustup target add wasm32-unknown-unknown
   ```

3. Check for compilation errors in the source code
4. Verify dependencies in `crates/alkanes-web-sys/Cargo.toml`

---

## npm Installation Issues

### 401 Unauthorized when installing

**Symptoms**:
```
npm ERR! code E401
npm ERR! 401 Unauthorized - GET https://us-central1-npm.pkg.dev/...
```

**Diagnosis**:
```bash
# Check if .npmrc is configured
cat .npmrc

# Check token
npm config get //us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/:_authToken
```

**Solutions**:

1. Configure .npmrc:
   ```bash
   cat > .npmrc <<EOF
   @alkanes:registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
   //us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/:always-auth=true
   EOF
   ```

2. Set authentication token:
   ```bash
   npm config set //us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/:_authToken $(gcloud auth print-access-token)
   ```

3. Verify gcloud authentication:
   ```bash
   gcloud auth list
   # Active account should be shown
   ```

4. Re-authenticate if needed:
   ```bash
   gcloud auth login
   ```

### Token expired

**Symptoms**:
```
npm ERR! 401 Unauthorized
# After working previously
```

**Cause**: GCP access tokens expire after 1 hour

**Solution**:
```bash
# Refresh token
npm config set //us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/:_authToken $(gcloud auth print-access-token)

# Or recreate .npmrc
cat > .npmrc <<EOF
@alkanes:registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
//us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/:_authToken=$(gcloud auth print-access-token)
EOF
```

### Package not found

**Symptoms**:
```
npm ERR! 404 Not Found - GET https://us-central1-npm.pkg.dev/.../alkanes/ts-sdk
npm ERR! 404 '@alkanes/ts-sdk@latest-dev' is not in this registry
```

**Diagnosis**:
1. Check if package is published:
   - Visit https://console.cloud.google.com/artifacts/npm/distributable-octet-pipeline/us-central1/npm-packages
   - Look for `@alkanes/ts-sdk`

2. Check workflow runs:
   - Visit https://github.com/kungfuflex/alkanes-rs/actions
   - Look for successful npm workflow runs

**Solutions**:

1. Verify registry URL:
   ```bash
   npm config get @alkanes:registry
   # Should be: https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
   ```

2. Try without version tag:
   ```bash
   npm install @alkanes/ts-sdk
   ```

3. List available versions:
   ```bash
   npm view @alkanes/ts-sdk versions --registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
   ```

---

## Cargo Installation Issues

### Authentication failed

**Symptoms**:
```
error: failed to get successful HTTP response from `https://us-central1-cargo.pkg.dev/...`
error: failed to authenticate when downloading repository
```

**Diagnosis**:
```bash
# Check credentials
cat ~/.cargo/credentials.toml

# Check config
cat ~/.cargo/config.toml
# Or for the workspace:
cat .cargo/config.toml
```

**Solutions**:

1. Set up credentials:
   ```bash
   mkdir -p ~/.cargo
   cat > ~/.cargo/credentials.toml <<EOF
   [registries.alkanes]
   token = "$(gcloud auth print-access-token)"
   EOF
   ```

2. Verify registry configuration:
   ```bash
   cat >> ~/.cargo/config.toml <<EOF
   [registries.alkanes]
   index = "sparse+https://us-central1-cargo.pkg.dev/distributable-octet-pipeline/cargo-packages/"

   [net]
   git-fetch-with-cli = true
   EOF
   ```

3. Test authentication:
   ```bash
   # This should succeed if auth is working
   cargo search --registry alkanes --limit 1
   ```

### Token expired (cargo)

**Symptoms**: Authentication fails after working previously

**Cause**: GCP access tokens expire after 1 hour

**Solution**:
```bash
# Quick refresh script
cat > refresh-cargo-token.sh <<'EOF'
#!/bin/bash
mkdir -p ~/.cargo
cat > ~/.cargo/credentials.toml <<CREDENTIALS
[registries.alkanes]
token = "$(gcloud auth print-access-token)"
CREDENTIALS
echo "Cargo credentials refreshed"
EOF

chmod +x refresh-cargo-token.sh
./refresh-cargo-token.sh
```

### Crate not found

**Symptoms**:
```
error: no matching package named `alkanes` found
```

**Diagnosis**:
```bash
# Check if crate is published
gcloud artifacts packages list --repository=cargo-packages --location=us-central1

# Or check web console
# https://console.cloud.google.com/artifacts/cargo/distributable-octet-pipeline/us-central1/cargo-packages
```

**Solutions**:

1. Verify you specified the registry:
   ```toml
   [dependencies]
   alkanes = { version = "10.0.0", registry = "alkanes" }
   ```

2. Check if crate publishing succeeded:
   - Check GitHub Actions: https://github.com/kungfuflex/alkanes-rs/actions
   - Look for cargo workflow runs

3. Try updating the index:
   ```bash
   cargo update
   ```

### Dependency resolution fails

**Symptoms**:
```
error: failed to select a version for `alkanes-support`
```

**Cause**: Dependent crate not published yet or version mismatch

**Solutions**:

1. Check all required crates are published:
   ```bash
   gcloud artifacts packages list --repository=cargo-packages --location=us-central1
   ```

2. Verify versions match in Cargo.toml:
   ```toml
   # All alkanes crates should use the same version
   alkanes = { version = "10.0.0", registry = "alkanes" }
   alkanes-support = { version = "10.0.0", registry = "alkanes" }
   ```

3. Use git dependency as fallback:
   ```toml
   # Use git instead of registry
   alkanes = { git = "https://github.com/kungfuflex/alkanes-rs", branch = "develop" }
   ```

---

## GCP Authentication Issues

### gcloud not authenticated

**Symptoms**:
```
ERROR: (gcloud.auth.print-access-token) You do not currently have an active account selected.
```

**Solution**:
```bash
gcloud auth login
gcloud config set project distributable-octet-pipeline
```

### Wrong GCP project

**Symptoms**: Resources not found, permission denied

**Solution**:
```bash
# Check current project
gcloud config get-value project

# Set correct project
gcloud config set project distributable-octet-pipeline

# Verify
gcloud config list
```

### Permission denied

**Symptoms**:
```
ERROR: (gcloud.artifacts...) PERMISSION_DENIED: Permission denied
```

**Solutions**:

1. Verify you're using the correct account:
   ```bash
   gcloud auth list
   # Should show deadlyowltrapper@gmail.com or authorized service account
   ```

2. Check IAM permissions:
   ```bash
   gcloud projects get-iam-policy distributable-octet-pipeline
   ```

3. Request access from project owner

---

## Cloudflare DNS Issues

### DNS records not created

**Symptoms**: Script runs but DNS not working

**Diagnosis**:
```bash
# Check DNS records
dig npm.pkg.alkanes.build CNAME +short
dig cargo.pkg.alkanes.build CNAME +short

# Should show:
# us-central1-npm.pkg.dev.
# us-central1-cargo.pkg.dev.
```

**Solutions**:

1. Verify CLOUDFLARE_API_TOKEN:
   ```bash
   echo $CLOUDFLARE_API_TOKEN
   # Should be set and not empty
   ```

2. Check token permissions:
   - Go to https://dash.cloudflare.com/profile/api-tokens
   - Token needs: Zone.DNS (Edit) permission for alkanes.build

3. Manually create DNS records:
   - Go to https://dash.cloudflare.com/
   - Select alkanes.build zone
   - DNS → Records → Add record
   - Type: CNAME, Name: `npm.pkg`, Content: `us-central1-npm.pkg.dev`, Proxy: Off
   - Repeat for `cargo.pkg`

### DNS propagation delay

**Symptoms**: Records created but not resolving

**Cause**: DNS propagation takes time

**Solution**:
```bash
# Wait 5-10 minutes, then check
dig npm.pkg.alkanes.build CNAME +short

# Force DNS refresh (macOS)
sudo dscacheutil -flushcache; sudo killall -HUP mDNSResponder

# Force DNS refresh (Linux)
sudo systemd-resolve --flush-caches
```

---

## Build Issues

### Out of memory during build

**Symptoms**:
```
error: linking with `rust-lld` failed: exit code: 1
signal: 9, SIGKILL: kill
```

**Solutions**:

1. Increase available memory (GitHub Actions):
   ```yaml
   # In workflow file
   env:
     CARGO_INCREMENTAL: 0
   ```

2. Reduce parallel builds:
   ```yaml
   - name: Build
     run: cargo build -j 1
   ```

3. Use release mode for smaller builds:
   ```yaml
   - name: Build
     run: cargo build --release
   ```

### Protobuf compiler not found

**Symptoms**:
```
error: failed to run custom build command for `...`
protoc-gen-prost: program not found
```

**Solution**:
```bash
# Ubuntu/Debian
sudo apt-get update && sudo apt-get install -y protobuf-compiler

# macOS
brew install protobuf

# Verify
protoc --version
```

### wasm-pack not found

**Symptoms**:
```
wasm-pack: command not found
```

**Solution**:
```bash
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
wasm-pack --version
```

---

## Rollback Procedures

### Rollback npm package

**Unpublish a version** (within 72 hours):
```bash
npm unpublish @alkanes/ts-sdk@VERSION \
  --registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
```

**Deprecate a version**:
```bash
npm deprecate @alkanes/ts-sdk@VERSION "This version has issues, use 0.1.0-dev.XXXXX instead" \
  --registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
```

**Update latest-dev tag** to previous version:
```bash
npm dist-tag add @alkanes/ts-sdk@PREVIOUS_VERSION latest-dev \
  --registry=https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
```

### Rollback cargo crate

**Yank a version**:
```bash
# Configure credentials first
cat > ~/.cargo/credentials.toml <<EOF
[registries.alkanes]
token = "$(gcloud auth print-access-token)"
EOF

# Yank the version
cargo yank --registry alkanes alkanes@10.0.0
```

**Unyank if needed**:
```bash
cargo yank --undo --registry alkanes alkanes@10.0.0
```

**Note**: Cargo doesn't support unpublishing. Yanked versions can't be used in new projects but existing users can still use them.

### Disable publishing workflows

**Temporarily disable**:
```bash
gh workflow disable publish-npm.yml
gh workflow disable publish-cargo.yml
```

**Re-enable**:
```bash
gh workflow enable publish-npm.yml
gh workflow enable publish-cargo.yml
```

---

## Getting Help

### Check workflow logs

```bash
# List recent workflow runs
gh run list --workflow=publish-npm.yml --limit 5
gh run list --workflow=publish-cargo.yml --limit 5

# View specific run
gh run view RUN_ID --log
```

### Check GCP Console

- Artifact Registry: https://console.cloud.google.com/artifacts?project=distributable-octet-pipeline
- IAM: https://console.cloud.google.com/iam-admin/iam?project=distributable-octet-pipeline
- Service Accounts: https://console.cloud.google.com/iam-admin/serviceaccounts?project=distributable-octet-pipeline

### Verify all components

**Quick health check script**:
```bash
#!/bin/bash
echo "=== Health Check ==="

echo "1. GCP Authentication:"
gcloud auth list

echo -e "\n2. GCP Project:"
gcloud config get-value project

echo -e "\n3. Artifact Repositories:"
gcloud artifacts repositories list --location=us-central1

echo -e "\n4. DNS Records:"
dig npm.pkg.alkanes.build CNAME +short
dig cargo.pkg.alkanes.build CNAME +short

echo -e "\n5. GitHub Secrets:"
gh secret list

echo -e "\n6. Workflow Status:"
gh workflow list

echo -e "\nHealth check complete!"
```

### Contact Information

- **GitHub Issues**: https://github.com/kungfuflex/alkanes-rs/issues
- **Repository**: https://github.com/kungfuflex/alkanes-rs
- **Documentation**: See PUBLISHING.md and README.md
