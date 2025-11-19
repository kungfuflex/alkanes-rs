# WASM Vendoring Documentation

## Overview

The ts-sdk has been reorganized to vendor all WASM builds in `./ts-sdk/build/*`, making it self-contained and portable. This allows the entire ts-sdk directory to be copied directly into other projects (like subfrost-app) without managing external directory dependencies.

## Directory Structure

```
ts-sdk/
├── build/                    # Vendored WASM files (gitignored, but included in npm package)
│   ├── wasm/                # alkanes-web-sys WASM module
│   │   ├── alkanes_web_sys.js
│   │   ├── alkanes_web_sys.d.ts
│   │   ├── alkanes_web_sys_bg.wasm
│   │   └── ...
│   ├── contracts/           # Production contract WASM files
│   │   ├── factory.wasm
│   │   ├── pool.wasm
│   │   ├── frost_token.wasm
│   │   └── ... (32 WASM files)
│   └── README.md
├── dist/                    # Built TypeScript (gitignored)
├── src/                     # TypeScript source
├── scripts/
│   └── vendor-wasms.js     # Script to copy prod_wasms
└── package.json
```

## Changes Made

### 1. Package Configuration (`package.json`)

- **Updated exports**: Changed WASM exports from `./wasm-pkg/*` to `./build/wasm/*`
- **Updated files**: Changed from `wasm-pkg` to `build` in the files array
- **New build steps**:
  - `build:wasm`: Builds alkanes-web-sys to `build/wasm/`
  - `build:vendor`: Copies prod_wasms to `build/contracts/`
  - `build:ts`: Builds TypeScript SDK
  - `clean`: Removes dist and build directories
  - `prepublishOnly`: Ensures clean build before publishing

### 2. Source Code Updates

- **`src/provider/index.ts`**: Updated import from `../../wasm-pkg/alkanes_web_sys` to `../../build/wasm/alkanes_web_sys`
- **`src/keystore/index.ts`**: Updated import from `../../wasm-pkg/alkanes_web_sys` to `../../build/wasm/alkanes_web_sys`

### 3. Build Scripts

- **`scripts/vendor-wasms.js`**: New script that copies all WASM files from `../prod_wasms/` to `build/contracts/`
- **`build-and-link.sh`**: Updated to run the full build pipeline including vendoring

### 4. Configuration Files

- **`.gitignore`**: Added `build/` and `wasm-pkg/` to ignore generated files
- **`.npmignore`**: Added `scripts/` and development files, but build/ is NOT ignored (it needs to be in the npm package)

## Build Process

### For Development

```bash
# Full build (WASM + vendor + TypeScript)
npm run build

# Or step by step:
npm run build:wasm      # Build alkanes-web-sys WASM
npm run build:vendor    # Copy prod WASM files
npm run build:ts        # Build TypeScript SDK
```

### For Publishing

```bash
npm publish
# This automatically runs: npm run clean && npm run build
```

## Usage

### In External Projects

You can now copy the entire ts-sdk directory into another project:

```bash
# Copy ts-sdk into subfrost-app
cp -r /path/to/alkanes-rs/ts-sdk /path/to/subfrost-app/ts-sdk

# Build it in place
cd /path/to/subfrost-app/ts-sdk
npm install
npm run build

# Use it in your project
npm link
cd /path/to/subfrost-app
npm link @alkanes/ts-sdk
```

### Importing WASM

```typescript
// Import the main SDK
import { createProvider, createWallet } from '@alkanes/ts-sdk';

// Import WASM module
import init, * as wasm from '@alkanes/ts-sdk/wasm';

// Initialize WASM
await init();

// Use with provider
const provider = createProvider(config, wasm);
```

### Contract WASM Files

All production contract WASM files are available at `build/contracts/*.wasm` and can be loaded as needed:

```typescript
import { readFileSync } from 'fs';
import { join } from 'path';

// In Node.js
const factoryWasm = readFileSync(
  join(__dirname, 'node_modules/@alkanes/ts-sdk/build/contracts/factory.wasm')
);

// In browser/bundler
const factoryWasm = await fetch('/path/to/build/contracts/factory.wasm')
  .then(r => r.arrayBuffer());
```

## Benefits

1. **Self-contained**: No external directory dependencies
2. **Portable**: Can be copied anywhere and built independently
3. **Clean npm package**: All necessary files included, no external references
4. **Version control**: Only source files in git, generated files excluded
5. **Build reproducibility**: Same build process everywhere

## Notes

- The `build/` directory is gitignored but included in npm packages
- The `wasm-pkg/` directory (old location) is now obsolete and ignored
- All WASM files are rebuilt/recopied on each build for consistency
- The vendor script automatically creates directories if they don't exist
