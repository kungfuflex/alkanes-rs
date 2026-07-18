/**
 * @alkanes/ts-sdk meta-extraction integration tests against mainnet.
 *
 * Verifies that `provider.alkanes.getMeta(alkaneId)` works end-to-end —
 * the helper that wraps:
 *   1. parse "block:tx" alkane id
 *   2. encode calldata as varint list [block, tx]
 *   3. encode a MessageContextParcel protobuf with that calldata
 *   4. metashrew_view("meta", "0x<protobuf-hex>", "latest")
 *   5. utf-8 decode + JSON.parse the response
 *
 * This is the right primitive for any Next.js / browser integration
 * that needs to discover an alkane's ABI (token name/symbol/decimals,
 * NFT data getters, AMM pool methods, etc.) — diogao's
 * ticket-0483 use case.
 *
 * Run with:
 *   INTEGRATION=true pnpm vitest run tests/alkane-meta.integration.test.ts
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { AlkanesProvider } from '../src/provider';

const INTEGRATION = process.env.INTEGRATION === 'true';

// Known mainnet alkanes — pick contracts representing the three
// shapes a marketplace / explorer will encounter.
const DIESEL_ID            = '2:0';       // canonical token (FT)
const FRBTC_ID             = '32:0';      // SyntheticBitcoin (FT, u64 supply)
const ORBITAL_INSTANCE_ID  = '2:76000';   // OrbitalInstance NFT (diogao's case)

describe.skipIf(!INTEGRATION)('alkane meta extraction (mainnet)', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    // alkanode is consistently faster for `meta` than the subfrost
    // mainnet endpoint (which sometimes 408s on cold cache for
    // less-frequently-queried alkanes like OrbitalInstance). Both serve
    // identical canonical data — see the equivalence verification at
    // /home/ubuntu/subkube/.fastpath-bug-investigation/.
    provider = new AlkanesProvider({
      network: 'mainnet',
      rpcUrl:  'https://mainnet.alkanode.com',
    });
    await provider.initialize();
  });

  it('getMeta returns the full ABI for DIESEL (2:0)', async () => {
    const raw = await provider.alkanes.getMeta(DIESEL_ID);

    // The ts-sdk's alkanesMeta returns a JSON string (or hex if the
    // response wasn't valid utf-8). For all production alkanes the
    // __meta export emits utf-8 JSON, so we expect a parseable string.
    expect(typeof raw).toBe('string');
    const meta = JSON.parse(raw);

    // Field is named `contract` (not `name`) — see the
    // `declare_alkane!` macro in alkanes-runtime which emits
    // {"contract": "<MessageType>", "methods": [...]}.
    expect(meta).toMatchObject({
      contract: expect.any(String),
      methods:  expect.any(Array),
    });

    // Every method must carry name, opcode, params, returns.
    for (const m of meta.methods) {
      expect(m).toMatchObject({
        name:    expect.any(String),
        opcode:  expect.any(Number),
        params:  expect.any(Array),
        returns: expect.any(String),
      });
    }

    // DIESEL's standard token interface: mint=77, get_name=99,
    // get_symbol=100, get_total_supply=101 — fixed by the
    // GenesisAlkane MessageDispatch enum.
    const byOpcode = new Map(meta.methods.map((m: any) => [m.opcode, m]));
    expect(byOpcode.get(77)?.name).toBe('mint');
    expect(byOpcode.get(99)?.name).toBe('get_name');
    expect(byOpcode.get(100)?.name).toBe('get_symbol');
    expect(byOpcode.get(101)?.name).toBe('get_total_supply');
  });

  it('getMeta returns the SyntheticBitcoin ABI for frBTC (32:0)', async () => {
    const raw = await provider.alkanes.getMeta(FRBTC_ID);
    const meta = JSON.parse(raw);

    // frBTC's contract self-name is "SyntheticBitcoin".
    expect(meta.contract).toMatch(/SyntheticBitcoin/);

    // SyntheticBitcoin has the wrap/unwrap methods plus the token
    // getters — opcode 105 is get_total_supply (u64, NOT u128 like
    // DIESEL — diogao's marketplace MUST honor `returns` to format
    // the value correctly).
    const byOpcode = new Map(meta.methods.map((m: any) => [m.opcode, m]));
    expect(byOpcode.get(77)?.name).toBe('wrap');
    expect(byOpcode.get(78)?.name).toBe('unwrap');
    expect(byOpcode.get(105)?.returns).toBe('u64');
  });

  it('getMeta returns the OrbitalInstance ABI for an NFT (2:76000)', async () => {
    // This is the exact scenario from ticket-0483 — a Next.js
    // marketplace wanting to render an orbital NFT instance.
    const raw = await provider.alkanes.getMeta(ORBITAL_INSTANCE_ID);
    const meta = JSON.parse(raw);

    // OrbitalInstance has the NFT-shape methods around opcodes 998-1005:
    //   998 get_collection_identifier
    //   999 get_nft_index
    //   1000 get_data            <-- the image / payload
    //   1001 get_content_type
    //   1002 get_attributes
    //   1003 get_profit
    //   1004 unstake
    //   1005 claim
    const byOpcode = new Map(meta.methods.map((m: any) => [m.opcode, m]));
    expect(byOpcode.get(1000)?.name).toBe('get_data');
    expect(byOpcode.get(1001)?.name).toBe('get_content_type');
    expect(byOpcode.get(1002)?.name).toBe('get_attributes');

    // None of these are tokens — they don't have decimals/totalSupply
    // in the alkanes_meta shape. This is why `metashrew_view "meta"`
    // (which getMeta wraps) is required: the narrow alkanes_meta
    // RPC would only return name/symbol/decimals/totalSupply and
    // miss everything that makes this contract actually queryable.
  });
});
