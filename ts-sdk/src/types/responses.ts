/**
 * Typed response interfaces for @alkanes/ts-sdk
 *
 * These interfaces match the actual response shapes from the various APIs.
 * All amounts are returned as strings and should be parsed with the amount utilities.
 */

// ============================================================================
// COMMON TYPES
// ============================================================================

/** Transaction status information */
export interface TxStatus {
  confirmed: boolean;
  block_height?: number;
  block_hash?: string;
  block_time?: number;
}

/** Script public key information */
export interface ScriptPubKey {
  asm: string;
  hex: string;
  type: string;
  address?: string;
  desc?: string;
}

// ============================================================================
// BITCOIN RPC RESPONSE TYPES
// ============================================================================

/** Response from getblockchaininfo */
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

/** Response from getnetworkinfo */
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

/** Response from getmempoolinfo */
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

/** Transaction input from Bitcoin RPC */
export interface BitcoinVin {
  txid?: string;
  vout?: number;
  scriptSig?: {
    asm: string;
    hex: string;
  };
  txinwitness?: string[];
  sequence: number;
  coinbase?: string;
}

/** Transaction output from Bitcoin RPC */
export interface BitcoinVout {
  value: number;
  n: number;
  scriptPubKey: ScriptPubKey;
}

/** Transaction from Bitcoin RPC getrawtransaction */
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

/** Block from Bitcoin RPC getblock */
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

/** Block header from Bitcoin RPC getblockheader */
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

/** Response from estimatesmartfee */
export interface SmartFeeEstimate {
  feerate?: number;
  errors?: string[];
  blocks: number;
}

/** Chain tip from getchaintips */
export interface ChainTip {
  height: number;
  hash: string;
  branchlen: number;
  status: 'active' | 'valid-fork' | 'valid-headers' | 'headers-only' | 'invalid';
}

// ============================================================================
// ESPLORA RESPONSE TYPES
// ============================================================================

/** Address statistics */
export interface AddressStats {
  funded_txo_count: number;
  funded_txo_sum: number;
  spent_txo_count: number;
  spent_txo_sum: number;
  tx_count: number;
}

/** Response from esplora address endpoint */
export interface EsploraAddressInfo {
  address: string;
  chain_stats: AddressStats;
  mempool_stats: AddressStats;
}

/** UTXO from esplora */
export interface EsploraUtxo {
  txid: string;
  vout: number;
  value: number;
  status: TxStatus;
}

/** Transaction input from Esplora */
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

/** Transaction output from Esplora */
export interface EsploraVout {
  scriptpubkey: string;
  scriptpubkey_asm: string;
  scriptpubkey_type: string;
  scriptpubkey_address?: string;
  value: number;
}

/** Transaction from Esplora */
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

/** Block from Esplora */
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

/** Fee estimates - maps confirmation target to fee rate */
export interface FeeEstimates {
  [confirmationTarget: string]: number;
}

/** Outspend information */
export interface Outspend {
  spent: boolean;
  txid?: string;
  vin?: number;
  status?: TxStatus;
}

/** Merkle proof */
export interface MerkleProof {
  block_height: number;
  merkle: string[];
  pos: number;
}

/** Mempool statistics */
export interface MempoolStats {
  count: number;
  vsize: number;
  total_fee: number;
  fee_histogram: [number, number][];
}

/** Recent mempool transaction */
export interface MempoolRecentTx {
  txid: string;
  fee: number;
  vsize: number;
  value: number;
}

// ============================================================================
// ALKANES RESPONSE TYPES
// ============================================================================

/** Alkane token ID */
export interface AlkaneIdResponse {
  block: number;
  tx: number;
}

/** Alkane balance entry */
export interface AlkaneBalanceResponse {
  id: AlkaneIdResponse | string;
  amount: string;
  name?: string;
  symbol?: string;
  decimals?: number;
}

/** Response from alkanes reflect */
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

/** Alkane outpoint with balance */
export interface AlkaneOutpoint {
  outpoint: string;
  alkanes: AlkaneBalanceResponse[];
  value?: number;
}

/** Response from alkanes spendables */
export interface AlkaneSpendablesResponse {
  outpoints: AlkaneOutpoint[];
}

/** Simulation context */
export interface SimulationContext {
  alkanes: any[];
  transaction: any[];
  block: any[];
  height: number;
  vout: number;
  txindex: number;
  calldata: number[];
  pointer: number;
  refund_pointer: number;
}

/** Response from alkanes simulate */
export interface AlkaneSimulateResponse {
  status: 'success' | 'error';
  gasUsed?: number;
  data?: any;
  error?: string;
  logs?: string[];
}

/** Response from alkanes trace */
export interface AlkaneTraceResponse {
  txid: string;
  vout: number;
  height: number;
  traces: AlkaneTraceEntry[];
}

export interface AlkaneTraceEntry {
  op: string;
  alkane?: string;
  amount?: string;
  from?: string;
  to?: string;
}

/** Pool information */
export interface AlkanePoolResponse {
  pool_id: string;
  token0: string;
  token1: string;
  reserve0: string;
  reserve1: string;
  total_supply: string;
}

/** Response from alkanes by-address */
export interface AlkanesByAddressResponse {
  address: string;
  outpoints: AlkaneOutpoint[];
}

/** Response from alkanes sequence */
export interface AlkaneSequenceResponse {
  block: number;
  tx: number;
}

// ============================================================================
// ORD (ORDINALS) RESPONSE TYPES
// ============================================================================

/** Inscription information */
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

/** Inscriptions list response */
export interface InscriptionsListResponse {
  inscriptions: InscriptionResponse[];
  more: boolean;
  page_index: number;
}

/** Rune information */
export interface RuneResponse {
  id: string;
  name: string;
  spaced_name: string;
  number: number;
  divisibility: number;
  symbol?: string;
  etching: string;
  mint?: {
    deadline?: number;
    limit?: string;
    end?: number;
  };
  supply: string;
  burned: string;
  premine: string;
  timestamp: number;
}

/** Ordinal output */
export interface OrdOutput {
  value: number;
  script_pubkey: string;
  address?: string;
  transaction: string;
  sat_ranges?: [number, number][];
  inscriptions?: string[];
  runes?: Record<string, { amount: string; divisibility: number; symbol?: string }>;
}

/** Block info from ord */
export interface OrdBlockInfo {
  hash: string;
  height: number;
  inscriptions: string[];
  runes: string[];
  timestamp: number;
}

// ============================================================================
// BRC20-PROG RESPONSE TYPES
// ============================================================================

/** BRC20-Prog balance */
export interface Brc20ProgBalance {
  [token: string]: string;
}

/** BRC20-Prog transaction receipt */
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

/** BRC20-Prog transaction */
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

/** BRC20-Prog block */
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

// ============================================================================
// METASHREW RESPONSE TYPES
// ============================================================================

/** Response from metashrew view calls */
export interface MetashrewViewResponse {
  result: string;
}

// ============================================================================
// DATA API RESPONSE TYPES (pools, trades, candles)
// ============================================================================

/** Trade from data API */
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

/** Candle (OHLCV) data */
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

/** Pool reserves */
export interface DataApiReserves {
  pool: string;
  token0: string;
  token1: string;
  reserve0: string;
  reserve1: string;
  total_supply: string;
  last_update: string;
}

/** Holder information */
export interface DataApiHolder {
  address: string;
  amount: string;
}

/** Bitcoin price data */
export interface BitcoinPriceResponse {
  price: number;
  currency: string;
  timestamp: number;
}

/** Market chart data */
export interface MarketChartResponse {
  prices: [number, number][];
  market_caps: [number, number][];
  total_volumes: [number, number][];
}
