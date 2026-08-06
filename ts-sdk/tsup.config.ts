import { defineConfig } from 'tsup';

export default defineConfig({
  // `src/walletconnect/index.ts` is a thin re-export of
  // `@subfrost/walletconnect` — it needs its own build entry so the
  // `./walletconnect` subpath export declared in package.json resolves
  // to a real `dist/walletconnect/index.{js,mjs}` file. tsup preserves
  // the source-relative directory structure for array entries, so
  // `src/walletconnect/index.ts` lands at `dist/walletconnect/index.*`
  // automatically.
  entry: ['src/index.ts', 'src/walletconnect/index.ts'],
  format: ['cjs', 'esm'],
  dts: false, // Disabled due to WASM module resolution issues - types available via source
  clean: true,
  splitting: false,
  sourcemap: true,
  platform: 'browser',
  target: 'es2020',
  external: [
    'node:crypto',
    'crypto',
    // WASM module - loaded separately via package exports
    '@alkanes/ts-sdk/wasm',
    '../wasm/alkanes_web_sys',
    '../wasm/alkanes_web_sys.js',
  ],
  noExternal: [
    'bip39',
    'bip32',
    'bitcoinjs-lib',
    '@bitcoinerlab/secp256k1',
    'tiny-secp256k1',
    'ecpair',
    'stream-browserify',
    'buffer',
    'events',
    'inherits',
    'string_decoder',
    'util-deprecate',
  ],
  esbuildOptions(options) {
    options.logLevel = 'warning';
    options.platform = 'browser';
    // Polyfill Node.js modules for browser
    options.inject = options.inject || [];
    // Map Node's stream to stream-browserify
    options.alias = options.alias || {};
    options.alias['stream'] = 'stream-browserify';
  },
  esbuildPlugins: [
    {
      name: 'externalize-wasm',
      setup(build) {
        // Mark all .wasm files and wasm directory imports as external
        build.onResolve({ filter: /\.wasm$/ }, (args) => ({
          path: args.path,
          external: true,
        }));
        build.onResolve({ filter: /\/wasm\// }, (args) => ({
          path: args.path,
          external: true,
        }));
      },
    },
  ],
});
