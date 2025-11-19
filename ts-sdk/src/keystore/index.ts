/**
 * Keystore management for Alkanes SDK
 * 
 * Provides ethers.js-style keystore encryption/decryption with password protection.
 * Compatible with the WASM keystore implementation in alkanes-web-sys.
 */

import * as bip39 from 'bip39';
import * as bitcoin from 'bitcoinjs-lib';
import BIP32Factory, { BIP32Interface } from 'bip32';
import * as ecc from '@bitcoinerlab/secp256k1';

const bip32 = BIP32Factory(ecc);
import {
  Keystore,
  EncryptedKeystore,
  KeystoreParams,
  NetworkType,
  HDPath,
  WalletConfig,
  ExportOptions,
  ImportOptions,
} from '../types';

// Re-export the WASM keystore functions
// @ts-ignore - WASM types are available at runtime
import type * as AlkanesWasm from '../../build/wasm/alkanes_web_sys';

/**
 * Default PBKDF2 parameters (matching ethers.js defaults)
 */
const DEFAULT_PBKDF2_ITERATIONS = 131072; // ethers.js default
const DEFAULT_SALT_SIZE = 32;
const DEFAULT_NONCE_SIZE = 12;

/**
 * Standard BIP44 derivation paths
 */
export const DERIVATION_PATHS = {
  BIP44: "m/44'/0'/0'/0",    // Legacy P2PKH
  BIP49: "m/49'/0'/0'/0",    // SegWit wrapped (P2SH-P2WPKH)
  BIP84: "m/84'/0'/0'/0",    // Native SegWit (P2WPKH)
  BIP86: "m/86'/0'/0'/0",    // Taproot (P2TR)
} as const;

/**
 * Keystore manager class
 * 
 * Manages wallet mnemonics with encryption compatible with ethers.js format.
 * Can be used standalone or integrated with WASM backend.
 */
export class KeystoreManager {
  private wasm?: typeof AlkanesWasm;

  constructor(wasmModule?: typeof AlkanesWasm) {
    this.wasm = wasmModule;
  }

  /**
   * Generate a new mnemonic phrase
   * 
   * @param wordCount - Number of words (12, 15, 18, 21, or 24)
   * @returns BIP39 mnemonic phrase
   */
  generateMnemonic(wordCount: 12 | 15 | 18 | 21 | 24 = 12): string {
    const strength = wordCount === 12 ? 128 :
                    wordCount === 15 ? 160 :
                    wordCount === 18 ? 192 :
                    wordCount === 21 ? 224 : 256;
    
    return bip39.generateMnemonic(strength);
  }

  /**
   * Validate a mnemonic phrase
   * 
   * @param mnemonic - BIP39 mnemonic to validate
   * @returns true if valid
   */
  validateMnemonic(mnemonic: string): boolean {
    return bip39.validateMnemonic(mnemonic);
  }

  /**
   * Create a new keystore from mnemonic
   * 
   * @param mnemonic - BIP39 mnemonic phrase
   * @param config - Wallet configuration
   * @returns Decrypted keystore object
   */
  createKeystore(mnemonic: string, config: WalletConfig): Keystore {
    if (!this.validateMnemonic(mnemonic)) {
      throw new Error('Invalid mnemonic phrase');
    }

    const network = this.getNetwork(config.network);
    const seed = bip39.mnemonicToSeedSync(mnemonic);
    const root = bip32.fromSeed(seed, network);
    
    // Get master fingerprint
    const masterFingerprint = root.fingerprint.toString('hex');
    
    // Derive account xpub (using BIP84 as default)
    const accountPath = config.derivationPath || DERIVATION_PATHS.BIP84;
    const accountNode = root.derivePath(accountPath.replace(/\/\d+$/, '')); // Remove last index
    const accountXpub = accountNode.neutered().toBase58();

    // Store HD paths
    const hdPaths: Record<string, HDPath> = {
      bip44: this.parsePath(DERIVATION_PATHS.BIP44),
      bip49: this.parsePath(DERIVATION_PATHS.BIP49),
      bip84: this.parsePath(DERIVATION_PATHS.BIP84),
      bip86: this.parsePath(DERIVATION_PATHS.BIP86),
    };

    return {
      mnemonic,
      masterFingerprint,
      accountXpub,
      hdPaths,
      network: config.network,
      createdAt: Date.now(),
    };
  }

  /**
   * Export keystore to encrypted JSON (ethers.js compatible)
   * 
   * @param keystore - Decrypted keystore object
   * @param password - Encryption password
   * @param options - Export options
   * @returns Encrypted keystore JSON
   */
  async exportKeystore(
    keystore: Keystore,
    password: string,
    options: ExportOptions = {}
  ): Promise<string | EncryptedKeystore> {
    if (!password || password.length < 8) {
      throw new Error('Password must be at least 8 characters');
    }

    // Use WASM implementation if available
    if (this.wasm) {
      return this.exportKeystoreWasm(keystore, password, options);
    }

    // Fallback to pure JS implementation
    return this.exportKeystoreJS(keystore, password, options);
  }

  /**
   * Import keystore from encrypted JSON (ethers.js compatible)
   * 
   * @param json - Encrypted keystore JSON string or object
   * @param password - Decryption password
   * @param options - Import options
   * @returns Decrypted keystore object
   */
  async importKeystore(
    json: string | EncryptedKeystore,
    password: string,
    options: ImportOptions = {}
  ): Promise<Keystore> {
    const encrypted = typeof json === 'string' ? JSON.parse(json) : json;

    if (!this.isValidEncryptedKeystore(encrypted)) {
      throw new Error('Invalid keystore format');
    }

    // Use WASM implementation if available
    if (this.wasm) {
      return this.importKeystoreWasm(encrypted, password, options);
    }

    // Fallback to pure JS implementation
    return this.importKeystoreJS(encrypted, password, options);
  }

  /**
   * Export using WASM backend (delegates to alkanes-web-sys)
   */
  private async exportKeystoreWasm(
    keystore: Keystore,
    password: string,
    options: ExportOptions
  ): Promise<string | EncryptedKeystore> {
    if (!this.wasm) {
      throw new Error('WASM module not loaded');
    }

    // Convert to WASM keystore format
    const wasmKeystore = new this.wasm.Keystore({
      encrypted_mnemonic: '', // Will be encrypted by WASM
      master_fingerprint: keystore.masterFingerprint,
      created_at: keystore.createdAt,
      version: '1.0',
      pbkdf2_params: {
        salt: '',
        iterations: DEFAULT_PBKDF2_ITERATIONS,
      },
      account_xpub: keystore.accountXpub,
      hd_paths: this.serializeHdPaths(keystore.hdPaths),
      seed: null,
    });

    // Call WASM encrypt method
    const encryptedJson = await wasmKeystore.encrypt(password);
    
    if (options.format === 'json') {
      return JSON.parse(encryptedJson);
    }

    return options.pretty ? 
      JSON.stringify(JSON.parse(encryptedJson), null, 2) : 
      encryptedJson;
  }

  /**
   * Import using WASM backend (delegates to alkanes-web-sys)
   */
  private async importKeystoreWasm(
    encrypted: EncryptedKeystore,
    password: string,
    options: ImportOptions
  ): Promise<Keystore> {
    if (!this.wasm) {
      throw new Error('WASM module not loaded');
    }

    // Create WASM keystore from encrypted data
    const wasmKeystore = new this.wasm.Keystore(encrypted);
    
    // Decrypt
    await wasmKeystore.decrypt(password);
    
    // Extract decrypted data
    const mnemonic = wasmKeystore.getMnemonic();
    
    if (options.validate && !this.validateMnemonic(mnemonic)) {
      throw new Error('Decrypted mnemonic is invalid');
    }

    return {
      mnemonic,
      masterFingerprint: encrypted.master_fingerprint,
      accountXpub: encrypted.account_xpub,
      hdPaths: this.deserializeHdPaths(encrypted.hd_paths),
      network: options.network || 'mainnet',
      createdAt: encrypted.created_at,
    };
  }

  /**
   * Pure JS encryption implementation (fallback)
   */
  private async exportKeystoreJS(
    keystore: Keystore,
    password: string,
    options: ExportOptions
  ): Promise<string | EncryptedKeystore> {
    // Import crypto dynamically (works in both Node and browser)
    const crypto = await this.getCrypto();
    
    // Generate salt and nonce
    const salt = crypto.getRandomValues(new Uint8Array(DEFAULT_SALT_SIZE));
    const nonce = crypto.getRandomValues(new Uint8Array(DEFAULT_NONCE_SIZE));
    
    // Derive key using PBKDF2
    const encoder = new TextEncoder();
    const passwordBuffer = encoder.encode(password);
    
    const keyMaterial = await crypto.subtle.importKey(
      'raw',
      passwordBuffer,
      'PBKDF2',
      false,
      ['deriveBits', 'deriveKey']
    );
    
    const key = await crypto.subtle.deriveKey(
      {
        name: 'PBKDF2',
        salt,
        iterations: DEFAULT_PBKDF2_ITERATIONS,
        hash: 'SHA-256',
      },
      keyMaterial,
      { name: 'AES-GCM', length: 256 },
      false,
      ['encrypt']
    );
    
    // Encrypt mnemonic
    const mnemonicBuffer = encoder.encode(keystore.mnemonic);
    const encryptedBuffer = await crypto.subtle.encrypt(
      { name: 'AES-GCM', iv: nonce },
      key,
      mnemonicBuffer
    );
    
    // Create encrypted keystore
    const encrypted: EncryptedKeystore = {
      encrypted_mnemonic: this.bufferToHex(new Uint8Array(encryptedBuffer)),
      master_fingerprint: keystore.masterFingerprint,
      created_at: keystore.createdAt,
      version: '1.0',
      pbkdf2_params: {
        salt: this.bufferToHex(salt),
        nonce: this.bufferToHex(nonce),
        iterations: DEFAULT_PBKDF2_ITERATIONS,
        algorithm: 'aes-256-gcm',
      },
      account_xpub: keystore.accountXpub,
      hd_paths: this.serializeHdPaths(keystore.hdPaths),
    };

    if (options.format === 'json') {
      return encrypted;
    }

    return options.pretty ? 
      JSON.stringify(encrypted, null, 2) : 
      JSON.stringify(encrypted);
  }

  /**
   * Pure JS decryption implementation (fallback)
   */
  private async importKeystoreJS(
    encrypted: EncryptedKeystore,
    password: string,
    options: ImportOptions
  ): Promise<Keystore> {
    const crypto = await this.getCrypto();
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    
    // Parse parameters
    const salt = this.hexToBuffer(encrypted.pbkdf2_params.salt);
    const nonce = encrypted.pbkdf2_params.nonce ? 
      this.hexToBuffer(encrypted.pbkdf2_params.nonce) : 
      new Uint8Array(DEFAULT_NONCE_SIZE);
    
    // Derive key
    const passwordBuffer = encoder.encode(password);
    const keyMaterial = await crypto.subtle.importKey(
      'raw',
      passwordBuffer,
      'PBKDF2',
      false,
      ['deriveBits', 'deriveKey']
    );
    
    const key = await crypto.subtle.deriveKey(
      {
        name: 'PBKDF2',
        salt,
        iterations: encrypted.pbkdf2_params.iterations,
        hash: 'SHA-256',
      },
      keyMaterial,
      { name: 'AES-GCM', length: 256 },
      false,
      ['decrypt']
    );
    
    // Decrypt mnemonic
    try {
      const encryptedBuffer = this.hexToBuffer(encrypted.encrypted_mnemonic);
      const decryptedBuffer = await crypto.subtle.decrypt(
        { name: 'AES-GCM', iv: nonce },
        key,
        encryptedBuffer
      );
      
      const mnemonic = decoder.decode(decryptedBuffer);
      
      if (options.validate && !this.validateMnemonic(mnemonic)) {
        throw new Error('Decrypted mnemonic is invalid');
      }

      return {
        mnemonic,
        masterFingerprint: encrypted.master_fingerprint,
        accountXpub: encrypted.account_xpub,
        hdPaths: this.deserializeHdPaths(encrypted.hd_paths),
        network: options.network || 'mainnet',
        createdAt: encrypted.created_at,
      };
    } catch (error) {
      throw new Error('Decryption failed: incorrect password or corrupted keystore');
    }
  }

  // Helper methods

  private getNetwork(networkType: NetworkType): bitcoin.networks.Network {
    switch (networkType) {
      case 'mainnet':
        return bitcoin.networks.bitcoin;
      case 'testnet':
        return bitcoin.networks.testnet;
      case 'regtest':
        return bitcoin.networks.regtest;
      default:
        return bitcoin.networks.testnet; // Signet uses testnet params
    }
  }

  private parsePath(path: string): HDPath {
    const parts = path.replace(/^m\//, '').split('/');
    return {
      purpose: parseInt(parts[0].replace("'", '')),
      coinType: parseInt(parts[1].replace("'", '')),
      account: parseInt(parts[2].replace("'", '')),
      change: parseInt(parts[3]),
      index: 0, // Will be incremented per address
    };
  }

  private serializeHdPaths(paths: Record<string, HDPath>): Record<string, string> {
    const serialized: Record<string, string> = {};
    for (const [key, path] of Object.entries(paths)) {
      serialized[key] = `m/${path.purpose}'/${path.coinType}'/${path.account}'/${path.change}/${path.index}`;
    }
    return serialized;
  }

  private deserializeHdPaths(paths: Record<string, string>): Record<string, HDPath> {
    const deserialized: Record<string, HDPath> = {};
    for (const [key, path] of Object.entries(paths)) {
      deserialized[key] = this.parsePath(path);
    }
    return deserialized;
  }

  private isValidEncryptedKeystore(obj: any): obj is EncryptedKeystore {
    return (
      typeof obj === 'object' &&
      typeof obj.encrypted_mnemonic === 'string' &&
      typeof obj.master_fingerprint === 'string' &&
      typeof obj.version === 'string' &&
      typeof obj.pbkdf2_params === 'object'
    );
  }

  private async getCrypto(): Promise<SubtleCrypto & { getRandomValues: (arr: Uint8Array) => Uint8Array }> {
    // Always use browser crypto API
    if (typeof window !== 'undefined' && window.crypto) {
      return window.crypto as any;
    }
    
    // For Node.js/SSR, use global crypto (Node 19+)
    if (typeof globalThis !== 'undefined' && (globalThis as any).crypto) {
      return (globalThis as any).crypto as any;
    }
    
    throw new Error('Web Crypto API not available. Please use a modern browser or Node.js 19+');
  }

  private bufferToHex(buffer: Uint8Array): string {
    return Array.from(buffer)
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');
  }

  private hexToBuffer(hex: string): Uint8Array {
    const matches = hex.match(/.{1,2}/g);
    if (!matches) {
      throw new Error('Invalid hex string');
    }
    return new Uint8Array(matches.map(byte => parseInt(byte, 16)));
  }
}

/**
 * Convenience function to create a new keystore
 */
export async function createKeystore(
  password: string,
  config: WalletConfig = { network: 'mainnet' },
  wordCount: 12 | 15 | 18 | 21 | 24 = 12
): Promise<{ keystore: string; mnemonic: string }> {
  const manager = new KeystoreManager();
  const mnemonic = manager.generateMnemonic(wordCount);
  const keystore = manager.createKeystore(mnemonic, config);
  const encrypted = await manager.exportKeystore(keystore, password, { pretty: true });
  
  return {
    keystore: typeof encrypted === 'string' ? encrypted : JSON.stringify(encrypted, null, 2),
    mnemonic,
  };
}

/**
 * Convenience function to unlock an encrypted keystore
 */
export async function unlockKeystore(
  keystoreJson: string,
  password: string
): Promise<Keystore> {
  const manager = new KeystoreManager();
  return manager.importKeystore(keystoreJson, password, { validate: true });
}
