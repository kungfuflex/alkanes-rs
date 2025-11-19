# Migration Summary: Vendored WASM Builds

## What Changed?

The ts-sdk has been reorganized to vendor all WASM builds in `./ts-sdk/build/*`, making it completely self-contained and portable.

## Quick Start

### Building the SDK

```bash
cd ts-sdk
npm install
npm run build
```

This will:
1. Build the alkanes-web-sys WASM module to `build/wasm/`
2. Copy all production WASM files from `../prod_wasms/` to `build/contracts/`
3. Build the TypeScript SDK to `dist/`

### Using in External Projects

You can now copy the entire ts-sdk directory directly:

```bash
# Copy to another project
cp -r /path/to/alkanes-rs/ts-sdk /path/to/subfrost-app/ts-sdk

# Build and use
cd /path/to/subfrost-app/ts-sdk
npm install
npm run build
npm link

# In your app
cd /path/to/subfrost-app
npm link @alkanes/ts-sdk
```

## Files Changed

### Modified Files
- `package.json` - Updated build scripts and exports
- `src/provider/index.ts` - Updated WASM import path
- `src/keystore/index.ts` - Updated WASM import path
- `examples/oyl-integration.ts` - Updated WASM import path
- `.gitignore` - Added build/ directory
- `.npmignore` - Updated to include build/, exclude wasm-pkg/
- `build-and-link.sh` - Updated to use new build process
- `INTEGRATION_README.md` - Updated documentation
- `TS_SDK_STATUS.md` - Updated documentation

### New Files
- `scripts/vendor-wasms.js` - Script to copy production WASMs
- `build/` directory structure:
  - `build/wasm/` - alkanes-web-sys WASM module
  - `build/contracts/` - 32 production contract WASM files
  - `build/README.md` - Documentation
- `VENDORING.md` - Detailed vendoring documentation
- `MIGRATION_SUMMARY.md` - This file

## Benefits

✅ **Self-contained**: No external directory dependencies  
✅ **Portable**: Copy anywhere and build independently  
✅ **Clean npm package**: All necessary files included  
✅ **Version control**: Generated files excluded from git  
✅ **Build reproducibility**: Same build process everywhere

## Next Steps

1. Test the build process: `npm run build`
2. Test in subfrost-app or another project
3. Verify all imports work correctly
4. Publish to npm when ready: `npm publish`

## Notes

- The `build/` directory is gitignored but included in npm packages
- The old `wasm-pkg/` location is now obsolete
- All WASM files are rebuilt/copied on each build
- Run `npm run clean` to remove generated files
