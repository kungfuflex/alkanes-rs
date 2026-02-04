/**
 * Type declarations for the alkanes-web-sys WASM module
 * This module is loaded dynamically at runtime from build/wasm/
 */

declare module '@alkanes/ts-sdk/wasm' {
  export class WebProvider {
    constructor(network: string, config?: Record<string, any>);

    // Bitcoin RPC methods
    bitcoindGetBlockCount(): Promise<number>;
    bitcoindGetBlockHash(height: number): Promise<string>;
    bitcoindGetBlock(hash: string, raw?: boolean): Promise<any>;
    bitcoindSendRawTransaction(hex: string): Promise<string>;
    bitcoindGetRawTransaction(txid: string, blockHash?: string): Promise<any>;
    bitcoindGetBlockchainInfo(): Promise<any>;
    bitcoindGetNetworkInfo(): Promise<any>;
    bitcoindGetMempoolInfo(): Promise<any>;
    bitcoindEstimateSmartFee(target: number): Promise<any>;
    bitcoindGenerateToAddress(nblocks: number, address: string): Promise<any>;

    // Esplora API methods
    esploraGetAddressInfo(address: string): Promise<any>;
    esploraGetAddressUtxo(address: string): Promise<any[]>;
    esploraGetAddressTxs(address: string): Promise<any[]>;
    esploraGetTx(txid: string): Promise<any>;
    esploraGetTxStatus(txid: string): Promise<any>;
    esploraGetTxHex(txid: string): Promise<string>;
    esploraGetBlocksTipHeight(): Promise<number>;
    esploraGetBlocksTipHash(): Promise<string>;
    esploraBroadcastTx(txHex: string): Promise<string>;

    // Alkanes RPC methods
    alkanesBalance(address?: string): Promise<any[]>;
    alkanesByAddress(address: string, blockTag?: string, protocolTag?: number): Promise<any>;
    alkanesByOutpoint(outpoint: string, blockTag?: string, protocolTag?: number): Promise<any>;
    alkanesBytecode(alkaneId: string, blockTag?: string): Promise<string>;
    alkanesSimulate(contractId: string, contextJson: string, blockTag?: string): Promise<any>;
    alkanesExecute(paramsJson: string): Promise<any>;
    alkanesTrace(outpoint: string): Promise<any>;
    alkanesView(contractId: string, viewFn: string, params?: Uint8Array, blockTag?: string): Promise<any>;
    alkanesGetAllPools(factoryId: string): Promise<any>;
    alkanesGetAllPoolsWithDetails(factoryId: string, chunkSize?: number, maxConcurrent?: number): Promise<any[]>;
    alkanesPendingUnwraps(blockTag?: string): Promise<any>;

    // Data API methods (oylapi REST endpoints)
    dataApiGetPools(factoryId: string): Promise<any>;
    dataApiGetAllPoolsDetails(factoryId: string, limit?: bigint, offset?: bigint, sortBy?: string, order?: string): Promise<any>;
    dataApiGetPoolDetails(factoryId: string, poolId: string): Promise<any>;
    dataApiGetPoolHistory(poolId: string, category?: string, limit?: bigint, offset?: bigint): Promise<any>;
    dataApiGetAllHistory(poolId: string, limit?: bigint, offset?: bigint): Promise<any>;
    dataApiGetSwapHistory(poolId: string, limit?: bigint, offset?: bigint): Promise<any>;
    dataApiGetMintHistory(poolId: string, limit?: bigint, offset?: bigint): Promise<any>;
    dataApiGetBurnHistory(poolId: string, limit?: bigint, offset?: bigint): Promise<any>;
    dataApiGetTrades(pool: string, startTime?: number, endTime?: number, limit?: bigint): Promise<any[]>;
    dataApiGetCandles(pool: string, interval: string, startTime?: number, endTime?: number, limit?: bigint): Promise<any[]>;
    dataApiGetReserves(pool: string): Promise<any>;
    dataApiGetAlkanesByAddress(address: string): Promise<any>;
    dataApiGetAddressBalances(address: string, includeOutpoints?: boolean): Promise<any>;
    dataApiGetHolders(alkane: string, page: bigint, limit: bigint): Promise<any[]>;
    dataApiGetHoldersCount(alkane: string): Promise<number>;
    dataApiGetKeys(alkane: string, prefix?: string, limit: bigint): Promise<any>;
    dataApiGetBitcoinPrice(): Promise<any>;
    dataApiGetBitcoinMarketChart(days: string): Promise<any>;

    // Utility methods
    getEnrichedBalances(address: string, protocolTag?: string): Promise<any>;
    getAddressTxs(address: string): Promise<any[]>;
    getAddressTxsWithTraces(address: string, excludeCoinbase?: boolean): Promise<any[]>;
    metashrewHeight(): Promise<number>;
    broadcastTransaction(txHex: string): Promise<string>;
  }

  export default function init(): Promise<void>;
}
