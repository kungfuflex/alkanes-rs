/**
 * @alkanes/ts-sdk Memory Edge Case Tests
 *
 * These tests check for edge cases that might cause "memory access out of bounds" errors.
 * Common causes:
 * 1. Invalid addresses
 * 2. Malformed RPC responses
 * 3. Concurrent operations exhausting WASM memory
 * 4. Very large responses
 *
 * Run with: LIVE_RPC_TEST=true pnpm vitest run tests/memory-edge-cases.test.ts
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { AlkanesProvider } from '../src/provider';

const LIVE_RPC_TEST = process.env.LIVE_RPC_TEST === 'true';
const REGTEST_RPC_URL = process.env.REGTEST_RPC_URL || 'https://regtest.subfrost.io/v4/jsonrpc';

// Various test addresses
const TEST_ADDRESSES = {
  // Valid regtest addresses
  validP2TR: 'bcrt1phye8p0njg5yyvjjteyfwhzkrh5g4fyc9q4fprhzqh06kur5a0z0s7heqsm',
  validP2WPKH: 'bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080',
  // Invalid addresses (might cause issues)
  empty: '',
  tooShort: 'bcrt1q',
  tooLong: 'bcrt1q' + 'a'.repeat(100),
  invalidChecksum: 'bcrt1phye8p0njg5yyvjjteyfwhzkrh5g4fyc9q4fprhzqh06kur5a0z0s7heqs0', // changed last char
  mainnetAddress: 'bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4', // mainnet on regtest
  // Known working mainnet
  mainnetWorking: 'bc1puvfmy5whzdq35nd2trckkm09em9u7ps6lal564jz92c9feswwrpsr7ach5',
};

describe.skipIf(!LIVE_RPC_TEST)('Memory Edge Case Tests', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'subfrost-regtest',
      rpcUrl: REGTEST_RPC_URL,
    });
    await provider.initialize();
  });

  describe('Address Edge Cases', () => {
    it('should handle empty address gracefully', async () => {
      try {
        await provider.alkanes.getBalance(TEST_ADDRESSES.empty);
        // If it succeeds, that's fine
      } catch (error) {
        // Should get a proper error, not a WASM crash
        expect(error).toBeInstanceOf(Error);
        const errorMessage = (error as Error).message.toLowerCase();
        expect(errorMessage).not.toContain('memory access out of bounds');
        console.log('Empty address error:', (error as Error).message);
      }
    });

    it('should handle too short address gracefully', async () => {
      try {
        await provider.alkanes.getBalance(TEST_ADDRESSES.tooShort);
      } catch (error) {
        expect(error).toBeInstanceOf(Error);
        const errorMessage = (error as Error).message.toLowerCase();
        expect(errorMessage).not.toContain('memory access out of bounds');
        console.log('Too short address error:', (error as Error).message);
      }
    });

    it('should handle valid P2WPKH address', async () => {
      try {
        const result = await provider.alkanes.getBalance(TEST_ADDRESSES.validP2WPKH);
        expect(Array.isArray(result)).toBe(true);
        console.log('P2WPKH result:', result);
      } catch (error) {
        console.log('P2WPKH error:', (error as Error).message);
        const errorMessage = (error as Error).message.toLowerCase();
        expect(errorMessage).not.toContain('memory access out of bounds');
      }
    });

    it('should handle valid P2TR address', async () => {
      const result = await provider.alkanes.getBalance(TEST_ADDRESSES.validP2TR);
      expect(Array.isArray(result)).toBe(true);
      console.log('P2TR result count:', result.length);
    });
  });

  describe('Concurrent Operations', () => {
    it('should handle multiple concurrent getBalance calls', async () => {
      const promises = [
        provider.alkanes.getBalance(TEST_ADDRESSES.validP2TR),
        provider.alkanes.getBalance(TEST_ADDRESSES.validP2TR),
        provider.alkanes.getBalance(TEST_ADDRESSES.validP2TR),
      ];

      const results = await Promise.all(promises);

      results.forEach((result, i) => {
        expect(Array.isArray(result)).toBe(true);
        console.log(`Concurrent call ${i + 1}:`, result.length, 'tokens');
      });
    });

    it('should handle mixed RPC calls concurrently', async () => {
      const promises = [
        provider.getBlockHeight(),
        provider.alkanes.getBalance(TEST_ADDRESSES.validP2TR),
        provider.esplora.getAddressUtxos(TEST_ADDRESSES.validP2TR),
      ];

      const [height, balances, utxos] = await Promise.all(promises);

      expect(height).toBeGreaterThan(0);
      expect(Array.isArray(balances)).toBe(true);
      expect(Array.isArray(utxos)).toBe(true);

      console.log('Mixed concurrent results:', { height, balances: balances.length, utxos: utxos.length });
    });
  });

  describe('Response Size Handling', () => {
    it('should handle addresses with many tokens', async () => {
      // The test address has multiple tokens
      const result = await provider.alkanes.getBalance(TEST_ADDRESSES.validP2TR);
      console.log('Token count:', result.length);

      // Verify structure of each balance
      result.forEach((balance: any, i: number) => {
        expect(balance).toHaveProperty('alkane_id');
        expect(balance.alkane_id).toHaveProperty('block');
        expect(balance.alkane_id).toHaveProperty('tx');
        expect(balance).toHaveProperty('balance');
      });
    });

    it('should handle addresses with many UTXOs', async () => {
      const utxos = await provider.esplora.getAddressUtxos(TEST_ADDRESSES.validP2TR);
      console.log('UTXO count:', utxos.length);

      // Verify structure of each UTXO
      utxos.forEach((utxo: any) => {
        expect(utxo).toHaveProperty('txid');
        expect(utxo).toHaveProperty('vout');
        expect(utxo).toHaveProperty('value');
        expect(utxo).toHaveProperty('status');
      });
    });
  });

  describe('Sequential Operations', () => {
    it('should handle sequential balance queries', async () => {
      for (let i = 0; i < 5; i++) {
        const result = await provider.alkanes.getBalance(TEST_ADDRESSES.validP2TR);
        expect(Array.isArray(result)).toBe(true);
        console.log(`Sequential call ${i + 1}: ${result.length} tokens`);
      }
    });

    it('should handle sequential height queries', async () => {
      const heights: number[] = [];
      for (let i = 0; i < 10; i++) {
        const height = await provider.getBlockHeight();
        heights.push(height);
      }

      // All heights should be the same or close
      const minHeight = Math.min(...heights);
      const maxHeight = Math.max(...heights);
      expect(maxHeight - minHeight).toBeLessThanOrEqual(1); // Allow for 1 block difference
      console.log('Heights range:', minHeight, '-', maxHeight);
    });
  });
});

describe.skipIf(!LIVE_RPC_TEST)('Mainnet Memory Tests', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    provider = new AlkanesProvider({ network: 'mainnet' });
    await provider.initialize();
  });

  it('should handle mainnet address balances', async () => {
    const result = await provider.alkanes.getBalance(TEST_ADDRESSES.mainnetWorking);
    expect(Array.isArray(result)).toBe(true);
    console.log('Mainnet tokens:', result.length);
  });

  it('should handle mainnet UTXOs', async () => {
    const utxos = await provider.esplora.getAddressUtxos(TEST_ADDRESSES.mainnetWorking);
    expect(Array.isArray(utxos)).toBe(true);
    console.log('Mainnet UTXOs:', utxos.length);
  });
});

describe.skipIf(!LIVE_RPC_TEST)('Provider Isolation Tests', () => {
  it('should handle multiple provider instances', async () => {
    // Create multiple providers
    const provider1 = new AlkanesProvider({ network: 'mainnet' });
    const provider2 = new AlkanesProvider({
      network: 'subfrost-regtest',
      rpcUrl: REGTEST_RPC_URL,
    });

    await Promise.all([provider1.initialize(), provider2.initialize()]);

    // Use them concurrently
    const [mainnetHeight, regtestHeight] = await Promise.all([
      provider1.getBlockHeight(),
      provider2.getBlockHeight(),
    ]);

    console.log('Mainnet height:', mainnetHeight);
    console.log('Regtest height:', regtestHeight);

    expect(mainnetHeight).toBeGreaterThan(800000);
    expect(regtestHeight).toBeGreaterThan(0);
    expect(mainnetHeight).toBeGreaterThan(regtestHeight);
  });
});
