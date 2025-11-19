# Copying ts-sdk to External Projects

## Summary

The alkanes-rs ts-sdk is now configured to be **copy-friendly**. You can copy the entire `ts-sdk/` directory to external projects and it will work with minimal configuration changes.

## What's Already Configured in Source

✅ **@ts-ignore comments** added to WASM imports  
✅ **peerDependencies** set to `"@oyl/sdk": "*"` for compatibility  
✅ **WASM files vendored** in `build/` directory  
✅ **Build scripts** ready for both source and deployed contexts

## Quick Copy Process

### 1. Build in Source (alkanes-rs)

```bash
cd /path/to/alkanes-rs/ts-sdk
npm install
npm run build
```

This creates:
- `build/wasm/` - alkanes-web-sys WASM module  
- `build/contracts/` - All 32 production contract WASMs
- `dist/` - Built TypeScript SDK

### 2. Copy to External Project

```bash
cp -r /path/to/alkanes-rs/ts-sdk /path/to/external-project/
```

### 3. Modify Copied ts-sdk

Edit `/path/to/external-project/ts-sdk/package.json`:

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

### 4. Configure External Project

**a) Add Type Declarations**

Create `types/alkanes-ts-sdk.d.ts`:

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

**b) Update tsconfig.json**

```json
{
  "exclude": [
    "node_modules",
    "ts-sdk"
  ]
}
```

**c) Update pnpm-workspace.yaml** (pnpm only)

```yaml
packages:
  - 'ts-sdk'
```

**d) Update package.json**

```json
{
  "dependencies": {
    "@alkanes/ts-sdk": "file:./ts-sdk"
  }
}
```

### 5. Install and Build

```bash
cd /path/to/external-project
pnpm install  # or npm install
# ts-sdk builds automatically via prepublishOnly hook
pnpm build
```

## What Gets Copied

```
ts-sdk/
├── build/              ✅ Vendored WASM files (INCLUDED)
│   ├── wasm/          # alkanes-web-sys
│   └── contracts/     # Production contracts
├── src/               ✅ TypeScript source (INCLUDED)
├── package.json       ✅ Config (MODIFY after copy)
├── tsconfig.json      ✅ TypeScript config
├── tsup.config.ts     ✅ Build config
├── scripts/           ✅ Build scripts
├── README.md          ✅ Documentation
├── DEPLOYMENT_GUIDE.md ✅ Full deployment guide
└── QUICK_DEPLOY.md    ✅ Quick reference
```

## Key Differences: Source vs Deployed

| Aspect | In alkanes-rs (source) | In external project (deployed) |
|--------|----------------------|-------------------------------|
| build:wasm | Builds from Rust crate | Echoes "already built" |
| build:vendor | Copies from prod_wasms | Echoes "already vendored" |
| build | Builds WASM + vendors + TS | Only builds TS |
| clean | Removes dist + build | Only removes dist |
| prepublishOnly | clean + build | Just build |

## Troubleshooting

### "Cannot find module '@alkanes/ts-sdk'"

**Solution**: Make sure dependencies are installed and ts-sdk is built:

```bash
cd ts-sdk && npm install && npm run build
cd .. && pnpm install
```

### "Property 'subtle' does not exist" or similar TS errors

**Solution**: Make sure ts-sdk is excluded in tsconfig.json:

```json
{
  "exclude": ["node_modules", "ts-sdk"]
}
```

### WASM files missing

**Solution**: Copy the built WASM files from alkanes-rs:

```bash
cp -r /path/to/alkanes-rs/ts-sdk/build /path/to/external-project/ts-sdk/
```

## Updating from alkanes-rs

When alkanes-rs updates the SDK:

```bash
# In alkanes-rs
cd ts-sdk && npm run build

# Copy updated files to external project
cp -r /path/to/alkanes-rs/ts-sdk/build /path/to/external-project/ts-sdk/
cp -r /path/to/alkanes-rs/ts-sdk/src /path/to/external-project/ts-sdk/
cp /path/to/alkanes-rs/ts-sdk/package.json /path/to/external-project/ts-sdk/
# Then reapply the script changes from Step 3

# Rebuild
cd /path/to/external-project
pnpm install
pnpm build
```

## Success Indicators

✅ `pnpm install` completes without errors  
✅ ts-sdk builds successfully (dist/ created)  
✅ External project builds successfully  
✅ Module resolution works (no "Cannot find module" errors)  
✅ WASM exports accessible (`@alkanes/ts-sdk/wasm`)

## Documentation

- **DEPLOYMENT_GUIDE.md** - Comprehensive deployment instructions
- **QUICK_DEPLOY.md** - Quick reference checklist  
- **VENDORING.md** - Details about WASM vendoring approach
- **README.md** - SDK usage and API documentation
