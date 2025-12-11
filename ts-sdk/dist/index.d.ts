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
  export class AlkanesProvider {
    constructor(config: {
      url: string;
      dataApiUrl?: string;
      network: any;
      networkType: string;
      projectId?: string;
      version?: string;
      [key: string]: any; // Allow additional properties
    });
    getBalance(address: string): Promise<number>;
    getUtxos(address: string): Promise<any[]>;
    getAddressUtxos(address: string, spendStrategy?: any): Promise<any>;
    broadcastTx(txHex: string): Promise<string>;
    // Data API methods via alkanes_web_sys.dataapi namespace
    [key: string]: any; // Allow dynamic access to data API methods
  }
  
  export function createProvider(config: any, wasmModule?: any): AlkanesProvider;
  
  // AMM and utility exports
  export const amm: any;
  export function executeWithBtcWrapUnwrap(...args: any[]): Promise<any>;
  export function wrapBtc(...args: any[]): Promise<any>;
  export function unwrapBtc(...args: any[]): Promise<any>;

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

  // Other exports
  export const VERSION: string;
  export function initSDK(wasmModule?: any): Promise<any>;
  export default function getAlkanesSDK(): Promise<any>;
}
