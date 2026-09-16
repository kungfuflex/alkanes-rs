/**
 * alkanesExecuteTyped forwards the #308 options into the WASM options JSON
 * (review lane B F8, 2026-09-16).
 *
 * Drives the REAL AlkanesProvider.alkanesExecuteTyped — not a copy of it — with the WASM
 * provider replaced by a stub that captures the options JSON handed to alkanesExecuteFull.
 */
import { describe, it, expect } from 'vitest';
import { AlkanesProvider } from '../src/provider';

function providerCapturing(): { provider: AlkanesProvider; lastOptions: () => Record<string, unknown> } {
  let captured = '';
  const provider = Object.create(AlkanesProvider.prototype) as AlkanesProvider;
  (provider as unknown as { _provider: unknown })._provider = {
    alkanesExecuteFull: async (_to: string, _req: string, _stones: string, _fee: unknown, _env: unknown, options: string | null) => {
      captured = options ?? '';
      return { reveal_txid: 'stub' };
    },
  };
  return { provider, lastOptions: () => JSON.parse(captured) };
}

describe('alkanesExecuteTyped — #308 options', () => {
  it('forwards splitAt, excludedUtxos, prefetchedUtxos (with required) and knownPendingTxHexes', async () => {
    const { provider, lastOptions } = providerCapturing();
    const prefetched = [{ outpoint: `${'ab'.repeat(32)}:0`, value: 546, script_pubkey_hex: '5120', required: true }];
    await provider.alkanesExecuteTyped({
      inputRequirements: 'B:1000',
      protostones: '[2,0,77]:v0:v0,[4,65522,13]:v1:v1',
      toAddresses: ['p2tr:0', 'p2tr:0'],
      splitTransactions: true,
      splitAt: 1,
      excludedUtxos: [`${'cd'.repeat(32)}:1`],
      prefetchedUtxos: prefetched,
      knownPendingTxHexes: ['0200'],
    });
    const opts = lastOptions();
    expect(opts.split_transactions).toBe(true);
    expect(opts.split_at).toBe(1);
    expect(opts.excluded_utxos).toEqual([`${'cd'.repeat(32)}:1`]);
    expect(opts.prefetched_utxos).toEqual(prefetched);
    expect(opts.known_pending_tx_hexes).toEqual(['0200']);
  });

  it('omits the #308 options when the caller does not pass them (no change for existing callers)', async () => {
    const { provider, lastOptions } = providerCapturing();
    await provider.alkanesExecuteTyped({ inputRequirements: 'B:1000', protostones: '[2,0,77]:v0:v0' });
    const opts = lastOptions();
    for (const key of ['split_at', 'excluded_utxos', 'prefetched_utxos', 'known_pending_tx_hexes']) {
      expect(opts).not.toHaveProperty(key);
    }
  });
});
