/**
 * Provider integration for Alkanes SDK
 *
 * Provides a clean TypeScript wrapper over the WebProvider WASM bindings.
 * Compatible with @oyl/sdk Provider interface patterns.
 */

import * as bitcoin from 'bitcoinjs-lib';
import {
  ProviderConfig,
  NetworkType,
  TransactionResult,
  BlockInfo,
  UTXO,
  AddressBalance,
  AlkaneBalance,
  AlkaneId,
} from '../types';

// WASM provider type - loaded dynamically at runtime
type WasmWebProvider = any;

// Network configuration presets
export const NETWORK_PRESETS: Record<string, { rpcUrl: string; dataApiUrl: string; networkType: NetworkType }> = {
  'mainnet': {
    rpcUrl: 'https://mainnet.subfrost.io/v4/subfrost',
    dataApiUrl: 'https://mainnet.subfrost.io/v4/subfrost',
    networkType: 'mainnet',
  },
  'testnet': {
    rpcUrl: 'https://testnet.subfrost.io/v4/subfrost',
    dataApiUrl: 'https://testnet.subfrost.io/v4/subfrost',
    networkType: 'testnet',
  },
  'signet': {
    rpcUrl: 'https://signet.subfrost.io/v4/subfrost',
    dataApiUrl: 'https://signet.subfrost.io/v4/subfrost',
    networkType: 'signet',
  },
  'subfrost-regtest': {
    rpcUrl: 'https://regtest.subfrost.io/v4/subfrost',
    dataApiUrl: 'https://regtest.subfrost.io/v4/subfrost',
    networkType: 'regtest',
  },
  'regtest': {
    rpcUrl: 'http://localhost:18888',
    dataApiUrl: 'http://localhost:18888',
    networkType: 'regtest',
  },
  'local': {
    rpcUrl: 'http://localhost:18888',
    dataApiUrl: 'http://localhost:18888',
    networkType: 'regtest',
  },
};

// Extended provider configuration
export interface AlkanesProviderConfig {
  /** Network type or preset name */
  network: string;
  /** Custom RPC URL (overrides preset) */
  rpcUrl?: string;
  /** Custom Data API URL (overrides preset, defaults to rpcUrl) */
  dataApiUrl?: string;
  /** bitcoinjs-lib network (auto-detected if not provided) */
  bitcoinNetwork?: bitcoin.Network;
}

// Pool details from factory
export interface PoolDetails {
  token0: AlkaneId;
  token1: AlkaneId;
  reserve0: string;
  reserve1: string;
  totalSupply: string;
}

export interface PoolWithDetails {
  poolId: AlkaneId;
  details: PoolDetails | null;
}

// Trade info from data API
export interface TradeInfo {
  txid: string;
  vout: number;
  token0: string;
  token1: string;
  amount0In: string;
  amount1In: string;
  amount0Out: string;
  amount1Out: string;
  reserve0After: string;
  reserve1After: string;
  timestamp: string;
  blockHeight: number;
}

// Candle (OHLCV) data
export interface CandleInfo {
  openTime: string;
  closeTime: string;
  open: string;
  high: string;
  low: string;
  close: string;
  volume0: string;
  volume1: string;
  tradeCount: number;
}

// Holder info
export interface HolderInfo {
  address: string;
  amount: string;
}

// ============================================================================
// ESPO API RESPONSE TYPES
// ============================================================================

/** Paginated response base */
export interface PaginatedResponse {
  ok: boolean;
  page: number;
  limit: number;
  total: number;
  has_more: boolean;
}

/** Address balance entry with outpoint details */
export interface OutpointEntry {
  alkane: string;
  amount: string;
}

export interface OutpointWithEntries {
  outpoint: string;
  entries: OutpointEntry[];
}

/** Response from getAddressBalances */
export interface AddressBalancesResponse {
  ok: boolean;
  address: string;
  balances: Record<string, string>; // alkane_id -> amount
  outpoints?: OutpointWithEntries[];
}

/** Response from getAddressOutpoints */
export interface AddressOutpointsResponse {
  ok: boolean;
  address: string;
  outpoints: OutpointWithEntries[];
}

/** Response from getOutpointBalances */
export interface OutpointBalancesResponse {
  ok: boolean;
  outpoint: string;
  items: OutpointWithEntries[];
}

/** Response from getHolders */
export interface HoldersResponse extends PaginatedResponse {
  alkane: string;
  items: HolderInfo[];
}

/** Response from getHoldersCount */
export interface HoldersCountResponse {
  ok: boolean;
  count: number;
}

/** Storage key entry */
export interface StorageKeyEntry {
  key: string;
  key_hex: string;
  value: string;
  value_hex: string;
}

/** Response from getKeys */
export interface KeysResponse extends PaginatedResponse {
  alkane: string;
  items: StorageKeyEntry[];
}

/** Candle data from AMM */
export interface EspoCandle {
  open_time: string;
  close_time: string;
  open: string;
  high: string;
  low: string;
  close: string;
  volume0: string;
  volume1: string;
  trade_count: number;
}

/** Response from getCandles */
export interface CandlesResponse extends PaginatedResponse {
  pool: string;
  timeframe: string;
  side: string;
  candles: EspoCandle[];
}

/** Trade data from AMM */
export interface EspoTrade {
  txid: string;
  vout: number;
  block_height: number;
  timestamp: string;
  side: string;
  amount_in: string;
  amount_out: string;
  price: string;
}

/** Response from getTrades */
export interface TradesResponse extends PaginatedResponse {
  pool: string;
  side: string;
  filter_side: string;
  sort: string;
  dir: string;
  trades: EspoTrade[];
}

/** Pool info from AMM */
export interface EspoPool {
  pool_id: string;
  token0: string;
  token1: string;
  reserve0: string;
  reserve1: string;
  total_supply: string;
}

/** Response from getPools */
export interface PoolsResponse extends PaginatedResponse {
  pools: EspoPool[];
}

/** Swap hop info */
export interface SwapHop {
  pool: string;
  token_in: string;
  token_out: string;
  amount_in: string;
  amount_out: string;
}

/** Response from findBestSwapPath */
export interface SwapPathResponse {
  ok: boolean;
  mode: string;
  token_in: string;
  token_out: string;
  fee_bps: number;
  max_hops: number;
  amount_in: string;
  amount_out: string;
  hops: SwapHop[];
}

/** Response from getBestMevSwap */
export interface MevSwapResponse {
  ok: boolean;
  token: string;
  fee_bps: number;
  max_hops: number;
  amount_in: string;
  amount_out: string;
  profit: string;
  hops: SwapHop[];
}

// Execute result
export interface ExecuteResult {
  txid: string;
  rawTx: string;
  fee: number;
  size: number;
}

/**
 * Bitcoin RPC client (uses WebProvider internally)
 */
export class BitcoinRpcClient {
  constructor(private provider: WasmWebProvider) {}

  async getBlockCount(): Promise<number> {
    return this.provider.bitcoindGetBlockCount();
  }

  async getBlockHash(height: number): Promise<string> {
    return this.provider.bitcoindGetBlockHash(height);
  }

  async getBlock(hash: string, raw: boolean = false): Promise<any> {
    return this.provider.bitcoindGetBlock(hash, raw);
  }

  async sendRawTransaction(hex: string): Promise<string> {
    return this.provider.bitcoindSendRawTransaction(hex);
  }

  async getTransaction(txid: string, blockHash?: string): Promise<any> {
    return this.provider.bitcoindGetRawTransaction(txid, blockHash);
  }

  async getBlockchainInfo(): Promise<any> {
    return this.provider.bitcoindGetBlockchainInfo();
  }

  async getNetworkInfo(): Promise<any> {
    return this.provider.bitcoindGetNetworkInfo();
  }

  async getMempoolInfo(): Promise<any> {
    return this.provider.bitcoindGetMempoolInfo();
  }

  async estimateSmartFee(target: number): Promise<any> {
    return this.provider.bitcoindEstimateSmartFee(target);
  }

  async generateToAddress(nblocks: number, address: string): Promise<any> {
    return this.provider.bitcoindGenerateToAddress(nblocks, address);
  }
}

/**
 * Esplora API client (uses WebProvider internally)
 */
export class EsploraClient {
  constructor(private provider: WasmWebProvider) {}

  async getAddressInfo(address: string): Promise<any> {
    return this.provider.esploraGetAddressInfo(address);
  }

  async getAddressUtxos(address: string): Promise<UTXO[]> {
    return this.provider.esploraGetAddressUtxo(address);
  }

  async getAddressTxs(address: string): Promise<any[]> {
    return this.provider.esploraGetAddressTxs(address);
  }

  async getTx(txid: string): Promise<any> {
    return this.provider.esploraGetTx(txid);
  }

  async getTxStatus(txid: string): Promise<any> {
    return this.provider.esploraGetTxStatus(txid);
  }

  async getTxHex(txid: string): Promise<string> {
    return this.provider.esploraGetTxHex(txid);
  }

  async getBlocksTipHeight(): Promise<number> {
    return this.provider.esploraGetBlocksTipHeight();
  }

  async getBlocksTipHash(): Promise<string> {
    return this.provider.esploraGetBlocksTipHash();
  }

  async broadcastTx(txHex: string): Promise<string> {
    return this.provider.esploraBroadcastTx(txHex);
  }
}

/**
 * Alkanes RPC client (uses WebProvider internally)
 */
export class AlkanesRpcClient {
  constructor(private provider: WasmWebProvider) {}

  async getBalance(address?: string): Promise<AlkaneBalance[]> {
    return this.provider.alkanesBalance(address);
  }

  async getByAddress(address: string, blockTag?: string, protocolTag?: number): Promise<any> {
    return this.provider.alkanesByAddress(address, blockTag, protocolTag);
  }

  async getByOutpoint(outpoint: string, blockTag?: string, protocolTag?: number): Promise<any> {
    return this.provider.alkanesByOutpoint(outpoint, blockTag, protocolTag);
  }

  async getBytecode(alkaneId: string, blockTag?: string): Promise<string> {
    return this.provider.alkanesBytecode(alkaneId, blockTag);
  }

  async simulate(contractId: string, contextJson: string, blockTag?: string): Promise<any> {
    return this.provider.alkanesSimulate(contractId, contextJson, blockTag);
  }

  async execute(paramsJson: string): Promise<any> {
    return this.provider.alkanesExecute(paramsJson);
  }

  async trace(outpoint: string): Promise<any> {
    return this.provider.alkanesTrace(outpoint);
  }

  async traceBlock(height: number): Promise<any> {
    return this.provider.traceBlock(height);
  }

  async view(contractId: string, viewFn: string, params?: Uint8Array, blockTag?: string): Promise<any> {
    return this.provider.alkanesView(contractId, viewFn, params, blockTag);
  }

  async getAllPools(factoryId: string): Promise<any> {
    return this.provider.alkanesGetAllPools(factoryId);
  }

  async getAllPoolsWithDetails(factoryId: string, chunkSize?: number, maxConcurrent?: number): Promise<PoolWithDetails[]> {
    return this.provider.alkanesGetAllPoolsWithDetails(factoryId, chunkSize, maxConcurrent);
  }

  async getPendingUnwraps(blockTag?: string): Promise<any> {
    return this.provider.alkanesPendingUnwraps(blockTag);
  }
}

/**
 * Metashrew RPC client (uses WebProvider internally)
 *
 * Provides low-level access to metashrew_view RPC calls.
 * For most use cases, prefer the higher-level methods on AlkanesRpcClient.
 */
export class MetashrewClient {
  constructor(private provider: WasmWebProvider) {}

  /**
   * Get current blockchain height
   */
  async getHeight(): Promise<number> {
    return this.provider.metashrewHeight();
  }

  /**
   * Get state root at a specific height
   */
  async getStateRoot(height?: number): Promise<string> {
    return this.provider.metashrewStateRoot(height);
  }

  /**
   * Get block hash at a specific height
   */
  async getBlockHash(height: number): Promise<string> {
    return this.provider.metashrewGetBlockHash(height);
  }

  /**
   * Call a metashrew view function
   *
   * This is the generic low-level method for calling any metashrew_view function.
   *
   * @param viewFn - The view function name (e.g., "simulate", "protorunesbyaddress")
   * @param payload - The hex-encoded payload (with or without 0x prefix)
   * @param blockTag - The block tag ("latest" or a block height as string)
   * @returns The hex-encoded response string
   */
  async view(viewFn: string, payload: string, blockTag: string = 'latest'): Promise<string> {
    return this.provider.metashrewView(viewFn, payload, blockTag);
  }
}

/**
 * Lua script execution result
 */
export interface LuaEvalResult {
  calls: number;
  returns: any;
  runtime: number;
}

/**
 * Lua RPC client (uses WebProvider internally)
 *
 * This client provides Lua script execution with automatic scripthash caching.
 * The luaEval method tries the cached scripthash first (lua_evalsaved),
 * falling back to the full script (lua_evalscript) if the hash isn't cached.
 */
export class LuaClient {
  constructor(private provider: WasmWebProvider) {}

  /**
   * Execute a Lua script with automatic scripthash caching
   *
   * This is the recommended way to execute Lua scripts. It:
   * 1. Computes the SHA256 hash of the script
   * 2. Tries to execute using the cached hash (lua_evalsaved)
   * 3. Falls back to full script execution (lua_evalscript) if not cached
   *
   * @param script - The Lua script content
   * @param args - Arguments to pass to the script
   * @returns The script execution result
   */
  async eval(script: string, args: any[] = []): Promise<LuaEvalResult> {
    return this.provider.luaEval(script, args);
  }

  /**
   * Execute a Lua script directly (no caching)
   *
   * Use this only when you need to bypass the scripthash cache.
   * For most use cases, prefer the eval() method.
   *
   * @param script - The Lua script content
   * @returns The script execution result
   */
  async evalScript(script: string): Promise<any> {
    return this.provider.luaEvalScript(script);
  }
}

/**
 * Data API client (uses WebProvider internally)
 */
export class DataApiClient {
  constructor(private provider: WasmWebProvider) {}

  // Pool operations
  async getPools(factoryId: string): Promise<any> {
    return this.provider.dataApiGetPools(factoryId);
  }

  async getPoolHistory(poolId: string, category?: string, limit?: number, offset?: number): Promise<any> {
    return this.provider.dataApiGetPoolHistory(poolId, category, limit ? BigInt(limit) : undefined, offset ? BigInt(offset) : undefined);
  }

  async getAllHistory(poolId: string, limit?: number, offset?: number): Promise<any> {
    return this.provider.dataApiGetAllHistory(poolId, limit ? BigInt(limit) : undefined, offset ? BigInt(offset) : undefined);
  }

  async getSwapHistory(poolId: string, limit?: number, offset?: number): Promise<any> {
    return this.provider.dataApiGetSwapHistory(poolId, limit ? BigInt(limit) : undefined, offset ? BigInt(offset) : undefined);
  }

  async getMintHistory(poolId: string, limit?: number, offset?: number): Promise<any> {
    return this.provider.dataApiGetMintHistory(poolId, limit ? BigInt(limit) : undefined, offset ? BigInt(offset) : undefined);
  }

  async getBurnHistory(poolId: string, limit?: number, offset?: number): Promise<any> {
    return this.provider.dataApiGetBurnHistory(poolId, limit ? BigInt(limit) : undefined, offset ? BigInt(offset) : undefined);
  }

  // Trading data
  async getTrades(pool: string, startTime?: number, endTime?: number, limit?: number): Promise<TradeInfo[]> {
    return this.provider.dataApiGetTrades(pool, startTime, endTime, limit ? BigInt(limit) : undefined);
  }

  async getCandles(pool: string, interval: string, startTime?: number, endTime?: number, limit?: number): Promise<CandleInfo[]> {
    return this.provider.dataApiGetCandles(pool, interval, startTime, endTime, limit ? BigInt(limit) : undefined);
  }

  async getReserves(pool: string): Promise<any> {
    return this.provider.dataApiGetReserves(pool);
  }

  // Balance operations
  async getAlkanesByAddress(address: string): Promise<any> {
    return this.provider.dataApiGetAlkanesByAddress(address);
  }

  async getAddressBalances(address: string, includeOutpoints: boolean = false): Promise<any> {
    return this.provider.dataApiGetAddressBalances(address, includeOutpoints);
  }

  // Token operations
  async getHolders(alkane: string, page: number = 0, limit: number = 100): Promise<HolderInfo[]> {
    return this.provider.dataApiGetHolders(alkane, BigInt(page), BigInt(limit));
  }

  async getHoldersCount(alkane: string): Promise<number> {
    return this.provider.dataApiGetHoldersCount(alkane);
  }

  async getKeys(alkane: string, prefix?: string, limit: number = 100): Promise<any> {
    return this.provider.dataApiGetKeys(alkane, prefix, BigInt(limit));
  }

  // Market data
  async getBitcoinPrice(): Promise<any> {
    return this.provider.dataApiGetBitcoinPrice();
  }

  async getBitcoinMarketChart(days: string): Promise<any> {
    return this.provider.dataApiGetBitcoinMarketChart(days);
  }
}

/**
 * Convert Map objects (from serde_wasm_bindgen) to plain objects recursively.
 * This is needed because serde_wasm_bindgen serializes JSON as JavaScript Maps.
 */
function mapToObject(value: any): any {
  if (value instanceof Map) {
    const obj: any = {};
    value.forEach((v, k) => {
      obj[k] = mapToObject(v);
    });
    return obj;
  }
  if (Array.isArray(value)) {
    return value.map(mapToObject);
  }
  return value;
}

/**
 * Espo client (uses WebProvider internally)
 *
 * Provides access to Espo indexer for alkanes data and AMM analytics.
 * Methods are organized into two modules:
 * - Essentials: Core alkanes data (balances, holders, storage keys)
 * - AMM Data: Trading and liquidity analytics (candles, trades, pools, swaps)
 */
export class EspoClient {
  constructor(private provider: WasmWebProvider) {}

  // ============================================================================
  // ESSENTIALS MODULE
  // ============================================================================

  /**
   * Get current Espo indexer height
   */
  async getHeight(): Promise<number> {
    return this.provider.espoGetHeight();
  }

  /**
   * Ping the Espo server
   */
  async ping(): Promise<string> {
    return this.provider.espoPing();
  }

  /**
   * Get alkanes balances for an address
   * @param address - Bitcoin address
   * @param includeOutpoints - Include detailed outpoint information
   */
  async getAddressBalances(address: string, includeOutpoints: boolean = false): Promise<AddressBalancesResponse> {
    const result = await this.provider.espoGetAddressBalances(address, includeOutpoints);
    return mapToObject(result);
  }

  /**
   * Get outpoints containing alkanes for an address
   * @param address - Bitcoin address
   */
  async getAddressOutpoints(address: string): Promise<AddressOutpointsResponse> {
    const result = await this.provider.espoGetAddressOutpoints(address);
    return mapToObject(result);
  }

  /**
   * Get alkanes balances at a specific outpoint
   * @param outpoint - Outpoint in format "txid:vout"
   */
  async getOutpointBalances(outpoint: string): Promise<OutpointBalancesResponse> {
    const result = await this.provider.espoGetOutpointBalances(outpoint);
    return mapToObject(result);
  }

  /**
   * Get holders of an alkane token with pagination
   * @param alkaneId - Alkane ID in format "block:tx"
   * @param page - Page number (default: 0)
   * @param limit - Items per page (default: 100)
   */
  async getHolders(alkaneId: string, page: number = 0, limit: number = 100): Promise<HoldersResponse> {
    const result = await this.provider.espoGetHolders(alkaneId, page, limit);
    return mapToObject(result);
  }

  /**
   * Get total holder count for an alkane
   * @param alkaneId - Alkane ID in format "block:tx"
   */
  async getHoldersCount(alkaneId: string): Promise<number> {
    // WASM method returns the count directly as a number
    const result = await this.provider.espoGetHoldersCount(alkaneId);
    return result;
  }

  /**
   * Get storage keys for an alkane contract with pagination
   * @param alkaneId - Alkane ID in format "block:tx"
   * @param page - Page number (default: 0)
   * @param limit - Items per page (default: 100)
   */
  async getKeys(alkaneId: string, page: number = 0, limit: number = 100): Promise<KeysResponse> {
    const result = await this.provider.espoGetKeys(alkaneId, page, limit);
    return mapToObject(result);
  }

  // ============================================================================
  // AMM DATA MODULE
  // ============================================================================

  /**
   * Ping the AMM Data module
   */
  async ammdataPing(): Promise<string> {
    return this.provider.espoAmmdataPing();
  }

  /**
   * Get OHLCV candlestick data for a pool
   * @param pool - Pool ID in format "block:tx"
   * @param timeframe - Candle timeframe: "10m" | "1h" | "1d" | "1w" | "1M"
   * @param side - Price side: "base" | "quote"
   * @param limit - Number of candles (default: 100)
   * @param page - Page number (default: 0)
   */
  async getCandles(
    pool: string,
    timeframe?: string,
    side?: string,
    limit?: number,
    page?: number
  ): Promise<CandlesResponse> {
    const result = await this.provider.espoGetCandles(
      pool,
      timeframe,
      side,
      limit,
      page
    );
    return mapToObject(result);
  }

  /**
   * Get trade history for a pool
   * @param pool - Pool ID in format "block:tx"
   * @param limit - Number of trades (default: 100)
   * @param page - Page number (default: 0)
   * @param side - Price side: "base" | "quote"
   * @param filterSide - Filter by trade side: "buy" | "sell" | "all"
   * @param sort - Sort field
   * @param dir - Sort direction: "asc" | "desc"
   */
  async getTrades(
    pool: string,
    limit?: number,
    page?: number,
    side?: string,
    filterSide?: string,
    sort?: string,
    dir?: string
  ): Promise<TradesResponse> {
    const result = await this.provider.espoGetTrades(
      pool,
      limit,
      page,
      side,
      filterSide,
      sort,
      dir
    );
    return mapToObject(result);
  }

  /**
   * Get all pools with pagination
   * @param limit - Number of pools (default: 100)
   * @param page - Page number (default: 0)
   */
  async getPools(limit?: number, page?: number): Promise<PoolsResponse> {
    const result = await this.provider.espoGetPools(limit, page);
    return mapToObject(result);
  }

  /**
   * Find the best swap path between two tokens
   * @param tokenIn - Input token ID
   * @param tokenOut - Output token ID
   * @param mode - Swap mode: "exact_in" | "exact_out" | "implicit"
   * @param amountIn - Input amount
   * @param amountOut - Output amount
   * @param amountOutMin - Minimum output amount
   * @param amountInMax - Maximum input amount
   * @param availableIn - Available input amount
   * @param feeBps - Fee in basis points
   * @param maxHops - Maximum swap hops
   */
  async findBestSwapPath(
    tokenIn: string,
    tokenOut: string,
    mode?: string,
    amountIn?: string,
    amountOut?: string,
    amountOutMin?: string,
    amountInMax?: string,
    availableIn?: string,
    feeBps?: number,
    maxHops?: number
  ): Promise<SwapPathResponse> {
    const result = await this.provider.espoFindBestSwapPath(
      tokenIn,
      tokenOut,
      mode,
      amountIn,
      amountOut,
      amountOutMin,
      amountInMax,
      availableIn,
      feeBps,
      maxHops
    );
    return mapToObject(result);
  }

  /**
   * Find the best MEV swap opportunity for a token
   * @param token - Token ID
   * @param feeBps - Fee in basis points
   * @param maxHops - Maximum swap hops
   */
  async getBestMevSwap(
    token: string,
    feeBps?: number,
    maxHops?: number
  ): Promise<MevSwapResponse> {
    const result = await this.provider.espoGetBestMevSwap(
      token,
      feeBps,
      maxHops
    );
    return mapToObject(result);
  }
}

/**
 * Main Alkanes Provider
 *
 * Provides a unified interface to all Alkanes functionality:
 * - Bitcoin RPC operations
 * - Esplora API operations
 * - Alkanes smart contract operations
 * - Data API for analytics and trading data
 * - Espo indexer for alkanes data and AMM analytics
 * - Lua script execution with caching
 * - Metashrew low-level RPC access
 */
export class AlkanesProvider {
  private _provider: WasmWebProvider | null = null;
  private _bitcoin: BitcoinRpcClient | null = null;
  private _esplora: EsploraClient | null = null;
  private _alkanes: AlkanesRpcClient | null = null;
  private _dataApi: DataApiClient | null = null;
  private _espo: EspoClient | null = null;
  private _lua: LuaClient | null = null;
  private _metashrew: MetashrewClient | null = null;

  public readonly network: bitcoin.Network;
  public readonly networkType: NetworkType;
  public readonly rpcUrl: string;
  public readonly dataApiUrl: string;
  private readonly networkPreset: string;

  constructor(config: AlkanesProviderConfig) {
    // Resolve network preset
    const preset = NETWORK_PRESETS[config.network] || NETWORK_PRESETS['mainnet'];
    this.networkPreset = config.network;
    this.networkType = preset.networkType;
    this.rpcUrl = config.rpcUrl || preset.rpcUrl;
    this.dataApiUrl = config.dataApiUrl || config.rpcUrl || preset.dataApiUrl;

    // Set bitcoinjs network
    if (config.bitcoinNetwork) {
      this.network = config.bitcoinNetwork;
    } else {
      switch (this.networkType) {
        case 'mainnet':
          this.network = bitcoin.networks.bitcoin;
          break;
        case 'testnet':
        case 'signet':
          this.network = bitcoin.networks.testnet;
          break;
        case 'regtest':
        default:
          this.network = bitcoin.networks.regtest;
      }
    }
  }

  /**
   * Initialize the provider (loads WASM if needed)
   *
   * This method handles cross-platform WASM loading for both Node.js and browser environments.
   */
  async initialize(): Promise<void> {
    if (this._provider) return;

    let WebProviderClass: any;

    // Detect environment and use appropriate loader
    const isNode = typeof process !== 'undefined' &&
      process.versions != null &&
      process.versions.node != null;

    if (isNode) {
      // Node.js: Use the CommonJS loader that manually instantiates WASM
      // Dynamic import of CommonJS module wraps exports in 'default'
      const nodeLoaderModule = await import(/* @vite-ignore */ '@alkanes/ts-sdk/wasm/node-loader.cjs');
      const nodeLoader = nodeLoaderModule.default || nodeLoaderModule;
      await nodeLoader.init();
      WebProviderClass = nodeLoader.WebProvider;
    } else {
      // Browser: Use the ESM module (expects bundler support)
      const wasm = await import(/* @vite-ignore */ '@alkanes/ts-sdk/wasm');
      WebProviderClass = wasm.WebProvider;
    }

    // Create provider with appropriate network name
    const providerName = this.networkPreset === 'local' ? 'regtest' : this.networkPreset;

    // Always pass rpcUrl as config override to ensure it's used
    const configOverride: any = {
      jsonrpc_url: this.rpcUrl
    };

    this._provider = new WebProviderClass(
      providerName,
      configOverride
    );
  }

  /**
   * Get the underlying WASM provider (initializes if needed)
   */
  private async getProvider(): Promise<WasmWebProvider> {
    if (!this._provider) {
      await this.initialize();
    }
    return this._provider!;
  }

  /**
   * Bitcoin RPC client
   */
  get bitcoin(): BitcoinRpcClient {
    if (!this._bitcoin) {
      if (!this._provider) {
        throw new Error('Provider not initialized. Call initialize() first.');
      }
      this._bitcoin = new BitcoinRpcClient(this._provider);
    }
    return this._bitcoin;
  }

  /**
   * Esplora API client
   */
  get esplora(): EsploraClient {
    if (!this._esplora) {
      if (!this._provider) {
        throw new Error('Provider not initialized. Call initialize() first.');
      }
      this._esplora = new EsploraClient(this._provider);
    }
    return this._esplora;
  }

  /**
   * Alkanes RPC client
   */
  get alkanes(): AlkanesRpcClient {
    if (!this._alkanes) {
      if (!this._provider) {
        throw new Error('Provider not initialized. Call initialize() first.');
      }
      this._alkanes = new AlkanesRpcClient(this._provider);
    }
    return this._alkanes;
  }

  /**
   * Data API client
   */
  get dataApi(): DataApiClient {
    if (!this._dataApi) {
      if (!this._provider) {
        throw new Error('Provider not initialized. Call initialize() first.');
      }
      this._dataApi = new DataApiClient(this._provider);
    }
    return this._dataApi;
  }

  /**
   * Espo client
   *
   * Provides access to Espo indexer for alkanes data and AMM analytics.
   * - Essentials module: balances, holders, storage keys
   * - AMM Data module: candles, trades, pools, swap routing
   */
  get espo(): EspoClient {
    if (!this._espo) {
      if (!this._provider) {
        throw new Error('Provider not initialized. Call initialize() first.');
      }
      this._espo = new EspoClient(this._provider);
    }
    return this._espo;
  }

  /**
   * Lua script execution client
   *
   * Provides Lua script execution with automatic scripthash caching.
   * This is the recommended way to execute Lua scripts for optimal performance.
   */
  get lua(): LuaClient {
    if (!this._lua) {
      if (!this._provider) {
        throw new Error('Provider not initialized. Call initialize() first.');
      }
      this._lua = new LuaClient(this._provider);
    }
    return this._lua;
  }

  /**
   * Metashrew RPC client
   *
   * Provides low-level access to metashrew_view RPC calls.
   * For most use cases, prefer the higher-level methods on alkanes or the convenience methods.
   */
  get metashrew(): MetashrewClient {
    if (!this._metashrew) {
      if (!this._provider) {
        throw new Error('Provider not initialized. Call initialize() first.');
      }
      this._metashrew = new MetashrewClient(this._provider);
    }
    return this._metashrew;
  }

  // ============================================================================
  // CONVENIENCE METHODS
  // ============================================================================

  /**
   * Get BTC balance for an address
   */
  async getBalance(address: string): Promise<AddressBalance> {
    const provider = await this.getProvider();
    const info = await provider.esploraGetAddressInfo(address);
    const utxos = await provider.esploraGetAddressUtxo(address);

    return {
      address,
      confirmed: info.chain_stats?.funded_txo_sum - info.chain_stats?.spent_txo_sum || 0,
      unconfirmed: info.mempool_stats?.funded_txo_sum - info.mempool_stats?.spent_txo_sum || 0,
      utxos,
    };
  }

  /**
   * Get enriched balances (BTC + alkanes) for an address
   */
  async getEnrichedBalances(address: string, protocolTag?: string): Promise<any> {
    const provider = await this.getProvider();
    return provider.getEnrichedBalances(address, protocolTag);
  }

  /**
   * Get alkane token balance for an address
   */
  async getAlkaneBalance(address: string, alkaneId?: AlkaneId): Promise<AlkaneBalance[]> {
    const provider = await this.getProvider();
    const balances = await provider.alkanesBalance(address);

    if (alkaneId) {
      // Filter to specific token
      return balances.filter((b: any) =>
        b.id?.block === alkaneId.block && b.id?.tx === alkaneId.tx
      );
    }
    return balances;
  }

  /**
   * Get alkane token details
   */
  async getAlkaneTokenDetails(params: { alkaneId: AlkaneId }): Promise<any> {
    const provider = await this.getProvider();
    const id = `${params.alkaneId.block}:${params.alkaneId.tx}`;

    // Get token info through view call
    const nameResult = await provider.alkanesView(id, 'name', undefined, undefined);
    const symbolResult = await provider.alkanesView(id, 'symbol', undefined, undefined);
    const decimalsResult = await provider.alkanesView(id, 'decimals', undefined, undefined);
    const totalSupplyResult = await provider.alkanesView(id, 'totalSupply', undefined, undefined);

    return {
      id: params.alkaneId,
      name: nameResult?.data || '',
      symbol: symbolResult?.data || '',
      decimals: decimalsResult?.data || 8,
      totalSupply: totalSupplyResult?.data || '0',
    };
  }

  /**
   * Get transaction history for an address (first page, max 25 transactions)
   */
  async getAddressHistory(address: string): Promise<any[]> {
    const provider = await this.getProvider();
    return provider.getAddressTxs(address);
  }

  /**
   * Get transaction history for an address from Esplora (first page, max 25 transactions)
   */
  async getAddressTxs(address: string): Promise<any[]> {
    const provider = await this.getProvider();
    return provider.esploraGetAddressTxs(address);
  }

  /**
   * Get next page of transaction history for an address
   * @param address The address to fetch transactions for
   * @param lastSeenTxid The last transaction ID from the previous page (undefined for first page)
   */
  async getAddressTxsChain(address: string, lastSeenTxid?: string): Promise<any[]> {
    const provider = await this.getProvider();
    return provider.esploraGetAddressTxsChain(address, lastSeenTxid);
  }

  /**
   * Get storage value at a specific path for an alkane
   * @param block - Block number of the alkane
   * @param tx - Transaction number of the alkane
   * @param path - Storage path as bytes (use TextEncoder to convert string to bytes)
   * @returns Hex string (0x-prefixed) of the storage value
   */
  async getStorageAt(block: number, tx: number, path: Uint8Array): Promise<string> {
    const provider = await this.getProvider();
    return provider.getStorageAt(BigInt(block), BigInt(tx), Array.from(path));
  }

  /**
   * Get address history with alkane traces
   */
  async getAddressHistoryWithTraces(address: string, excludeCoinbase?: boolean): Promise<any[]> {
    const provider = await this.getProvider();
    return provider.getAddressTxsWithTraces(address, excludeCoinbase);
  }

  /**
   * Get current block height
   */
  async getBlockHeight(): Promise<number> {
    const provider = await this.getProvider();
    return provider.metashrewHeight();
  }

  /**
   * Broadcast a transaction
   */
  async broadcastTransaction(txHex: string): Promise<string> {
    const provider = await this.getProvider();
    return provider.broadcastTransaction(txHex);
  }

  /**
   * Get all AMM pools from a factory
   */
  async getAllPools(factoryId: string): Promise<PoolWithDetails[]> {
    const provider = await this.getProvider();
    return provider.alkanesGetAllPoolsWithDetails(factoryId, undefined, undefined);
  }

  /**
   * Get pool reserves
   */
  async getPoolReserves(poolId: string): Promise<any> {
    const provider = await this.getProvider();
    return provider.dataApiGetReserves(poolId);
  }

  /**
   * Get recent trades for a pool
   */
  async getPoolTrades(poolId: string, limit?: number): Promise<TradeInfo[]> {
    const provider = await this.getProvider();
    return provider.dataApiGetTrades(poolId, undefined, undefined, limit ? BigInt(limit) : undefined);
  }

  /**
   * Get candle data for a pool
   */
  async getPoolCandles(poolId: string, interval: string = '1h', limit?: number): Promise<CandleInfo[]> {
    const provider = await this.getProvider();
    return provider.dataApiGetCandles(poolId, interval, undefined, undefined, limit ? BigInt(limit) : undefined);
  }

  /**
   * Get Bitcoin price in USD
   */
  async getBitcoinPrice(): Promise<number> {
    const provider = await this.getProvider();
    const result = await provider.dataApiGetBitcoinPrice();
    return result?.price || 0;
  }

  /**
   * Execute an alkanes contract call
   */
  async executeAlkanes(params: {
    contractId: string;
    calldata: number[];
    feeRate?: number;
    inputs?: any[];
  }): Promise<ExecuteResult> {
    const provider = await this.getProvider();
    const paramsJson = JSON.stringify({
      target: params.contractId,
      calldata: params.calldata,
      fee_rate: params.feeRate,
      inputs: params.inputs,
    });
    return provider.alkanesExecute(paramsJson);
  }

  /**
   * Simulate an alkanes contract call (read-only)
   */
  async simulateAlkanes(contractId: string, calldata: number[], blockTag?: string): Promise<any> {
    const provider = await this.getProvider();
    const context = {
      alkanes: [],
      transaction: [],
      block: [],
      height: 0,
      vout: 0,
      txindex: 0,
      calldata,
      pointer: 0,
      refund_pointer: 0,
    };
    return provider.alkanesSimulate(contractId, JSON.stringify(context), blockTag);
  }
}

/**
 * Create an Alkanes provider instance
 *
 * @param config - Provider configuration
 * @returns AlkanesProvider instance
 *
 * @example
 * ```typescript
 * // Use a preset network
 * const provider = await createProvider({ network: 'subfrost-regtest' });
 * await provider.initialize();
 *
 * // Use custom URLs
 * const provider = await createProvider({
 *   network: 'regtest',
 *   rpcUrl: 'http://localhost:18888',
 * });
 * await provider.initialize();
 * ```
 */
export function createProvider(config: AlkanesProviderConfig): AlkanesProvider {
  return new AlkanesProvider(config);
}
