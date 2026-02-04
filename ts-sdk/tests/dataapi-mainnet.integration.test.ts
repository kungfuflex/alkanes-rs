/**
 * @alkanes/ts-sdk DataApi Integration Test (Mainnet)
 *
 * Tests the dataApi namespace methods against live mainnet data.
 * Verifies data structures returned by the Subfrost API.
 *
 * Run with: INTEGRATION=true pnpm vitest run tests/dataapi-mainnet.integration.test.ts
 */

import { describe, it, expect, beforeAll } from 'vitest';
import { AlkanesProvider } from '../src/provider';

const INTEGRATION = process.env.INTEGRATION === 'true';

// Mainnet factory ID and known addresses
const MAINNET_FACTORY_ID = '4:65522';
const KNOWN_MAINNET_ADDRESS = 'bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq'; // Known active address
const DIESEL_ID = '2:0';
const FRBTC_ID = '32:0';

describe.skipIf(!INTEGRATION)('DataApi Integration Tests (Mainnet)', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'mainnet',
      rpcUrl: 'https://mainnet.subfrost.io/v4/subfrost',
      dataApiUrl: 'https://mainnet.subfrost.io/v4/subfrost',
    });
    await provider.initialize();
  });

  describe('dataApi.getAlkanesByAddress()', () => {
    it('should return response for a known address', async () => {
      const result = await provider.dataApi.getAlkanesByAddress(KNOWN_MAINNET_ADDRESS);

      console.log('getAlkanesByAddress result:', JSON.stringify(result, null, 2));

      // Response should be defined (structure may vary)
      expect(result).toBeDefined();

      // Check for alkanes in either result.alkanes or result.data
      const alkanes = result.alkanes || result.data || [];
      console.log('Alkanes found:', alkanes.length);
    });

    it('should handle addresses with no tokens gracefully', async () => {
      // Use a random address that likely has no alkanes
      const emptyAddress = 'bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4';
      const result = await provider.dataApi.getAlkanesByAddress(emptyAddress);

      console.log('Empty address result:', JSON.stringify(result, null, 2));

      expect(result).toBeDefined();
      // Should have data array (empty or populated)
      const alkanes = result.alkanes || result.data || [];
      expect(Array.isArray(alkanes)).toBe(true);
    });
  });

  describe('dataApi.getPools()', () => {
    it('should return pools list from factory', async () => {
      const result = await provider.dataApi.getPools(MAINNET_FACTORY_ID);

      console.log('getPools result structure:', Object.keys(result));
      console.log('First 2 pools:', JSON.stringify((result.pools || result.data)?.slice(0, 2), null, 2));

      expect(result).toBeDefined();

      // Pools in result.pools or result.data
      const pools = result.pools || result.data || [];
      expect(Array.isArray(pools)).toBe(true);
      expect(pools.length).toBeGreaterThan(0);

      console.log(`Total pools: ${pools.length}`);
    });

    it('should have pool IDs with block and tx fields', async () => {
      const result = await provider.dataApi.getPools(MAINNET_FACTORY_ID);
      const pools = result.pools || result.data || [];

      if (pools.length > 0) {
        const pool = pools[0];
        console.log('Sample pool structure:', JSON.stringify(pool, null, 2));

        // Pool should have block/tx or similar identifier
        expect(pool).toBeDefined();
        expect(pool.block || pool.pool_id || pool.id).toBeDefined();
      }
    });
  });

  describe('dataApi.getBitcoinPrice()', () => {
    it('should return current BTC price', async () => {
      const result = await provider.dataApi.getBitcoinPrice();

      console.log('getBitcoinPrice result:', JSON.stringify(result, null, 2));

      expect(result).toBeDefined();

      // Extract price from nested structure
      const priceUsd =
        result.usd ||
        result.price ||
        result.data?.bitcoin?.usd ||
        result.data?.usd ||
        0;

      console.log('BTC Price (USD):', priceUsd);
      expect(priceUsd).toBeGreaterThan(0);
    });
  });
});

describe.skipIf(!INTEGRATION)('DataApi Pool History Endpoints', () => {
  let provider: AlkanesProvider;
  let samplePoolId: string;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'mainnet',
      rpcUrl: 'https://mainnet.subfrost.io/v4/subfrost',
      dataApiUrl: 'https://mainnet.subfrost.io/v4/subfrost',
    });
    await provider.initialize();

    // Get a sample pool ID for testing
    const poolsResult = await provider.dataApi.getPools(MAINNET_FACTORY_ID);
    const pools = poolsResult.pools || poolsResult.data || [];
    if (pools.length > 0) {
      const pool = pools[0];
      samplePoolId = pool.id || `${pool.block}:${pool.tx}`;
    }
  });

  it('should return swap history for a pool', async () => {
    if (!samplePoolId) {
      console.log('No sample pool available, skipping');
      return;
    }

    console.log(`\n=== Testing getSwapHistory for pool ${samplePoolId} ===`);
    const result = await provider.dataApi.getSwapHistory(samplePoolId, 10, 0);

    console.log('getSwapHistory result:', JSON.stringify(result, null, 2).slice(0, 1000));
    expect(result).toBeDefined();
  });

  it('should return mint history for a pool', async () => {
    if (!samplePoolId) {
      console.log('No sample pool available, skipping');
      return;
    }

    console.log(`\n=== Testing getMintHistory for pool ${samplePoolId} ===`);
    const result = await provider.dataApi.getMintHistory(samplePoolId, 10, 0);

    console.log('getMintHistory result:', JSON.stringify(result, null, 2).slice(0, 1000));
    expect(result).toBeDefined();
  });

  it('should return burn history for a pool', async () => {
    if (!samplePoolId) {
      console.log('No sample pool available, skipping');
      return;
    }

    console.log(`\n=== Testing getBurnHistory for pool ${samplePoolId} ===`);
    const result = await provider.dataApi.getBurnHistory(samplePoolId, 10, 0);

    console.log('getBurnHistory result:', JSON.stringify(result, null, 2).slice(0, 1000));
    expect(result).toBeDefined();
  });
});

describe.skipIf(!INTEGRATION)('DataApi getAllPoolsDetails', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'mainnet',
      rpcUrl: 'https://mainnet.subfrost.io/v4/subfrost',
      dataApiUrl: 'https://mainnet.subfrost.io/v4/subfrost',
    });
    await provider.initialize();
  });

  it('should return all pools with details', async () => {
    console.log('\n=== Testing getAllPoolsDetails ===');
    const result = await provider.dataApi.getAllPoolsDetails(MAINNET_FACTORY_ID);

    console.log('getAllPoolsDetails response keys:', Object.keys(result || {}));

    // Extract pools from various possible response structures
    const pools = result?.data?.pools || result?.pools || result?.data || [];
    console.log('Number of pools with details:', Array.isArray(pools) ? pools.length : 'N/A');

    if (Array.isArray(pools) && pools.length > 0) {
      console.log('First pool structure:', JSON.stringify(pools[0], null, 2).slice(0, 1000));
    }

    expect(result).toBeDefined();
  });

  it('should support pagination', async () => {
    console.log('\n=== Testing getAllPoolsDetails with pagination ===');
    const result = await provider.dataApi.getAllPoolsDetails(MAINNET_FACTORY_ID, { limit: 5, offset: 0 });

    console.log('Paginated result:', JSON.stringify(result, null, 2).slice(0, 500));
    expect(result).toBeDefined();
  });
});

describe.skipIf(!INTEGRATION)('DataApi Response Structure Verification', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'mainnet',
      rpcUrl: 'https://mainnet.subfrost.io/v4/subfrost',
      dataApiUrl: 'https://mainnet.subfrost.io/v4/subfrost',
    });
    await provider.initialize();
  });

  it('documents getAlkanesByAddress response structure', async () => {
    const result = await provider.dataApi.getAlkanesByAddress(KNOWN_MAINNET_ADDRESS);

    console.log('\n=== getAlkanesByAddress Response Structure ===');
    console.log('Top-level keys:', Object.keys(result));
    console.log('Full response:', JSON.stringify(result, null, 2).slice(0, 2000));

    // Document the actual structure for future reference
    expect(result).toBeDefined();
  });

  it('documents getPools response structure', async () => {
    const result = await provider.dataApi.getPools(MAINNET_FACTORY_ID);

    console.log('\n=== getPools Response Structure ===');
    console.log('Top-level keys:', Object.keys(result));

    const pools = result.pools || result.data || [];
    console.log('Number of pools:', pools.length);

    if (pools.length > 0) {
      console.log('Sample pool keys:', Object.keys(pools[0]));
      console.log('First pool:', JSON.stringify(pools[0], null, 2));
    }

    expect(pools.length).toBeGreaterThan(0);
  });

  it('documents getBitcoinPrice response structure', async () => {
    const result = await provider.dataApi.getBitcoinPrice();

    console.log('\n=== getBitcoinPrice Response Structure ===');
    console.log('Top-level keys:', Object.keys(result));
    console.log('Full response:', JSON.stringify(result, null, 2));

    expect(result).toBeDefined();
  });
});

describe.skipIf(!INTEGRATION)('DataApi Usage Pattern for subfrost-app', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'mainnet',
      rpcUrl: 'https://mainnet.subfrost.io/v4/subfrost',
      dataApiUrl: 'https://mainnet.subfrost.io/v4/subfrost',
    });
    await provider.initialize();
  });

  it('demonstrates correct pattern for fetching user alkanes', async () => {
    const address = KNOWN_MAINNET_ADDRESS;
    const result = await provider.dataApi.getAlkanesByAddress(address);

    console.log('\n=== Pattern: Fetching User Alkanes ===');

    // Handle both possible response structures
    const alkanes = result.alkanes || result.data || [];

    console.log(`Address: ${address}`);
    console.log(`Alkanes found: ${alkanes.length}`);

    for (const alkane of alkanes) {
      // Handle different field names
      const id = alkane.id || `${alkane.block}:${alkane.tx}` || alkane.alkaneId;
      const balance = alkane.balance || alkane.amount || '0';
      console.log(`  Token ${id}: ${balance}`);
    }

    expect(Array.isArray(alkanes)).toBe(true);
  });

  it('demonstrates correct pattern for fetching pools', async () => {
    const result = await provider.dataApi.getPools(MAINNET_FACTORY_ID);

    console.log('\n=== Pattern: Fetching Pools ===');

    // Handle both possible response structures
    const pools = result.pools || result.data || [];

    console.log(`Factory: ${MAINNET_FACTORY_ID}`);
    console.log(`Total pools: ${pools.length}`);

    // Show first 5 pools
    for (const pool of pools.slice(0, 5)) {
      const id = pool.pool_id || pool.id || `${pool.block}:${pool.tx}`;
      console.log(`  Pool: ${id}`);
    }

    expect(pools.length).toBeGreaterThan(0);
  });

  it('demonstrates correct pattern for fetching BTC price', async () => {
    const result = await provider.dataApi.getBitcoinPrice();

    console.log('\n=== Pattern: Fetching BTC Price ===');

    // Extract price from possibly nested structure
    let priceUsd: number;
    if (typeof result.usd === 'number') {
      priceUsd = result.usd;
    } else if (result.data?.bitcoin?.usd) {
      priceUsd = result.data.bitcoin.usd;
    } else if (result.data?.usd) {
      priceUsd = result.data.usd;
    } else if (result.price) {
      priceUsd = result.price;
    } else {
      priceUsd = 0;
    }

    console.log(`BTC/USD: $${priceUsd.toLocaleString()}`);

    // Example: Convert sats to USD
    const sats = 100000;
    const btc = sats / 100_000_000;
    const usd = btc * priceUsd;
    console.log(`${sats.toLocaleString()} sats = $${usd.toFixed(2)} USD`);

    expect(priceUsd).toBeGreaterThan(0);
  });
});

describe.skipIf(!INTEGRATION)('Espo Client Integration Tests', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'mainnet',
      rpcUrl: 'https://mainnet.subfrost.io/v4/subfrost',
      dataApiUrl: 'https://mainnet.subfrost.io/v4/subfrost',
      // espoRpcUrl should default to https://mainnet.subfrost.io/v4/subfrost/espo
    });
    await provider.initialize();
  });

  it('espo.getHeight() should return current indexer height', async () => {
    console.log('\n=== Testing espo.getHeight() ===');
    const height = await provider.espo.getHeight();

    console.log('Espo height:', height);
    expect(typeof height).toBe('number');
    expect(height).toBeGreaterThan(0);
  });

  it('espo.ping() should return pong', async () => {
    console.log('\n=== Testing espo.ping() ===');
    const result = await provider.espo.ping();

    console.log('Espo ping result:', result);
    expect(result).toBeDefined();
  });

  it('espo.getHolders() should return token holders', async () => {
    console.log('\n=== Testing espo.getHolders() for DIESEL ===');
    const result = await provider.espo.getHolders(DIESEL_ID, 0, 10);

    console.log('Holders response:', JSON.stringify(result, null, 2).slice(0, 1000));
    expect(result).toBeDefined();
  });

  it('espo.getHoldersCount() should return holder count', async () => {
    console.log('\n=== Testing espo.getHoldersCount() for DIESEL ===');
    const count = await provider.espo.getHoldersCount(DIESEL_ID);

    console.log('DIESEL holder count:', count);
    expect(typeof count).toBe('number');
  });

  it('espo.getCandles() should return OHLCV data', async () => {
    // Get a pool ID first
    const poolsResult = await provider.dataApi.getPools(MAINNET_FACTORY_ID);
    const pools = poolsResult.pools || poolsResult.data || [];

    if (pools.length > 0) {
      const pool = pools[0];
      const poolId = `${pool.block}:${pool.tx}`;

      console.log(`\n=== Testing espo.getCandles() for pool ${poolId} ===`);
      const result = await provider.espo.getCandles(poolId, '1h', 'base', 10, 0);

      console.log('Candles response:', JSON.stringify(result, null, 2).slice(0, 1000));
      expect(result).toBeDefined();
    } else {
      console.log('No pools available for candle test');
    }
  });

  it('espo.getPools() should return pools via JSON-RPC', async () => {
    console.log('\n=== Testing espo.getPools() via JSON-RPC ===');
    const result = await provider.espo.getPools(10, 0);

    console.log('Espo getPools response:', JSON.stringify(result, null, 2).slice(0, 1000));
    expect(result).toBeDefined();
  });
});

describe.skipIf(!INTEGRATION)('WebProvider Direct Methods (Alternative)', () => {
  let provider: AlkanesProvider;

  beforeAll(async () => {
    provider = new AlkanesProvider({
      network: 'mainnet',
      rpcUrl: 'https://mainnet.subfrost.io/v4/subfrost',
      dataApiUrl: 'https://mainnet.subfrost.io/v4/subfrost',
    });
    await provider.initialize();
  });

  it('can use WebProvider.dataApiGetAlkanesByAddress directly', async () => {
    // This is how subfrost-app currently uses it (raw WebProvider)
    const webProvider = (provider as any)._provider;

    if (webProvider && typeof webProvider.dataApiGetAlkanesByAddress === 'function') {
      const result = await webProvider.dataApiGetAlkanesByAddress(KNOWN_MAINNET_ADDRESS);

      console.log('\n=== WebProvider.dataApiGetAlkanesByAddress ===');
      console.log('Result:', JSON.stringify(result, null, 2).slice(0, 1000));

      expect(result).toBeDefined();
    } else {
      console.log('WebProvider not available or method not found');
    }
  });

  it('can use WebProvider.dataApiGetPools directly', async () => {
    const webProvider = (provider as any)._provider;

    if (webProvider && typeof webProvider.dataApiGetPools === 'function') {
      const result = await webProvider.dataApiGetPools(MAINNET_FACTORY_ID);

      console.log('\n=== WebProvider.dataApiGetPools ===');
      console.log('Result keys:', Object.keys(result || {}));

      expect(result).toBeDefined();
    } else {
      console.log('WebProvider not available or method not found');
    }
  });

  it('can use WebProvider.dataApiGetBitcoinPrice directly', async () => {
    const webProvider = (provider as any)._provider;

    if (webProvider && typeof webProvider.dataApiGetBitcoinPrice === 'function') {
      const result = await webProvider.dataApiGetBitcoinPrice();

      console.log('\n=== WebProvider.dataApiGetBitcoinPrice ===');
      console.log('Result:', JSON.stringify(result, null, 2));

      expect(result).toBeDefined();
    } else {
      console.log('WebProvider not available or method not found');
    }
  });
});
