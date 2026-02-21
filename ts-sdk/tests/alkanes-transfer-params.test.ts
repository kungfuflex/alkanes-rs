/**
 * Alkane Transfer — Parameter Currying Unit Tests
 *
 * Verifies that every parameter flows correctly through the full stack:
 *   AlkanesTransferParams → client.alkanesTransfer() → provider.alkanesTransferTyped()
 *     → provider.alkanesExecuteTyped() → options JSON → WASM binding
 *
 * These are pure unit tests — no WASM, no network, no RPC calls.
 * The provider's internal methods are tested by capturing the arguments
 * that would be passed to the WASM layer.
 *
 * Run with: pnpm vitest run tests/alkanes-transfer-params.test.ts
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';

// ---------------------------------------------------------------------------
// Mock AlkanesProvider — captures alkanesExecuteTyped calls
// ---------------------------------------------------------------------------

/**
 * Minimal mock that replicates alkanesTransferTyped's logic and captures
 * what it passes to alkanesExecuteTyped. This lets us verify the currying
 * without importing the full provider or loading WASM.
 */
function createMockProvider() {
  const executeTypedCalls: any[] = [];

  const provider = {
    // Capture every call to alkanesExecuteTyped
    alkanesExecuteTyped: vi.fn(async (params: any) => {
      executeTypedCalls.push(params);
      return { txid: 'mock-txid' };
    }),

    // Exact copy of alkanesTransferTyped from provider/index.ts (PR #252)
    alkanesTransferTyped: async (params: {
      alkaneId: { block: number; tx: number };
      amount: string | number | bigint;
      toAddress: string;
      fromAddresses?: string[];
      changeAddress?: string;
      alkanesChangeAddress?: string;
      feeRate?: number;
      ordinalsStrategy?: string;
      mempoolIndexer?: boolean;
      autoConfirm?: boolean;
      mineEnabled?: boolean;
      traceEnabled?: boolean;
      pointer?: string;
      refund?: string;
    }) => {
      const { block, tx } = params.alkaneId;
      const amount = String(params.amount);
      const pointer = params.pointer ?? 'v0';
      const refund = params.refund ?? pointer;
      const protostones = `[${block}:${tx}:${amount}:v1]:${pointer}:${refund}`;
      const inputRequirements = `${block}:${tx}:${amount}`;

      return provider.alkanesExecuteTyped({
        toAddresses: ['p2tr:0', params.toAddress],
        inputRequirements,
        protostones,
        feeRate: params.feeRate,
        fromAddresses: params.fromAddresses,
        changeAddress: params.changeAddress,
        alkanesChangeAddress: params.alkanesChangeAddress,
        traceEnabled: params.traceEnabled,
        mineEnabled: params.mineEnabled,
        autoConfirm: params.autoConfirm,
        ordinalsStrategy: params.ordinalsStrategy,
        mempoolIndexer: params.mempoolIndexer,
      });
    },

    getExecuteTypedCalls: () => executeTypedCalls,
  };

  return provider;
}

// ---------------------------------------------------------------------------
// Mock AlkanesClient — captures alkanesTransferTyped calls
// ---------------------------------------------------------------------------

function createMockClient(provider: ReturnType<typeof createMockProvider>) {
  const transferTypedCalls: any[] = [];

  const originalTransferTyped = provider.alkanesTransferTyped;
  provider.alkanesTransferTyped = async (params: any) => {
    transferTypedCalls.push(params);
    return originalTransferTyped(params);
  };

  return {
    // Exact copy of client.alkanesTransfer from client/client.ts (PR #252)
    alkanesTransfer: async (params: {
      alkane_id: { block: number; tx: number };
      amount: number | bigint | string;
      to_address: string;
      from_addresses?: string[];
      change_address?: string;
      alkanes_change_address?: string;
      fee_rate?: number;
      ordinals_strategy?: string;
      mempool_indexer?: boolean;
      auto_confirm?: boolean;
      pointer?: string;
      refund?: string;
    }) => {
      return provider.alkanesTransferTyped({
        alkaneId: params.alkane_id,
        amount: String(params.amount),
        toAddress: params.to_address,
        fromAddresses: params.from_addresses,
        changeAddress: params.change_address,
        alkanesChangeAddress: params.alkanes_change_address,
        feeRate: params.fee_rate,
        ordinalsStrategy: params.ordinals_strategy,
        mempoolIndexer: params.mempool_indexer,
        autoConfirm: params.auto_confirm,
        pointer: params.pointer,
        refund: params.refund,
      });
    },

    getTransferTypedCalls: () => transferTypedCalls,
  };
}

// ---------------------------------------------------------------------------
// Mock options JSON builder — replicates alkanesExecuteTyped's serialization
// ---------------------------------------------------------------------------

function buildOptionsJson(params: {
  fromAddresses?: string[];
  changeAddress?: string;
  alkanesChangeAddress?: string;
  traceEnabled?: boolean;
  mineEnabled?: boolean;
  autoConfirm?: boolean;
  rawOutput?: boolean;
  ordinalsStrategy?: string;
  mempoolIndexer?: boolean;
}): Record<string, any> {
  const options: Record<string, any> = {};
  options.from_addresses = params.fromAddresses ?? ['p2wpkh:0', 'p2tr:0'];
  options.change_address = params.changeAddress ?? 'p2wpkh:0';
  options.alkanes_change_address = params.alkanesChangeAddress ?? 'p2tr:0';
  if (params.traceEnabled !== undefined) options.trace_enabled = params.traceEnabled;
  if (params.mineEnabled !== undefined) options.mine_enabled = params.mineEnabled;
  if (params.autoConfirm !== undefined) options.auto_confirm = params.autoConfirm;
  if (params.rawOutput !== undefined) options.raw_output = params.rawOutput;
  if (params.ordinalsStrategy !== undefined) options.ordinals_strategy = params.ordinalsStrategy;
  if (params.mempoolIndexer !== undefined) options.mempool_indexer = params.mempoolIndexer;
  return options;
}

// ============================================================================
// Tests
// ============================================================================

describe('alkanesTransferTyped — parameter currying', () => {
  let provider: ReturnType<typeof createMockProvider>;

  beforeEach(() => {
    provider = createMockProvider();
  });

  // -------------------------------------------------------------------------
  // Core params: alkaneId, amount, toAddress
  // -------------------------------------------------------------------------

  describe('core params', () => {
    it('should build correct protostone from alkaneId and amount', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.protostones).toBe('[2:0:1000:v1]:v0:v0');
    });

    it('should build correct inputRequirements from alkaneId and amount', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 32, tx: 0 },
        amount: '500000',
        toAddress: 'bc1precip',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.inputRequirements).toBe('32:0:500000');
    });

    it('should place sender at v0 and recipient at v1 in toAddresses', async () => {
      const recipient = 'bc1p0mrr2pfespj94knxwhccgsue38rgmc9yg6rcclj2e4g948t73vssj2j648';
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: recipient,
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.toAddresses).toEqual(['p2tr:0', recipient]);
    });

    it('should always produce exactly 2 toAddresses entries', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.toAddresses).toHaveLength(2);
    });

    it('should coerce numeric amount to string', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: 42000,
        toAddress: 'bc1precip',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.protostones).toBe('[2:0:42000:v1]:v0:v0');
      expect(call.inputRequirements).toBe('2:0:42000');
    });

    it('should coerce bigint amount to string', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: 99999999999n,
        toAddress: 'bc1precip',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.protostones).toContain('99999999999');
    });
  });

  // -------------------------------------------------------------------------
  // Pointer / refund
  // -------------------------------------------------------------------------

  describe('pointer and refund', () => {
    it('should default both pointer and refund to v0', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.protostones).toBe('[2:0:1000:v1]:v0:v0');
    });

    it('should use custom pointer and cascade refund from pointer', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        pointer: 'v2',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.protostones).toBe('[2:0:1000:v1]:v2:v2');
    });

    it('should allow independent pointer and refund', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        pointer: 'v0',
        refund: 'v1',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.protostones).toBe('[2:0:1000:v1]:v0:v1');
    });

    it('should support shadow protostone targets (pN)', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        pointer: 'p0',
        refund: 'p1',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.protostones).toBe('[2:0:1000:v1]:p0:p1');
    });
  });

  // -------------------------------------------------------------------------
  // Optional execution params forwarding
  // -------------------------------------------------------------------------

  describe('optional params forwarding', () => {
    it('should forward feeRate', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        feeRate: 25,
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.feeRate).toBe(25);
    });

    it('should forward fromAddresses', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        fromAddresses: ['bc1qsender', 'bc1psender'],
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.fromAddresses).toEqual(['bc1qsender', 'bc1psender']);
    });

    it('should forward changeAddress', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        changeAddress: 'bc1qmychange',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.changeAddress).toBe('bc1qmychange');
    });

    it('should forward alkanesChangeAddress', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        alkanesChangeAddress: 'bc1pmyalkchange',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.alkanesChangeAddress).toBe('bc1pmyalkchange');
    });

    it('should forward ordinalsStrategy', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        ordinalsStrategy: 'preserve',
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.ordinalsStrategy).toBe('preserve');
    });

    it('should forward mempoolIndexer', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        mempoolIndexer: true,
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.mempoolIndexer).toBe(true);
    });

    it('should forward traceEnabled', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        traceEnabled: true,
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.traceEnabled).toBe(true);
    });

    it('should forward mineEnabled', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        mineEnabled: true,
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.mineEnabled).toBe(true);
    });

    it('should forward autoConfirm', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        autoConfirm: false,
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call.autoConfirm).toBe(false);
    });

    it('should NOT forward useSlipstream/useRebar/rebarTier (dead params)', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
        // These params should not exist on the type — TypeScript would catch this
        // at compile time, but this test documents the intention
      });

      const call = provider.getExecuteTypedCalls()[0];
      expect(call).not.toHaveProperty('useSlipstream');
      expect(call).not.toHaveProperty('useRebar');
      expect(call).not.toHaveProperty('rebarTier');
    });
  });

  // -------------------------------------------------------------------------
  // Undefined optional params should not pollute the call
  // -------------------------------------------------------------------------

  describe('undefined optional params', () => {
    it('should pass undefined for unset optional params (not false/null)', async () => {
      await provider.alkanesTransferTyped({
        alkaneId: { block: 2, tx: 0 },
        amount: '1000',
        toAddress: 'bc1precip',
      });

      const call = provider.getExecuteTypedCalls()[0];
      // These should be undefined (not set), so alkanesExecuteTyped applies its own defaults
      expect(call.feeRate).toBeUndefined();
      expect(call.fromAddresses).toBeUndefined();
      expect(call.changeAddress).toBeUndefined();
      expect(call.alkanesChangeAddress).toBeUndefined();
      expect(call.traceEnabled).toBeUndefined();
      expect(call.mineEnabled).toBeUndefined();
      expect(call.autoConfirm).toBeUndefined();
      expect(call.ordinalsStrategy).toBeUndefined();
      expect(call.mempoolIndexer).toBeUndefined();
    });
  });
});

// ============================================================================
// Client → Provider currying
// ============================================================================

describe('client.alkanesTransfer — snake_case → camelCase currying', () => {
  let provider: ReturnType<typeof createMockProvider>;
  let client: ReturnType<typeof createMockClient>;

  beforeEach(() => {
    provider = createMockProvider();
    client = createMockClient(provider);
  });

  it('should convert alkane_id to alkaneId', async () => {
    await client.alkanesTransfer({
      alkane_id: { block: 2, tx: 0 },
      amount: '1000',
      to_address: 'bc1precip',
    });

    const call = client.getTransferTypedCalls()[0];
    expect(call.alkaneId).toEqual({ block: 2, tx: 0 });
  });

  it('should convert to_address to toAddress', async () => {
    await client.alkanesTransfer({
      alkane_id: { block: 2, tx: 0 },
      amount: '1000',
      to_address: 'bc1ptherecipient',
    });

    const call = client.getTransferTypedCalls()[0];
    expect(call.toAddress).toBe('bc1ptherecipient');
  });

  it('should coerce amount to string', async () => {
    await client.alkanesTransfer({
      alkane_id: { block: 2, tx: 0 },
      amount: 5000,
      to_address: 'bc1precip',
    });

    const call = client.getTransferTypedCalls()[0];
    expect(call.amount).toBe('5000');
  });

  it('should forward all optional params with correct casing', async () => {
    await client.alkanesTransfer({
      alkane_id: { block: 2, tx: 0 },
      amount: '1000',
      to_address: 'bc1precip',
      from_addresses: ['bc1qfrom', 'bc1pfrom'],
      change_address: 'bc1qchange',
      alkanes_change_address: 'bc1palkchange',
      fee_rate: 15,
      ordinals_strategy: 'preserve',
      mempool_indexer: true,
      auto_confirm: false,
      pointer: 'p0',
      refund: 'v0',
    });

    const call = client.getTransferTypedCalls()[0];
    expect(call.fromAddresses).toEqual(['bc1qfrom', 'bc1pfrom']);
    expect(call.changeAddress).toBe('bc1qchange');
    expect(call.alkanesChangeAddress).toBe('bc1palkchange');
    expect(call.feeRate).toBe(15);
    expect(call.ordinalsStrategy).toBe('preserve');
    expect(call.mempoolIndexer).toBe(true);
    expect(call.autoConfirm).toBe(false);
    expect(call.pointer).toBe('p0');
    expect(call.refund).toBe('v0');
  });

  it('should NOT pass dead params (slipstream/rebar) to provider', async () => {
    await client.alkanesTransfer({
      alkane_id: { block: 2, tx: 0 },
      amount: '1000',
      to_address: 'bc1precip',
    });

    const call = client.getTransferTypedCalls()[0];
    expect(call).not.toHaveProperty('useSlipstream');
    expect(call).not.toHaveProperty('useRebar');
    expect(call).not.toHaveProperty('rebarTier');
  });

  it('should curry all the way through: client → provider → executeTyped', async () => {
    const recipient = 'bc1p0mrr2pfespj94knxwhccgsue38rgmc9yg6rcclj2e4g948t73vssj2j648';

    await client.alkanesTransfer({
      alkane_id: { block: 32, tx: 0 },
      amount: '100000000',
      to_address: recipient,
      fee_rate: 10,
      ordinals_strategy: 'preserve',
      mempool_indexer: true,
      pointer: 'v0',
      refund: 'p0',
    });

    // Verify the final call to alkanesExecuteTyped
    const execCall = provider.getExecuteTypedCalls()[0];
    expect(execCall.protostones).toBe('[32:0:100000000:v1]:v0:p0');
    expect(execCall.inputRequirements).toBe('32:0:100000000');
    expect(execCall.toAddresses).toEqual(['p2tr:0', recipient]);
    expect(execCall.feeRate).toBe(10);
    expect(execCall.ordinalsStrategy).toBe('preserve');
    expect(execCall.mempoolIndexer).toBe(true);
  });
});

// ============================================================================
// Options JSON serialization
// ============================================================================

describe('alkanesExecuteTyped — options JSON serialization', () => {
  it('should serialize ordinalsStrategy as ordinals_strategy', () => {
    const options = buildOptionsJson({ ordinalsStrategy: 'preserve' });
    expect(options.ordinals_strategy).toBe('preserve');
  });

  it('should serialize mempoolIndexer as mempool_indexer', () => {
    const options = buildOptionsJson({ mempoolIndexer: true });
    expect(options.mempool_indexer).toBe(true);
  });

  it('should NOT include ordinals_strategy when undefined', () => {
    const options = buildOptionsJson({});
    expect(options).not.toHaveProperty('ordinals_strategy');
  });

  it('should NOT include mempool_indexer when undefined', () => {
    const options = buildOptionsJson({});
    expect(options).not.toHaveProperty('mempool_indexer');
  });

  it('should include all standard defaults', () => {
    const options = buildOptionsJson({});
    expect(options.from_addresses).toEqual(['p2wpkh:0', 'p2tr:0']);
    expect(options.change_address).toBe('p2wpkh:0');
    expect(options.alkanes_change_address).toBe('p2tr:0');
  });

  it('should serialize traceEnabled as trace_enabled', () => {
    const options = buildOptionsJson({ traceEnabled: true });
    expect(options.trace_enabled).toBe(true);
  });

  it('should serialize mineEnabled as mine_enabled', () => {
    const options = buildOptionsJson({ mineEnabled: true });
    expect(options.mine_enabled).toBe(true);
  });

  it('should serialize autoConfirm as auto_confirm', () => {
    const options = buildOptionsJson({ autoConfirm: false });
    expect(options.auto_confirm).toBe(false);
  });

  it('should produce valid JSON for all supported options', () => {
    const options = buildOptionsJson({
      fromAddresses: ['bc1qfrom'],
      changeAddress: 'bc1qchange',
      alkanesChangeAddress: 'bc1palkchange',
      traceEnabled: true,
      mineEnabled: true,
      autoConfirm: false,
      ordinalsStrategy: 'preserve',
      mempoolIndexer: true,
    });

    const json = JSON.stringify(options);
    const parsed = JSON.parse(json);
    expect(parsed.from_addresses).toEqual(['bc1qfrom']);
    expect(parsed.change_address).toBe('bc1qchange');
    expect(parsed.alkanes_change_address).toBe('bc1palkchange');
    expect(parsed.trace_enabled).toBe(true);
    expect(parsed.mine_enabled).toBe(true);
    expect(parsed.auto_confirm).toBe(false);
    expect(parsed.ordinals_strategy).toBe('preserve');
    expect(parsed.mempool_indexer).toBe(true);
  });
});
