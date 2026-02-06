// Type declarations for @alkanes/ts-sdk
// This is a temporary workaround until proper type generation is fixed

declare module '@alkanes/ts-sdk' {
  import * as bitcoin from 'bitcoinjs-lib';
  
  // Wallet exports
  export class AlkanesWallet {
    constructor(config: any);
    deriveAddress(addressType: AddressType | string, change: number, index: number): {
      address: string;
      publicKey: string;
      path: string;
    };
    signPsbt(psbtBase64: string): Promise<string>;
    signMessage(message: string, index?: number): Promise<string>;
  }
  
  export enum AddressType {
    P2PKH = 'p2pkh',
    P2WPKH = 'p2wpkh',
    P2TR = 'p2tr',
    P2SH_P2WPKH = 'p2sh-p2wpkh',
  }
  
  export function createWallet(keystore: any): AlkanesWallet;
  export function createWalletFromMnemonic(mnemonic: string, network?: string): AlkanesWallet;
  
  // Keystore exports
  export class KeystoreManager {
    constructor();
    encrypt(mnemonic: string, password: string): Promise<any>;
    decrypt(encryptedKeystore: any, password: string): Promise<string>;
    validateMnemonic(mnemonic: string): boolean;
    createKeystore(mnemonic: string, options?: any): any;
    exportKeystore(keystore: any, password: string, options?: any): Promise<any>;
    deriveAddress(keystore: any, path: string, network?: any, options?: any): any;
  }
  
  export function createKeystore(password: string, options?: string | { network?: string; wordCount?: number; [key: string]: any }): Promise<{
    keystore: any;
    mnemonic: string;
  }>;
  
  export function unlockKeystore(encryptedKeystore: any, password: string): Promise<any>;
  
  // Provider exports
  export interface AlkanesProviderConfig {
    network?: string;
    networkType?: string;
    url?: string;
    rpcUrl?: string;
    dataApiUrl?: string;
    projectId?: string;
    version?: string;
    [key: string]: any;
  }

  // ============================================================================
  // RPC CLIENT TYPES
  // ============================================================================

  export class BitcoinRpcClient {
    getBlockCount(): Promise<number>;
    getBlockHash(height: number): Promise<string>;
    getBlock(hash: string, raw?: boolean): Promise<BitcoinBlock>;
    sendRawTransaction(hex: string): Promise<string>;
    getTransaction(txid: string, blockHash?: string): Promise<BitcoinTransaction>;
    getBlockchainInfo(): Promise<BlockchainInfo>;
    getNetworkInfo(): Promise<NetworkInfo>;
    getMempoolInfo(): Promise<MempoolInfo>;
    estimateSmartFee(target: number): Promise<SmartFeeEstimate>;
    generateToAddress(nblocks: number, address: string): Promise<string[]>;
    generateFuture(address: string): Promise<string[]>;
    getBlockHeader(hash: string): Promise<BitcoinBlockHeader>;
    getBlockStats(hash: string): Promise<Record<string, number>>;
    getChainTips(): Promise<ChainTip[]>;
    getRawMempool(): Promise<string[]>;
    getTxOut(txid: string, vout: number, includeMempool?: boolean): Promise<TxOutResponse | null>;
    decodeRawTransaction(hex: string): Promise<BitcoinTransaction>;
    decodePsbt(psbt: string): Promise<DecodedPsbt>;
  }

  export class EsploraClient {
    getAddressInfo(address: string): Promise<EsploraAddressInfo>;
    getAddressUtxos(address: string): Promise<EsploraUtxo[]>;
    getAddressTxs(address: string): Promise<EsploraTransaction[]>;
    getTx(txid: string): Promise<EsploraTransaction>;
    getTxStatus(txid: string): Promise<TxStatus>;
    getTxHex(txid: string): Promise<string>;
    getBlocksTipHeight(): Promise<number>;
    getBlocksTipHash(): Promise<string>;
    broadcastTx(txHex: string): Promise<string>;
    getFeeEstimates(): Promise<FeeEstimates>;
    getBlocks(startHeight?: number): Promise<EsploraBlock[]>;
    getBlockByHeight(height: number): Promise<EsploraBlock>;
    getBlock(hash: string): Promise<EsploraBlock>;
    getBlockStatus(hash: string): Promise<{ in_best_chain: boolean; height?: number; next_best?: string }>;
    getBlockTxids(hash: string): Promise<string[]>;
    getBlockHeader(hash: string): Promise<string>;
    getBlockRaw(hash: string): Promise<Uint8Array>;
    getBlockTxid(hash: string, index: number): Promise<string>;
    getBlockTxs(hash: string, startIndex?: number): Promise<EsploraTransaction[]>;
    getAddressTxsChain(address: string, lastSeenTxid?: string): Promise<EsploraTransaction[]>;
    getAddressTxsMempool(address: string): Promise<EsploraTransaction[]>;
    getAddressPrefix(prefix: string): Promise<string[]>;
    getTxRaw(txid: string): Promise<Uint8Array>;
    getTxMerkleProof(txid: string): Promise<MerkleProof>;
    getTxMerkleblockProof(txid: string): Promise<string>;
    getTxOutspend(txid: string, index: number): Promise<Outspend>;
    getTxOutspends(txid: string): Promise<Outspend[]>;
    getMempool(): Promise<MempoolStats>;
    getMempoolTxids(): Promise<string[]>;
    getMempoolRecent(): Promise<MempoolRecentTx[]>;
  }

  export class AlkanesRpcClient {
    getBalance(address?: string): Promise<AlkaneBalanceResponse[]>;
    getByAddress(address: string, blockTag?: string, protocolTag?: number): Promise<AlkanesByAddressResponse>;
    getByOutpoint(outpoint: string, blockTag?: string, protocolTag?: number): Promise<AlkaneOutpointResponse>;
    getBytecode(alkaneId: string, blockTag?: string): Promise<string>;
    simulate(contractId: string, contextJson: string, blockTag?: string): Promise<AlkaneSimulateResponse>;
    execute(paramsJson: string): Promise<ExecuteResult>;
    trace(outpoint: string): Promise<AlkaneTraceResponse>;
    traceBlock(height: number): Promise<AlkaneTraceResponse[]>;
    view(contractId: string, viewFn: string, params?: Uint8Array, blockTag?: string): Promise<{ data?: any; error?: string }>;
    getAllPools(factoryId: string): Promise<string[]>;
    getAllPoolsWithDetails(factoryId: string, chunkSize?: number, maxConcurrent?: number): Promise<PoolWithDetails[]>;
    getPendingUnwraps(blockTag?: string): Promise<PendingUnwrapsResponse>;
    reflect(alkaneId: string): Promise<AlkaneReflectResponse>;
    getSequence(blockTag?: string): Promise<AlkaneSequenceResponse>;
    getSpendables(address: string): Promise<AlkaneSpendablesResponse>;
    getPoolDetails(poolId: string): Promise<AlkanePoolResponse>;
    reflectAlkaneRange(block: number, startTx: number, endTx: number): Promise<AlkaneReflectResponse[]>;
    inspect(target: string, config: any): Promise<AlkaneInspectResponse>;
  }

  export class DataApiClient {
    getPools(factoryId: string): Promise<DataApiPoolsResponse>;
    getPoolHistory(poolId: string, category?: string, limit?: number, offset?: number): Promise<DataApiPoolHistoryEvent[]>;
    getAllHistory(poolId: string, limit?: number, offset?: number): Promise<DataApiPoolHistoryEvent[]>;
    getSwapHistory(poolId: string, limit?: number, offset?: number): Promise<DataApiPoolHistoryEvent[]>;
    getMintHistory(poolId: string, limit?: number, offset?: number): Promise<DataApiPoolHistoryEvent[]>;
    getBurnHistory(poolId: string, limit?: number, offset?: number): Promise<DataApiPoolHistoryEvent[]>;
    getTrades(pool: string, startTime?: number, endTime?: number, limit?: number): Promise<TradeInfo[]>;
    getCandles(pool: string, interval: string, startTime?: number, endTime?: number, limit?: number): Promise<CandleInfo[]>;
    getReserves(pool: string): Promise<DataApiReserves>;
    getAlkanesByAddress(address: string): Promise<DataApiAddressAlkanes>;
    getAddressBalances(address: string, includeOutpoints?: boolean): Promise<AddressBalancesResponse>;
    getHolders(alkane: string, page?: number, limit?: number): Promise<HolderInfo[]>;
    getHoldersCount(alkane: string): Promise<number>;
    getKeys(alkane: string, prefix?: string, limit?: number): Promise<DataApiStorageKey[]>;
    getBitcoinPrice(): Promise<BitcoinPriceResponse>;
    getBitcoinMarketChart(days: string): Promise<MarketChartResponse>;
  }

  export class LuaClient {
    eval(script: string, args?: any[]): Promise<LuaEvalResult>;
    evalScript(script: string): Promise<LuaEvalResult>;
  }

  export class MetashrewClient {
    getHeight(): Promise<number>;
    getStateRoot(height?: number): Promise<string>;
    getBlockHash(height: number): Promise<string>;
    view(viewFn: string, payload: string, blockTag?: string): Promise<string>;
  }

  export class OrdClient {
    getInscription(id: string): Promise<InscriptionResponse>;
    getInscriptions(page?: number): Promise<InscriptionsListResponse>;
    getOutputs(address: string): Promise<OrdOutput[]>;
    getRune(name: string): Promise<RuneResponse>;
    list(outpoint: string): Promise<OrdOutput>;
    find(sat: number): Promise<{ outpoint: string; offset: number }>;
    getAddressInfo(address: string): Promise<OrdAddressInfo>;
    getBlockInfo(query: string): Promise<OrdBlockInfo>;
    getBlockCount(): Promise<number>;
    getBlocks(): Promise<OrdBlockInfo[]>;
    getChildren(inscriptionId: string, page?: number): Promise<InscriptionsListResponse>;
    getContent(inscriptionId: string): Promise<{ content_type: string; content: Uint8Array }>;
    getParents(inscriptionId: string, page?: number): Promise<InscriptionsListResponse>;
    getTxInfo(txid: string): Promise<OrdTxInfo>;
  }

  export class Brc20ProgClient {
    getBalance(address: string): Promise<Brc20ProgBalance>;
    getCode(address: string): Promise<string>;
    getBlockNumber(): Promise<number>;
    getChainId(): Promise<number>;
    getTxReceipt(hash: string): Promise<Brc20ProgTxReceipt | null>;
    getTx(hash: string): Promise<Brc20ProgTransaction | null>;
    getBlock(number: string | number, includeTxs?: boolean): Promise<Brc20ProgBlock | null>;
    call(to: string, data: string, from?: string, blockTag?: string): Promise<string>;
    estimateGas(to: string, data: string, from?: string): Promise<string>;
  }

  export class EspoClient {
    getHeight(): Promise<number>;
    ping(): Promise<string>;
    getAddressBalances(address: string, includeOutpoints?: boolean): Promise<AddressBalancesResponse>;
    getAddressOutpoints(address: string): Promise<AddressOutpointsResponse>;
    getOutpointBalances(outpoint: string): Promise<OutpointBalancesResponse>;
    getHolders(alkaneId: string, page?: number, limit?: number): Promise<HoldersResponse>;
    getHoldersCount(alkaneId: string): Promise<number>;
    getKeys(alkaneId: string, page?: number, limit?: number): Promise<KeysResponse>;
    ammdataPing(): Promise<string>;
    getCandles(pool: string, timeframe?: string, side?: string, limit?: number, page?: number): Promise<CandlesResponse>;
    getTrades(pool: string, limit?: number, page?: number, side?: string, filterSide?: string, sort?: string, dir?: string): Promise<TradesResponse>;
    getPools(limit?: number, page?: number): Promise<PoolsResponse>;
    findBestSwapPath(tokenIn: string, tokenOut: string, mode?: string, amountIn?: string, amountOut?: string, amountOutMin?: string, amountInMax?: string, availableIn?: string, feeBps?: number, maxHops?: number): Promise<SwapPathResponse>;
    getBestMevSwap(token: string, feeBps?: number, maxHops?: number): Promise<MevSwapResponse>;
  }

  export class AlkanesProvider {
    constructor(config: AlkanesProviderConfig);
    readonly networkType: 'mainnet' | 'testnet' | 'regtest';
    readonly bitcoin: BitcoinRpcClient;
    readonly esplora: EsploraClient;
    readonly alkanes: AlkanesRpcClient;
    readonly dataApi: DataApiClient;
    readonly lua: LuaClient;
    readonly metashrew: MetashrewClient;
    readonly ord: OrdClient;
    readonly brc20prog: Brc20ProgClient;
    readonly espo: EspoClient;

    initialize(): Promise<void>;
    getBalance(address: string): Promise<AddressBalance>;
    getUtxos(address: string): Promise<EsploraUtxo[]>;
    getAddressUtxos(address: string, spendStrategy?: any): Promise<EsploraUtxo[]>;
    broadcastTransaction(txHex: string): Promise<string>;
    broadcastTx(txHex: string): Promise<string>;
    getBlockHeight(): Promise<number>;
    getAddressHistory(address: string): Promise<EsploraTransaction[]>;
    getAddressTxs(address: string): Promise<EsploraTransaction[]>;
    getAddressTxsChain(address: string, lastSeenTxid?: string): Promise<EsploraTransaction[]>;
    getAddressHistoryWithTraces(address: string, excludeCoinbase?: boolean): Promise<Array<EsploraTransaction & { alkane_traces?: AlkaneTraceEntry[] }>>;
    getAlkaneBalance(address: string, alkaneId?: AlkaneId): Promise<AlkaneBalance[]>;
    getEnrichedBalances(address: string, protocolTag?: string): Promise<EnrichedBalancesResponse>;
    getAlkaneTokenDetails(params: { alkaneId: AlkaneId }): Promise<AlkaneTokenDetails>;
    getStorageAt(block: number, tx: number, path: Uint8Array): Promise<string>;
    simulateAlkanes(contractId: string, calldata: number[], blockTag?: string): Promise<AlkaneSimulateResponse>;
    executeAlkanes(params: ExecuteAlkanesParams): Promise<ExecuteResult>;
    getAllPools(factoryId: string): Promise<PoolWithDetails[]>;
    getPoolReserves(poolId: string): Promise<DataApiReserves>;
    getPoolTrades(poolId: string, limit?: number): Promise<TradeInfo[]>;
    getPoolCandles(poolId: string, interval?: string, limit?: number): Promise<CandleInfo[]>;
    getBitcoinPrice(): Promise<number>;
  }
  
  export function createProvider(config: any, wasmModule?: any): AlkanesProvider;
  
  // AMM and utility exports
  export const amm: any;
  export function executeWithBtcWrapUnwrap(...args: any[]): Promise<any>;
  export function wrapBtc(...args: any[]): Promise<any>;
  export function unwrapBtc(...args: any[]): Promise<any>;

  // UTXO type
  export interface UTXO {
    txid: string;
    vout: number;
    value: number;
    scriptPubKey?: string;
    status?: {
      confirmed: boolean;
      block_height?: number;
      block_hash?: string;
      block_time?: number;
    };
    address?: string;
  }

  // Alkane types
  export interface AlkaneId {
    block: number;
    tx: number;
  }

  export interface AlkaneBalance {
    id?: string;
    alkane_id?: string;
    balance: string;
    name?: string;
    symbol?: string;
    decimals?: number;
    [key: string]: any;
  }

  export interface FeeEstimation {
    fee: number;
    numOutputs: number;
    change: number;
    vsize: number;
    effectiveFeeRate: number;
  }

  // ============================================================================
  // RESPONSE TYPES
  // ============================================================================

  // Common types
  export interface TxStatus {
    confirmed: boolean;
    block_height?: number;
    block_hash?: string;
    block_time?: number;
  }

  export interface ScriptPubKey {
    asm: string;
    hex: string;
    type: string;
    address?: string;
    desc?: string;
  }

  // Bitcoin RPC response types
  export interface BlockchainInfo {
    chain: string;
    blocks: number;
    headers: number;
    bestblockhash: string;
    difficulty: number;
    mediantime: number;
    verificationprogress: number;
    initialblockdownload: boolean;
    chainwork: string;
    size_on_disk: number;
    pruned: boolean;
    warnings: string[];
    time: number;
    bits: string;
    target: string;
  }

  export interface NetworkInfo {
    version: number;
    subversion: string;
    protocolversion: number;
    localservices: string;
    localservicesnames: string[];
    localrelay: boolean;
    timeoffset: number;
    connections: number;
    connections_in: number;
    connections_out: number;
    networkactive: boolean;
    networks: NetworkDetails[];
    relayfee: number;
    incrementalfee: number;
    localaddresses: LocalAddress[];
    warnings: string;
  }

  export interface NetworkDetails {
    name: string;
    limited: boolean;
    reachable: boolean;
    proxy: string;
    proxy_randomize_credentials: boolean;
  }

  export interface LocalAddress {
    address: string;
    port: number;
    score: number;
  }

  export interface MempoolInfo {
    loaded: boolean;
    size: number;
    bytes: number;
    usage: number;
    total_fee: number;
    maxmempool: number;
    mempoolminfee: number;
    minrelaytxfee: number;
    incrementalrelayfee: number;
    unbroadcastcount: number;
    fullrbf: boolean;
  }

  export interface BitcoinVin {
    txid?: string;
    vout?: number;
    scriptSig?: { asm: string; hex: string };
    txinwitness?: string[];
    sequence: number;
    coinbase?: string;
  }

  export interface BitcoinVout {
    value: number;
    n: number;
    scriptPubKey: ScriptPubKey;
  }

  export interface BitcoinTransaction {
    txid: string;
    hash: string;
    version: number;
    size: number;
    vsize?: number;
    weight?: number;
    locktime: number;
    vin: BitcoinVin[];
    vout: BitcoinVout[];
    hex?: string;
    blockhash?: string;
    confirmations?: number;
    time?: number;
    blocktime?: number;
  }

  export interface BitcoinBlock {
    hash: string;
    confirmations: number;
    height: number;
    version: number;
    versionHex?: string;
    merkleroot: string;
    time: number;
    mediantime: number;
    nonce: number;
    bits: string;
    difficulty: number;
    chainwork: string;
    nTx: number;
    previousblockhash?: string;
    nextblockhash?: string;
    strippedsize: number;
    size: number;
    weight?: number;
    target: string;
    tx: BitcoinTransaction[] | string[];
  }

  export interface BitcoinBlockHeader {
    hash: string;
    confirmations: number;
    height: number;
    version: number;
    versionHex: string;
    merkleroot: string;
    time: number;
    mediantime: number;
    nonce: number;
    bits: string;
    difficulty: number;
    chainwork: string;
    nTx: number;
    previousblockhash?: string;
    nextblockhash?: string;
  }

  export interface SmartFeeEstimate {
    feerate?: number;
    errors?: string[];
    blocks: number;
  }

  export interface ChainTip {
    height: number;
    hash: string;
    branchlen: number;
    status: 'active' | 'valid-fork' | 'valid-headers' | 'headers-only' | 'invalid';
  }

  export interface TxOutResponse {
    bestblock: string;
    confirmations: number;
    value: number;
    scriptPubKey: { asm: string; hex: string; type: string; address?: string };
    coinbase: boolean;
  }

  export interface DecodedPsbt {
    tx: BitcoinTransaction;
    unknown: Record<string, string>;
    inputs: any[];
    outputs: any[];
    fee?: number;
  }

  // Esplora response types
  export interface AddressStats {
    funded_txo_count: number;
    funded_txo_sum: number;
    spent_txo_count: number;
    spent_txo_sum: number;
    tx_count: number;
  }

  export interface EsploraAddressInfo {
    address: string;
    chain_stats: AddressStats;
    mempool_stats: AddressStats;
  }

  export interface EsploraUtxo {
    txid: string;
    vout: number;
    value: number;
    status: TxStatus;
  }

  export interface EsploraVin {
    txid: string;
    vout: number;
    scriptsig: string;
    scriptsig_asm: string;
    witness?: string[];
    is_coinbase: boolean;
    sequence: number;
    prevout?: EsploraVout;
  }

  export interface EsploraVout {
    scriptpubkey: string;
    scriptpubkey_asm: string;
    scriptpubkey_type: string;
    scriptpubkey_address?: string;
    value: number;
  }

  export interface EsploraTransaction {
    txid: string;
    version: number;
    locktime: number;
    vin: EsploraVin[];
    vout: EsploraVout[];
    size: number;
    weight: number;
    fee: number;
    status: TxStatus;
  }

  export interface EsploraBlock {
    id: string;
    height: number;
    version: number;
    timestamp: number;
    tx_count: number;
    size: number;
    weight: number;
    merkle_root: string;
    previousblockhash: string;
    mediantime: number;
    nonce: number;
    bits: number;
    difficulty: number;
  }

  export interface FeeEstimates {
    [confirmationTarget: string]: number;
  }

  export interface Outspend {
    spent: boolean;
    txid?: string;
    vin?: number;
    status?: TxStatus;
  }

  export interface MerkleProof {
    block_height: number;
    merkle: string[];
    pos: number;
  }

  export interface MempoolStats {
    count: number;
    vsize: number;
    total_fee: number;
    fee_histogram: [number, number][];
  }

  export interface MempoolRecentTx {
    txid: string;
    fee: number;
    vsize: number;
    value: number;
  }

  // Alkanes response types
  export interface AlkaneIdResponse {
    block: number;
    tx: number;
  }

  export interface AlkaneBalanceResponse {
    id: AlkaneIdResponse | string;
    amount: string;
    name?: string;
    symbol?: string;
    decimals?: number;
  }

  export interface AlkaneReflectResponse {
    id: string;
    name: string;
    symbol: string;
    total_supply: string;
    cap: string;
    minted: string;
    value_per_mint: string;
    data: string;
    premine: string;
    decimals: number;
  }

  export interface AlkaneOutpoint {
    outpoint: string;
    alkanes: AlkaneBalanceResponse[];
    value?: number;
  }

  export interface AlkaneSpendablesResponse {
    outpoints: AlkaneOutpoint[];
  }

  export interface AlkaneSimulateResponse {
    status: 'success' | 'error';
    gasUsed?: number;
    data?: any;
    error?: string;
    logs?: string[];
  }

  export interface AlkaneTraceEntry {
    op: string;
    alkane?: string;
    amount?: string;
    from?: string;
    to?: string;
  }

  export interface AlkaneTraceResponse {
    txid: string;
    vout: number;
    height: number;
    traces: AlkaneTraceEntry[];
  }

  export interface AlkanePoolResponse {
    pool_id: string;
    token0: string;
    token1: string;
    reserve0: string;
    reserve1: string;
    total_supply: string;
  }

  export interface AlkanesByAddressResponse {
    address: string;
    outpoints: AlkaneOutpoint[];
  }

  export interface AlkaneSequenceResponse {
    block: number;
    tx: number;
  }

  export interface AlkaneOutpointResponse {
    outpoint: string;
    alkanes: AlkaneBalanceResponse[];
    value?: number;
  }

  export interface PendingUnwrapsResponse {
    unwraps: Array<{ txid: string; vout: number; amount: string; recipient: string }>;
  }

  export interface AlkaneInspectResponse {
    storage?: Record<string, string>;
    balances?: AlkaneBalanceResponse[];
    metadata?: AlkaneReflectResponse;
  }

  export interface ExecuteResult {
    txid: string;
    rawTx: string;
    fee: number;
    size: number;
  }

  export interface ExecuteAlkanesParams {
    contractId: string;
    calldata: number[];
    feeRate?: number;
    inputs?: any[];
  }

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

  export interface HolderInfo {
    address: string;
    amount: string;
  }

  export interface EnrichedBalancesResponse {
    address: string;
    btc: { confirmed: number; unconfirmed: number };
    alkanes: AlkaneBalanceResponse[];
    outpoints: Array<{ outpoint: string; value: number; alkanes: AlkaneBalanceResponse[] }>;
  }

  export interface AlkaneTokenDetails {
    id: AlkaneId;
    name: string;
    symbol: string;
    decimals: number;
    totalSupply: string;
  }

  export interface AddressBalance {
    address: string;
    confirmed: number;
    unconfirmed: number;
    utxos: EsploraUtxo[];
  }

  // Ord response types
  export interface InscriptionResponse {
    id: string;
    number: number;
    address?: string;
    content_type?: string;
    content_length?: number;
    genesis_height: number;
    genesis_fee: number;
    genesis_transaction: string;
    location: string;
    output: string;
    offset: number;
    sat?: number;
    timestamp: number;
  }

  export interface InscriptionsListResponse {
    inscriptions: InscriptionResponse[];
    more: boolean;
    page_index: number;
  }

  export interface RuneResponse {
    id: string;
    name: string;
    spaced_name: string;
    number: number;
    divisibility: number;
    symbol?: string;
    etching: string;
    mint?: { deadline?: number; limit?: string; end?: number };
    supply: string;
    burned: string;
    premine: string;
    timestamp: number;
  }

  export interface OrdOutput {
    value: number;
    script_pubkey: string;
    address?: string;
    transaction: string;
    sat_ranges?: [number, number][];
    inscriptions?: string[];
    runes?: Record<string, { amount: string; divisibility: number; symbol?: string }>;
  }

  export interface OrdBlockInfo {
    hash: string;
    height: number;
    inscriptions: string[];
    runes: string[];
    timestamp: number;
  }

  export interface OrdAddressInfo {
    outputs: OrdOutput[];
    inscriptions: string[];
    sat_balance: number;
    runes_balances: Record<string, { amount: string; divisibility: number; symbol?: string }>;
  }

  export interface OrdTxInfo {
    txid: string;
    inscriptions: string[];
    runes: Record<string, { amount: string; divisibility: number; symbol?: string }>;
  }

  // BRC20-Prog response types
  export interface Brc20ProgBalance {
    [token: string]: string;
  }

  export interface Brc20ProgLog {
    address: string;
    topics: string[];
    data: string;
    blockNumber: number;
    transactionHash: string;
    transactionIndex: number;
    blockHash: string;
    logIndex: number;
    removed: boolean;
  }

  export interface Brc20ProgTxReceipt {
    transactionHash: string;
    transactionIndex: number;
    blockHash: string;
    blockNumber: number;
    from: string;
    to?: string;
    cumulativeGasUsed: number;
    gasUsed: number;
    contractAddress?: string;
    logs: Brc20ProgLog[];
    logsBloom: string;
    status: number;
  }

  export interface Brc20ProgTransaction {
    hash: string;
    nonce: number;
    blockHash?: string;
    blockNumber?: number;
    transactionIndex?: number;
    from: string;
    to?: string;
    value: string;
    gas: number;
    gasPrice: string;
    input: string;
  }

  export interface Brc20ProgBlock {
    number: number;
    hash: string;
    parentHash: string;
    nonce: string;
    sha3Uncles: string;
    logsBloom: string;
    transactionsRoot: string;
    stateRoot: string;
    receiptsRoot: string;
    miner: string;
    difficulty: string;
    totalDifficulty: string;
    extraData: string;
    size: number;
    gasLimit: number;
    gasUsed: number;
    timestamp: number;
    transactions: Brc20ProgTransaction[] | string[];
    uncles: string[];
  }

  // Data API response types
  export interface DataApiTrade {
    txid: string;
    vout: number;
    token0: string;
    token1: string;
    amount0_in: string;
    amount1_in: string;
    amount0_out: string;
    amount1_out: string;
    reserve0_after: string;
    reserve1_after: string;
    timestamp: string;
    block_height: number;
  }

  export interface DataApiCandle {
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

  export interface DataApiReserves {
    pool: string;
    token0: string;
    token1: string;
    reserve0: string;
    reserve1: string;
    total_supply: string;
    last_update: string;
  }

  export interface DataApiHolder {
    address: string;
    amount: string;
  }

  export interface BitcoinPriceResponse {
    price: number;
    currency: string;
    timestamp: number;
  }

  export interface MarketChartResponse {
    prices: [number, number][];
    market_caps: [number, number][];
    total_volumes: [number, number][];
  }

  export interface DataApiPoolHistoryEvent {
    txid: string;
    vout: number;
    block_height: number;
    timestamp: string;
    category: 'swap' | 'mint' | 'burn';
    amount0: string;
    amount1: string;
    reserve0_after: string;
    reserve1_after: string;
  }

  export interface DataApiPoolsResponse {
    pools: Array<{
      pool_id: string;
      token0: string;
      token1: string;
      reserve0: string;
      reserve1: string;
      total_supply: string;
    }>;
  }

  export interface DataApiStorageKey {
    key: string;
    value: string;
  }

  export interface DataApiAddressAlkanes {
    address: string;
    alkanes: Array<{
      id: string;
      amount: string;
      name?: string;
      symbol?: string;
      decimals?: number;
    }>;
  }

  export interface LuaEvalResult {
    calls: number;
    returns: any;
    runtime: number;
  }

  // Espo API response types
  export interface PaginatedResponse {
    ok: boolean;
    page: number;
    limit: number;
    total: number;
    has_more: boolean;
  }

  export interface OutpointEntry {
    alkane: string;
    amount: string;
  }

  export interface OutpointWithEntries {
    outpoint: string;
    entries: OutpointEntry[];
  }

  export interface AddressBalancesResponse {
    ok: boolean;
    address: string;
    balances: Record<string, string>;
    outpoints?: OutpointWithEntries[];
  }

  export interface AddressOutpointsResponse {
    ok: boolean;
    address: string;
    outpoints: OutpointWithEntries[];
  }

  export interface OutpointBalancesResponse {
    ok: boolean;
    outpoint: string;
    items: OutpointWithEntries[];
  }

  export interface HoldersResponse extends PaginatedResponse {
    alkane: string;
    items: HolderInfo[];
  }

  export interface HoldersCountResponse {
    ok: boolean;
    count: number;
  }

  export interface StorageKeyEntry {
    key: string;
    key_hex: string;
    value: string;
    value_hex: string;
  }

  export interface KeysResponse extends PaginatedResponse {
    alkane: string;
    items: StorageKeyEntry[];
  }

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

  export interface CandlesResponse extends PaginatedResponse {
    pool: string;
    timeframe: string;
    side: string;
    candles: EspoCandle[];
  }

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

  export interface TradesResponse extends PaginatedResponse {
    pool: string;
    side: string;
    filter_side: string;
    sort: string;
    dir: string;
    trades: EspoTrade[];
  }

  export interface EspoPool {
    pool_id: string;
    token0: string;
    token1: string;
    reserve0: string;
    reserve1: string;
    total_supply: string;
  }

  export interface PoolsResponse extends PaginatedResponse {
    pools: EspoPool[];
  }

  export interface SwapHop {
    pool: string;
    token_in: string;
    token_out: string;
    amount_in: string;
    amount_out: string;
  }

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

  // Browser wallet types
  export interface BrowserWalletInfo {
    id: string;
    name: string;
    icon: string;
    website: string;
    injectionKey: string;
    supportsPsbt: boolean;
    supportsTaproot: boolean;
    supportsOrdinals: boolean;
    mobileSupport: boolean;
    deepLinkScheme?: string;
  }

  export interface WalletAccount {
    address: string;
    publicKey?: string;
    addressType?: string;
  }

  export interface PsbtSigningOptions {
    autoFinalized?: boolean;
    toSignInputs?: Array<{
      index: number;
      address?: string;
      sighashTypes?: number[];
      disableTweakedPublicKey?: boolean;
    }>;
  }

  export class ConnectedWallet {
    readonly info: BrowserWalletInfo;
    readonly account: WalletAccount;
    readonly address: string;
    readonly publicKey: string | undefined;
    signMessage(message: string): Promise<string>;
    signPsbt(psbtHex: string, options?: PsbtSigningOptions): Promise<string>;
    getNetwork(): Promise<string>;
    disconnect(): Promise<void>;
  }

  export class WalletConnector {
    detectWallets(): Promise<BrowserWalletInfo[]>;
    connect(wallet: BrowserWalletInfo): Promise<ConnectedWallet>;
    getConnectedWallet(): ConnectedWallet | null;
    disconnect(): Promise<void>;
    isConnected(): boolean;
  }

  export const BROWSER_WALLETS: BrowserWalletInfo[];
  export function isWalletInstalled(wallet: BrowserWalletInfo): boolean;
  export function getInstalledWallets(): BrowserWalletInfo[];
  export function getWalletById(id: string): BrowserWalletInfo | undefined;

  // Storage types
  export interface WalletBackupInfo {
    folderId: string;
    folderName: string;
    walletLabel: string;
    timestamp: string;
    createdDate: string;
    hasPasswordHint: boolean;
    folderUrl: string;
  }

  export interface RestoreWalletResult {
    encryptedKeystore: string;
    passwordHint: string | null;
    walletLabel: string;
    timestamp: string;
  }

  export class KeystoreStorage {
    saveKeystore(keystoreJson: string, network: string): void;
    loadKeystore(): { keystore: string; network: string } | null;
    hasKeystore(): boolean;
    clearKeystore(): void;
    saveSessionWallet(walletState: any): void;
    loadSessionWallet(): any | null;
    clearSessionWallet(): void;
  }

  export class GoogleDriveBackup {
    constructor(clientId?: string);
    isConfigured(): boolean;
    initialize(): Promise<void>;
    requestAccess(): Promise<string>;
    clearAccess(): void;
    backupWallet(
      encryptedKeystore: string,
      walletLabel?: string,
      passwordHint?: string
    ): Promise<{ folderId: string; folderName: string; timestamp: string; folderUrl: string }>;
    listWallets(): Promise<WalletBackupInfo[]>;
    restoreWallet(folderId: string): Promise<RestoreWalletResult>;
    deleteWallet(folderId: string): Promise<void>;
  }

  export function formatBackupDate(timestamp: string): string;
  export function getRelativeTime(timestamp: string): string;

  // ============================================================================
  // Client Module - Unified ethers.js-style interface
  // ============================================================================

  // Network type
  export type NetworkType = 'mainnet' | 'testnet' | 'regtest';

  // Signer interfaces
  export interface SignerAccount {
    address: string;
    publicKey: string;
    addressType?: string;
  }

  export interface SignPsbtOptions {
    finalize?: boolean;
    extractTx?: boolean;
    inputsToSign?: Array<{
      index: number;
      address?: string;
      sighashTypes?: number[];
    }>;
  }

  export interface SignMessageOptions {
    address?: string;
  }

  export interface SignedPsbt {
    psbtHex: string;
    psbtBase64: string;
    txHex?: string;
  }

  export type SignerEventType = 'accountsChanged' | 'networkChanged' | 'disconnect';
  export type SignerEvents = {
    accountsChanged: (accounts: string[]) => void;
    networkChanged: (network: string) => void;
    disconnect: () => void;
  };

  // Abstract signer base class
  export abstract class AlkanesSigner {
    abstract readonly network: NetworkType;
    abstract getAccount(): Promise<SignerAccount>;
    abstract getAddress(): Promise<string>;
    abstract getPublicKey(): Promise<string>;
    abstract signMessage(message: string, options?: SignMessageOptions): Promise<string>;
    abstract signPsbt(psbt: string, options?: SignPsbtOptions): Promise<SignedPsbt>;
    abstract signPsbts(psbts: string[], options?: SignPsbtOptions): Promise<SignedPsbt[]>;
    abstract isConnected(): Promise<boolean>;
    abstract disconnect(): Promise<void>;
    abstract getSignerType(): string;
    protected parsePsbt(psbt: string): any;
  }

  // Event emitting signer
  export abstract class EventEmittingSigner extends AlkanesSigner {
    on<E extends SignerEventType>(event: E, callback: SignerEvents[E]): void;
    off<E extends SignerEventType>(event: E, callback: SignerEvents[E]): void;
    protected emit<E extends SignerEventType>(event: E, ...args: any[]): void;
  }

  // Keystore signer config
  export interface KeystoreSignerConfig {
    network: NetworkType;
    addressType?: 'p2wpkh' | 'p2tr' | 'p2pkh' | 'p2sh-p2wpkh';
    accountIndex?: number;
    addressIndex?: number;
  }

  // Keystore signer
  export class KeystoreSigner extends AlkanesSigner {
    static fromMnemonic(mnemonic: string, config?: Partial<KeystoreSignerConfig>): KeystoreSigner;
    static fromEncrypted(keystoreJson: string, password: string, config?: Partial<KeystoreSignerConfig>): Promise<KeystoreSigner>;
    static fromKeystore(keystore: any, config?: Partial<KeystoreSignerConfig>): KeystoreSigner;
    static generate(config?: Partial<KeystoreSignerConfig>, wordCount?: 12 | 24): KeystoreSigner;

    readonly network: NetworkType;
    getAccount(): Promise<SignerAccount>;
    getAddress(): Promise<string>;
    getPublicKey(): Promise<string>;
    signMessage(message: string, options?: SignMessageOptions): Promise<string>;
    signPsbt(psbt: string, options?: SignPsbtOptions): Promise<SignedPsbt>;
    signPsbts(psbts: string[], options?: SignPsbtOptions): Promise<SignedPsbt[]>;
    isConnected(): Promise<boolean>;
    disconnect(): Promise<void>;
    getSignerType(): string;

    exportMnemonic(): string;
    exportToKeystore(password: string): Promise<string>;
    getAddressInfo(addressType: AddressType | string, index?: number): {
      address: string;
      publicKey: string;
      path: string;
    };
    deriveAddress(type: 'p2wpkh' | 'p2tr' | 'p2pkh' | 'p2sh-p2wpkh', index: number): {
      address: string;
      publicKey: string;
      path: string;
    };
    getAddresses(count: number): Array<{ index: number; address: string; publicKey: string; path: string }>;
  }

  // Browser wallet signer config
  export interface BrowserWalletSignerConfig {
    autoReconnect?: boolean;
    preferredAddressType?: 'payment' | 'ordinals' | 'both';
  }

  export interface WalletSelection {
    walletId: string;
    walletName: string;
    walletInfo: BrowserWalletInfo;
  }

  // Browser wallet signer
  export class BrowserWalletSigner extends EventEmittingSigner {
    static getAvailableWallets(): Promise<BrowserWalletInfo[]>;
    static getSupportedWallets(): BrowserWalletInfo[];
    static isWalletInstalled(walletId: string): boolean;
    static connect(walletId: string, config?: BrowserWalletSignerConfig): Promise<BrowserWalletSigner>;
    static connectAny(config?: BrowserWalletSignerConfig): Promise<BrowserWalletSigner>;
    static fromConnectedWallet(wallet: ConnectedWallet, config?: BrowserWalletSignerConfig): BrowserWalletSigner;

    readonly network: NetworkType;
    getSignerType(): string;
    getWalletInfo(): BrowserWalletInfo;
    getAdapter(): any; // JsWalletAdapter for WASM integration
    getAccount(): Promise<SignerAccount>;
    getAddress(): Promise<string>;
    getPublicKey(): Promise<string>;
    signMessage(message: string, options?: SignMessageOptions): Promise<string>;
    signPsbt(psbt: string, options?: SignPsbtOptions): Promise<SignedPsbt>;
    signPsbts(psbts: string[], options?: SignPsbtOptions): Promise<SignedPsbt[]>;
    isConnected(): Promise<boolean>;
    disconnect(): Promise<void>;

    pushTransaction(txHex: string): Promise<string>;
    pushPsbt(psbtHex: string): Promise<string>;
    getBalance(): Promise<number | null>;
    getInscriptions(cursor?: number, size?: number): Promise<any>;
    switchNetwork(network: NetworkType): Promise<void>;
  }

  // Transaction result
  export interface TransactionResult {
    txid: string;
    rawTx: string;
    broadcast: boolean;
  }

  // Balance summary
  export interface BalanceSummary {
    confirmed: number;
    unconfirmed: number;
    total: number;
    utxos: any[];
  }

  export interface EnrichedBalance extends BalanceSummary {
    alkanes: any[];
  }

  // Wallet option for UI
  export interface WalletOption {
    id: string;
    name: string;
    icon: string;
    installed: boolean;
  }

  // Unified AlkanesClient
  export class AlkanesClient {
    constructor(provider: AlkanesProvider, signer: AlkanesSigner);

    readonly provider: AlkanesProvider;
    readonly signer: AlkanesSigner;

    // Static factory methods
    static withBrowserWallet(walletId: string, network?: string, signerConfig?: BrowserWalletSignerConfig): Promise<AlkanesClient>;
    static withAnyBrowserWallet(network?: string, signerConfig?: BrowserWalletSignerConfig): Promise<AlkanesClient>;
    static withKeystore(keystoreJson: string, password: string, network?: string, signerConfig?: Partial<KeystoreSignerConfig>): Promise<AlkanesClient>;
    static withMnemonic(mnemonic: string, network?: string, signerConfig?: Partial<KeystoreSignerConfig>): AlkanesClient;
    static fromKeystore(keystore: any, network?: string, signerConfig?: Partial<KeystoreSignerConfig>): AlkanesClient;
    static generate(network?: string, wordCount?: 12 | 24, signerConfig?: Partial<KeystoreSignerConfig>): AlkanesClient;

    // Initialization
    initialize(): Promise<void>;
    isReady(): Promise<boolean>;

    // Account methods (from Signer)
    getAddress(): Promise<string>;
    getPublicKey(): Promise<string>;
    getAccount(): Promise<SignerAccount>;
    getSignerType(): string;
    getNetwork(): NetworkType;

    // Balance methods (from Provider)
    getBalance(address?: string): Promise<BalanceSummary>;
    getEnrichedBalances(address?: string): Promise<any>;
    getAlkaneBalances(address?: string): Promise<any[]>;
    getUtxos(address?: string): Promise<any[]>;

    // Signing methods (from Signer)
    signMessage(message: string, options?: SignMessageOptions): Promise<string>;
    signPsbt(psbt: string, options?: SignPsbtOptions): Promise<SignedPsbt>;
    signPsbts(psbts: string[], options?: SignPsbtOptions): Promise<SignedPsbt[]>;

    // Transaction methods
    sendTransaction(psbt: string, options?: SignPsbtOptions): Promise<TransactionResult>;
    signTransaction(psbt: string, options?: SignPsbtOptions): Promise<SignedPsbt>;
    broadcastTransaction(txHex: string): Promise<string>;

    // Alkanes methods
    getBlockHeight(): Promise<number>;
    getTransactionHistory(address?: string): Promise<any[]>;
    getTransactionHistoryWithTraces(address?: string): Promise<any[]>;
    getAlkaneTokenDetails(alkaneId: any): Promise<any>;
    simulateAlkanes(contractId: string, calldata: number[]): Promise<any>;

    // AMM/DEX methods
    getPools(factoryId: string): Promise<any[]>;
    getPoolReserves(poolId: string): Promise<any>;
    getPoolTrades(poolId: string, limit?: number): Promise<any[]>;
    getPoolCandles(poolId: string, interval?: string, limit?: number): Promise<any[]>;

    // Utility methods
    getBitcoinPrice(): Promise<number>;
    disconnect(): Promise<void>;

    // Sub-clients
    readonly bitcoin: any;
    readonly esplora: any;
    readonly alkanes: any;
    readonly dataApi: any;
    readonly lua: any;
    readonly metashrew: any;
  }

  // Connect wallet utilities
  export function getAvailableWallets(): Promise<WalletOption[]>;
  export function connectWallet(walletId: string, network?: string): Promise<AlkanesClient>;
  export function connectAnyWallet(network?: string): Promise<AlkanesClient>;
  export function createReadOnlyProvider(network?: string): AlkanesProvider;
  export function getWalletOptions(): Promise<Array<{
    id: string;
    name: string;
    icon: string;
    installed: boolean;
    info: BrowserWalletInfo;
  }>>;

  // WASM wallet adapter types
  export interface JsWalletAdapter {
    getInfo(): any;
    connect(): Promise<any>;
    disconnect(): Promise<void>;
    getAccounts(): Promise<any[]>;
    getNetwork(): Promise<string>;
    getPublicKey(): Promise<string>;
    getBalance(): Promise<number | null>;
    signMessage(message: string, address: string): Promise<string>;
    signPsbt(psbtHex: string, options?: any): Promise<string>;
    signPsbts(psbtHexs: string[], options?: any): Promise<string[]>;
    pushTx(txHex: string): Promise<string>;
    pushPsbt(psbtHex: string): Promise<string>;
    switchNetwork(network: string): Promise<void>;
    getInscriptions(cursor?: number, size?: number): Promise<any>;
  }

  export interface WalletInfoForWasm {
    id: string;
    name: string;
    icon: string;
    injection_key: string;
    supports_psbt: boolean;
    supports_taproot: boolean;
    supports_ordinals: boolean;
    mobile_support: boolean;
  }

  export interface WalletAccountForWasm {
    address: string;
    public_key?: string;
    address_type?: string;
  }

  export interface PsbtSigningOptionsForWasm {
    auto_finalized?: boolean;
    to_sign_inputs?: Array<{
      index: number;
      address?: string;
      sighash_types?: number[];
    }>;
  }

  export function createWalletAdapter(wallet: ConnectedWallet): JsWalletAdapter;
  export class MockWalletAdapter implements JsWalletAdapter {
    constructor(network?: string, address?: string);
    getInfo(): any;
    connect(): Promise<any>;
    disconnect(): Promise<void>;
    getAccounts(): Promise<any[]>;
    getNetwork(): Promise<string>;
    getPublicKey(): Promise<string>;
    getBalance(): Promise<number | null>;
    signMessage(message: string, address: string): Promise<string>;
    signPsbt(psbtHex: string, options?: any): Promise<string>;
    signPsbts(psbtHexs: string[], options?: any): Promise<string[]>;
    pushTx(txHex: string): Promise<string>;
    pushPsbt(psbtHex: string): Promise<string>;
    switchNetwork(network: string): Promise<void>;
    getInscriptions(cursor?: number, size?: number): Promise<any>;
  }
  export class BaseWalletAdapter implements JsWalletAdapter {
    constructor(wallet: ConnectedWallet);
    getInfo(): any;
    connect(): Promise<any>;
    disconnect(): Promise<void>;
    getAccounts(): Promise<any[]>;
    getNetwork(): Promise<string>;
    getPublicKey(): Promise<string>;
    getBalance(): Promise<number | null>;
    signMessage(message: string, address: string): Promise<string>;
    signPsbt(psbtHex: string, options?: any): Promise<string>;
    signPsbts(psbtHexs: string[], options?: any): Promise<string[]>;
    pushTx(txHex: string): Promise<string>;
    pushPsbt(psbtHex: string): Promise<string>;
    switchNetwork(network: string): Promise<void>;
    getInscriptions(cursor?: number, size?: number): Promise<any>;
  }
  export class UnisatAdapter extends BaseWalletAdapter {}
  export class XverseAdapter extends BaseWalletAdapter {}
  export class OkxAdapter extends BaseWalletAdapter {}
  export class LeatherAdapter extends BaseWalletAdapter {}
  export class PhantomAdapter extends BaseWalletAdapter {}
  export class MagicEdenAdapter extends BaseWalletAdapter {}
  export class WizzAdapter extends BaseWalletAdapter {}

  // Utility functions
  export function getNetwork(networkType: string): any;
  export function validateAddress(address: string, network?: any): boolean;
  export function satoshisToBTC(satoshis: number): number;
  export function btcToSatoshis(btc: number): number;
  export function formatAlkaneId(alkaneId: AlkaneId | string): string;
  export function parseAlkaneId(alkaneIdStr: string): AlkaneId;
  export function delay(ms: number): Promise<void>;
  export function retry<T>(fn: () => Promise<T>, retries?: number, delayMs?: number): Promise<T>;
  export function calculateFee(vbytes: number, feeRate: number): number;
  export function estimateTxSize(inputs: number, outputs: number): number;
  export function hexToBytes(hex: string): Uint8Array;
  export function bytesToHex(bytes: Uint8Array): string;
  export function reverseBytes(bytes: Uint8Array): Uint8Array;
  export function reversedHex(hex: string): string;
  export function isBrowser(): boolean;
  export function isNode(): boolean;
  export function safeJsonParse<T>(json: string, defaultValue: T): T;
  export function formatTimestamp(timestamp: number): string;
  export function calculateWeight(vbytes: number): number;
  export function weightToVsize(weight: number): number;

  // Fee estimation
  export const DUST_THRESHOLD: number;
  export const INPUT_VSIZE: Record<string, number>;
  export const OUTPUT_VSIZE: Record<string, number>;
  export const TX_OVERHEAD_VSIZE: number;
  export function computeSendFee(params: {
    inputCount: number;
    sendAmount: number;
    totalInputValue: number;
    feeRate: number;
    inputType?: 'legacy' | 'segwit' | 'taproot';
    recipientType?: 'legacy' | 'segwit' | 'taproot';
    changeType?: 'legacy' | 'segwit' | 'taproot';
    dustThreshold?: number;
  }): FeeEstimation;
  export function estimateSelectionFee(
    inputCount: number,
    feeRate: number,
    inputType?: 'legacy' | 'segwit' | 'taproot',
    outputCount?: number,
    outputType?: 'legacy' | 'segwit' | 'taproot',
  ): number;

  // Network presets
  export const NETWORK_PRESETS: {
    mainnet: AlkanesProviderConfig;
    testnet: AlkanesProviderConfig;
    signet: AlkanesProviderConfig;
    regtest: AlkanesProviderConfig;
  };

  // Other exports
  export const VERSION: string;
  export function initSDK(wasmModule?: any): Promise<any>;
  export default function getAlkanesSDK(): Promise<any>;
}
