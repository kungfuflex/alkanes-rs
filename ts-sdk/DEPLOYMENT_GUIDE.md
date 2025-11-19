# Deployment Guide: Using ts-sdk in External Projects

This guide explains how to use the vendored ts-sdk in external projects like subfrost-app.

## Overview

The ts-sdk now includes all WASM files in the `build/` directory, making it completely self-contained. When copying to external projects, the WASM files come with it and don't need to be rebuilt.

## Method 1: Copy the Built SDK (Recommended)

This is the simplest approach for projects that just need to use the SDK.

### Step 1: Build the SDK in alkanes-rs

```bash
cd /path/to/alkanes-rs/ts-sdk
npm install
npm run build
```

This creates:
- `build/wasm/` - alkanes-web-sys WASM module
- `build/contracts/` - All production contract WASMs
- `dist/` - Built TypeScript SDK

### Step 2: Copy to External Project

```bash
# Copy the entire ts-sdk directory
cp -r /path/to/alkanes-rs/ts-sdk /path/to/external-project/ts-sdk
```

### Step 3: Update package.json Scripts

In the copied `ts-sdk/package.json`, update the scripts to skip WASM building:

```json
{
  "scripts": {
    "build": "npm run build:ts",
    "build:wasm": "echo 'WASM already built and vendored in build/ directory'",
    "build:vendor": "echo 'WASM already vendored in build/ directory'",
    "build:ts": "tsup src/index.ts --format cjs,esm --clean",
    "clean": "rm -rf dist",
    "prepublishOnly": "npm run build"
  }
}
```

### Step 4: Add Type Declarations (for TypeScript projects)

Create `types/alkanes-ts-sdk.d.ts` in your project root:

```typescript
declare module '@alkanes/ts-sdk' {
  export const KeystoreManager: any;
  export const createKeystore: any;
  export const unlockKeystore: any;
  export const createWallet: any;
  export const createWalletFromMnemonic: any;
  export const createProvider: any;
  export const parseAlkaneId: any;
  export type NetworkType = any;
  export type Keystore = any;
  export type WalletConfig = any;
  export type AlkanesProvider = any;
  export type AddressType = any;
  export const VERSION: string;
  export const btcToSatoshis: any;
  export const satoshisToBtc: any;
  export const DERIVATION_PATHS: any;
}
```

### Step 5: Update TypeScript Configuration

Add ts-sdk to your tsconfig.json exclude list:

```json
{
  "exclude": [
    "node_modules",
    "ts-sdk"
  ]
}
```

### Step 6: Update pnpm-workspace.yaml (if using pnpm)

Add ts-sdk to the workspace:

```yaml
packages:
  - 'ts-sdk'
```

### Step 7: Update External Project's package.json

```json
{
  "dependencies": {
    "@alkanes/ts-sdk": "file:./ts-sdk"
  }
}
```

### Step 8: Install and Build

```bash
cd /path/to/external-project
pnpm install  # or npm install
pnpm build    # ts-sdk will build automatically via prepublishOnly
```

## Method 2: Git Submodule (For Development)

If you want to keep the ts-sdk in sync with alkanes-rs updates:

```bash
cd /path/to/external-project
git submodule add https://github.com/kungfuflex/alkanes-rs.git alkanes-rs
ln -s alkanes-rs/ts-sdk ts-sdk
```

Then follow steps 3-6 from Method 1.

## Troubleshooting

### Issue: "Cannot find module '@alkanes/ts-sdk'"

**Solution**: Make sure dependencies are installed and ts-sdk is built:
```bash
cd ts-sdk
npm install
npm run build
cd ..
npm install  # or pnpm install
```

### Issue: "No matching version found for @oyl/sdk"

**Solution**: Update ts-sdk/package.json peerDependencies:
```json
{
  "peerDependencies": {
    "@oyl/sdk": "*"
  }
}
```

### Issue: "can't cd to ../crates/alkanes-web-sys"

**Solution**: This means the build:wasm script is still trying to build from source. Update the scripts as shown in Step 3.

### Issue: "WASM files missing in build/ directory"

**Solution**: Copy the WASM files from a built version:
```bash
cp -r /path/to/alkanes-rs/ts-sdk/build /path/to/external-project/ts-sdk/
```

## Structure After Deployment

```
external-project/
├── ts-sdk/                  # Copied from alkanes-rs
│   ├── build/              # Vendored WASM files (included)
│   │   ├── wasm/          # alkanes-web-sys
│   │   └── contracts/     # Production contracts
│   ├── dist/              # Built TypeScript (generated)
│   ├── src/               # TypeScript source
│   ├── package.json       # Modified scripts
│   └── ...
├── package.json           # References file:./ts-sdk
├── pnpm-workspace.yaml    # Includes ts-sdk
└── ...
```

## Key Points

✅ **WASM files are vendored**: No need to rebuild Rust/WASM in external projects
✅ **Self-contained**: Everything needed is in the ts-sdk directory  
✅ **TypeScript only**: Only TypeScript compilation happens in external projects  
✅ **Version control**: Add ts-sdk to .gitignore or commit it as-is  
✅ **Updates**: Re-copy from alkanes-rs when updates are needed

## Usage Example

```typescript
import { createWallet, createProvider } from '@alkanes/ts-sdk';
import init, * as wasm from '@alkanes/ts-sdk/wasm';

// Initialize WASM
await init();

// Create wallet
const wallet = createWallet(/* ... */);

// Create provider with WASM
const provider = createProvider(config, wasm);
```

## Updating the SDK

When alkanes-rs updates the SDK:

```bash
# In alkanes-rs
cd ts-sdk
npm run build

# Copy to external project
cp -r /path/to/alkanes-rs/ts-sdk/build /path/to/external-project/ts-sdk/
cp -r /path/to/alkanes-rs/ts-sdk/src /path/to/external-project/ts-sdk/

# Rebuild
cd /path/to/external-project
pnpm install
pnpm build
```
