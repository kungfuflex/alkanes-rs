import * as bitcoin from 'bitcoinjs-lib';

/**
 * Core type definitions for @alkanes/ts-sdk
 */

/**
 * Bitcoin network types
 */
type NetworkType = 'mainnet' | 'testnet' | 'signet' | 'regtest';
/**
 * HD wallet derivation path configuration
 */
interface HDPath {
    purpose: number;
    coinType: number;
    account: number;
    change: number;
    index: number;
}
/**
 * Keystore encryption parameters (compatible with ethers.js style)
 */
interface KeystoreParams {
    salt: string;
    nonce?: string;
    iterations: number;
    algorithm?: string;
}
/**
 * Encrypted keystore JSON structure (ethers.js compatible)
 */
interface EncryptedKeystore {
    encrypted_mnemonic: string;
    master_fingerprint: string;
    created_at: number;
    version: string;
    pbkdf2_params: KeystoreParams;
    account_xpub: string;
    hd_paths: Record<string, string>;
}
/**
 * Decrypted keystore object (in-memory only)
 */
interface Keystore {
    mnemonic: string;
    masterFingerprint: string;
    accountXpub: string;
    hdPaths: Record<string, HDPath>;
    network: NetworkType;
    createdAt: number;
}
/**
 * Wallet configuration
 */
interface WalletConfig {
    network: NetworkType;
    derivationPath?: string;
    account?: number;
}
/**
 * Address information
 */
interface AddressInfo {
    address: string;
    path: string;
    publicKey: string;
    index: number;
}
/**
 * Transaction input
 */
interface TxInput {
    txid: string;
    vout: number;
    value: number;
    address: string;
}
/**
 * Transaction output
 */
interface TxOutput {
    address: string;
    value: number;
}
/**
 * PSBT build options
 */
interface PsbtOptions {
    inputs: TxInput[];
    outputs: TxOutput[];
    feeRate?: number;
    network?: bitcoin.networks.Network;
}
/**
 * Alkane token ID
 */
interface AlkaneId {
    block: number;
    tx: number;
}
/**
 * Alkane balance information
 */
interface AlkaneBalance {
    id: AlkaneId;
    amount: string;
    name?: string;
    symbol?: string;
    decimals?: number;
}
/**
 * Alkane call parameters
 */
interface AlkaneCallParams {
    alkaneId: AlkaneId;
    method: string;
    args: any[];
    value?: number;
}
/**
 * Provider configuration for @oyl/sdk compatibility
 */
interface ProviderConfig {
    url: string;
    projectId?: string;
    network: bitcoin.networks.Network;
    networkType: NetworkType;
    version?: string;
}
/**
 * Transaction result
 */
interface TransactionResult {
    txId: string;
    rawTx: string;
    size: number;
    weight: number;
    fee: number;
    satsPerVByte: string;
}
/**
 * Block information
 */
interface BlockInfo {
    hash: string;
    height: number;
    timestamp: number;
    txCount: number;
}
/**
 * UTXO information
 */
interface UTXO {
    txid: string;
    vout: number;
    value: number;
    status: {
        confirmed: boolean;
        block_height?: number;
        block_hash?: string;
        block_time?: number;
    };
}
/**
 * Address balance
 */
interface AddressBalance {
    address: string;
    confirmed: number;
    unconfirmed: number;
    utxos: UTXO[];
}
/**
 * Export options
 */
interface ExportOptions {
    format?: 'json' | 'string';
    pretty?: boolean;
}
/**
 * Import options
 */
interface ImportOptions {
    validate?: boolean;
    network?: NetworkType;
}

/**
 * Provider integration for Alkanes SDK
 *
 * Provides a clean TypeScript wrapper over the WebProvider WASM bindings.
 * Compatible with @oyl/sdk Provider interface patterns.
 */

type WasmWebProvider = any;
declare const NETWORK_PRESETS: Record<string, {
    rpcUrl: string;
    dataApiUrl: string;
    networkType: NetworkType;
}>;
interface AlkanesProviderConfig {
    /** Network type or preset name */
    network: string;
    /** Custom RPC URL (overrides preset) */
    rpcUrl?: string;
    /** Custom Data API URL (overrides preset, defaults to rpcUrl) */
    dataApiUrl?: string;
    /** bitcoinjs-lib network (auto-detected if not provided) */
    bitcoinNetwork?: bitcoin.Network;
}
interface PoolDetails {
    token0: AlkaneId;
    token1: AlkaneId;
    reserve0: string;
    reserve1: string;
    totalSupply: string;
}
interface PoolWithDetails {
    poolId: AlkaneId;
    details: PoolDetails | null;
}
interface TradeInfo {
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
interface CandleInfo {
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
interface HolderInfo {
    address: string;
    amount: string;
}
interface ExecuteResult {
    txid: string;
    rawTx: string;
    fee: number;
    size: number;
}
/**
 * Bitcoin RPC client (uses WebProvider internally)
 */
declare class BitcoinRpcClient {
    private provider;
    constructor(provider: WasmWebProvider);
    getBlockCount(): Promise<number>;
    getBlockHash(height: number): Promise<string>;
    getBlock(hash: string, raw?: boolean): Promise<any>;
    sendRawTransaction(hex: string): Promise<string>;
    getTransaction(txid: string, blockHash?: string): Promise<any>;
    getBlockchainInfo(): Promise<any>;
    getNetworkInfo(): Promise<any>;
    getMempoolInfo(): Promise<any>;
    estimateSmartFee(target: number): Promise<any>;
    generateToAddress(nblocks: number, address: string): Promise<any>;
}
/**
 * Esplora API client (uses WebProvider internally)
 */
declare class EsploraClient {
    private provider;
    constructor(provider: WasmWebProvider);
    getAddressInfo(address: string): Promise<any>;
    getAddressUtxos(address: string): Promise<UTXO[]>;
    getAddressTxs(address: string): Promise<any[]>;
    getTx(txid: string): Promise<any>;
    getTxStatus(txid: string): Promise<any>;
    getTxHex(txid: string): Promise<string>;
    getBlocksTipHeight(): Promise<number>;
    getBlocksTipHash(): Promise<string>;
    broadcastTx(txHex: string): Promise<string>;
}
/**
 * Alkanes RPC client (uses WebProvider internally)
 */
declare class AlkanesRpcClient {
    private provider;
    constructor(provider: WasmWebProvider);
    getBalance(address?: string): Promise<AlkaneBalance[]>;
    getByAddress(address: string, blockTag?: string, protocolTag?: number): Promise<any>;
    getByOutpoint(outpoint: string, blockTag?: string, protocolTag?: number): Promise<any>;
    getBytecode(alkaneId: string, blockTag?: string): Promise<string>;
    simulate(contractId: string, contextJson: string, blockTag?: string): Promise<any>;
    execute(paramsJson: string): Promise<any>;
    trace(outpoint: string): Promise<any>;
    view(contractId: string, viewFn: string, params?: Uint8Array, blockTag?: string): Promise<any>;
    getAllPools(factoryId: string): Promise<any>;
    getAllPoolsWithDetails(factoryId: string, chunkSize?: number, maxConcurrent?: number): Promise<PoolWithDetails[]>;
    getPendingUnwraps(blockTag?: string): Promise<any>;
}
/**
 * Data API client (uses WebProvider internally)
 */
declare class DataApiClient {
    private provider;
    constructor(provider: WasmWebProvider);
    getPools(factoryId: string): Promise<any>;
    getPoolHistory(poolId: string, category?: string, limit?: number, offset?: number): Promise<any>;
    getAllHistory(poolId: string, limit?: number, offset?: number): Promise<any>;
    getSwapHistory(poolId: string, limit?: number, offset?: number): Promise<any>;
    getMintHistory(poolId: string, limit?: number, offset?: number): Promise<any>;
    getBurnHistory(poolId: string, limit?: number, offset?: number): Promise<any>;
    getTrades(pool: string, startTime?: number, endTime?: number, limit?: number): Promise<TradeInfo[]>;
    getCandles(pool: string, interval: string, startTime?: number, endTime?: number, limit?: number): Promise<CandleInfo[]>;
    getReserves(pool: string): Promise<any>;
    getAlkanesByAddress(address: string): Promise<any>;
    getAddressBalances(address: string, includeOutpoints?: boolean): Promise<any>;
    getHolders(alkane: string, page?: number, limit?: number): Promise<HolderInfo[]>;
    getHoldersCount(alkane: string): Promise<number>;
    getKeys(alkane: string, prefix?: string, limit?: number): Promise<any>;
    getBitcoinPrice(): Promise<any>;
    getBitcoinMarketChart(days: string): Promise<any>;
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
declare class AlkanesProvider {
    private _provider;
    private _bitcoin;
    private _esplora;
    private _alkanes;
    private _dataApi;
    readonly network: bitcoin.Network;
    readonly networkType: NetworkType;
    readonly rpcUrl: string;
    readonly dataApiUrl: string;
    private readonly networkPreset;
    constructor(config: AlkanesProviderConfig);
    /**
     * Initialize the provider (loads WASM if needed)
     */
    initialize(): Promise<void>;
    /**
     * Get the underlying WASM provider (initializes if needed)
     */
    private getProvider;
    /**
     * Bitcoin RPC client
     */
    get bitcoin(): BitcoinRpcClient;
    /**
     * Esplora API client
     */
    get esplora(): EsploraClient;
    /**
     * Alkanes RPC client
     */
    get alkanes(): AlkanesRpcClient;
    /**
     * Data API client
     */
    get dataApi(): DataApiClient;
    /**
     * Get BTC balance for an address
     */
    getBalance(address: string): Promise<AddressBalance>;
    /**
     * Get enriched balances (BTC + alkanes) for an address
     */
    getEnrichedBalances(address: string, protocolTag?: string): Promise<any>;
    /**
     * Get alkane token balance for an address
     */
    getAlkaneBalance(address: string, alkaneId?: AlkaneId): Promise<AlkaneBalance[]>;
    /**
     * Get alkane token details
     */
    getAlkaneTokenDetails(params: {
        alkaneId: AlkaneId;
    }): Promise<any>;
    /**
     * Get transaction history for an address
     */
    getAddressHistory(address: string): Promise<any[]>;
    /**
     * Get address history with alkane traces
     */
    getAddressHistoryWithTraces(address: string, excludeCoinbase?: boolean): Promise<any[]>;
    /**
     * Get current block height
     */
    getBlockHeight(): Promise<number>;
    /**
     * Broadcast a transaction
     */
    broadcastTransaction(txHex: string): Promise<string>;
    /**
     * Get all AMM pools from a factory
     */
    getAllPools(factoryId: string): Promise<PoolWithDetails[]>;
    /**
     * Get pool reserves
     */
    getPoolReserves(poolId: string): Promise<any>;
    /**
     * Get recent trades for a pool
     */
    getPoolTrades(poolId: string, limit?: number): Promise<TradeInfo[]>;
    /**
     * Get candle data for a pool
     */
    getPoolCandles(poolId: string, interval?: string, limit?: number): Promise<CandleInfo[]>;
    /**
     * Get Bitcoin price in USD
     */
    getBitcoinPrice(): Promise<number>;
    /**
     * Execute an alkanes contract call
     */
    executeAlkanes(params: {
        contractId: string;
        calldata: number[];
        feeRate?: number;
        inputs?: any[];
    }): Promise<ExecuteResult>;
    /**
     * Simulate an alkanes contract call (read-only)
     */
    simulateAlkanes(contractId: string, calldata: number[], blockTag?: string): Promise<any>;
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
declare function createProvider(config: AlkanesProviderConfig): AlkanesProvider;

/**
 * Wallet management for Alkanes SDK
 *
 * Provides Bitcoin wallet functionality with HD derivation,
 * address generation, and PSBT signing.
 */

/**
 * Address type enumeration
 */
declare enum AddressType {
    P2PKH = "p2pkh",// Legacy
    P2SH = "p2sh",// Script hash
    P2WPKH = "p2wpkh",// Native SegWit
    P2TR = "p2tr"
}
/**
 * Wallet class for managing Bitcoin addresses and transactions
 */
declare class AlkanesWallet {
    private root;
    private network;
    private keystore;
    private accountNode;
    constructor(keystore: Keystore);
    /**
     * Get master fingerprint
     */
    getMasterFingerprint(): string;
    /**
     * Get account extended public key
     */
    getAccountXpub(): string;
    /**
     * Get mnemonic (use with caution!)
     */
    getMnemonic(): string;
    /**
     * Get the coin type for the current network
     * BIP44 uses coin type 0 for mainnet, 1 for testnet/regtest
     */
    private getCoinType;
    /**
     * Get the correct derivation path base for an address type
     * Adjusts coin type based on network (0 for mainnet, 1 for testnet/regtest)
     */
    private getDerivationPathForType;
    /**
     * Derive address at specific index
     *
     * @param type - Address type (p2wpkh, p2tr, etc.)
     * @param index - Derivation index
     * @param change - Change address (0 = receiving, 1 = change)
     * @returns Address information
     */
    deriveAddress(type?: AddressType, index?: number, change?: number): AddressInfo;
    /**
     * Get receiving address at index
     */
    getReceivingAddress(index?: number, type?: AddressType): string;
    /**
     * Get change address at index
     */
    getChangeAddress(index?: number, type?: AddressType): string;
    /**
     * Get multiple addresses in a range
     */
    getAddresses(startIndex?: number, count?: number, type?: AddressType): AddressInfo[];
    /**
     * Sign a message with address at specific index
     *
     * @param message - Message to sign
     * @param index - Address index
     * @returns Signature in base64
     */
    signMessage(message: string, index?: number): string;
    /**
     * Create and sign a PSBT
     *
     * @param options - PSBT build options
     * @returns Signed PSBT in base64
     */
    createPsbt(options: PsbtOptions): Promise<string>;
    /**
     * Sign an existing PSBT
     *
     * @param psbtBase64 - PSBT in base64 format
     * @returns Signed PSBT in base64
     */
    signPsbt(psbtBase64: string): string;
    /**
     * Extract transaction from finalized PSBT
     */
    extractTransaction(psbtBase64: string): string;
    /**
     * Get WIF (Wallet Import Format) for specific index
     * Use with caution! This exposes the private key.
     */
    getPrivateKeyWIF(index?: number): string;
    private getNetwork;
}
/**
 * Create a wallet from a keystore
 */
declare function createWallet(keystore: Keystore): AlkanesWallet;
/**
 * Create a wallet from a mnemonic
 */
declare function createWalletFromMnemonic(mnemonic: string, network?: NetworkType): AlkanesWallet;

/**
 * Keystore management for Alkanes SDK
 *
 * Provides ethers.js-style keystore encryption/decryption with password protection.
 * Compatible with the WASM keystore implementation in alkanes-web-sys.
 */

type AlkanesWasm = any;
/**
 * Standard BIP44 derivation paths
 */
declare const DERIVATION_PATHS: {
    readonly BIP44: "m/44'/0'/0'/0";
    readonly BIP49: "m/49'/0'/0'/0";
    readonly BIP84: "m/84'/0'/0'/0";
    readonly BIP86: "m/86'/0'/0'/0";
};
/**
 * Keystore manager class
 *
 * Manages wallet mnemonics with encryption compatible with ethers.js format.
 * Can be used standalone or integrated with WASM backend.
 */
declare class KeystoreManager {
    private wasm?;
    constructor(wasmModule?: AlkanesWasm);
    /**
     * Generate a new mnemonic phrase
     *
     * @param wordCount - Number of words (12, 15, 18, 21, or 24)
     * @returns BIP39 mnemonic phrase
     */
    generateMnemonic(wordCount?: 12 | 15 | 18 | 21 | 24): string;
    /**
     * Validate a mnemonic phrase
     *
     * @param mnemonic - BIP39 mnemonic to validate
     * @returns true if valid
     */
    validateMnemonic(mnemonic: string): boolean;
    /**
     * Create a new keystore from mnemonic
     *
     * @param mnemonic - BIP39 mnemonic phrase
     * @param config - Wallet configuration
     * @returns Decrypted keystore object
     */
    createKeystore(mnemonic: string, config: WalletConfig): Keystore;
    /**
     * Export keystore to encrypted JSON (ethers.js compatible)
     *
     * @param keystore - Decrypted keystore object
     * @param password - Encryption password
     * @param options - Export options
     * @returns Encrypted keystore JSON
     */
    exportKeystore(keystore: Keystore, password: string, options?: ExportOptions): Promise<string | EncryptedKeystore>;
    /**
     * Import keystore from encrypted JSON (ethers.js compatible)
     *
     * @param json - Encrypted keystore JSON string or object
     * @param password - Decryption password
     * @param options - Import options
     * @returns Decrypted keystore object
     */
    importKeystore(json: string | EncryptedKeystore, password: string, options?: ImportOptions): Promise<Keystore>;
    /**
     * Export using WASM backend (delegates to alkanes-web-sys)
     */
    private exportKeystoreWasm;
    /**
     * Import using WASM backend (delegates to alkanes-web-sys)
     */
    private importKeystoreWasm;
    /**
     * Pure JS encryption implementation (fallback)
     */
    private exportKeystoreJS;
    /**
     * Pure JS decryption implementation (fallback)
     */
    private importKeystoreJS;
    private getNetwork;
    private parsePath;
    private serializeHdPaths;
    private deserializeHdPaths;
    private isValidEncryptedKeystore;
    private getCrypto;
    private bufferToHex;
    private hexToBuffer;
}
/**
 * Convenience function to create a new keystore
 */
declare function createKeystore(password: string, config?: WalletConfig, wordCount?: 12 | 15 | 18 | 21 | 24): Promise<{
    keystore: string;
    mnemonic: string;
}>;
/**
 * Convenience function to unlock an encrypted keystore
 */
declare function unlockKeystore(keystoreJson: string, password: string): Promise<Keystore>;

/**
 * Utility functions for Alkanes SDK
 */

/**
 * Convert network type string to bitcoinjs-lib network object
 */
declare function getNetwork(networkType: NetworkType): bitcoin.networks.Network;
/**
 * Validate Bitcoin address for a specific network
 */
declare function validateAddress(address: string, network?: bitcoin.networks.Network): boolean;
/**
 * Convert satoshis to BTC
 */
declare function satoshisToBTC(satoshis: number): number;
/**
 * Convert BTC to satoshis
 */
declare function btcToSatoshis(btc: number): number;
/**
 * Format AlkaneId as string
 */
declare function formatAlkaneId(id: AlkaneId): string;
/**
 * Parse AlkaneId from string
 */
declare function parseAlkaneId(idString: string): AlkaneId;
/**
 * Wait for a specific amount of time
 */
declare function delay(ms: number): Promise<void>;
/**
 * Retry a function with exponential backoff
 */
declare function retry<T>(fn: () => Promise<T>, maxAttempts?: number, delayMs?: number): Promise<T>;
/**
 * Calculate transaction fee for given size and fee rate
 */
declare function calculateFee(vsize: number, feeRate: number): number;
/**
 * Estimate transaction vsize
 */
declare function estimateTxSize(inputCount: number, outputCount: number, inputType?: 'legacy' | 'segwit' | 'taproot'): number;
/**
 * Convert hex string to Uint8Array
 */
declare function hexToBytes(hex: string): Uint8Array;
/**
 * Convert Uint8Array to hex string
 */
declare function bytesToHex(bytes: Uint8Array): string;
/**
 * Reverse byte order (for block hashes, txids, etc.)
 */
declare function reverseBytes(bytes: Uint8Array): Uint8Array;
/**
 * Convert little-endian hex to big-endian
 */
declare function reversedHex(hex: string): string;
/**
 * Check if running in browser
 */
declare function isBrowser(): boolean;
/**
 * Check if running in Node.js
 */
declare function isNode(): boolean;
/**
 * Safe JSON parse with error handling
 */
declare function safeJsonParse<T>(json: string, defaultValue?: T): T | null;
/**
 * Format timestamp to readable date
 */
declare function formatTimestamp(timestamp: number): string;
/**
 * Calculate transaction weight
 */
declare function calculateWeight(baseSize: number, witnessSize: number): number;
/**
 * Convert weight to vsize
 */
declare function weightToVsize(weight: number): number;

declare const VERSION = "0.1.0";
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
declare function initSDK(): Promise<{
    KeystoreManager: typeof KeystoreManager;
    AlkanesWallet: typeof AlkanesWallet;
    AlkanesProvider: typeof AlkanesProvider;
    createKeystore: typeof createKeystore;
    unlockKeystore: typeof unlockKeystore;
    createWallet: typeof createWallet;
    createWalletFromMnemonic: typeof createWalletFromMnemonic;
    createProvider: typeof createProvider;
    version: string;
}>;
declare function getAlkanesSDK(): Promise<{
    KeystoreManager: typeof KeystoreManager;
    AlkanesWallet: typeof AlkanesWallet;
    AlkanesProvider: typeof AlkanesProvider;
    createKeystore: typeof createKeystore;
    unlockKeystore: typeof unlockKeystore;
    createWallet: typeof createWallet;
    createWalletFromMnemonic: typeof createWalletFromMnemonic;
    createProvider: typeof createProvider;
    initSDK: typeof initSDK;
    VERSION: string;
}>;

export { type AddressBalance, type AddressInfo, AddressType, type AlkaneBalance, type AlkaneCallParams, type AlkaneId, AlkanesProvider, type AlkanesProviderConfig, AlkanesRpcClient, AlkanesWallet, BitcoinRpcClient, type BlockInfo, type CandleInfo, DERIVATION_PATHS, DataApiClient, type EncryptedKeystore, EsploraClient, type ExecuteResult, type ExportOptions, type HDPath, type HolderInfo, type ImportOptions, type Keystore, KeystoreManager, type KeystoreParams, NETWORK_PRESETS, type NetworkType, type PoolDetails, type PoolWithDetails, type ProviderConfig, type PsbtOptions, type TradeInfo, type TransactionResult, type TxInput, type TxOutput, type UTXO, VERSION, type WalletConfig, btcToSatoshis, bytesToHex, calculateFee, calculateWeight, createKeystore, createProvider, createWallet, createWalletFromMnemonic, getAlkanesSDK as default, delay, estimateTxSize, formatAlkaneId, formatTimestamp, getNetwork, hexToBytes, initSDK, isBrowser, isNode, parseAlkaneId, retry, reverseBytes, reversedHex, safeJsonParse, satoshisToBTC, unlockKeystore, validateAddress, weightToVsize };
