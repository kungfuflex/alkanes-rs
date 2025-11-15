/**
 * Wallet management for Alkanes SDK
 * 
 * Provides Bitcoin wallet functionality with HD derivation,
 * address generation, and PSBT signing.
 */

import * as bitcoin from 'bitcoinjs-lib';
import * as bip39 from 'bip39';
import BIP32Factory, { BIP32Interface } from 'bip32';
import { ECPairFactory, ECPairInterface } from 'ecpair';
import * as ecc from '@bitcoinerlab/secp256k1';

// Initialize ECC library for bitcoinjs-lib
bitcoin.initEccLib(ecc);

const bip32 = BIP32Factory(ecc);
import {
  Keystore,
  WalletConfig,
  AddressInfo,
  NetworkType,
  TxInput,
  TxOutput,
  PsbtOptions,
} from '../types';
import { DERIVATION_PATHS } from '../keystore';

const ECPair = ECPairFactory(ecc);

/**
 * Address type enumeration
 */
export enum AddressType {
  P2PKH = 'p2pkh',      // Legacy
  P2SH = 'p2sh',        // Script hash
  P2WPKH = 'p2wpkh',    // Native SegWit
  P2TR = 'p2tr',        // Taproot
}

/**
 * Wallet class for managing Bitcoin addresses and transactions
 */
export class AlkanesWallet {
  private root: BIP32Interface;
  private network: bitcoin.networks.Network;
  private keystore: Keystore;
  private accountNode: BIP32Interface;

  constructor(keystore: Keystore) {
    this.keystore = keystore;
    this.network = this.getNetwork(keystore.network);
    
    // Derive root from mnemonic
    const seed = bip39.mnemonicToSeedSync(keystore.mnemonic);
    this.root = bip32.fromSeed(seed, this.network);
    
    // Set up account node (using BIP84 by default)
    const accountPath = DERIVATION_PATHS.BIP84.replace(/\/\d+$/, ''); // Remove last index
    this.accountNode = this.root.derivePath(accountPath);
  }

  /**
   * Get master fingerprint
   */
  getMasterFingerprint(): string {
    return this.keystore.masterFingerprint;
  }

  /**
   * Get account extended public key
   */
  getAccountXpub(): string {
    return this.keystore.accountXpub;
  }

  /**
   * Get mnemonic (use with caution!)
   */
  getMnemonic(): string {
    return this.keystore.mnemonic;
  }

  /**
   * Derive address at specific index
   * 
   * @param type - Address type (p2wpkh, p2tr, etc.)
   * @param index - Derivation index
   * @param change - Change address (0 = receiving, 1 = change)
   * @returns Address information
   */
  deriveAddress(
    type: AddressType = AddressType.P2WPKH,
    index: number = 0,
    change: number = 0
  ): AddressInfo {
    const node = this.accountNode.derive(change).derive(index);
    const pubkey = node.publicKey;
    
    let address: string;
    let payment: bitcoin.Payment;
    
    switch (type) {
      case AddressType.P2PKH:
        payment = bitcoin.payments.p2pkh({ pubkey, network: this.network });
        address = payment.address!;
        break;
        
      case AddressType.P2WPKH:
        payment = bitcoin.payments.p2wpkh({ pubkey, network: this.network });
        address = payment.address!;
        break;
        
      case AddressType.P2TR:
        const internalPubkey = pubkey.slice(1, 33); // Remove first byte for x-only
        payment = bitcoin.payments.p2tr({ 
          internalPubkey, 
          network: this.network 
        });
        address = payment.address!;
        break;
        
      default:
        throw new Error(`Unsupported address type: ${type}`);
    }
    
    const path = `${DERIVATION_PATHS.BIP84}/${change}/${index}`;
    
    return {
      address,
      path,
      publicKey: pubkey.toString('hex'),
      index,
    };
  }

  /**
   * Get receiving address at index
   */
  getReceivingAddress(index: number = 0, type: AddressType = AddressType.P2WPKH): string {
    return this.deriveAddress(type, index, 0).address;
  }

  /**
   * Get change address at index
   */
  getChangeAddress(index: number = 0, type: AddressType = AddressType.P2WPKH): string {
    return this.deriveAddress(type, index, 1).address;
  }

  /**
   * Get multiple addresses in a range
   */
  getAddresses(
    startIndex: number = 0,
    count: number = 20,
    type: AddressType = AddressType.P2WPKH
  ): AddressInfo[] {
    const addresses: AddressInfo[] = [];
    for (let i = startIndex; i < startIndex + count; i++) {
      addresses.push(this.deriveAddress(type, i, 0));
    }
    return addresses;
  }

  /**
   * Sign a message with address at specific index
   * 
   * @param message - Message to sign
   * @param index - Address index
   * @returns Signature in base64
   */
  signMessage(message: string, index: number = 0): string {
    const node = this.accountNode.derive(0).derive(index);
    const keyPair = ECPair.fromPrivateKey(node.privateKey!, { network: this.network });
    
    const messageBuffer = Buffer.from(message, 'utf8');
    const signature = keyPair.sign(bitcoin.crypto.sha256(messageBuffer));
    
    return signature.toString('base64');
  }

  /**
   * Create and sign a PSBT
   * 
   * @param options - PSBT build options
   * @returns Signed PSBT in base64
   */
  async createPsbt(options: PsbtOptions): Promise<string> {
    const psbt = new bitcoin.Psbt({ network: this.network });
    
    // Add inputs
    for (const input of options.inputs) {
      // For each input, we need to find which address/index it belongs to
      // For now, we'll assume index 0
      const addressInfo = this.deriveAddress(AddressType.P2WPKH, 0, 0);
      const node = this.accountNode.derive(0).derive(0);
      
      const payment = bitcoin.payments.p2wpkh({
        pubkey: node.publicKey,
        network: this.network,
      });
      
      psbt.addInput({
        hash: input.txid,
        index: input.vout,
        witnessUtxo: {
          script: payment.output!,
          value: input.value,
        },
      });
    }
    
    // Add outputs
    for (const output of options.outputs) {
      psbt.addOutput({
        address: output.address,
        value: output.value,
      });
    }
    
    // Sign all inputs
    for (let i = 0; i < options.inputs.length; i++) {
      const node = this.accountNode.derive(0).derive(0); // TODO: Find correct index
      const keyPair = ECPair.fromPrivateKey(node.privateKey!, { network: this.network });
      psbt.signInput(i, keyPair);
    }
    
    // Finalize all inputs
    psbt.finalizeAllInputs();
    
    return psbt.toBase64();
  }

  /**
   * Sign an existing PSBT
   * 
   * @param psbtBase64 - PSBT in base64 format
   * @returns Signed PSBT in base64
   */
  signPsbt(psbtBase64: string): string {
    const psbt = bitcoin.Psbt.fromBase64(psbtBase64, { network: this.network });
    
    // Sign all inputs that we can
    psbt.data.inputs.forEach((input, index) => {
      try {
        // Derive the key for this input
        // For now, using index 0 - in production, would need to identify correct path
        const node = this.accountNode.derive(0).derive(0);
        const keyPair = ECPair.fromPrivateKey(node.privateKey!, { network: this.network });
        psbt.signInput(index, keyPair);
      } catch (error) {
        // Input might not be ours or already signed
        console.warn(`Could not sign input ${index}:`, error);
      }
    });
    
    return psbt.toBase64();
  }

  /**
   * Extract transaction from finalized PSBT
   */
  extractTransaction(psbtBase64: string): string {
    const psbt = bitcoin.Psbt.fromBase64(psbtBase64, { network: this.network });
    const tx = psbt.extractTransaction();
    return tx.toHex();
  }

  /**
   * Get WIF (Wallet Import Format) for specific index
   * Use with caution! This exposes the private key.
   */
  getPrivateKeyWIF(index: number = 0): string {
    const node = this.accountNode.derive(0).derive(index);
    const keyPair = ECPair.fromPrivateKey(node.privateKey!, { network: this.network });
    return keyPair.toWIF();
  }

  private getNetwork(networkType: NetworkType): bitcoin.networks.Network {
    switch (networkType) {
      case 'mainnet':
        return bitcoin.networks.bitcoin;
      case 'testnet':
        return bitcoin.networks.testnet;
      case 'regtest':
        return bitcoin.networks.regtest;
      default:
        return bitcoin.networks.testnet;
    }
  }
}

/**
 * Create a wallet from a keystore
 */
export function createWallet(keystore: Keystore): AlkanesWallet {
  return new AlkanesWallet(keystore);
}

/**
 * Create a wallet from a mnemonic
 */
export function createWalletFromMnemonic(
  mnemonic: string,
  network: NetworkType = 'mainnet'
): AlkanesWallet {
  if (!bip39.validateMnemonic(mnemonic)) {
    throw new Error('Invalid mnemonic');
  }

  const seed = bip39.mnemonicToSeedSync(mnemonic);
  const networkObj = network === 'mainnet' ? bitcoin.networks.bitcoin :
                     network === 'testnet' ? bitcoin.networks.testnet :
                     network === 'regtest' ? bitcoin.networks.regtest :
                     bitcoin.networks.testnet;
  
  const root = bip32.fromSeed(seed, networkObj);
  const masterFingerprint = root.fingerprint.toString('hex');
  
  const accountPath = DERIVATION_PATHS.BIP84.replace(/\/\d+$/, '');
  const accountNode = root.derivePath(accountPath);
  const accountXpub = accountNode.neutered().toBase58();

  const keystore: Keystore = {
    mnemonic,
    masterFingerprint,
    accountXpub,
    hdPaths: {
      bip84: {
        purpose: 84,
        coinType: 0,
        account: 0,
        change: 0,
        index: 0,
      },
    },
    network,
    createdAt: Date.now(),
  };

  return new AlkanesWallet(keystore);
}
