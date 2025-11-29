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
  
  // Other exports
  export const VERSION: string;
  export function initSDK(wasmModule?: any): Promise<any>;
  export default function getAlkanesSDK(): Promise<any>;
}
