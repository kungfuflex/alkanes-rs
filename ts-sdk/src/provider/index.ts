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
 * Main Alkanes Provider
 *
 * Provides a unified interface to all Alkanes functionality:
 * - Bitcoin RPC operations
 * - Esplora API operations
 * - Alkanes smart contract operations
 * - Data API for analytics and trading data
 */
export class AlkanesProvider {
  private _provider: WasmWebProvider | null = null;
  private _bitcoin: BitcoinRpcClient | null = null;
  private _esplora: EsploraClient | null = null;
  private _alkanes: AlkanesRpcClient | null = null;
  private _dataApi: DataApiClient | null = null;

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
   */
  async initialize(): Promise<void> {
    if (this._provider) return;

    // Dynamic import of WASM module
    // Path is relative to the ts-sdk package root
    const wasm = await import('@alkanes/ts-sdk/wasm');

    // Create provider with appropriate network name
    const providerName = this.networkPreset === 'local' ? 'regtest' : this.networkPreset;

    // Create config override if custom URLs provided
    const configOverride: any = {};
    if (this.rpcUrl !== NETWORK_PRESETS[this.networkPreset]?.rpcUrl) {
      configOverride.sandshrew_rpc_url = this.rpcUrl;
    }

    this._provider = new wasm.WebProvider(
      providerName,
      Object.keys(configOverride).length > 0 ? configOverride : undefined
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
   * Get transaction history for an address
   */
  async getAddressHistory(address: string): Promise<any[]> {
    const provider = await this.getProvider();
    return provider.getAddressTxs(address);
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
