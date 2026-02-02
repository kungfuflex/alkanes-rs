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

import {
  // Bitcoin RPC response types
  BlockchainInfo,
  NetworkInfo,
  MempoolInfo,
  BitcoinTransaction,
  BitcoinBlock,
  BitcoinBlockHeader,
  SmartFeeEstimate,
  ChainTip,
  // Esplora response types
  EsploraAddressInfo,
  EsploraUtxo,
  EsploraTransaction,
  EsploraBlock,
  FeeEstimates,
  Outspend,
  MerkleProof,
  MempoolStats,
  MempoolRecentTx,
  TxStatus,
  // Alkanes response types
  AlkaneBalanceResponse,
  AlkaneReflectResponse,
  AlkanesByAddressResponse,
  AlkaneSpendablesResponse,
  AlkaneSimulateResponse,
  AlkaneTraceResponse,
  AlkaneTraceEntry,
  AlkaneSequenceResponse,
  AlkanePoolResponse,
  SimulationContext,
  // Ord response types
  InscriptionResponse,
  InscriptionsListResponse,
  RuneResponse,
  OrdOutput,
  OrdBlockInfo,
  // BRC20-Prog response types
  Brc20ProgBalance,
  Brc20ProgTxReceipt,
  Brc20ProgTransaction,
  Brc20ProgBlock,
  // Data API response types
  DataApiReserves,
  DataApiPoolHistoryEvent,
  DataApiPoolsResponse,
  DataApiStorageKey,
  DataApiAddressAlkanes,
  BitcoinPriceResponse,
  MarketChartResponse,
} from '../types/responses';

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

/**
 * Log level for SDK operations
 * - 'off': No logging (default)
 * - 'error': Only errors
 * - 'warn': Errors and warnings
 * - 'info': Errors, warnings, and info messages
 * - 'debug': All messages including debug output
 * - 'trace': Most verbose, includes WASM internals
 */
export type LogLevel = 'off' | 'error' | 'warn' | 'info' | 'debug' | 'trace';

// Extended provider configuration
export interface AlkanesProviderConfig {
  /** Network type or preset name */
  network: string;
  /** Custom RPC URL (overrides preset) */
  rpcUrl?: string;
  /** Custom Bitcoin RPC URL (overrides rpcUrl for Bitcoin Core calls) */
  bitcoinRpcUrl?: string;
  /** Custom Metashrew RPC URL (overrides rpcUrl for Metashrew calls) */
  metashrewRpcUrl?: string;
  /** Custom Data API URL (overrides preset, defaults to rpcUrl) */
  dataApiUrl?: string;
  /** bitcoinjs-lib network (auto-detected if not provided) */
  bitcoinNetwork?: bitcoin.Network;
  /**
   * Log level for SDK operations.
   * Can also be set via RUST_LOG or ALKANES_LOG_LEVEL environment variables.
   * Priority: config > ALKANES_LOG_LEVEL > RUST_LOG > 'off'
   */
  logLevel?: LogLevel;
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

// Execute params for AlkanesRpcClient.execute()
export interface RpcExecuteParams {
  /** Target contract ID in "block:tx" format */
  target: string;
  /** Calldata as array of bytes */
  calldata: number[];
  /** Fee rate in sat/vB (optional) */
  fee_rate?: number;
  /** Input UTXOs to use (optional) */
  inputs?: any[];
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

  async getBlock(hash: string, raw: boolean = false): Promise<BitcoinBlock> {
    const result = await this.provider.bitcoindGetBlock(hash, raw);
    return mapToObject(result);
  }

  async sendRawTransaction(hex: string): Promise<string> {
    return this.provider.bitcoindSendRawTransaction(hex);
  }

  async getTransaction(txid: string, blockHash?: string): Promise<BitcoinTransaction> {
    const result = await this.provider.bitcoindGetRawTransaction(txid, blockHash);
    return mapToObject(result);
  }

  async getBlockchainInfo(): Promise<BlockchainInfo> {
    const result = await this.provider.bitcoindGetBlockchainInfo();
    return mapToObject(result);
  }

  async getNetworkInfo(): Promise<NetworkInfo> {
    const result = await this.provider.bitcoindGetNetworkInfo();
    return mapToObject(result);
  }

  async getMempoolInfo(): Promise<MempoolInfo> {
    const result = await this.provider.bitcoindGetMempoolInfo();
    return mapToObject(result);
  }

  async estimateSmartFee(target: number): Promise<SmartFeeEstimate> {
    const result = await this.provider.bitcoindEstimateSmartFee(target);
    return mapToObject(result);
  }

  async generateToAddress(nblocks: number, address: string): Promise<string[]> {
    const result = await this.provider.bitcoindGenerateToAddress(nblocks, address);
    return mapToObject(result);
  }

  async generateFuture(address: string): Promise<string[]> {
    const result = await this.provider.bitcoindGenerateFuture(address);
    return mapToObject(result);
  }

  async getBlockHeader(hash: string): Promise<BitcoinBlockHeader> {
    const result = await this.provider.bitcoindGetBlockHeader(hash);
    return mapToObject(result);
  }

  async getBlockStats(hash: string): Promise<Record<string, number>> {
    const result = await this.provider.bitcoindGetBlockStats(hash);
    return mapToObject(result);
  }

  async getChainTips(): Promise<ChainTip[]> {
    const result = await this.provider.bitcoindGetChainTips();
    return mapToObject(result);
  }

  async getRawMempool(): Promise<string[]> {
    const result = await this.provider.bitcoindGetRawMempool();
    return mapToObject(result);
  }

  async getTxOut(txid: string, vout: number, includeMempool?: boolean): Promise<{
    bestblock: string;
    confirmations: number;
    value: number;
    scriptPubKey: { asm: string; hex: string; type: string; address?: string };
    coinbase: boolean;
  } | null> {
    const result = await this.provider.bitcoindGetTxOut(txid, vout, includeMempool);
    return mapToObject(result);
  }

  async decodeRawTransaction(hex: string): Promise<BitcoinTransaction> {
    const result = await this.provider.bitcoindDecodeRawTransaction(hex);
    return mapToObject(result);
  }

  async decodePsbt(psbt: string): Promise<{
    tx: BitcoinTransaction;
    unknown: Record<string, string>;
    inputs: any[];
    outputs: any[];
    fee?: number;
  }> {
    const result = await this.provider.bitcoindDecodePsbt(psbt);
    return mapToObject(result);
  }
}

/**
 * Esplora API client (uses WebProvider internally)
 */
export class EsploraClient {
  constructor(private provider: WasmWebProvider) {}

  async getAddressInfo(address: string): Promise<EsploraAddressInfo> {
    const result = await this.provider.esploraGetAddressInfo(address);
    return mapToObject(result);
  }

  async getAddressUtxos(address: string): Promise<EsploraUtxo[]> {
    const result = await this.provider.esploraGetAddressUtxo(address);
    return mapToObject(result);
  }

  async getAddressTxs(address: string): Promise<EsploraTransaction[]> {
    const result = await this.provider.esploraGetAddressTxs(address);
    return mapToObject(result);
  }

  async getTx(txid: string): Promise<EsploraTransaction> {
    const result = await this.provider.esploraGetTx(txid);
    return mapToObject(result);
  }

  async getTxStatus(txid: string): Promise<TxStatus> {
    const result = await this.provider.esploraGetTxStatus(txid);
    return mapToObject(result);
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

  async getFeeEstimates(): Promise<FeeEstimates> {
    const result = await this.provider.esploraGetFeeEstimates();
    return mapToObject(result);
  }

  async getBlocks(startHeight?: number): Promise<EsploraBlock[]> {
    const result = await this.provider.esploraGetBlocks(startHeight);
    return mapToObject(result);
  }

  async getBlockByHeight(height: number): Promise<EsploraBlock> {
    const result = await this.provider.esploraGetBlockByHeight(height);
    return mapToObject(result);
  }

  async getBlock(hash: string): Promise<EsploraBlock> {
    const result = await this.provider.esploraGetBlock(hash);
    return mapToObject(result);
  }

  async getBlockStatus(hash: string): Promise<{ in_best_chain: boolean; height?: number; next_best?: string }> {
    const result = await this.provider.esploraGetBlockStatus(hash);
    return mapToObject(result);
  }

  async getBlockTxids(hash: string): Promise<string[]> {
    return this.provider.esploraGetBlockTxids(hash);
  }

  async getBlockHeader(hash: string): Promise<string> {
    const result = await this.provider.esploraGetBlockHeader(hash);
    return mapToObject(result);
  }

  async getBlockRaw(hash: string): Promise<Uint8Array> {
    return this.provider.esploraGetBlockRaw(hash);
  }

  async getBlockTxid(hash: string, index: number): Promise<string> {
    return this.provider.esploraGetBlockTxid(hash, index);
  }

  async getBlockTxs(hash: string, startIndex?: number): Promise<EsploraTransaction[]> {
    const result = await this.provider.esploraGetBlockTxs(hash, startIndex);
    return mapToObject(result);
  }

  async getAddressTxsChain(address: string, lastSeenTxid?: string): Promise<EsploraTransaction[]> {
    const result = await this.provider.esploraGetAddressTxsChain(address, lastSeenTxid);
    return mapToObject(result);
  }

  async getAddressTxsMempool(address: string): Promise<EsploraTransaction[]> {
    const result = await this.provider.esploraGetAddressTxsMempool(address);
    return mapToObject(result);
  }

  async getAddressPrefix(prefix: string): Promise<string[]> {
    const result = await this.provider.esploraGetAddressPrefix(prefix);
    return mapToObject(result);
  }

  async getTxRaw(txid: string): Promise<Uint8Array> {
    return this.provider.esploraGetTxRaw(txid);
  }

  async getTxMerkleProof(txid: string): Promise<MerkleProof> {
    const result = await this.provider.esploraGetTxMerkleProof(txid);
    return mapToObject(result);
  }

  async getTxMerkleblockProof(txid: string): Promise<string> {
    const result = await this.provider.esploraGetTxMerkleblockProof(txid);
    return mapToObject(result);
  }

  async getTxOutspend(txid: string, index: number): Promise<Outspend> {
    const result = await this.provider.esploraGetTxOutspend(txid, index);
    return mapToObject(result);
  }

  async getTxOutspends(txid: string): Promise<Outspend[]> {
    const result = await this.provider.esploraGetTxOutspends(txid);
    return mapToObject(result);
  }

  async getMempool(): Promise<MempoolStats> {
    const result = await this.provider.esploraGetMempool();
    return mapToObject(result);
  }

  async getMempoolTxids(): Promise<string[]> {
    return this.provider.esploraGetMempoolTxids();
  }

  async getMempoolRecent(): Promise<MempoolRecentTx[]> {
    const result = await this.provider.esploraGetMempoolRecent();
    return mapToObject(result);
  }
}

/**
 * Alkanes RPC client (uses WebProvider internally)
 */
export class AlkanesRpcClient {
  constructor(private provider: WasmWebProvider) {}

  async getBalance(address?: string): Promise<AlkaneBalanceResponse[]> {
    const result = await this.provider.alkanesBalance(address);
    return mapToObject(result);
  }

  async getByAddress(address: string, blockTag?: string, protocolTag?: number): Promise<AlkanesByAddressResponse> {
    const result = await this.provider.alkanesByAddress(address, blockTag, protocolTag);
    return mapToObject(result);
  }

  async getByOutpoint(outpoint: string, blockTag?: string, protocolTag?: number): Promise<{
    outpoint: string;
    alkanes: AlkaneBalanceResponse[];
    value?: number;
  }> {
    const result = await this.provider.alkanesByOutpoint(outpoint, blockTag, protocolTag);
    return mapToObject(result);
  }

  async getBytecode(alkaneId: string, blockTag?: string): Promise<string> {
    return this.provider.alkanesBytecode(alkaneId, blockTag);
  }

  /**
   * Get metadata (ABI) for an alkanes contract
   *
   * @param alkaneId - Alkane ID in "block:tx" format (e.g., "2:0")
   * @param blockTag - Optional block tag for historical queries
   * @returns Contract metadata as JSON string or hex
   *
   * @example
   * ```typescript
   * const meta = await provider.alkanes.getMeta('2:0');
   * console.log('Contract metadata:', meta);
   * ```
   */
  async getMeta(alkaneId: string, blockTag?: string): Promise<string> {
    return this.provider.alkanesMeta(alkaneId, blockTag);
  }

  /**
   * Simulate an Alkanes contract call (read-only)
   *
   * @param contractId - Contract ID in "block:tx" format (e.g., "2:0")
   * @param context - Simulation context (object or JSON string for backward compatibility)
   * @param blockTag - Optional block tag for historical simulation
   * @returns Simulation result
   *
   * @example
   * ```typescript
   * const result = await provider.alkanes.simulate('2:0', {
   *   alkanes: [],
   *   transaction: [],
   *   block: [],
   *   height: 800000,
   *   vout: 0,
   *   txindex: 0,
   *   calldata: [77],  // opcode 77 = mint
   *   pointer: 0,
   *   refund_pointer: 0,
   * });
   * ```
   */
  async simulate(contractId: string, context: SimulationContext | string, blockTag?: string): Promise<AlkaneSimulateResponse> {
    // Accept both object and JSON string for backward compatibility
    const contextJson = typeof context === 'string' ? context : JSON.stringify(context);
    const result = await this.provider.alkanesSimulate(contractId, contextJson, blockTag);
    return mapToObject(result);
  }

  /**
   * Execute an Alkanes contract call
   *
   * @param params - Execute parameters (object or JSON string for backward compatibility)
   * @returns Execution result with txid
   *
   * @example
   * ```typescript
   * const result = await provider.alkanes.execute({
   *   target: '2:0',
   *   calldata: [77],  // opcode 77 = mint
   *   fee_rate: 10,
   * });
   * console.log('TXID:', result.txid);
   * ```
   */
  async execute(params: RpcExecuteParams | string): Promise<ExecuteResult> {
    // Accept both object and JSON string for backward compatibility
    const paramsJson = typeof params === 'string' ? params : JSON.stringify(params);
    const result = await this.provider.alkanesExecute(paramsJson);
    return mapToObject(result);
  }

  async trace(outpoint: string): Promise<AlkaneTraceResponse> {
    const result = await this.provider.alkanesTrace(outpoint);
    return mapToObject(result);
  }

  async traceBlock(height: number): Promise<AlkaneTraceResponse[]> {
    const result = await this.provider.traceBlock(height);
    return mapToObject(result);
  }

  async view(contractId: string, viewFn: string, params?: Uint8Array, blockTag?: string): Promise<{ data?: any; error?: string }> {
    const result = await this.provider.alkanesView(contractId, viewFn, params, blockTag);
    return mapToObject(result);
  }

  async getAllPools(factoryId: string): Promise<string[]> {
    const result = await this.provider.alkanesGetAllPools(factoryId);
    return mapToObject(result);
  }

  async getAllPoolsWithDetails(factoryId: string, chunkSize?: number, maxConcurrent?: number): Promise<PoolWithDetails[]> {
    const result = await this.provider.alkanesGetAllPoolsWithDetails(factoryId, chunkSize, maxConcurrent);
    return mapToObject(result);
  }

  async getPendingUnwraps(blockTag?: string): Promise<{
    unwraps: Array<{ txid: string; vout: number; amount: string; recipient: string }>;
  }> {
    const result = await this.provider.alkanesPendingUnwraps(blockTag);
    return mapToObject(result);
  }

  async reflect(alkaneId: string): Promise<AlkaneReflectResponse> {
    const result = await this.provider.alkanesReflect(alkaneId);
    return mapToObject(result);
  }

  async getSequence(blockTag?: string): Promise<AlkaneSequenceResponse> {
    const result = await this.provider.alkanesSequence(blockTag);
    return mapToObject(result);
  }

  async getSpendables(address: string): Promise<AlkaneSpendablesResponse> {
    const result = await this.provider.alkanesSpendables(address);
    return mapToObject(result);
  }

  async getPoolDetails(poolId: string): Promise<AlkanePoolResponse> {
    const result = await this.provider.alkanesPoolDetails(poolId);
    return mapToObject(result);
  }

  async reflectAlkaneRange(block: number, startTx: number, endTx: number): Promise<AlkaneReflectResponse[]> {
    const result = await this.provider.alkanesReflectAlkaneRange(block, startTx, endTx);
    return mapToObject(result);
  }

  async inspect(target: string, config: any): Promise<{
    storage?: Record<string, string>;
    balances?: AlkaneBalanceResponse[];
    metadata?: AlkaneReflectResponse;
  }> {
    const result = await this.provider.alkanesInspect(target, config);
    return mapToObject(result);
  }

  /**
   * Inspect alkanes bytecode directly from WASM bytes.
   * This allows inspection without fetching from RPC - useful for local/offline analysis.
   *
   * @param bytecodeHex - The WASM bytecode as hex string (with or without 0x prefix)
   * @param alkaneId - The alkane ID in format "block:tx"
   * @param config - Inspection configuration
   * @returns Inspection result with codehash, disassembly, metadata, and fuzzing results
   */
  async inspectBytecode(bytecodeHex: string, alkaneId: string, config: {
    disasm?: boolean;
    fuzz?: boolean;
    fuzz_ranges?: string;
    meta?: boolean;
    codehash?: boolean;
    raw?: boolean;
  }): Promise<{
    alkane_id: { block: number; tx: number };
    bytecode_length: number;
    codehash?: string;
    disassembly?: string;
    metadata?: {
      name: string;
      version: string;
      description?: string;
      methods: Array<{
        name: string;
        opcode: number;
        params: string[];
        returns: string;
      }>;
    };
    metadata_error?: string;
    fuzzing_results?: {
      total_opcodes_tested: number;
      opcodes_filtered_out: number;
      successful_executions: number;
      failed_executions: number;
      implemented_opcodes: number[];
      opcode_results: Array<{
        success: boolean;
        return_value?: number;
        return_data: number[];
        error?: string;
        execution_time_micros: number;
        opcode: number;
        host_calls: Array<{
          function_name: string;
          parameters: string[];
          result: string;
          timestamp_micros: number;
        }>;
      }>;
    };
  }> {
    const result = await this.provider.alkanesInspectBytecode(bytecodeHex, alkaneId, config);
    return mapToObject(result);
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
 * Ord (Ordinals) RPC client (uses WebProvider internally)
 */
export class OrdClient {
  constructor(private provider: WasmWebProvider) {}

  async getInscription(id: string): Promise<InscriptionResponse> {
    const result = await this.provider.ordInscription(id);
    return mapToObject(result);
  }

  async getInscriptions(page?: number): Promise<InscriptionsListResponse> {
    const result = await this.provider.ordInscriptions(page);
    return mapToObject(result);
  }

  async getOutputs(address: string): Promise<OrdOutput[]> {
    const result = await this.provider.ordOutputs(address);
    return mapToObject(result);
  }

  async getRune(name: string): Promise<RuneResponse> {
    const result = await this.provider.ordRune(name);
    return mapToObject(result);
  }

  async list(outpoint: string): Promise<OrdOutput> {
    const result = await this.provider.ordList(outpoint);
    return mapToObject(result);
  }

  async find(sat: number): Promise<{ outpoint: string; offset: number }> {
    const result = await this.provider.ordFind(sat);
    return mapToObject(result);
  }

  async getAddressInfo(address: string): Promise<{
    outputs: OrdOutput[];
    inscriptions: string[];
    sat_balance: number;
    runes_balances: Record<string, { amount: string; divisibility: number; symbol?: string }>;
  }> {
    const result = await this.provider.ordAddressInfo(address);
    return mapToObject(result);
  }

  async getBlockInfo(query: string): Promise<OrdBlockInfo> {
    const result = await this.provider.ordBlockInfo(query);
    return mapToObject(result);
  }

  async getBlockCount(): Promise<number> {
    return this.provider.ordBlockCount();
  }

  async getBlocks(): Promise<OrdBlockInfo[]> {
    const result = await this.provider.ordBlocks();
    return mapToObject(result);
  }

  async getChildren(inscriptionId: string, page?: number): Promise<InscriptionsListResponse> {
    const result = await this.provider.ordChildren(inscriptionId, page);
    return mapToObject(result);
  }

  async getContent(inscriptionId: string): Promise<{ content_type: string; content: Uint8Array }> {
    const result = await this.provider.ordContent(inscriptionId);
    return mapToObject(result);
  }

  async getParents(inscriptionId: string, page?: number): Promise<InscriptionsListResponse> {
    const result = await this.provider.ordParents(inscriptionId, page);
    return mapToObject(result);
  }

  async getTxInfo(txid: string): Promise<{
    txid: string;
    inscriptions: string[];
    runes: Record<string, { amount: string; divisibility: number; symbol?: string }>;
  }> {
    const result = await this.provider.ordTxInfo(txid);
    return mapToObject(result);
  }
}

/**
 * BRC-20 Prog (Programmable BRC-20) RPC client (uses WebProvider internally)
 */
export class Brc20ProgClient {
  constructor(private provider: WasmWebProvider) {}

  async getBalance(address: string): Promise<Brc20ProgBalance> {
    const result = await this.provider.brc20progGetBalance(address);
    return mapToObject(result);
  }

  async getCode(address: string): Promise<string> {
    const result = await this.provider.brc20progGetCode(address);
    return mapToObject(result);
  }

  async getBlockNumber(): Promise<number> {
    return this.provider.brc20progBlockNumber();
  }

  async getChainId(): Promise<number> {
    return this.provider.brc20progChainId();
  }

  async getTxReceipt(hash: string): Promise<Brc20ProgTxReceipt | null> {
    const result = await this.provider.brc20progGetTransactionReceipt(hash);
    return mapToObject(result);
  }

  async getTx(hash: string): Promise<Brc20ProgTransaction | null> {
    const result = await this.provider.brc20progGetTransactionByHash(hash);
    return mapToObject(result);
  }

  async getBlock(number: string | number, includeTxs?: boolean): Promise<Brc20ProgBlock | null> {
    const result = await this.provider.brc20progGetBlockByNumber(String(number), includeTxs);
    return mapToObject(result);
  }

  async call(to: string, data: string, from?: string, blockTag?: string): Promise<string> {
    const result = await this.provider.brc20progCall(to, data, from, blockTag);
    return mapToObject(result);
  }

  async estimateGas(to: string, data: string, from?: string): Promise<string> {
    const result = await this.provider.brc20progEstimateGas(to, data, from);
    return mapToObject(result);
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
  async evalScript(script: string): Promise<LuaEvalResult> {
    return this.provider.luaEvalScript(script);
  }
}

/**
 * Data API client (uses WebProvider internally)
 */
export class DataApiClient {
  constructor(private provider: WasmWebProvider) {}

  // Pool operations
  async getPools(factoryId: string): Promise<DataApiPoolsResponse> {
    return this.provider.dataApiGetPools(factoryId);
  }

  async getPoolHistory(poolId: string, category?: string, limit?: number, offset?: number): Promise<DataApiPoolHistoryEvent[]> {
    return this.provider.dataApiGetPoolHistory(poolId, category, limit ? BigInt(limit) : undefined, offset ? BigInt(offset) : undefined);
  }

  async getAllHistory(poolId: string, limit?: number, offset?: number): Promise<DataApiPoolHistoryEvent[]> {
    return this.provider.dataApiGetAllHistory(poolId, limit ? BigInt(limit) : undefined, offset ? BigInt(offset) : undefined);
  }

  async getSwapHistory(poolId: string, limit?: number, offset?: number): Promise<DataApiPoolHistoryEvent[]> {
    return this.provider.dataApiGetSwapHistory(poolId, limit ? BigInt(limit) : undefined, offset ? BigInt(offset) : undefined);
  }

  async getMintHistory(poolId: string, limit?: number, offset?: number): Promise<DataApiPoolHistoryEvent[]> {
    return this.provider.dataApiGetMintHistory(poolId, limit ? BigInt(limit) : undefined, offset ? BigInt(offset) : undefined);
  }

  async getBurnHistory(poolId: string, limit?: number, offset?: number): Promise<DataApiPoolHistoryEvent[]> {
    return this.provider.dataApiGetBurnHistory(poolId, limit ? BigInt(limit) : undefined, offset ? BigInt(offset) : undefined);
  }

  // Trading data
  async getTrades(pool: string, startTime?: number, endTime?: number, limit?: number): Promise<TradeInfo[]> {
    return this.provider.dataApiGetTrades(pool, startTime, endTime, limit ? BigInt(limit) : undefined);
  }

  async getCandles(pool: string, interval: string, startTime?: number, endTime?: number, limit?: number): Promise<CandleInfo[]> {
    return this.provider.dataApiGetCandles(pool, interval, startTime, endTime, limit ? BigInt(limit) : undefined);
  }

  async getReserves(pool: string): Promise<DataApiReserves> {
    return this.provider.dataApiGetReserves(pool);
  }

  // Balance operations
  async getAlkanesByAddress(address: string): Promise<DataApiAddressAlkanes> {
    return this.provider.dataApiGetAlkanesByAddress(address);
  }

  async getAddressBalances(address: string, includeOutpoints: boolean = false): Promise<AddressBalancesResponse> {
    return this.provider.dataApiGetAddressBalances(address, includeOutpoints);
  }

  // Token operations
  async getHolders(alkane: string, page: number = 0, limit: number = 100): Promise<HolderInfo[]> {
    return this.provider.dataApiGetHolders(alkane, BigInt(page), BigInt(limit));
  }

  async getHoldersCount(alkane: string): Promise<number> {
    return this.provider.dataApiGetHoldersCount(alkane);
  }

  async getKeys(alkane: string, prefix?: string, limit: number = 100): Promise<DataApiStorageKey[]> {
    return this.provider.dataApiGetKeys(alkane, prefix, BigInt(limit));
  }

  // Market data
  async getBitcoinPrice(): Promise<BitcoinPriceResponse> {
    return this.provider.dataApiGetBitcoinPrice();
  }

  async getBitcoinMarketChart(days: string): Promise<MarketChartResponse> {
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
/**
 * Get log level from environment variables
 */
function getLogLevelFromEnv(): LogLevel | undefined {
  // Only check env vars in Node.js environment
  if (typeof process !== 'undefined' && process.env) {
    const alkLog = process.env.ALKANES_LOG_LEVEL;
    const rustLog = process.env.RUST_LOG;

    const level = alkLog || rustLog;
    if (level) {
      const normalized = level.toLowerCase();
      if (['off', 'error', 'warn', 'info', 'debug', 'trace'].includes(normalized)) {
        return normalized as LogLevel;
      }
    }
  }
  return undefined;
}

/**
 * Logger instance that respects log level configuration
 */
class Logger {
  private level: LogLevel;
  private readonly levels: Record<LogLevel, number> = {
    off: 0,
    error: 1,
    warn: 2,
    info: 3,
    debug: 4,
    trace: 5,
  };

  constructor(level: LogLevel = 'off') {
    this.level = level;
  }

  setLevel(level: LogLevel): void {
    this.level = level;
  }

  private shouldLog(msgLevel: LogLevel): boolean {
    return this.levels[msgLevel] <= this.levels[this.level];
  }

  error(...args: any[]): void {
    if (this.shouldLog('error')) console.error('[SDK Error]', ...args);
  }

  warn(...args: any[]): void {
    if (this.shouldLog('warn')) console.warn('[SDK Warn]', ...args);
  }

  info(...args: any[]): void {
    if (this.shouldLog('info')) console.info('[SDK Info]', ...args);
  }

  debug(...args: any[]): void {
    if (this.shouldLog('debug')) console.log('[SDK Debug]', ...args);
  }

  trace(...args: any[]): void {
    if (this.shouldLog('trace')) console.log('[SDK Trace]', ...args);
  }
}

// Global logger instance
const logger = new Logger();

export class AlkanesProvider {
  private _provider: WasmWebProvider | null = null;
  private _bitcoin: BitcoinRpcClient | null = null;
  private _esplora: EsploraClient | null = null;
  private _alkanes: AlkanesRpcClient | null = null;
  private _dataApi: DataApiClient | null = null;
  private _espo: EspoClient | null = null;
  private _lua: LuaClient | null = null;
  private _metashrew: MetashrewClient | null = null;
  private _ord: OrdClient | null = null;
  private _brc20prog: Brc20ProgClient | null = null;

  public readonly network: bitcoin.Network;
  public readonly networkType: NetworkType;
  public readonly rpcUrl: string;
  public readonly bitcoinRpcUrl?: string;
  public readonly metashrewRpcUrl?: string;
  public readonly dataApiUrl: string;
  public readonly logLevel: LogLevel;
  private readonly networkPreset: string;

  constructor(config: AlkanesProviderConfig) {
    // Resolve network preset
    const preset = NETWORK_PRESETS[config.network] || NETWORK_PRESETS['mainnet'];
    this.networkPreset = config.network;
    this.networkType = preset.networkType;
    this.rpcUrl = config.rpcUrl || preset.rpcUrl;
    this.bitcoinRpcUrl = config.bitcoinRpcUrl;
    this.metashrewRpcUrl = config.metashrewRpcUrl;
    this.dataApiUrl = config.dataApiUrl || config.rpcUrl || preset.dataApiUrl;

    // Resolve log level: config > env > off
    this.logLevel = config.logLevel || getLogLevelFromEnv() || 'off';
    logger.setLevel(this.logLevel);

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

    logger.debug(`Provider configured for ${this.networkType} (${this.rpcUrl})`);
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
      // Use string concatenation to prevent bundler static analysis issues
      const loaderPath = '@alkanes/ts-sdk' + '/wasm/node-loader.cjs';
      const nodeLoaderModule = await import(/* @vite-ignore */ loaderPath);
      const nodeLoader = nodeLoaderModule.default || nodeLoaderModule;
      await nodeLoader.init();
      // Initialize panic hook for better error messages
      if (nodeLoader.init_panic_hook) {
        nodeLoader.init_panic_hook();
      }
      WebProviderClass = nodeLoader.WebProvider;
    } else {
      // Browser: Use the ESM module (expects bundler support)
      const wasmPath = '@alkanes/ts-sdk' + '/wasm';
      const wasm = await import(/* @vite-ignore */ wasmPath);
      // Initialize panic hook for better error messages
      if (wasm.init_panic_hook) {
        wasm.init_panic_hook();
      }
      WebProviderClass = wasm.WebProvider;
    }

    // Create provider with appropriate network name
    const providerName = this.networkPreset === 'local' ? 'regtest' : this.networkPreset;

    // Always pass rpcUrl as config override to ensure it's used
    const configOverride: any = {
      jsonrpc_url: this.rpcUrl,
      ...(this.bitcoinRpcUrl && { bitcoin_rpc_url: this.bitcoinRpcUrl }),
      ...(this.metashrewRpcUrl && { metashrew_rpc_url: this.metashrewRpcUrl }),
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
   * Get the raw WASM WebProvider for direct access to low-level methods.
   *
   * This is useful for CLI tools that need access to wallet methods
   * like wallet_create_js, wallet_load_js, etc. that are not wrapped
   * by the higher-level API.
   *
   * @throws Error if provider is not initialized
   */
  get rawProvider(): WasmWebProvider {
    if (!this._provider) {
      throw new Error('Provider not initialized. Call initialize() first.');
    }
    return this._provider;
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

  /**
   * Ord (Ordinals) RPC client
   */
  get ord(): OrdClient {
    if (!this._ord) {
      if (!this._provider) {
        throw new Error('Provider not initialized. Call initialize() first.');
      }
      this._ord = new OrdClient(this._provider);
    }
    return this._ord;
  }

  /**
   * BRC-20 Prog (Programmable BRC-20) RPC client
   */
  get brc20prog(): Brc20ProgClient {
    if (!this._brc20prog) {
      if (!this._provider) {
        throw new Error('Provider not initialized. Call initialize() first.');
      }
      this._brc20prog = new Brc20ProgClient(this._provider);
    }
    return this._brc20prog;
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
  async getEnrichedBalances(address: string, protocolTag?: string): Promise<{
    address: string;
    btc: { confirmed: number; unconfirmed: number };
    alkanes: AlkaneBalanceResponse[];
    outpoints: Array<{ outpoint: string; value: number; alkanes: AlkaneBalanceResponse[] }>;
  }> {
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
  async getAlkaneTokenDetails(params: { alkaneId: AlkaneId }): Promise<{
    id: AlkaneId;
    name: string;
    symbol: string;
    decimals: number;
    totalSupply: string;
  }> {
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
  async getAddressHistory(address: string): Promise<EsploraTransaction[]> {
    const provider = await this.getProvider();
    return provider.getAddressTxs(address);
  }

  /**
   * Get transaction history for an address from Esplora (first page, max 25 transactions)
   */
  async getAddressTxs(address: string): Promise<EsploraTransaction[]> {
    const provider = await this.getProvider();
    return provider.esploraGetAddressTxs(address);
  }

  /**
   * Get next page of transaction history for an address
   * @param address The address to fetch transactions for
   * @param lastSeenTxid The last transaction ID from the previous page (undefined for first page)
   */
  async getAddressTxsChain(address: string, lastSeenTxid?: string): Promise<EsploraTransaction[]> {
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
  async getAddressHistoryWithTraces(address: string, excludeCoinbase?: boolean): Promise<Array<
    EsploraTransaction & { alkane_traces?: AlkaneTraceEntry[] }
  >> {
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
  async getPoolReserves(poolId: string): Promise<DataApiReserves> {
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
  async simulateAlkanes(contractId: string, calldata: number[], blockTag?: string): Promise<AlkaneSimulateResponse> {
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

  // ============================================================================
  // WALLET OPERATIONS
  // ============================================================================

  /**
   * Create a new wallet
   *
   * @param options - Wallet creation options
   * @returns Wallet info including address and mnemonic
   *
   * @example
   * ```typescript
   * const wallet = await provider.walletCreate();
   * console.log('Address:', wallet.address);
   * console.log('Mnemonic:', wallet.mnemonic); // Save this!
   * ```
   */
  walletCreate(options?: {
    mnemonic?: string;
    passphrase?: string;
  }): { address: string; mnemonic: string; network: string } {
    if (!this._provider) {
      throw new Error('Provider not initialized. Call initialize() first.');
    }
    return this._provider.walletCreate(
      options?.mnemonic ?? undefined,
      options?.passphrase ?? undefined
    );
  }

  /**
   * Load an existing wallet from storage
   *
   * @param passphrase - Optional passphrase for BIP39
   */
  async walletLoad(passphrase?: string): Promise<any> {
    const provider = await this.getProvider();
    return provider.walletLoad(passphrase ?? undefined);
  }

  /**
   * Load a wallet from mnemonic for signing transactions
   *
   * @param mnemonic - The mnemonic phrase
   * @param passphrase - Optional BIP39 passphrase
   */
  walletLoadMnemonic(mnemonic: string, passphrase?: string): void {
    if (!this._provider) {
      throw new Error('Provider not initialized. Call initialize() first.');
    }
    this._provider.walletLoadMnemonic(mnemonic, passphrase ?? undefined);
  }

  /**
   * Check if wallet is loaded (has keystore for signing)
   */
  walletIsLoaded(): boolean {
    if (!this._provider) {
      return false;
    }
    return this._provider.walletIsLoaded();
  }

  /**
   * Get the wallet's primary address
   */
  async walletGetAddress(): Promise<string> {
    const provider = await this.getProvider();
    const result = await provider.walletGetAddress();
    return result?.address || result;
  }

  /**
   * Get addresses from the loaded wallet
   *
   * @param addressType - Address type: 'p2tr', 'p2wpkh', 'p2sh-p2wpkh', 'p2pkh'
   * @param startIndex - Starting derivation index
   * @param count - Number of addresses to generate
   * @param chain - Optional chain (0 = external/receiving, 1 = internal/change)
   * @returns Array of address info objects
   *
   * @example
   * ```typescript
   * const addresses = provider.walletGetAddresses('p2tr', 0, 5);
   * console.log(addresses[0].address);
   * ```
   */
  walletGetAddresses(
    addressType: 'p2tr' | 'p2wpkh' | 'p2sh-p2wpkh' | 'p2pkh',
    startIndex: number,
    count: number,
    chain?: number
  ): Array<{ address: string; path: string; index: number }> {
    if (!this._provider) {
      throw new Error('Provider not initialized. Call initialize() first.');
    }
    return this._provider.walletGetAddresses(addressType, startIndex, count, chain ?? undefined);
  }

  /**
   * Get wallet BTC balance
   *
   * @param addresses - Optional specific addresses to check
   */
  async walletGetBalance(addresses?: string[]): Promise<{ confirmed: number; unconfirmed: number }> {
    const provider = await this.getProvider();
    return provider.walletGetBalance(addresses ?? undefined);
  }

  /**
   * Get wallet UTXOs
   *
   * @param addresses - Optional specific addresses to check
   */
  async walletGetUtxos(addresses?: string[]): Promise<any[]> {
    const provider = await this.getProvider();
    return provider.walletGetUtxos(addresses ?? undefined);
  }

  // ============================================================================
  // TYPED ALKANES EXECUTE OPERATIONS
  // ============================================================================

  /**
   * Execute an Alkanes contract call with full control (typed parameters)
   *
   * This method accepts TypeScript objects and handles JSON serialization internally.
   * Automatically applies sensible defaults:
   * - `toAddresses`: Auto-generated as p2tr:0 for each protostone vN reference if not provided
   * - `fromAddresses`: Defaults to [p2wpkh:0, p2tr:0] if not provided
   * - `changeAddress`: Defaults to p2wpkh:0 if not provided
   * - `alkanesChangeAddress`: Defaults to p2tr:0 if not provided
   *
   * @param params - Execute parameters
   * @returns Execution result
   *
   * @example
   * ```typescript
   * // Mint DIESEL tokens - only specify protostones, defaults handle the rest!
   * const result = await provider.alkanesExecuteTyped({
   *   inputRequirements: 'B:10000',
   *   protostones: '[2,0,77]:v0:v0',
   *   feeRate: 100,
   * });
   * ```
   */
  async alkanesExecuteTyped(params: {
    toAddresses?: string[];
    inputRequirements: string;
    protostones: string;
    feeRate?: number;
    envelopeHex?: string;
    fromAddresses?: string[];
    changeAddress?: string;
    alkanesChangeAddress?: string;
    traceEnabled?: boolean;
    mineEnabled?: boolean;
    autoConfirm?: boolean;
    rawOutput?: boolean;
  }): Promise<any> {
    const provider = await this.getProvider();

    // Parse protostones to determine how many vN outputs are referenced
    const maxVout = this._parseMaxVoutFromProtostones(params.protostones);

    // Auto-generate toAddresses if not provided
    // Creates one p2tr:0 output for each vN reference (v0, v1, v2, etc.)
    const toAddresses = params.toAddresses ?? Array(maxVout + 1).fill('p2tr:0');

    const options: Record<string, any> = {};

    // Apply automatic defaults
    options.from_addresses = params.fromAddresses ?? ['p2wpkh:0', 'p2tr:0'];
    options.change_address = params.changeAddress ?? 'p2wpkh:0';
    options.alkanes_change_address = params.alkanesChangeAddress ?? 'p2tr:0';

    if (params.traceEnabled !== undefined) options.trace_enabled = params.traceEnabled;
    if (params.mineEnabled !== undefined) options.mine_enabled = params.mineEnabled;
    if (params.autoConfirm !== undefined) options.auto_confirm = params.autoConfirm;
    if (params.rawOutput !== undefined) options.raw_output = params.rawOutput;

    const optionsJson = Object.keys(options).length > 0 ? JSON.stringify(options) : null;

    // Use alkanesExecuteFull which handles the complete flow internally
    // This avoids serialization issues when passing state between JS and Rust
    const result = await provider.alkanesExecuteFull(
      JSON.stringify(toAddresses),
      params.inputRequirements,
      params.protostones,
      params.feeRate ?? null,
      params.envelopeHex ?? null,
      optionsJson
    );

    return typeof result === 'string' ? JSON.parse(result) : result;
  }

  /**
   * Parse protostones string to find the maximum vN output index referenced
   * This is used to auto-generate the correct number of to_addresses
   *
   * @param protostones - Protostone specification string
   * @returns Maximum vout index found (e.g., "v2" returns 2)
   */
  private _parseMaxVoutFromProtostones(protostones: string): number {
    let maxVout = 0;

    // Match all vN patterns in the protostones string
    const voutMatches = protostones.matchAll(/v(\d+)/g);

    for (const match of voutMatches) {
      const voutIndex = parseInt(match[1], 10);
      if (voutIndex > maxVout) {
        maxVout = voutIndex;
      }
    }

    return maxVout;
  }

  /**
   * Wrap BTC to frBTC (typed parameters)
   *
   * @param params - Wrap parameters
   * @returns Transaction result
   *
   * @example
   * ```typescript
   * const result = await provider.frbtcWrapTyped({
   *   amount: 100000n,
   *   toAddress: myAddress,
   *   feeRate: 100,
   *   mineEnabled: true, // Auto-mine on regtest
   * });
   * ```
   */
  async frbtcWrapTyped(params: {
    amount: bigint | number;
    toAddress: string;
    fromAddress?: string;
    changeAddress?: string;
    feeRate?: number;
    traceEnabled?: boolean;
    mineEnabled?: boolean;
    autoConfirm?: boolean;
  }): Promise<any> {
    const provider = await this.getProvider();

    const wrapParams: Record<string, any> = {
      amount: String(params.amount),
      to_address: params.toAddress,
      fee_rate: params.feeRate ?? 1,
      auto_confirm: params.autoConfirm ?? true,
      trace_enabled: params.traceEnabled ?? false,
      mine_enabled: params.mineEnabled ?? false,
    };
    if (params.fromAddress) wrapParams.from_address = params.fromAddress;
    if (params.changeAddress) wrapParams.change_address = params.changeAddress;

    const result = await provider.alkanesWrapBtc(JSON.stringify(wrapParams));
    return typeof result === 'string' ? JSON.parse(result) : result;
  }

  /**
   * Initialize a new AMM liquidity pool (typed parameters)
   *
   * @param params - Pool initialization parameters
   * @returns Transaction ID
   *
   * @example
   * ```typescript
   * const txid = await provider.alkanesInitPoolTyped({
   *   factoryId: { block: 4, tx: 65522 },
   *   token0: { block: 2, tx: 0 },
   *   token1: { block: 32, tx: 0 },
   *   amount0: '300000000',
   *   amount1: '50000',
   *   toAddress: myAddress,
   *   feeRate: 100,
   * });
   * ```
   */
  async alkanesInitPoolTyped(params: {
    factoryId: { block: number; tx: number };
    token0: { block: number; tx: number };
    token1: { block: number; tx: number };
    amount0: string | number | bigint;
    amount1: string | number | bigint;
    minimumLp?: string | number | bigint;
    toAddress: string;
    fromAddress?: string;
    changeAddress?: string;
    feeRate?: number;
    trace?: boolean;
    autoConfirm?: boolean;
  }): Promise<string> {
    const provider = await this.getProvider();

    const poolParams: Record<string, any> = {
      factory_id: params.factoryId,
      token0: params.token0,
      token1: params.token1,
      amount0: String(params.amount0),
      amount1: String(params.amount1),
      to_address: params.toAddress,
      fee_rate: params.feeRate ?? 1,
      trace: params.trace ?? false,
      auto_confirm: params.autoConfirm ?? true,
    };
    if (params.minimumLp) poolParams.minimum_lp = String(params.minimumLp);
    if (params.fromAddress) poolParams.from_address = params.fromAddress;
    if (params.changeAddress) poolParams.change_address = params.changeAddress;

    return provider.alkanesInitPool(JSON.stringify(poolParams));
  }

  /**
   * Execute an AMM swap (typed parameters)
   *
   * @param params - Swap parameters
   * @returns Transaction ID
   */
  async alkanesSwapTyped(params: {
    factoryId: { block: number; tx: number };
    path: Array<{ block: number; tx: number }>;
    inputAmount: string | number | bigint;
    minimumOutput: string | number | bigint;
    expires: number;
    toAddress: string;
    fromAddress?: string;
    changeAddress?: string;
    feeRate?: number;
    trace?: boolean;
    autoConfirm?: boolean;
  }): Promise<string> {
    const provider = await this.getProvider();

    const swapParams: Record<string, any> = {
      factory_id: params.factoryId,
      path: params.path,
      input_amount: String(params.inputAmount),
      minimum_output: String(params.minimumOutput),
      expires: params.expires,
      to_address: params.toAddress,
      fee_rate: params.feeRate ?? 1,
      trace: params.trace ?? false,
      auto_confirm: params.autoConfirm ?? true,
    };
    if (params.fromAddress) swapParams.from_address = params.fromAddress;
    if (params.changeAddress) swapParams.change_address = params.changeAddress;

    return provider.alkanesSwap(JSON.stringify(swapParams));
  }

  // ============================================================================
  // BRC20-PROG DEPLOY/TRANSACT OPERATIONS
  // ============================================================================

  /**
   * Deploy a BRC20-prog smart contract (typed parameters)
   *
   * Uses the presign anti-frontrunning strategy by default, which:
   * 1. Pre-signs all transactions (split, commit, reveal, activation)
   * 2. Broadcasts all transactions atomically via sendrawtransactions
   * 3. Protects inscribed UTXOs by splitting them if necessary
   *
   * @param params - Deployment parameters
   * @returns Deployment result with transaction IDs and fees
   *
   * @example
   * ```typescript
   * const result = await provider.brc20ProgDeployTyped({
   *   foundryJson: contractJson,  // Foundry build output JSON
   *   feeRate: 10,
   *   strategy: 'presign',        // Anti-frontrunning strategy
   *   mempool_indexer: true,      // Trace pending UTXO inscriptions
   *   mineEnabled: true,          // Auto-mine on regtest
   * });
   * console.log('Deployed! Reveal:', result.reveal_txid);
   * ```
   */
  async brc20ProgDeployTyped(params: {
    /** Foundry build JSON containing contract bytecode (string or object) */
    foundryJson: string | object;
    /** Addresses to source UTXOs from (optional) */
    fromAddresses?: string[];
    /** Change address (optional, defaults to signer address) */
    changeAddress?: string;
    /** Fee rate in sat/vB (optional, defaults to 10) */
    feeRate?: number;
    /** Use 3-transaction activation pattern (optional) */
    useActivation?: boolean;
    /** Use MARA Slipstream service for broadcasting (optional) */
    useSlipstream?: boolean;
    /** Use Rebar Shield for private transaction relay (optional) */
    useRebar?: boolean;
    /** Rebar fee tier: 1 (~8% hashrate) or 2 (~16% hashrate) (optional) */
    rebarTier?: 1 | 2;
    /** Resume from existing commit transaction (txid) (optional) */
    resumeFromCommit?: string;
    /** Anti-frontrunning strategy (optional, defaults to 'presign') */
    strategy?: 'presign' | 'cpfp' | 'cltv' | 'rbf';
    /** Enable mempool indexer for pending UTXO inscription tracing (optional) */
    mempool_indexer?: boolean;
    /** Enable transaction tracing (optional) */
    traceEnabled?: boolean;
    /** Mine a block after broadcasting - regtest only (optional) */
    mineEnabled?: boolean;
    /** Automatically confirm the transaction preview (optional) */
    autoConfirm?: boolean;
  }): Promise<{
    split_txid?: string;
    split_fee?: number;
    commit_txid: string;
    reveal_txid: string;
    activation_txid?: string;
    commit_fee: number;
    reveal_fee: number;
    activation_fee?: number;
    inputs_used: string[];
    outputs_created: string[];
    traces?: any[];
  }> {
    const provider = await this.getProvider();

    // Convert foundryJson to string if it's an object
    const foundryJsonStr = typeof params.foundryJson === 'string'
      ? params.foundryJson
      : JSON.stringify(params.foundryJson);

    // Build execution params
    const executeParams: Record<string, any> = {
      fee_rate: params.feeRate ?? 10,
      use_activation: params.useActivation ?? false,
      use_slipstream: params.useSlipstream ?? false,
      use_rebar: params.useRebar ?? false,
      auto_confirm: params.autoConfirm ?? true,
      trace_enabled: params.traceEnabled ?? false,
      mine_enabled: params.mineEnabled ?? false,
      raw_output: false,
    };
    if (params.fromAddresses) executeParams.from_addresses = params.fromAddresses;
    if (params.changeAddress) executeParams.change_address = params.changeAddress;
    if (params.rebarTier) executeParams.rebar_tier = params.rebarTier;
    if (params.resumeFromCommit) executeParams.resume_from_commit = params.resumeFromCommit;
    if (params.strategy) executeParams.strategy = params.strategy;
    if (params.mempool_indexer !== undefined) executeParams.mempool_indexer = params.mempool_indexer;

    // Call the WASM binding
    const result = await provider.brc20ProgDeployContract(
      foundryJsonStr,
      JSON.stringify(executeParams)
    );

    return typeof result === 'string' ? JSON.parse(result) : result;
  }

  /**
   * Call a BRC20-prog contract function (typed parameters)
   *
   * Uses the presign anti-frontrunning strategy by default, which:
   * 1. Pre-signs all transactions (split, commit, reveal, activation)
   * 2. Broadcasts all transactions atomically via sendrawtransactions
   * 3. Protects inscribed UTXOs by splitting them if necessary
   *
   * @param params - Transaction parameters
   * @returns Transaction result with IDs and fees
   *
   * @example
   * ```typescript
   * const result = await provider.brc20ProgTransactTyped({
   *   contractAddress: '0x1234...abcd',
   *   functionSignature: 'transfer(address,uint256)',
   *   calldata: ['0xrecipient', '1000'],
   *   feeRate: 10,
   *   strategy: 'presign',
   * });
   * console.log('Transaction sent! Reveal:', result.reveal_txid);
   * ```
   */
  async brc20ProgTransactTyped(params: {
    /** Contract address to call (0x-prefixed hex) */
    contractAddress: string;
    /** Function signature (e.g., "transfer(address,uint256)") */
    functionSignature: string;
    /** Calldata arguments as array or comma-separated string */
    calldata: string[] | string;
    /** Addresses to source UTXOs from (optional) */
    fromAddresses?: string[];
    /** Change address (optional, defaults to signer address) */
    changeAddress?: string;
    /** Fee rate in sat/vB (optional, defaults to 10) */
    feeRate?: number;
    /** Use MARA Slipstream service for broadcasting (optional) */
    useSlipstream?: boolean;
    /** Use Rebar Shield for private transaction relay (optional) */
    useRebar?: boolean;
    /** Rebar fee tier: 1 (~8% hashrate) or 2 (~16% hashrate) (optional) */
    rebarTier?: 1 | 2;
    /** Resume from existing commit transaction (txid) (optional) */
    resumeFromCommit?: string;
    /** Anti-frontrunning strategy (optional, defaults to 'presign') */
    strategy?: 'presign' | 'cpfp' | 'cltv' | 'rbf';
    /** Enable mempool indexer for pending UTXO inscription tracing (optional) */
    mempool_indexer?: boolean;
    /** Enable transaction tracing (optional) */
    traceEnabled?: boolean;
    /** Mine a block after broadcasting - regtest only (optional) */
    mineEnabled?: boolean;
    /** Automatically confirm the transaction preview (optional) */
    autoConfirm?: boolean;
  }): Promise<{
    split_txid?: string;
    split_fee?: number;
    commit_txid: string;
    reveal_txid: string;
    activation_txid?: string;
    commit_fee: number;
    reveal_fee: number;
    activation_fee?: number;
    inputs_used: string[];
    outputs_created: string[];
    traces?: any[];
  }> {
    const provider = await this.getProvider();

    // Convert calldata to comma-separated string if it's an array
    const calldataStr = Array.isArray(params.calldata)
      ? params.calldata.join(',')
      : params.calldata;

    // Build execution params
    const executeParams: Record<string, any> = {
      fee_rate: params.feeRate ?? 10,
      use_slipstream: params.useSlipstream ?? false,
      use_rebar: params.useRebar ?? false,
      auto_confirm: params.autoConfirm ?? true,
      trace_enabled: params.traceEnabled ?? false,
      mine_enabled: params.mineEnabled ?? false,
      raw_output: false,
    };
    if (params.fromAddresses) executeParams.from_addresses = params.fromAddresses;
    if (params.changeAddress) executeParams.change_address = params.changeAddress;
    if (params.rebarTier) executeParams.rebar_tier = params.rebarTier;
    if (params.resumeFromCommit) executeParams.resume_from_commit = params.resumeFromCommit;
    if (params.strategy) executeParams.strategy = params.strategy;
    if (params.mempool_indexer !== undefined) executeParams.mempool_indexer = params.mempool_indexer;

    // Call the WASM binding
    const result = await provider.brc20ProgTransact(
      params.contractAddress,
      params.functionSignature,
      calldataStr,
      JSON.stringify(executeParams)
    );

    return typeof result === 'string' ? JSON.parse(result) : result;
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
