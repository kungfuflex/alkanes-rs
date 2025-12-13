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

  export class AlkanesProvider {
    constructor(config: AlkanesProviderConfig);
    readonly networkType: 'mainnet' | 'testnet' | 'regtest';
    readonly bitcoin: any;
    readonly esplora: any;
    readonly alkanes: any;
    readonly dataApi: any;
    readonly lua: any;
    readonly metashrew: any;

    initialize(): Promise<void>;
    getBalance(address: string): Promise<{ confirmed: number; unconfirmed: number; utxos: any[] }>;
    getUtxos(address: string): Promise<any[]>;
    getAddressUtxos(address: string, spendStrategy?: any): Promise<any>;
    broadcastTransaction(txHex: string): Promise<string>;
    broadcastTx(txHex: string): Promise<string>;
    getBlockHeight(): Promise<number>;
    getAddressHistory(address: string): Promise<any[]>;
    getAddressHistoryWithTraces(address: string): Promise<any[]>;
    getAlkaneBalance(address: string): Promise<AlkaneBalance[]>;
    getEnrichedBalances(address: string): Promise<any>;
    getAlkaneTokenDetails(params: { alkaneId: AlkaneId }): Promise<any>;
    simulateAlkanes(contractId: string, calldata: number[]): Promise<any>;
    getAllPools(factoryId: string): Promise<any[]>;
    getPoolReserves(poolId: string): Promise<any>;
    getPoolTrades(poolId: string, limit?: number): Promise<any[]>;
    getPoolCandles(poolId: string, interval?: string, limit?: number): Promise<any[]>;
    getBitcoinPrice(): Promise<number>;
    // Allow dynamic access to data API methods
    [key: string]: any;
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
