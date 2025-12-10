# Build Directory

This directory contains vendored WASM files that make the ts-sdk self-contained and portable.

## Structure

```
build/
├── wasm/          - alkanes-web-sys WASM module (built from crates/alkanes-web-sys)
│   ├── alkanes_web_sys.js
│   ├── alkanes_web_sys.d.ts
│   ├── alkanes_web_sys_bg.wasm
│   └── ...
└── contracts/     - Production contract WASM files (copied from prod_wasms/)
    ├── factory.wasm
    ├── pool.wasm
    ├── frost_token.wasm
    └── ...
```

## Build Process

These files are automatically generated during the build process:

1. `npm run build:wasm` - Builds the alkanes-web-sys WASM module into `build/wasm/`
2. `npm run build:vendor` - Copies production WASM files from `../prod_wasms/` to `build/contracts/`
3. `npm run build:ts` - Builds the TypeScript SDK

## Usage

The ts-sdk can now be copied directly into other projects (like subfrost-app) without needing to manage external directory dependencies.

All WASM files are bundled with the package when published to npm.
