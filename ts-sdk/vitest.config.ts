import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    testTimeout: 60000, // 60s for RPC calls
    hookTimeout: 30000,
    include: ['src/**/*.test.ts', 'tests/**/*.test.ts'],
    exclude: ['node_modules', 'dist', 'build'],
    // Pool settings for WASM
    pool: 'forks',
    poolOptions: {
      forks: {
        singleFork: true, // Run all tests in the same fork to share WASM state
      },
    },
    // Reporter for CI
    reporters: ['verbose'],
  },
});
