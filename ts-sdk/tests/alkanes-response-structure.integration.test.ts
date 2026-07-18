/**
 * @alkanes/ts-sdk Response Structure Integration Test
 *
 * This test verifies the CORRECT way to parse responses from:
 * - provider.alkanes.getByAddress(address) - per-UTXO token balances
 * - provider.alkanes.getBalance(address) - aggregated token balances
 *
 * IMPORTANT: The balance_sheet structure is:
 * {
 *   "cached": {
 *     "balances": {
 *       "2:20": "10",    // alkane_id -> balance
 *       "2:24": "1"
 *     }
 *   }
 * }
 *
 * NOT balance_sheet.entries or balance_sheet itself!
 *
 * Run with: LIVE_RPC_TEST=true pnpm vitest run tests/alkanes-response-structure.integration.test.ts
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { AlkanesProvider } from '../src/provider';

const LIVE_RPC_TEST = process.env.LIVE_RPC_TEST === 'true';
const REGTEST_RPC_URL = process.env.REGTEST_RPC_URL || 'https://regtest.subfrost.io/v4/jsonrpc';

// Known address on subfrost regtest that has alkane tokens
const KNOWN_ADDRESS_WITH_ALKANES = 'bcrt1phye8p0njg5yyvjjteyfwhzkrh5g4fyc9q4fprhzqh06kur5a0z0s7heqsm';

describe.skipIf(!LIVE_RPC_TEST)('Alkanes Response Structure Tests', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'subfrost-regtest',
      rpcUrl: REGTEST_RPC_URL,
    });
    await provider.initialize();
  });

  describe('getByAddress() response structure', () => {
    it('should return balances array with correct nested structure', async () => {
      const result = await provider.alkanes.getByAddress(KNOWN_ADDRESS_WITH_ALKANES);

      // Top level structure
      expect(result).toHaveProperty('balances');
      expect(Array.isArray(result.balances)).toBe(true);
      console.log(`getByAddress returned ${result.balances.length} UTXOs`);
    });

    it('should have correct fields in each balance entry', async () => {
      const result = await provider.alkanes.getByAddress(KNOWN_ADDRESS_WITH_ALKANES);

      if (result.balances.length === 0) {
        console.log('WARN: No balances returned - address may have no UTXOs');
        return;
      }

      // Check first entry structure
      const entry = result.balances[0];

      // Required fields
      expect(entry).toHaveProperty('outpoint');
      expect(entry).toHaveProperty('output');
      expect(entry).toHaveProperty('balance_sheet');

      // outpoint should be "txid:vout" format string
      expect(typeof entry.outpoint).toBe('string');
      expect(entry.outpoint).toMatch(/^[a-f0-9]{64}:\d+$/);

      // output should have value and script_pubkey
      expect(entry.output).toHaveProperty('value');
      expect(entry.output).toHaveProperty('script_pubkey');

      // CRITICAL: balance_sheet has NESTED structure
      // balance_sheet.cached.balances is where the tokens are!
      expect(entry.balance_sheet).toHaveProperty('cached');
      expect(entry.balance_sheet.cached).toHaveProperty('balances');

      console.log('Sample balance entry structure:', JSON.stringify(entry, null, 2));
    });

    it('should demonstrate CORRECT way to extract token balances from balance_sheet', async () => {
      const result = await provider.alkanes.getByAddress(KNOWN_ADDRESS_WITH_ALKANES);

      let totalAlkanesFound = 0;
      const allAlkanes: Array<{
        outpoint: string;
        alkane_id: { block: number; tx: number };
        balance: string;
      }> = [];

      for (const entry of result.balances) {
        // ✅ CORRECT: Access balance_sheet.cached.balances
        const tokenBalances = entry.balance_sheet?.cached?.balances || {};

        // ❌ WRONG approaches (these will NOT work):
        // - entry.balance_sheet.entries
        // - Array.isArray(entry.balance_sheet)
        // - Object.entries(entry.balance_sheet) directly

        // tokenBalances is an object: { "block:tx": "amount", ... }
        const tokenEntries = Object.entries(tokenBalances);

        if (tokenEntries.length > 0) {
          console.log(`UTXO ${entry.outpoint} has ${tokenEntries.length} token(s)`);

          for (const [alkaneIdStr, amount] of tokenEntries) {
            // Parse alkane_id string like "2:20" into { block: 2, tx: 20 }
            const [blockStr, txStr] = alkaneIdStr.split(':');
            const block = parseInt(blockStr, 10);
            const tx = parseInt(txStr, 10);

            allAlkanes.push({
              outpoint: entry.outpoint,
              alkane_id: { block, tx },
              balance: String(amount),
            });

            totalAlkanesFound++;
            console.log(`  - Alkane ${alkaneIdStr}: ${amount}`);
          }
        } else {
          // This UTXO has no alkane tokens (just BTC)
          console.log(`UTXO ${entry.outpoint} has no alkanes (regular BTC UTXO)`);
        }
      }

      console.log(`\nTotal alkanes found across all UTXOs: ${totalAlkanesFound}`);
      console.log('All alkanes:', JSON.stringify(allAlkanes, null, 2));

      // The address should have at least some alkanes
      expect(totalAlkanesFound).toBeGreaterThan(0);
      expect(allAlkanes.length).toBeGreaterThan(0);

      // Verify parsed structure
      for (const alkane of allAlkanes) {
        expect(typeof alkane.alkane_id.block).toBe('number');
        expect(typeof alkane.alkane_id.tx).toBe('number');
        expect(typeof alkane.balance).toBe('string');
      }
    });

    it('should show that many UTXOs have empty balance_sheet (normal behavior)', async () => {
      const result = await provider.alkanes.getByAddress(KNOWN_ADDRESS_WITH_ALKANES);

      let emptyCount = 0;
      let nonEmptyCount = 0;

      for (const entry of result.balances) {
        const tokenBalances = entry.balance_sheet?.cached?.balances || {};
        const tokenCount = Object.keys(tokenBalances).length;

        if (tokenCount === 0) {
          emptyCount++;
        } else {
          nonEmptyCount++;
        }
      }

      console.log(`UTXOs with alkanes: ${nonEmptyCount}`);
      console.log(`UTXOs without alkanes (regular BTC): ${emptyCount}`);
      console.log(`Total UTXOs: ${result.balances.length}`);

      // This demonstrates that NOT all UTXOs have alkanes
      // Empty balance_sheet.cached.balances is NORMAL for regular BTC UTXOs
      expect(result.balances.length).toBeGreaterThanOrEqual(nonEmptyCount);
    });
  });

  describe('getBalance() response structure (aggregated)', () => {
    it('should return aggregated balances array', async () => {
      const balances = await provider.alkanes.getBalance(KNOWN_ADDRESS_WITH_ALKANES);

      expect(Array.isArray(balances)).toBe(true);
      console.log(`getBalance returned ${balances.length} token type(s)`);
      console.log('Aggregated balances:', JSON.stringify(balances, null, 2));
    });

    it('should have correct structure for each balance entry', async () => {
      const balances = await provider.alkanes.getBalance(KNOWN_ADDRESS_WITH_ALKANES);

      expect(balances.length).toBeGreaterThan(0);

      for (const balance of balances) {
        // Each entry should have alkane_id and balance
        expect(balance).toHaveProperty('alkane_id');
        expect(balance).toHaveProperty('balance');

        // alkane_id should have block and tx
        expect(balance.alkane_id).toHaveProperty('block');
        expect(balance.alkane_id).toHaveProperty('tx');

        // Types should be correct
        expect(typeof balance.alkane_id.block).toBe('number');
        expect(typeof balance.alkane_id.tx).toBe('number');

        console.log(`Token ${balance.alkane_id.block}:${balance.alkane_id.tx} = ${balance.balance}`);
      }
    });

    it('should demonstrate getBalance is simpler for total holdings', async () => {
      // getBalance() is the RECOMMENDED method when you just need totals
      const balances = await provider.alkanes.getBalance(KNOWN_ADDRESS_WITH_ALKANES);

      // Easy to use - no parsing needed!
      const tokenHoldings = balances.map((b: any) => ({
        tokenId: `${b.alkane_id.block}:${b.alkane_id.tx}`,
        totalBalance: b.balance,
      }));

      console.log('Token holdings (using getBalance):', tokenHoldings);

      expect(tokenHoldings.length).toBeGreaterThan(0);
    });
  });

  describe('Comparison: getByAddress vs getBalance', () => {
    it('should show getBalance aggregates what getByAddress shows per-UTXO', async () => {
      // Get both responses
      const byAddress = await provider.alkanes.getByAddress(KNOWN_ADDRESS_WITH_ALKANES);
      const aggregated = await provider.alkanes.getBalance(KNOWN_ADDRESS_WITH_ALKANES);

      // Manually aggregate from getByAddress
      const manualAggregation: Record<string, bigint> = {};

      for (const entry of byAddress.balances) {
        const tokenBalances = entry.balance_sheet?.cached?.balances || {};

        for (const [alkaneIdStr, amount] of Object.entries(tokenBalances)) {
          const currentBalance = manualAggregation[alkaneIdStr] || BigInt(0);
          manualAggregation[alkaneIdStr] = currentBalance + BigInt(String(amount));
        }
      }

      console.log('Manual aggregation from getByAddress:', manualAggregation);
      console.log('getBalance result:', aggregated);

      // Each token in getBalance should match manual aggregation
      for (const balance of aggregated) {
        const tokenId = `${balance.alkane_id.block}:${balance.alkane_id.tx}`;
        const manualTotal = manualAggregation[tokenId];

        if (manualTotal !== undefined) {
          console.log(`Token ${tokenId}: getBalance=${balance.balance}, manual=${manualTotal}`);
          expect(BigInt(balance.balance)).toBe(manualTotal);
        }
      }
    });
  });
});

describe.skipIf(!LIVE_RPC_TEST)('Example: Correct API Usage Pattern', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'subfrost-regtest',
      rpcUrl: REGTEST_RPC_URL,
    });
    await provider.initialize();
  });

  it('demonstrates the recommended pattern for getting alkane balances', async () => {
    const address = KNOWN_ADDRESS_WITH_ALKANES;

    // OPTION 1: Simple aggregated balances (RECOMMENDED for most use cases)
    const balances = await provider.alkanes.getBalance(address);

    console.log('\n=== OPTION 1: getBalance() (aggregated) ===');
    console.log('Use this when you just need to know what tokens an address holds:\n');
    balances.forEach((b: any) => {
      console.log(`  Token ${b.alkane_id.block}:${b.alkane_id.tx} = ${b.balance}`);
    });

    // OPTION 2: Per-UTXO breakdown (for advanced use cases)
    const byAddress = await provider.alkanes.getByAddress(address);

    console.log('\n=== OPTION 2: getByAddress() (per-UTXO) ===');
    console.log('Use this when you need to know WHICH UTXOs hold tokens:\n');

    for (const entry of byAddress.balances) {
      // CORRECT path to tokens!
      const tokens = entry.balance_sheet?.cached?.balances || {};
      const tokenList = Object.entries(tokens);

      if (tokenList.length > 0) {
        console.log(`  UTXO ${entry.outpoint}:`);
        console.log(`    BTC value: ${entry.output.value} sats`);
        tokenList.forEach(([id, amount]) => {
          console.log(`    Token ${id}: ${amount}`);
        });
      }
    }

    // Both methods should show tokens exist
    expect(balances.length).toBeGreaterThan(0);
  });
});
