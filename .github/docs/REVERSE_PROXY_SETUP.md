# Reverse Proxy Setup for Simple Package Installation

This guide explains how to set up a reverse proxy at `pkg.alkanes.build` that allows installing packages without custom registry configuration.

## Goal

Enable simple installation:
```bash
# npm - just a URL
npm install https://pkg.alkanes.build/npm/@alkanes/ts-sdk/latest.tgz

# cargo - git dependencies (simpler than custom registries)
alkanes = { git = "https://github.com/kungfuflex/alkanes-rs", branch = "develop" }
```

## Architecture

We have two options:

### Option 1: Public Artifact Registry (Simplest)
- Make GCP Artifact Registry repositories public (read-only)
- No authentication needed for downloads
- Workflows still use Workload Identity for publishing

### Option 2: Cloud Run Reverse Proxy (More Control)
- Deploy a proxy service that handles authentication
- Public frontend at `pkg.alkanes.build`
- Backend authenticates to private Artifact Registry
- Allows custom logic (analytics, rate limiting, etc.)

I'll document both approaches below.

---

## Option 1: Public Artifact Registry (Recommended)

### Step 1: Make Repositories Public

```bash
# Make npm repository publicly readable
gcloud artifacts repositories add-iam-policy-binding npm-packages \
  --location=us-central1 \
  --member="allUsers" \
  --role="roles/artifactregistry.reader"

# Make cargo repository publicly readable
gcloud artifacts repositories add-iam-policy-binding cargo-packages \
  --location=us-central1 \
  --member="allUsers" \
  --role="roles/artifactregistry.reader"

# Verify
gcloud artifacts repositories get-iam-policy npm-packages --location=us-central1
```

### Step 2: Test Public Access

```bash
# Test npm access (no auth)
curl -I https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/@alkanes/ts-sdk

# Should return 200 OK
```

### Step 3: Update DNS (Already Done)

DNS already points to Artifact Registry:
- `npm.pkg.alkanes.build` → `us-central1-npm.pkg.dev`
- `cargo.pkg.alkanes.build` → `us-central1-cargo.pkg.dev`

### Step 4: Configure npm for Direct Tarball Installation

Users can now install directly via tarball URL:

```bash
# Install specific version
npm install https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/@alkanes/ts-sdk/-/ts-sdk-0.1.0-dev.20251215.tgz

# Or configure registry (but no auth needed!)
npm config set @alkanes:registry https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
npm install @alkanes/ts-sdk
```

### Step 5: Configure Cargo

For cargo, git dependencies are simpler:

```toml
[dependencies]
alkanes = { git = "https://github.com/kungfuflex/alkanes-rs", branch = "develop" }
# Or specific tag:
# alkanes = { git = "https://github.com/kungfuflex/alkanes-rs", tag = "v10.0.0" }
```

Alternatively, if you want to use the registry:
```toml
# In ~/.cargo/config.toml (now no credentials needed!)
[registries.alkanes]
index = "sparse+https://us-central1-cargo.pkg.dev/distributable-octet-pipeline/cargo-packages/"

# In Cargo.toml
[dependencies]
alkanes = { version = "10.0.0", registry = "alkanes" }
```

---

## Option 2: Cloud Run Reverse Proxy (Advanced)

If you want more control (analytics, custom URLs, rate limiting), deploy a reverse proxy.

### Architecture

```
User Request → pkg.alkanes.build
              ↓
         Cloud Run Proxy
              ↓
  Authenticates with Service Account
              ↓
    Google Artifact Registry
```

### Step 1: Create Proxy Service

Create `proxy/Dockerfile`:

```dockerfile
FROM nginx:alpine

# Install gcloud for authentication
RUN apk add --no-cache python3 py3-pip curl bash
RUN curl -sSL https://sdk.cloud.google.com | bash
ENV PATH=$PATH:/root/google-cloud-sdk/bin

# Copy nginx config
COPY nginx.conf /etc/nginx/nginx.conf
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 8080

ENTRYPOINT ["/entrypoint.sh"]
CMD ["nginx", "-g", "daemon off;"]
```

Create `proxy/nginx.conf`:

```nginx
events {
    worker_connections 1024;
}

http {
    # Upstream to Artifact Registry
    upstream npm_registry {
        server us-central1-npm.pkg.dev:443;
    }

    upstream cargo_registry {
        server us-central1-cargo.pkg.dev:443;
    }

    # npm proxy
    server {
        listen 8080;
        server_name npm.pkg.alkanes.build;

        location / {
            # Inject auth header
            proxy_set_header Authorization "Bearer ${GCP_TOKEN}";
            proxy_set_header Host "us-central1-npm.pkg.dev";

            proxy_pass https://npm_registry/distributable-octet-pipeline/npm-packages;
            proxy_ssl_server_name on;
            proxy_ssl_name us-central1-npm.pkg.dev;

            # Cache responses
            proxy_cache_valid 200 10m;
            proxy_cache_valid 404 1m;
        }
    }

    # cargo proxy
    server {
        listen 8080;
        server_name cargo.pkg.alkanes.build;

        location / {
            # Inject auth header
            proxy_set_header Authorization "Bearer ${GCP_TOKEN}";
            proxy_set_header Host "us-central1-cargo.pkg.dev";

            proxy_pass https://cargo_registry/distributable-octet-pipeline/cargo-packages;
            proxy_ssl_server_name on;
            proxy_ssl_name us-central1-cargo.pkg.dev;
        }
    }
}
```

Create `proxy/entrypoint.sh`:

```bash
#!/bin/bash
set -e

# Get GCP access token
export GCP_TOKEN=$(gcloud auth print-access-token)

# Update nginx config with token
envsubst '${GCP_TOKEN}' < /etc/nginx/nginx.conf > /tmp/nginx.conf
mv /tmp/nginx.conf /etc/nginx/nginx.conf

# Start token refresh in background
(
  while true; do
    sleep 3000  # Refresh every 50 minutes (tokens last 1 hour)
    export GCP_TOKEN=$(gcloud auth print-access-token)
    envsubst '${GCP_TOKEN}' < /etc/nginx/nginx.conf > /tmp/nginx.conf
    mv /tmp/nginx.conf /etc/nginx/nginx.conf
    nginx -s reload
  done
) &

# Start nginx
exec "$@"
```

### Step 2: Deploy to Cloud Run

```bash
cd proxy

# Build container
gcloud builds submit --tag gcr.io/distributable-octet-pipeline/pkg-proxy

# Deploy to Cloud Run
gcloud run deploy pkg-proxy \
  --image gcr.io/distributable-octet-pipeline/pkg-proxy \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated \
  --service-account github-actions-publisher@distributable-octet-pipeline.iam.gserviceaccount.com \
  --port 8080

# Get service URL
gcloud run services describe pkg-proxy --region us-central1 --format 'value(status.url)'
```

### Step 3: Update DNS to Point to Cloud Run

```bash
# Get Cloud Run service URL
CLOUD_RUN_URL=$(gcloud run services describe pkg-proxy --region us-central1 --format 'value(status.url)' | sed 's|https://||')

# Update Cloudflare DNS to CNAME to Cloud Run
# npm.pkg.alkanes.build → CNAME → $CLOUD_RUN_URL
# cargo.pkg.alkanes.build → CNAME → $CLOUD_RUN_URL
```

Or update `.github/scripts/setup-cloudflare-dns.sh` to point to Cloud Run instead of Artifact Registry directly.

---

## Recommendation

**Use Option 1 (Public Artifact Registry)** because:
1. Simpler - no proxy to maintain
2. Lower cost - no Cloud Run charges
3. Better performance - direct from GCP
4. More reliable - fewer moving parts

**Use Option 2 (Cloud Run Proxy)** if you need:
1. Analytics on package downloads
2. Rate limiting
3. Custom URLs
4. Access control beyond public/private

---

## Simple Installation (After Setup)

### npm Installation (Public Registry)

**Option A: Direct tarball URL**
```bash
npm install https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/@alkanes/ts-sdk/-/ts-sdk-VERSION.tgz
```

**Option B: Registry (no auth needed)**
```bash
npm config set @alkanes:registry https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
npm install @alkanes/ts-sdk
```

**Option C: package.json**
```json
{
  "dependencies": {
    "@alkanes/ts-sdk": "https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/@alkanes/ts-sdk/-/ts-sdk-0.1.0-dev.20251215.tgz"
  }
}
```

### Cargo Installation

**Option A: Git dependencies (recommended)**
```toml
[dependencies]
alkanes = { git = "https://github.com/kungfuflex/alkanes-rs", branch = "develop" }
```

**Option B: Public registry**
```toml
# ~/.cargo/config.toml (no credentials needed!)
[registries.alkanes]
index = "sparse+https://us-central1-cargo.pkg.dev/distributable-octet-pipeline/cargo-packages/"

# Cargo.toml
[dependencies]
alkanes = { version = "10.0.0", registry = "alkanes" }
```

---

## Security Considerations

### Public Registry
- ✅ Anyone can download packages (intended)
- ✅ Only authorized workflows can publish (protected by Workload Identity)
- ✅ No credentials leaked
- ⚠️  No download analytics
- ⚠️  No rate limiting

### Cloud Run Proxy
- ✅ Full control over access
- ✅ Can add analytics
- ✅ Can add rate limiting
- ⚠️  More complex to maintain
- ⚠️  Additional cost (~$5-10/month)

---

## Implementation Steps

### Quick Start (Public Registry)

1. Make repositories public:
   ```bash
   gcloud artifacts repositories add-iam-policy-binding npm-packages \
     --location=us-central1 \
     --member="allUsers" \
     --role="roles/artifactregistry.reader"

   gcloud artifacts repositories add-iam-policy-binding cargo-packages \
     --location=us-central1 \
     --member="allUsers" \
     --role="roles/artifactregistry.reader"
   ```

2. Test access (no auth):
   ```bash
   curl -I https://us-central1-npm.pkg.dev/distributable-octet-pipeline/npm-packages/
   ```

3. Update documentation with simplified install instructions

Done! Users can now install without any configuration.

---

## Rollback

If you need to make repositories private again:

```bash
gcloud artifacts repositories remove-iam-policy-binding npm-packages \
  --location=us-central1 \
  --member="allUsers" \
  --role="roles/artifactregistry.reader"

gcloud artifacts repositories remove-iam-policy-binding cargo-packages \
  --location=us-central1 \
  --member="allUsers" \
  --role="roles/artifactregistry.reader"
```
