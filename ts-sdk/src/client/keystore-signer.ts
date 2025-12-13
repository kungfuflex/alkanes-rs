/**
 * Keystore Signer Implementation
 *
 * Signs transactions using an in-memory keystore (HD wallet).
 * This is the "hot wallet" approach - keys are derived from a mnemonic.
 *
 * @example
 * ```typescript
 * // Create from mnemonic
 * const signer = KeystoreSigner.fromMnemonic(mnemonic, 'mainnet');
 *
 * // Create from encrypted keystore
 * const signer = await KeystoreSigner.fromEncrypted(keystoreJson, password);
 *
 * // Sign a PSBT
 * const signed = await signer.signPsbt(psbtHex);
 * ```
 */

import * as bitcoin from 'bitcoinjs-lib';
import * as bip39 from 'bip39';
import BIP32Factory, { BIP32Interface } from 'bip32';
import { ECPairFactory, ECPairInterface } from 'ecpair';
import * as ecc from '@bitcoinerlab/secp256k1';

import {
  AlkanesSigner,
  SignerAccount,
  SignPsbtOptions,
  SignMessageOptions,
  SignedPsbt,
} from './signer';
import { Keystore, NetworkType } from '../types';
import { KeystoreManager, DERIVATION_PATHS } from '../keystore';
import { AddressType } from '../wallet';

// Initialize ECC
bitcoin.initEccLib(ecc);
const bip32 = BIP32Factory(ecc);
const ECPair = ECPairFactory(ecc);

/**
 * Keystore signer configuration
 */
export interface KeystoreSignerConfig {
  /** Network type */
  network: NetworkType;
  /** Primary address type (default: P2WPKH) */
  addressType?: AddressType;
  /** Account index for HD derivation */
  accountIndex?: number;
  /** Address index for HD derivation */
  addressIndex?: number;
}

/**
 * Keystore-based signer implementation
 */
export class KeystoreSigner extends AlkanesSigner {
  readonly network: NetworkType;
  private readonly mnemonic: string;
  private readonly root: BIP32Interface;
  private readonly bitcoinNetwork: bitcoin.Network;
  private readonly addressType: AddressType;
  private readonly accountIndex: number;
  private readonly addressIndex: number;
  private cachedAccount?: SignerAccount;

  private constructor(mnemonic: string, config: KeystoreSignerConfig) {
    super();
    this.mnemonic = mnemonic;
    this.network = config.network;
    this.bitcoinNetwork = this.getBitcoinNetwork(config.network);
    this.addressType = config.addressType || AddressType.P2WPKH;
    this.accountIndex = config.accountIndex || 0;
    this.addressIndex = config.addressIndex || 0;

    // Derive root from mnemonic
    const seed = bip39.mnemonicToSeedSync(mnemonic);
    this.root = bip32.fromSeed(seed, this.bitcoinNetwork);
  }

  /**
   * Create signer from mnemonic phrase
   */
  static fromMnemonic(mnemonic: string, config: KeystoreSignerConfig): KeystoreSigner {
    if (!bip39.validateMnemonic(mnemonic)) {
      throw new Error('Invalid mnemonic phrase');
    }
    return new KeystoreSigner(mnemonic, config);
  }

  /**
   * Create signer from encrypted keystore JSON
   */
  static async fromEncrypted(
    keystoreJson: string,
    password: string,
    config?: Partial<KeystoreSignerConfig>
  ): Promise<KeystoreSigner> {
    const manager = new KeystoreManager();
    const keystore = await manager.importKeystore(keystoreJson, password, { validate: true });

    return new KeystoreSigner(keystore.mnemonic, {
      network: config?.network || keystore.network,
      addressType: config?.addressType,
      accountIndex: config?.accountIndex,
      addressIndex: config?.addressIndex,
    });
  }

  /**
   * Create signer from Keystore object
   */
  static fromKeystore(keystore: Keystore, config?: Partial<KeystoreSignerConfig>): KeystoreSigner {
    return new KeystoreSigner(keystore.mnemonic, {
      network: config?.network || keystore.network,
      addressType: config?.addressType,
      accountIndex: config?.accountIndex,
      addressIndex: config?.addressIndex,
    });
  }

  /**
   * Generate a new keystore signer with a fresh mnemonic
   */
  static generate(config: KeystoreSignerConfig, wordCount: 12 | 24 = 12): KeystoreSigner {
    const strength = wordCount === 12 ? 128 : 256;
    const mnemonic = bip39.generateMnemonic(strength);
    return new KeystoreSigner(mnemonic, config);
  }

  getSignerType(): string {
    return 'keystore';
  }

  async isConnected(): Promise<boolean> {
    return true; // Always connected for in-memory signer
  }

  async disconnect(): Promise<void> {
    // Clear cached data
    this.cachedAccount = undefined;
  }

  async getAccount(): Promise<SignerAccount> {
    if (this.cachedAccount) {
      return this.cachedAccount;
    }

    const { address, publicKey, addressType } = this.deriveAddressInfo(
      this.addressType,
      this.addressIndex
    );

    this.cachedAccount = {
      address,
      publicKey,
      addressType,
    };

    return this.cachedAccount;
  }

  async getAddress(): Promise<string> {
    const account = await this.getAccount();
    return account.address;
  }

  async getPublicKey(): Promise<string> {
    const account = await this.getAccount();
    return account.publicKey;
  }

  async signMessage(message: string, options?: SignMessageOptions): Promise<string> {
    const node = this.getSigningNode(options?.address);
    const keyPair = ECPair.fromPrivateKey(node.privateKey!, { network: this.bitcoinNetwork });

    const messageBuffer = Buffer.from(message, 'utf8');
    const hash = bitcoin.crypto.sha256(messageBuffer);
    const signature = keyPair.sign(hash);

    return signature.toString('base64');
  }

  async signPsbt(psbt: string, options?: SignPsbtOptions): Promise<SignedPsbt> {
    const psbtObj = this.parsePsbt(psbt);

    // Sign inputs
    if (options?.inputsToSign) {
      for (const input of options.inputsToSign) {
        const node = this.getSigningNode(input.address);
        const keyPair = ECPair.fromPrivateKey(node.privateKey!, { network: this.bitcoinNetwork });

        try {
          if (input.sighashTypes) {
            psbtObj.signInput(input.index, keyPair, input.sighashTypes);
          } else {
            psbtObj.signInput(input.index, keyPair);
          }
        } catch (e) {
          // Input might not be ours
          console.warn(`Could not sign input ${input.index}:`, e);
        }
      }
    } else {
      // Sign all inputs we can
      this.signAllInputs(psbtObj);
    }

    // Finalize if requested
    if (options?.finalize !== false) {
      try {
        psbtObj.finalizeAllInputs();
      } catch {
        // Not all inputs could be finalized (might need more signatures)
      }
    }

    return this.formatSignedPsbt(psbtObj, options);
  }

  async signPsbts(psbts: string[], options?: SignPsbtOptions): Promise<SignedPsbt[]> {
    const results: SignedPsbt[] = [];
    for (const psbt of psbts) {
      const signed = await this.signPsbt(psbt, options);
      results.push(signed);
    }
    return results;
  }

  /**
   * Export the mnemonic (use with caution!)
   */
  exportMnemonic(): string {
    return this.mnemonic;
  }

  /**
   * Export to encrypted keystore JSON
   */
  async exportToKeystore(password: string): Promise<string> {
    const manager = new KeystoreManager();
    const keystore = manager.createKeystore(this.mnemonic, { network: this.network });
    const encrypted = await manager.exportKeystore(keystore, password, { pretty: true });
    return typeof encrypted === 'string' ? encrypted : JSON.stringify(encrypted, null, 2);
  }

  /**
   * Derive address at specific path
   */
  deriveAddress(
    addressType: AddressType = AddressType.P2WPKH,
    index: number = 0,
    change: number = 0
  ): string {
    return this.deriveAddressInfo(addressType, index, change).address;
  }

  /**
   * Get multiple addresses
   */
  getAddresses(
    count: number = 10,
    addressType: AddressType = AddressType.P2WPKH
  ): Array<{ address: string; index: number }> {
    const addresses: Array<{ address: string; index: number }> = [];
    for (let i = 0; i < count; i++) {
      addresses.push({
        address: this.deriveAddress(addressType, i),
        index: i,
      });
    }
    return addresses;
  }

  // Private methods

  private getDerivationPath(addressType: AddressType): string {
    const coinType = this.bitcoinNetwork === bitcoin.networks.bitcoin ? 0 : 1;

    switch (addressType) {
      case AddressType.P2PKH:
        return `m/44'/${coinType}'/${this.accountIndex}'`;
      case AddressType.P2SH:
        return `m/49'/${coinType}'/${this.accountIndex}'`;
      case AddressType.P2WPKH:
        return `m/84'/${coinType}'/${this.accountIndex}'`;
      case AddressType.P2TR:
        return `m/86'/${coinType}'/${this.accountIndex}'`;
      default:
        return `m/84'/${coinType}'/${this.accountIndex}'`;
    }
  }

  private deriveAddressInfo(
    addressType: AddressType,
    index: number,
    change: number = 0
  ): { address: string; publicKey: string; addressType: string } {
    const basePath = this.getDerivationPath(addressType);
    const node = this.root.derivePath(`${basePath}/${change}/${index}`);
    const pubkey = node.publicKey;

    let address: string;

    switch (addressType) {
      case AddressType.P2PKH:
        address = bitcoin.payments.p2pkh({ pubkey, network: this.bitcoinNetwork }).address!;
        break;

      case AddressType.P2WPKH:
        address = bitcoin.payments.p2wpkh({ pubkey, network: this.bitcoinNetwork }).address!;
        break;

      case AddressType.P2TR:
        const internalPubkey = pubkey.slice(1, 33); // x-only pubkey
        address = bitcoin.payments.p2tr({
          internalPubkey,
          network: this.bitcoinNetwork,
        }).address!;
        break;

      default:
        address = bitcoin.payments.p2wpkh({ pubkey, network: this.bitcoinNetwork }).address!;
    }

    return {
      address,
      publicKey: pubkey.toString('hex'),
      addressType,
    };
  }

  private getSigningNode(address?: string): BIP32Interface {
    // For simplicity, use default index. In production, would look up address -> path mapping
    const basePath = this.getDerivationPath(this.addressType);
    return this.root.derivePath(`${basePath}/0/${this.addressIndex}`);
  }

  private signAllInputs(psbt: bitcoin.Psbt): void {
    // Try to sign all inputs with our key(s)
    // For a full implementation, would try multiple derivation paths
    const node = this.getSigningNode();
    const keyPair = ECPair.fromPrivateKey(node.privateKey!, { network: this.bitcoinNetwork });

    for (let i = 0; i < psbt.data.inputs.length; i++) {
      try {
        psbt.signInput(i, keyPair);
      } catch {
        // Input might not be ours - that's OK
      }
    }

    // Also try taproot signing for P2TR inputs
    if (this.addressType === AddressType.P2TR) {
      const taprootKeyPair = this.getTaprootSigner(node);
      for (let i = 0; i < psbt.data.inputs.length; i++) {
        try {
          psbt.signInput(i, taprootKeyPair);
        } catch {
          // Input might not be ours - that's OK
        }
      }
    }
  }

  private getTaprootSigner(node: BIP32Interface): bitcoin.Signer {
    const privateKey = node.privateKey!;
    const publicKey = node.publicKey.slice(1); // x-only

    return {
      publicKey: node.publicKey,
      sign: (hash: Buffer): Buffer => {
        const keyPair = ECPair.fromPrivateKey(privateKey, { network: this.bitcoinNetwork });
        return keyPair.sign(hash);
      },
      signSchnorr: (hash: Buffer): Buffer => {
        // Use bitcoinjs-lib's built-in schnorr signing
        const signature = ecc.signSchnorr(hash, privateKey);
        return Buffer.from(signature);
      },
    };
  }
}
