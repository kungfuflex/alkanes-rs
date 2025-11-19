# Quick Deploy Checklist

Quick reference for deploying ts-sdk to external projects.

## In alkanes-rs (Source)

```bash
cd /path/to/alkanes-rs/ts-sdk
npm install
npm run build
```

## Copy to External Project

```bash
cp -r /path/to/alkanes-rs/ts-sdk /path/to/external-project/
```

## Update ts-sdk/package.json

Replace the scripts section:

```json
"scripts": {
  "build": "npm run build:ts",
  "build:wasm": "echo 'WASM already built and vendored in build/ directory'",
  "build:vendor": "echo 'WASM already vendored in build/ directory'",
  "build:ts": "tsup src/index.ts --format cjs,esm --clean",
  "clean": "rm -rf dist",
  "prepublishOnly": "npm run build"
}
```

**Note**: peerDependencies already set to `"@oyl/sdk": "*"` in source.

## Add Type Declarations

Create `types/alkanes-ts-sdk.d.ts`:

```typescript
declare module '@alkanes/ts-sdk' {
  export const KeystoreManager: any;
  export const createKeystore: any;
  export const unlockKeystore: any;
  // ... (see full list in DEPLOYMENT_GUIDE.md)
}
```

## Update tsconfig.json

```json
{
  "exclude": ["node_modules", "ts-sdk"]
}
```

## Update pnpm-workspace.yaml (pnpm only)

```yaml
packages:
  - 'ts-sdk'
```

## Update External Project's package.json

```json
"dependencies": {
  "@alkanes/ts-sdk": "file:./ts-sdk"
}
```

## Build

```bash
cd /path/to/external-project
pnpm install
# ts-sdk builds automatically via prepublishOnly hook
```

## Done!

Your external project can now import:

```typescript
import { createWallet } from '@alkanes/ts-sdk';
import init, * as wasm from '@alkanes/ts-sdk/wasm';
```
