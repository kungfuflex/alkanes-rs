import { defineConfig } from 'tsup';

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['cjs', 'esm'],
  dts: false,
  clean: true,
  splitting: false,
  sourcemap: true,
  platform: 'browser',
  target: 'es2020',
  external: [
    'node:crypto',
    'crypto',
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
});
