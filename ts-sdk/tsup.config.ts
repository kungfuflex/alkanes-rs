import { defineConfig } from 'tsup';

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['esm'],
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
  ],
  esbuildOptions(options) {
    options.logLevel = 'warning';
    options.platform = 'browser';
  },
});
