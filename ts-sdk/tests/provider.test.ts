/**
 * @alkanes/ts-sdk Provider Integration Tests
 *
 * These tests verify the AlkanesProvider works correctly across different networks.
 * Run with: pnpm vitest run tests/provider.test.ts
 *
 * For live RPC tests: LIVE_RPC_TEST=true pnpm vitest run tests/provider.test.ts
 */

import { describe, it, expect, beforeAll, beforeEach, afterAll } from 'vitest';
import { AlkanesProvider, NETWORK_PRESETS } from '../src/provider';

// Test addresses for different networks
const TEST_ADDRESSES = {
  mainnet: 'bc1puvfmy5whzdq35nd2trckkm09em9u7ps6lal564jz92c9feswwrpsr7ach5',
  regtest: 'bcrt1phye8p0njg5yyvjjteyfwhzkrh5g4fyc9q4fprhzqh06kur5a0z0s7heqsm',
  // Add more test addresses as needed
};

// Check if live RPC tests should run
const LIVE_RPC_TEST = process.env.LIVE_RPC_TEST === 'true';
const REGTEST_RPC_URL = process.env.REGTEST_RPC_URL || 'https://regtest.subfrost.io/v4/jsonrpc';

describe('AlkanesProvider', () => {
  describe('Provider Creation', () => {
    it('should create a provider with mainnet preset', () => {
      const provider = new AlkanesProvider({ network: 'mainnet' });
      expect(provider.networkType).toBe('mainnet');
      expect(provider.rpcUrl).toBe(NETWORK_PRESETS.mainnet.rpcUrl);
    });

    it('should create a provider with regtest preset', () => {
      const provider = new AlkanesProvider({ network: 'regtest' });
      expect(provider.networkType).toBe('regtest');
    });

    it('should create a provider with subfrost-regtest preset', () => {
      const provider = new AlkanesProvider({ network: 'subfrost-regtest' });
      expect(provider.networkType).toBe('regtest');
      expect(provider.rpcUrl).toBe(NETWORK_PRESETS['subfrost-regtest'].rpcUrl);
    });

    it('should create a provider with custom RPC URL', () => {
      const customUrl = 'https://custom.example.com/rpc';
      const provider = new AlkanesProvider({
        network: 'regtest',
        rpcUrl: customUrl,
      });
      expect(provider.rpcUrl).toBe(customUrl);
    });

    it('should default to mainnet for unknown network', () => {
      const provider = new AlkanesProvider({ network: 'unknown' as any });
      expect(provider.networkType).toBe('mainnet');
    });
  });

  describe('Provider Initialization', () => {
    it('should initialize successfully', async () => {
      const provider = new AlkanesProvider({ network: 'mainnet' });
      await expect(provider.initialize()).resolves.not.toThrow();
    });

    it('should be idempotent - multiple initializations should not throw', async () => {
      const provider = new AlkanesProvider({ network: 'mainnet' });
      await provider.initialize();
      await expect(provider.initialize()).resolves.not.toThrow();
    });

    it('should provide bitcoin client after initialization', async () => {
      const provider = new AlkanesProvider({ network: 'mainnet' });
      await provider.initialize();
      expect(provider.bitcoin).toBeDefined();
    });

    it('should provide esplora client after initialization', async () => {
      const provider = new AlkanesProvider({ network: 'mainnet' });
      await provider.initialize();
      expect(provider.esplora).toBeDefined();
    });

    it('should provide alkanes client after initialization', async () => {
      const provider = new AlkanesProvider({ network: 'mainnet' });
      await provider.initialize();
      expect(provider.alkanes).toBeDefined();
    });

    it('should provide metashrew client after initialization', async () => {
      const provider = new AlkanesProvider({ network: 'mainnet' });
      await provider.initialize();
      expect(provider.metashrew).toBeDefined();
    });

    it('should provide lua client after initialization', async () => {
      const provider = new AlkanesProvider({ network: 'mainnet' });
      await provider.initialize();
      expect(provider.lua).toBeDefined();
    });

    it('should provide dataApi client after initialization', async () => {
      const provider = new AlkanesProvider({ network: 'mainnet' });
      await provider.initialize();
      expect(provider.dataApi).toBeDefined();
    });

    it('should throw if accessing clients before initialization', () => {
      const provider = new AlkanesProvider({ network: 'mainnet' });
      expect(() => provider.bitcoin).toThrow('Provider not initialized');
      expect(() => provider.esplora).toThrow('Provider not initialized');
      expect(() => provider.alkanes).toThrow('Provider not initialized');
    });
  });
});

// Live RPC tests - only run when LIVE_RPC_TEST=true
describe.skipIf(!LIVE_RPC_TEST)('Live RPC Tests', () => {
  describe('Mainnet Integration', () => {
    let provider: AlkanesProvider;

    beforeAll(async () => {
      provider = new AlkanesProvider({ network: 'mainnet' });
      await provider.initialize();
    });

    it('should fetch current block height', async () => {
      const height = await provider.getBlockHeight();
      expect(height).toBeGreaterThan(800000); // We're well past block 800k
      console.log('Mainnet height:', height);
    });

    it('should fetch alkane balances for known address', async () => {
      const balances = await provider.alkanes.getBalance(TEST_ADDRESSES.mainnet);
      expect(Array.isArray(balances)).toBe(true);
      console.log('Mainnet balances:', balances.length, 'tokens');
    });

    it('should fetch UTXOs for address', async () => {
      const utxos = await provider.esplora.getAddressUtxos(TEST_ADDRESSES.mainnet);
      expect(Array.isArray(utxos)).toBe(true);
      console.log('Mainnet UTXOs:', utxos.length);
    });

    it('should get BTC balance for address', async () => {
      const balance = await provider.getBalance(TEST_ADDRESSES.mainnet);
      expect(balance).toHaveProperty('confirmed');
      expect(balance).toHaveProperty('unconfirmed');
      expect(balance).toHaveProperty('utxos');
      console.log('Mainnet BTC balance:', balance.confirmed, 'sats');
    });
  });

  describe('Subfrost Regtest Integration', () => {
    let provider: AlkanesProvider;

    beforeAll(async () => {
      provider = new AlkanesProvider({
        network: 'subfrost-regtest',
        rpcUrl: REGTEST_RPC_URL,
      });
      await provider.initialize();
    });

    it('should initialize with correct network settings', () => {
      expect(provider.networkType).toBe('regtest');
      expect(provider.network.bech32).toBe('bcrt');
    });

    it('should fetch current block height', async () => {
      const height = await provider.getBlockHeight();
      expect(height).toBeGreaterThan(0);
      console.log('Regtest height:', height);
    });

    it('should handle getBalance for regtest address', async () => {
      // This is the test case that was failing with "memory access out of bounds"
      try {
        const balances = await provider.alkanes.getBalance(TEST_ADDRESSES.regtest);
        expect(Array.isArray(balances)).toBe(true);
        console.log('Regtest balances:', balances);
      } catch (error) {
        console.error('Regtest getBalance error:', error);
        // If we get "memory access out of bounds", this is the bug we need to fix
        if (error instanceof Error && error.message.includes('memory access out of bounds')) {
          throw new Error('WASM memory access out of bounds - this indicates a bug in the WASM bindings or incorrect data handling');
        }
        throw error;
      }
    });

    it('should fetch UTXOs for regtest address', async () => {
      try {
        const utxos = await provider.esplora.getAddressUtxos(TEST_ADDRESSES.regtest);
        expect(Array.isArray(utxos)).toBe(true);
        console.log('Regtest UTXOs:', utxos.length);
      } catch (error) {
        console.error('Regtest UTXO fetch error:', error);
        throw error;
      }
    });
  });

  describe('Custom Regtest Configuration', () => {
    it('should work with oylnet-style configuration', async () => {
      // Simulate oylnet configuration
      const provider = new AlkanesProvider({
        network: 'regtest',
        rpcUrl: REGTEST_RPC_URL,
      });
      await provider.initialize();

      console.log('Provider initialized:', {
        networkType: provider.networkType,
        rpcUrl: provider.rpcUrl,
        bech32: provider.network.bech32,
      });

      try {
        const height = await provider.getBlockHeight();
        expect(height).toBeGreaterThan(0);
        console.log('Custom regtest height:', height);
      } catch (error) {
        console.error('Custom regtest height error:', error);
        throw error;
      }
    });
  });
});

describe('Network Presets', () => {
  it('should have correct mainnet preset', () => {
    expect(NETWORK_PRESETS.mainnet).toBeDefined();
    expect(NETWORK_PRESETS.mainnet.networkType).toBe('mainnet');
    expect(NETWORK_PRESETS.mainnet.rpcUrl).toContain('mainnet');
  });

  it('should have correct regtest preset', () => {
    expect(NETWORK_PRESETS.regtest).toBeDefined();
    expect(NETWORK_PRESETS.regtest.networkType).toBe('regtest');
  });

  it('should have correct subfrost-regtest preset', () => {
    expect(NETWORK_PRESETS['subfrost-regtest']).toBeDefined();
    expect(NETWORK_PRESETS['subfrost-regtest'].networkType).toBe('regtest');
    expect(NETWORK_PRESETS['subfrost-regtest'].rpcUrl).toContain('regtest.subfrost.io');
  });

  it('should have correct testnet preset', () => {
    expect(NETWORK_PRESETS.testnet).toBeDefined();
    expect(NETWORK_PRESETS.testnet.networkType).toBe('testnet');
  });

  it('should have correct signet preset', () => {
    expect(NETWORK_PRESETS.signet).toBeDefined();
    expect(NETWORK_PRESETS.signet.networkType).toBe('signet');
  });
});
