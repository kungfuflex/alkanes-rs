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

    // Data API methods (new oylapi REST endpoints - 35 new bindings)
    dataApiGetPoolCreationHistory(limit?: number, offset?: number): Promise<any>;
    dataApiGetPoolSwapHistory(poolId?: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetTokenSwapHistory(alkaneId: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetPoolMintHistory(poolId?: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetPoolBurnHistory(poolId?: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetAddressSwapHistoryForPool(address: string, poolId: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetAddressSwapHistoryForToken(address: string, alkaneId: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetAddressWrapHistory(address: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetAddressUnwrapHistory(address: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetAllWrapHistory(limit?: number, offset?: number): Promise<any>;
    dataApiGetAllUnwrapHistory(limit?: number, offset?: number): Promise<any>;
    dataApiGetTotalUnwrapAmount(): Promise<any>;
    dataApiGetAddressPoolCreationHistory(address: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetAddressPoolMintHistory(address: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetAddressPoolBurnHistory(address: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetAllAddressAmmTxHistory(address: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetAllAmmTxHistory(limit?: number, offset?: number): Promise<any>;
    dataApiGetAddressPositions(address: string, factoryId: string): Promise<any>;
    dataApiGetTokenPairs(factoryId: string, alkaneId?: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetAllTokenPairs(factoryId: string, limit?: number, offset?: number): Promise<any>;
    dataApiGetAlkaneSwapPairDetails(factoryId: string, tokenAId: string, tokenBId: string): Promise<any>;
    dataApiGetAlkanesUtxo(address: string): Promise<any>;
    dataApiGetAmmUtxos(address: string): Promise<any>;
    dataApiGetAddressUtxos(address: string): Promise<any>;
    dataApiGetAddressBalance(address: string): Promise<any>;
    dataApiGetTaprootBalance(address: string): Promise<any>;
    dataApiGetAccountUtxos(account: string): Promise<any>;
    dataApiGetAccountBalance(account: string): Promise<any>;
    dataApiGetAddressOutpoints(address: string): Promise<any>;
    dataApiGlobalAlkanesSearch(query: string, limit?: number, offset?: number): Promise<any>;
    dataApiPathfind(tokenIn: string, tokenOut: string, amountIn: string, maxHops?: number): Promise<any>;
    dataApiGetBitcoinMarketWeekly(): Promise<any>;
    dataApiGetBitcoinMarkets(): Promise<any>;
    dataApiGetTaprootHistory(taprootAddress: string, totalTxs: number): Promise<any>;
    dataApiGetIntentHistory(address: string, totalTxs?: number, lastSeenTxId?: string): Promise<any>;

    // Utility methods
    getEnrichedBalances(address: string, protocolTag?: string): Promise<any>;
    getAddressTxs(address: string): Promise<any[]>;
    getAddressTxsWithTraces(address: string, excludeCoinbase?: boolean): Promise<any[]>;
    metashrewHeight(): Promise<number>;
    broadcastTransaction(txHex: string): Promise<string>;

    // Espo JSON-RPC methods (essentials module)
    espoGetHeight(): Promise<number>;
    espoPing(): Promise<string>;
    espoGetAddressBalances(address: string, includeOutpoints: boolean): Promise<any>;
    espoGetAddressOutpoints(address: string): Promise<any>;
    espoGetOutpointBalances(outpoint: string): Promise<any>;
    espoGetHolders(alkaneId: string, page: number, limit: number): Promise<any>;
    espoGetHoldersCount(alkaneId: string): Promise<number>;
    espoGetKeys(alkaneId: string, page: number, limit: number): Promise<any>;

    // Espo JSON-RPC methods (ammdata module)
    espoAmmdataPing(): Promise<string>;
    espoGetCandles(pool: string, timeframe?: string, side?: string, limit?: number, page?: number): Promise<any>;
    espoGetTrades(pool: string, limit?: number, page?: number, side?: string, filterSide?: string, sort?: string, dir?: string): Promise<any>;
    espoGetPools(limit?: number, page?: number): Promise<any>;
    espoFindBestSwapPath(tokenIn: string, tokenOut: string, mode?: string, amountIn?: string, amountOut?: string, amountOutMin?: string, amountInMax?: string, availableIn?: string, feeBps?: number, maxHops?: number): Promise<any>;
    espoGetBestMevSwap(token: string, feeBps?: number, maxHops?: number): Promise<any>;
    espoGetAmmFactories(page?: number, limit?: number): Promise<any>;

    // Espo JSON-RPC methods (essentials module - extended)
    espoGetAllAlkanes(page?: number, limit?: number): Promise<any>;
    espoGetAlkaneInfo(alkaneId: string): Promise<any>;
    espoGetBlockSummary(height: number): Promise<any>;
    espoGetCirculatingSupply(alkaneId: string, height?: number): Promise<any>;
    espoGetTransferVolume(alkaneId: string, page?: number, limit?: number): Promise<any>;
    espoGetTotalReceived(alkaneId: string, page?: number, limit?: number): Promise<any>;
    espoGetAddressActivity(address: string): Promise<any>;
    espoGetAlkaneBalances(alkaneId: string): Promise<any>;
    espoGetAlkaneBalanceMetashrew(owner: string, target: string, height?: number): Promise<any>;
    espoGetAlkaneBalanceTxs(alkaneId: string, page?: number, limit?: number): Promise<any>;
    espoGetAlkaneBalanceTxsByToken(owner: string, token: string, page?: number, limit?: number): Promise<any>;
    espoGetBlockTraces(height: number): Promise<any>;
    espoGetAlkaneTxSummary(txid: string): Promise<any>;
    espoGetAlkaneBlockTxs(height: number, page?: number, limit?: number): Promise<any>;
    espoGetAlkaneAddressTxs(address: string, page?: number, limit?: number): Promise<any>;
    espoGetAddressTransactions(address: string, page?: number, limit?: number, onlyAlkaneTxs?: boolean): Promise<any>;
    espoGetAlkaneLatestTraces(): Promise<any>;
    espoGetMempoolTraces(page?: number, limit?: number, address?: string): Promise<any>;

    // Espo JSON-RPC methods (subfrost module)
    espoGetWrapEvents(count?: number, offset?: number, successful?: boolean): Promise<any>;
    espoGetWrapEventsByAddress(address: string, count?: number, offset?: number, successful?: boolean): Promise<any>;
    espoGetUnwrapEvents(count?: number, offset?: number, successful?: boolean): Promise<any>;
    espoGetUnwrapEventsByAddress(address: string, count?: number, offset?: number, successful?: boolean): Promise<any>;

    // Espo JSON-RPC methods (pizzafun module)
    espoGetSeriesIdFromAlkaneId(alkaneId: string): Promise<any>;
    espoGetSeriesIdsFromAlkaneIds(alkaneIds: string[]): Promise<any>;
    espoGetAlkaneIdFromSeriesId(seriesId: string): Promise<any>;
    espoGetAlkaneIdsFromSeriesIds(seriesIds: string[]): Promise<any>;
  }

  export default function init(): Promise<void>;
}
