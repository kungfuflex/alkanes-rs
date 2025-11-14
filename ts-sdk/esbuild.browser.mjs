import * as esbuild from 'esbuild';

await esbuild.build({
  entryPoints: ['src/index.ts'],
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2020',
  outfile: 'dist/index.mjs',
  sourcemap: true,
  mainFields: ['browser', 'module', 'main'],
  resolveExtensions: ['.js', '.ts', '.json'],
  external: ['@alkanes/ts-sdk/wasm'],
  alias: {
    'stream': 'stream-browserify',
  },
  define: {
    'global': 'globalThis',
    'process.env.NODE_ENV': '"production"',
    'process.browser': 'true',
  },
  inject: ['./polyfills.js'],
  logLevel: 'info',
});

console.log('✅ Browser bundle built');
