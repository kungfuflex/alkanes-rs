# Google Cloud Platform Setup Guide

This guide walks through setting up the GCP infrastructure for the alkanes-rs distributable pipeline.

## Prerequisites

- Access to deadlyowltrapper@gmail.com Google account
- gcloud CLI installed: https://cloud.google.com/sdk/docs/install
- Billing account linked to the Google account

## Step 1: Authenticate with GCP

```bash
gcloud auth login --email=deadlyowltrapper@gmail.com
```

This will open a browser window for authentication.

## Step 2: Create the GCP Project

```bash
# Create project
gcloud projects create pkg-alkanes-build \
  --name="Alkanes Distributable Pipeline" \
  --set-as-default

# Set as default project
gcloud config set project pkg-alkanes-build

# Verify project creation
gcloud projects describe pkg-alkanes-build
```

## Step 3: Link Billing Account

```bash
# List available billing accounts
gcloud billing accounts list

# Copy the ACCOUNT_ID from the output above, then link it:
gcloud billing projects link pkg-alkanes-build \
  --billing-account=XXXXXX-XXXXXX-XXXXXX

# Verify billing is linked
gcloud billing projects describe pkg-alkanes-build
```

## Step 4: Enable Required APIs

```bash
# Enable all necessary GCP services
gcloud services enable artifactregistry.googleapis.com
gcloud services enable iamcredentials.googleapis.com
gcloud services enable iam.googleapis.com
gcloud services enable cloudresourcemanager.googleapis.com

# Verify APIs are enabled
gcloud services list --enabled
```

## Step 5: Create Artifact Registry Repositories

```bash
# Create npm repository
gcloud artifacts repositories create npm-packages \
  --repository-format=npm \
  --location=us-central1 \
  --description="Alkanes TypeScript SDK npm packages"

# Create cargo repository
gcloud artifacts repositories create cargo-packages \
  --repository-format=cargo \
  --location=us-central1 \
  --description="Alkanes Rust crates"

# Verify repositories were created
gcloud artifacts repositories list --location=us-central1
```

**Expected output:**
```
npm-packages    NPM     us-central1    Alkanes TypeScript SDK npm packages
cargo-packages  CARGO   us-central1    Alkanes Rust crates
```

**Repository URLs:**
- npm: https://us-central1-npm.pkg.dev/pkg-alkanes-build/npm-packages/
- cargo: sparse+https://us-central1-cargo.pkg.dev/pkg-alkanes-build/cargo-packages/

## Step 6: Create Workload Identity Pool

```bash
# Create identity pool for GitHub Actions
gcloud iam workload-identity-pools create "github-actions-pool" \
  --project="pkg-alkanes-build" \
  --location="global" \
  --display-name="GitHub Actions Pool"

# Verify pool creation
gcloud iam workload-identity-pools describe "github-actions-pool" \
  --project="pkg-alkanes-build" \
  --location="global"
```

## Step 7: Create OIDC Provider for GitHub

```bash
# Create OIDC provider
gcloud iam workload-identity-pools providers create-oidc "github-provider" \
  --project="pkg-alkanes-build" \
  --location="global" \
  --workload-identity-pool="github-actions-pool" \
  --display-name="GitHub Provider" \
  --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository,attribute.repository_owner=assertion.repository_owner" \
  --issuer-uri="https://token.actions.githubusercontent.com"

# Verify provider creation
gcloud iam workload-identity-pools providers describe "github-provider" \
  --project="pkg-alkanes-build" \
  --location="global" \
  --workload-identity-pool="github-actions-pool"
```

## Step 8: Create Service Account

```bash
# Create service account for GitHub Actions
gcloud iam service-accounts create github-actions-publisher \
  --project="pkg-alkanes-build" \
  --display-name="GitHub Actions Publisher" \
  --description="Service account for publishing packages from GitHub Actions"

# Set service account email variable for later use
SA_EMAIL="github-actions-publisher@pkg-alkanes-build.iam.gserviceaccount.com"

# Verify service account creation
gcloud iam service-accounts describe ${SA_EMAIL}
```

## Step 9: Grant Permissions to Service Account

```bash
# Grant Artifact Registry Writer role for npm repository
gcloud artifacts repositories add-iam-policy-binding npm-packages \
  --project="pkg-alkanes-build" \
  --location=us-central1 \
  --member="serviceAccount:${SA_EMAIL}" \
  --role="roles/artifactregistry.writer"

# Grant Artifact Registry Writer role for cargo repository
gcloud artifacts repositories add-iam-policy-binding cargo-packages \
  --project="pkg-alkanes-build" \
  --location=us-central1 \
  --member="serviceAccount:${SA_EMAIL}" \
  --role="roles/artifactregistry.writer"

# Verify permissions
gcloud artifacts repositories get-iam-policy npm-packages --location=us-central1
gcloud artifacts repositories get-iam-policy cargo-packages --location=us-central1
```

## Step 10: Allow GitHub Actions to Impersonate Service Account

```bash
# Set variables
GITHUB_REPO="kungfuflex/alkanes-rs"
PROJECT_NUMBER=$(gcloud projects describe pkg-alkanes-build --format='value(projectNumber)')

echo "Project Number: ${PROJECT_NUMBER}"

# Allow workload identity user binding
gcloud iam service-accounts add-iam-policy-binding "${SA_EMAIL}" \
  --project="pkg-alkanes-build" \
  --role="roles/iam.workloadIdentityUser" \
  --member="principalSet://iam.googleapis.com/projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/github-actions-pool/attribute.repository/${GITHUB_REPO}"

# Verify binding
gcloud iam service-accounts get-iam-policy "${SA_EMAIL}"
```

## Step 11: Get Workload Identity Provider Name

This value will be used in GitHub Secrets.

```bash
# Get the full provider name
PROVIDER_NAME=$(gcloud iam workload-identity-pools providers describe "github-provider" \
  --project="pkg-alkanes-build" \
  --location="global" \
  --workload-identity-pool="github-actions-pool" \
  --format="value(name)")

echo "GCP_WORKLOAD_IDENTITY_PROVIDER: ${PROVIDER_NAME}"
```

**Save this output** - you'll need it for GitHub Secrets configuration.

## Step 12: Prepare GitHub Secrets Values

Run these commands to get all the values you'll need for GitHub Secrets:

```bash
echo "=== GitHub Secrets Configuration ==="
echo ""
echo "GCP_WORKLOAD_IDENTITY_PROVIDER:"
gcloud iam workload-identity-pools providers describe "github-provider" \
  --location="global" \
  --workload-identity-pool="github-actions-pool" \
  --format="value(name)"
echo ""
echo "GCP_SERVICE_ACCOUNT:"
echo "github-actions-publisher@pkg-alkanes-build.iam.gserviceaccount.com"
echo ""
echo "GCP_PROJECT_ID:"
echo "pkg-alkanes-build"
echo ""
```

Copy these values - you'll add them to GitHub in the next section.

## Step 13: Configure GitHub Secrets

### Option A: Using GitHub Web UI

1. Go to https://github.com/kungfuflex/alkanes-rs/settings/secrets/actions
2. Click "New repository secret"
3. Add each secret:

- **GCP_WORKLOAD_IDENTITY_PROVIDER**: `projects/PROJECT_NUMBER/locations/global/workloadIdentityPools/github-actions-pool/providers/github-provider`
- **GCP_SERVICE_ACCOUNT**: `github-actions-publisher@pkg-alkanes-build.iam.gserviceaccount.com`
- **GCP_PROJECT_ID**: `pkg-alkanes-build`
- **CLOUDFLARE_API_TOKEN**: Your Cloudflare API token (create at https://dash.cloudflare.com/profile/api-tokens)

### Option B: Using GitHub CLI

```bash
# Install gh CLI if needed: https://cli.github.com/

# Get the provider name
PROVIDER_NAME=$(gcloud iam workload-identity-pools providers describe "github-provider" \
  --location="global" \
  --workload-identity-pool="github-actions-pool" \
  --format="value(name)")

# Set secrets
gh secret set GCP_WORKLOAD_IDENTITY_PROVIDER --body "${PROVIDER_NAME}" --repo kungfuflex/alkanes-rs
gh secret set GCP_SERVICE_ACCOUNT --body "github-actions-publisher@pkg-alkanes-build.iam.gserviceaccount.com" --repo kungfuflex/alkanes-rs
gh secret set GCP_PROJECT_ID --body "pkg-alkanes-build" --repo kungfuflex/alkanes-rs

# You'll need to set CLOUDFLARE_API_TOKEN manually with your token:
gh secret set CLOUDFLARE_API_TOKEN --body "YOUR_CLOUDFLARE_TOKEN" --repo kungfuflex/alkanes-rs

# Verify secrets were set
gh secret list --repo kungfuflex/alkanes-rs
```

## Step 14: Test Authentication (Optional)

You can test the service account authentication locally:

```bash
# Test npm registry access
gcloud artifacts print-settings npm \
  --repository=npm-packages \
  --location=us-central1

# Test cargo registry access
gcloud artifacts print-settings cargo \
  --repository=cargo-packages \
  --location=us-central1
```

## Verification Checklist

Before proceeding to workflow setup, verify:

- [ ] GCP project `pkg-alkanes-build` exists
- [ ] Billing is enabled on the project
- [ ] Both Artifact Registry repositories are created (npm-packages, cargo-packages)
- [ ] Workload Identity Pool `github-actions-pool` exists
- [ ] OIDC Provider `github-provider` is configured
- [ ] Service account `github-actions-publisher@...` exists
- [ ] Service account has `artifactregistry.writer` role on both repositories
- [ ] Workload Identity binding allows GitHub repo to impersonate service account
- [ ] All 4 GitHub secrets are configured

## Troubleshooting

### Error: Project ID already exists

If the project ID is taken, choose a different name:
```bash
gcloud projects create pkg-alkanes-build-v2
```
Update all subsequent commands to use the new project ID.

### Error: Billing not enabled

Ensure billing is linked:
```bash
gcloud billing projects describe pkg-alkanes-build
```

If not linked, run:
```bash
gcloud billing projects link pkg-alkanes-build --billing-account=YOUR_BILLING_ID
```

### Error: Permission denied

Ensure you're authenticated as the correct user:
```bash
gcloud auth list
# Should show deadlyowltrapper@gmail.com as active
```

### Verify workload identity configuration

```bash
# Check pool
gcloud iam workload-identity-pools describe github-actions-pool \
  --location=global

# Check provider
gcloud iam workload-identity-pools providers describe github-provider \
  --location=global \
  --workload-identity-pool=github-actions-pool

# Check service account permissions
gcloud iam service-accounts get-iam-policy github-actions-publisher@pkg-alkanes-build.iam.gserviceaccount.com
```

## Next Steps

After completing this setup:

1. Verify GitHub Secrets are configured
2. Set up Cloudflare DNS (see `.github/scripts/setup-cloudflare-dns.sh`)
3. The GitHub workflows will automatically use these credentials
4. Test the workflows by pushing to the develop branch

## Cost Management

Monitor costs at: https://console.cloud.google.com/billing

Artifact Registry pricing:
- Storage: ~$0.10/GB/month
- Network egress: ~$0.12/GB
- Expected monthly cost: $5-20

Set up budget alerts:
```bash
# Create budget (example: $50/month)
gcloud billing budgets create \
  --billing-account=YOUR_BILLING_ACCOUNT_ID \
  --display-name="Alkanes Pipeline Budget" \
  --budget-amount=50 \
  --threshold-rule=percent=50 \
  --threshold-rule=percent=90 \
  --threshold-rule=percent=100
```

## Cleanup (if needed)

To remove all resources:

```bash
# Delete repositories
gcloud artifacts repositories delete npm-packages --location=us-central1
gcloud artifacts repositories delete cargo-packages --location=us-central1

# Delete service account
gcloud iam service-accounts delete github-actions-publisher@pkg-alkanes-build.iam.gserviceaccount.com

# Delete workload identity pool (deletes provider too)
gcloud iam workload-identity-pools delete github-actions-pool --location=global

# Delete project (WARNING: This deletes everything)
gcloud projects delete pkg-alkanes-build
```
