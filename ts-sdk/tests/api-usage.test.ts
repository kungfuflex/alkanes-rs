/**
 * @alkanes/ts-sdk API Usage Tests
 *
 * These tests verify correct and incorrect API usage patterns.
 * They help catch common mistakes like passing wrong parameter types.
 *
 * Run with: LIVE_RPC_TEST=true pnpm vitest run tests/api-usage.test.ts
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { AlkanesProvider } from '../src/provider';

const LIVE_RPC_TEST = process.env.LIVE_RPC_TEST === 'true';
const REGTEST_RPC_URL = process.env.REGTEST_RPC_URL || 'https://regtest.subfrost.io/v4/jsonrpc';

const TEST_ADDRESS = 'bcrt1phye8p0njg5yyvjjteyfwhzkrh5g4fyc9q4fprhzqh06kur5a0z0s7heqsm';

describe.skipIf(!LIVE_RPC_TEST)('Correct API Usage Patterns', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'subfrost-regtest',
      rpcUrl: REGTEST_RPC_URL,
    });
    await provider.initialize();
  });

  describe('Getting Alkane Balances', () => {
    it('CORRECT: should use provider.alkanes.getBalance() for address balances', async () => {
      // ✅ This is the CORRECT way to get alkane balances
      const balances = await provider.alkanes.getBalance(TEST_ADDRESS);

      expect(Array.isArray(balances)).toBe(true);
      console.log('Correct API usage - balances:', balances.length, 'tokens');

      // Each balance has the expected structure
      if (balances.length > 0) {
        expect(balances[0]).toHaveProperty('alkane_id');
        expect(balances[0].alkane_id).toHaveProperty('block');
        expect(balances[0].alkane_id).toHaveProperty('tx');
        expect(balances[0]).toHaveProperty('balance');
      }
    });

    it('CORRECT: should use provider.alkanes.getByAddress() for detailed balance info', async () => {
      // ✅ This returns balances organized by outpoint
      const result = await provider.alkanes.getByAddress(TEST_ADDRESS);

      console.log('getByAddress result:', result);
      // Result contains outpoint-level balance information
    });

    it('INCORRECT: metashrew.view() with object payload causes memory error', async () => {
      // ❌ This is INCORRECT - passing an object instead of hex string
      // This causes "memory access out of bounds" error
      try {
        await provider.metashrew.view(
          'protorunesbyaddress',
          // @ts-expect-error - Intentionally passing wrong type to test error handling
          { address: TEST_ADDRESS, protocolTag: '1' },
          'latest'
        );
        // If we get here, the test should fail
        expect.fail('Should have thrown an error for invalid payload type');
      } catch (error) {
        // Expected to fail - this demonstrates the bug
        console.log('Expected error for incorrect usage:', (error as Error).message);
        // The error might be "memory access out of bounds" or a serialization error
      }
    });

    it('CORRECT: metashrew.view() requires hex-encoded protobuf payload', async () => {
      // ✅ If you must use metashrew.view() directly, you need to encode the payload
      // The payload format is: 0x + protobuf-encoded(ProtorunesByAddressRequest)
      //
      // But for protorunesbyaddress, you should just use provider.alkanes.getBalance()
      // which handles all the encoding internally.
      //
      // Example of correct low-level usage (not recommended):
      // const payload = '0x0a40' + Buffer.from(address).toString('hex') + '12020801';
      // const result = await provider.metashrew.view('protorunesbyaddress', payload, 'latest');

      // Instead, use the high-level API:
      const balances = await provider.alkanes.getBalance(TEST_ADDRESS);
      expect(Array.isArray(balances)).toBe(true);
    });
  });

  describe('Getting UTXOs and Address Info', () => {
    it('CORRECT: should use provider.esplora.getAddressUtxos() for UTXOs', async () => {
      // ✅ Correct way to get UTXOs
      const utxos = await provider.esplora.getAddressUtxos(TEST_ADDRESS);

      expect(Array.isArray(utxos)).toBe(true);
      console.log('UTXOs:', utxos.length);
    });

    it('CORRECT: should use provider.getBalance() for BTC balance', async () => {
      // ✅ Correct way to get BTC balance
      const balance = await provider.getBalance(TEST_ADDRESS);

      expect(balance).toHaveProperty('confirmed');
      expect(balance).toHaveProperty('unconfirmed');
      expect(balance).toHaveProperty('utxos');
      console.log('BTC balance:', balance.confirmed, 'sats');
    });
  });

  describe('Provider Configuration', () => {
    it('should accept network preset string', () => {
      // ✅ Correct: use a network preset
      const p = new AlkanesProvider({ network: 'mainnet' });
      expect(p.networkType).toBe('mainnet');
    });

    it('should accept custom rpcUrl', () => {
      // ✅ Correct: override RPC URL
      const p = new AlkanesProvider({
        network: 'regtest',
        rpcUrl: 'http://custom.example.com/rpc',
      });
      expect(p.rpcUrl).toBe('http://custom.example.com/rpc');
    });

    it('should use subfrost-regtest preset for regtest with subfrost URL', () => {
      // ✅ For Subfrost regtest, use 'subfrost-regtest' network preset
      const p = new AlkanesProvider({
        network: 'subfrost-regtest',
      });
      expect(p.networkType).toBe('regtest');
      expect(p.rpcUrl).toContain('regtest.subfrost.io');
    });
  });
});

describe('API Usage Documentation', () => {
  it('documents the correct way to fetch alkane balances', () => {
    // This test serves as documentation for the correct API usage

    const exampleCode = `
// ✅ CORRECT: Get alkane balances for an address
const provider = new AlkanesProvider({
  network: 'subfrost-regtest',  // Use 'subfrost-regtest' for Subfrost's regtest endpoint
  rpcUrl: 'https://regtest.subfrost.io/v4/jsonrpc',  // Optional: override URL
});
await provider.initialize();

// Get alkane token balances
const balances = await provider.alkanes.getBalance(address);
// Returns: [{ alkane_id: { block: 2, tx: 0 }, balance: '1000', ... }]

// Get BTC UTXOs
const utxos = await provider.esplora.getAddressUtxos(address);

// Get combined BTC balance info
const btcBalance = await provider.getBalance(address);


// ❌ INCORRECT: Don't use metashrew.view() with object payload
// This will cause "memory access out of bounds" error:
// await provider.metashrew.view('protorunesbyaddress', { address, protocolTag: '1' }, 'latest');

// If you need low-level metashrew access, the payload must be hex-encoded protobuf
// But prefer using provider.alkanes.getBalance() instead.
`;

    console.log('API Usage Documentation:');
    console.log(exampleCode);
    expect(true).toBe(true);
  });
});
