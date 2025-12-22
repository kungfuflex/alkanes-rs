/**
 * @alkanes/ts-sdk
 * 
 * TypeScript SDK for Alkanes - Bitcoin smart contracts
 * 
 * This SDK provides:
 * - Wallet management with HD derivation (BIP32/44/84/86)
 * - Keystore encryption (ethers.js compatible)
 * - PSBT creation and signing
 * - Provider integration (@oyl/sdk compatible)
 * - Alkanes contract interaction
 * - WASM backend integration (alkanes-web-sys)
 * 
 * @example
 * ```typescript
 * import { createKeystore, unlockKeystore, createWallet, createProvider } from '@alkanes/ts-sdk';
 * 
 * // Create a new wallet
 * const { keystore, mnemonic } = await createKeystore('password123');
 * console.log('Save this mnemonic:', mnemonic);
 * 
 * // Later, unlock the keystore
 * const unlockedKeystore = await unlockKeystore(keystore, 'password123');
 * const wallet = createWallet(unlockedKeystore);
 * 
 * // Get addresses
 * const address = wallet.getReceivingAddress(0);
 * console.log('Address:', address);
 * 
 * // Create provider
 * const provider = createProvider({
 *   url: 'https://api.example.com',
 *   network: bitcoin.networks.bitcoin,
 *   networkType: 'mainnet',
 * });
 * 
 * // Get balance
 * const balance = await provider.getBalance(address);
 * console.log('Balance:', balance);
 * ```
 */

// Keystore exports
export {
  KeystoreManager,
  DERIVATION_PATHS,
  createKeystore,
  unlockKeystore,
} from './keystore';

// Wallet exports
export {
  AlkanesWallet,
  AddressType,
  createWallet,
  createWalletFromMnemonic,
} from './wallet';

// Provider exports
export {
  AlkanesProvider,
  BitcoinRpcClient,
  EsploraClient,
  AlkanesRpcClient,
  MetashrewClient,
  DataApiClient,
  LuaClient,
  EspoClient,
  createProvider,
  NETWORK_PRESETS,
} from './provider';

export type {
  AlkanesProviderConfig,
  LogLevel,
  PoolDetails,
  PoolWithDetails,
  TradeInfo,
  CandleInfo,
  HolderInfo,
  ExecuteResult,
  RpcExecuteParams,
  LuaEvalResult,
  // Espo API response types
  PaginatedResponse,
  OutpointEntry,
  OutpointWithEntries,
  AddressBalancesResponse,
  AddressOutpointsResponse,
  OutpointBalancesResponse,
  HoldersResponse,
  HoldersCountResponse,
  StorageKeyEntry,
  KeysResponse,
  EspoCandle,
  CandlesResponse,
  EspoTrade,
  TradesResponse,
  EspoPool,
  PoolsResponse,
  SwapHop,
  SwapPathResponse,
  MevSwapResponse,
} from './provider';

// Amount parsing utilities
export {
  DEFAULT_DECIMALS,
  parseAmount,
  formatAmount,
  toRawAmount,
  parseAlkaneBalance,
  parseAlkaneBalances,
  parseReflectMetadata,
  parsePoolDetails,
  parseTrade,
  satsToBtc,
  btcToSats,
} from './utils/amounts';

export type {
  ParsedAlkaneBalance,
  ParsedReflectMetadata,
  ParsedPoolDetails,
  ParsedTrade,
} from './utils/amounts';

// Type exports
export type {
  NetworkType,
  HDPath,
  KeystoreParams,
  EncryptedKeystore,
  Keystore,
  WalletConfig,
  AddressInfo,
  TxInput,
  TxOutput,
  PsbtOptions,
  AlkaneId,
  AlkaneBalance,
  AlkaneCallParams,
  ProviderConfig,
  TransactionResult,
  BlockInfo,
  UTXO,
  AddressBalance,
  ExportOptions,
  ImportOptions,
  // BRC20-Prog types
  Brc20ProgExecuteParams,
  Brc20ProgDeployParams,
  Brc20ProgTransactParams,
  Brc20ProgWrapBtcParams,
  Brc20ProgExecuteResult,
  // Alkanes types (frBTC, AMM, execute)
  AlkanesAlkaneId,
  AlkanesExecuteBaseParams,
  FrbtcWrapParams,
  FrbtcUnwrapParams,
  FrbtcWrapAndExecuteParams,
  FrbtcWrapAndExecute2Params,
  AlkanesSwapParams,
  AlkanesInitPoolParams,
  AlkanesExecuteParams,
  AlkanesExecuteResult,
  PendingUnwrap,
  PendingUnwrapsResult,
  PoolDetailsResult,
  SignerAddressResult,
} from './types';

// Response type exports (typed API responses)
export type {
  // Common types
  TxStatus,
  ScriptPubKey,
  // Bitcoin RPC response types
  BlockchainInfo,
  NetworkInfo,
  NetworkDetails,
  LocalAddress,
  MempoolInfo,
  BitcoinVin,
  BitcoinVout,
  BitcoinTransaction,
  BitcoinBlock,
  BitcoinBlockHeader,
  SmartFeeEstimate,
  ChainTip,
  // Esplora response types
  AddressStats,
  EsploraAddressInfo,
  EsploraUtxo,
  EsploraVin,
  EsploraVout,
  EsploraTransaction,
  EsploraBlock,
  FeeEstimates,
  Outspend,
  MerkleProof,
  MempoolStats,
  MempoolRecentTx,
  // Alkanes response types
  AlkaneIdResponse,
  AlkaneBalanceResponse,
  AlkaneReflectResponse,
  AlkaneOutpoint,
  AlkaneSpendablesResponse,
  SimulationContext,
  AlkaneSimulateResponse,
  AlkaneTraceResponse,
  AlkaneTraceEntry,
  AlkanePoolResponse,
  AlkanesByAddressResponse,
  AlkaneSequenceResponse,
  // Ord (Ordinals) response types
  InscriptionResponse,
  InscriptionsListResponse,
  RuneResponse,
  OrdOutput,
  OrdBlockInfo,
  // BRC20-Prog response types
  Brc20ProgBalance,
  Brc20ProgTxReceipt,
  Brc20ProgLog,
  Brc20ProgTransaction,
  Brc20ProgBlock,
  // Metashrew response types
  MetashrewViewResponse,
  // Data API response types
  DataApiTrade,
  DataApiCandle,
  DataApiReserves,
  DataApiHolder,
  BitcoinPriceResponse,
  MarketChartResponse,
  DataApiPoolHistoryEvent,
  DataApiPoolsResponse,
  DataApiStorageKey,
  DataApiAddressAlkanes,
} from './types/responses';

// Utility exports
export {
  getNetwork,
  validateAddress,
  satoshisToBTC,
  btcToSatoshis,
  formatAlkaneId,
  parseAlkaneId,
  delay,
  retry,
  calculateFee,
  estimateTxSize,
  hexToBytes,
  bytesToHex,
  reverseBytes,
  reversedHex,
  isBrowser,
  isNode,
  safeJsonParse,
  formatTimestamp,
  calculateWeight,
  weightToVsize,
  // WASM utilities
  analyzeRunestone,
} from './utils';

export type {
  Protostone,
  ProtostoneEdict,
  RunestoneAnalysisResult,
} from './utils';

// Browser wallet exports
export {
  WalletConnector,
  ConnectedWallet,
  BROWSER_WALLETS,
  isWalletInstalled,
  getInstalledWallets,
  getWalletById,
  // Wallet adapters for WASM integration
  createWalletAdapter,
  MockWalletAdapter,
  BaseWalletAdapter,
  UnisatAdapter,
  XverseAdapter,
  OkxAdapter,
  LeatherAdapter,
  PhantomAdapter,
  MagicEdenAdapter,
  WizzAdapter,
} from './browser-wallets';

export type {
  BrowserWalletInfo,
  WalletAccount,
  PsbtSigningOptions,
  // WASM adapter types
  JsWalletAdapter,
  WalletInfoForWasm,
  WalletAccountForWasm,
  PsbtSigningOptionsForWasm,
} from './browser-wallets';

// Client module - unified ethers.js-style interface
export {
  // Core client
  AlkanesClient,
  // Signers
  AlkanesSigner,
  KeystoreSigner,
  BrowserWalletSigner,
  EventEmittingSigner,
  // Utilities
  connectWallet,
  connectAnyWallet,
  getAvailableWallets,
  createReadOnlyProvider,
  getWalletOptions,
} from './client';

export type {
  SignPsbtOptions,
  SignMessageOptions,
  SignerAccount,
  SignedPsbt,
  SignerEventType,
  SignerEvents,
  KeystoreSignerConfig,
  BrowserWalletSignerConfig,
  WalletSelection,
  // Note: TransactionResult is already exported from './types'
  BalanceSummary,
  EnrichedBalance,
  WalletOption,
} from './client';

// Storage and backup exports
export {
  KeystoreStorage,
  GoogleDriveBackup,
  formatBackupDate,
  getRelativeTime,
} from './storage';

export type {
  WalletBackupInfo,
  RestoreWalletResult,
} from './storage';

// Version
export const VERSION = '0.1.0';

/**
 * Initialize the SDK with WASM module
 * 
 * @example
 * ```typescript
 * import { initSDK } from '@alkanes/ts-sdk';
 *
 * const sdk = await initSDK();
 * ```
 */
export async function initSDK() {
  // Import dynamically to avoid circular dependencies
  const { KeystoreManager, createKeystore, unlockKeystore } = await import('./keystore');
  const { AlkanesWallet, createWallet, createWalletFromMnemonic } = await import('./wallet');
  const { AlkanesProvider, createProvider } = await import('./provider');
  const {
    WalletConnector,
    ConnectedWallet,
    BROWSER_WALLETS,
    isWalletInstalled,
    getInstalledWallets,
    createWalletAdapter,
    MockWalletAdapter,
    BaseWalletAdapter,
    UnisatAdapter,
    XverseAdapter,
    OkxAdapter,
    LeatherAdapter,
    PhantomAdapter,
    MagicEdenAdapter,
    WizzAdapter,
  } = await import('./browser-wallets');
  const { KeystoreStorage, GoogleDriveBackup } = await import('./storage');

  return {
    // Keystore
    KeystoreManager,
    createKeystore,
    unlockKeystore,
    // Wallet
    AlkanesWallet,
    createWallet,
    createWalletFromMnemonic,
    // Provider
    AlkanesProvider,
    createProvider,
    // Browser wallets
    WalletConnector,
    ConnectedWallet,
    BROWSER_WALLETS,
    isWalletInstalled,
    getInstalledWallets,
    // Wallet adapters for WASM integration
    createWalletAdapter,
    MockWalletAdapter,
    BaseWalletAdapter,
    UnisatAdapter,
    XverseAdapter,
    OkxAdapter,
    LeatherAdapter,
    PhantomAdapter,
    MagicEdenAdapter,
    WizzAdapter,
    // Storage
    KeystoreStorage,
    GoogleDriveBackup,
    // Meta
    version: VERSION,
  };
}

// Default export - function that returns SDK object at call time (not module load time)
export default async function getAlkanesSDK() {
  const { KeystoreManager, createKeystore, unlockKeystore } = await import('./keystore');
  const { AlkanesWallet, createWallet, createWalletFromMnemonic } = await import('./wallet');
  const { AlkanesProvider, createProvider } = await import('./provider');
  const {
    WalletConnector,
    ConnectedWallet,
    BROWSER_WALLETS,
    isWalletInstalled,
    getInstalledWallets,
    createWalletAdapter,
    MockWalletAdapter,
    BaseWalletAdapter,
    UnisatAdapter,
    XverseAdapter,
    OkxAdapter,
    LeatherAdapter,
    PhantomAdapter,
    MagicEdenAdapter,
    WizzAdapter,
  } = await import('./browser-wallets');
  const { KeystoreStorage, GoogleDriveBackup } = await import('./storage');

  return {
    // Keystore
    KeystoreManager,
    createKeystore,
    unlockKeystore,
    // Wallet
    AlkanesWallet,
    createWallet,
    createWalletFromMnemonic,
    // Provider
    AlkanesProvider,
    createProvider,
    // Browser wallets
    WalletConnector,
    ConnectedWallet,
    BROWSER_WALLETS,
    isWalletInstalled,
    getInstalledWallets,
    // Wallet adapters for WASM integration
    createWalletAdapter,
    MockWalletAdapter,
    BaseWalletAdapter,
    UnisatAdapter,
    XverseAdapter,
    OkxAdapter,
    LeatherAdapter,
    PhantomAdapter,
    MagicEdenAdapter,
    WizzAdapter,
    // Storage
    KeystoreStorage,
    GoogleDriveBackup,
    // Meta
    initSDK,
    VERSION,
  };
}
