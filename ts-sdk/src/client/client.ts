/**
 * AlkanesClient - Unified Client combining Provider + Signer
 *
 * Following the ethers.js pattern, AlkanesClient combines:
 * - Provider: Read-only blockchain access (balance queries, tx lookup, etc.)
 * - Signer: Transaction signing capability
 *
 * This separation allows:
 * - Using a read-only provider without a signer
 * - Swapping signers while keeping the same provider
 * - Easy testing with mock signers
 *
 * @example
 * ```typescript
 * // Create client with browser wallet
 * const signer = await BrowserWalletSigner.connect('unisat');
 * const client = new AlkanesClient(provider, signer);
 *
 * // Or use the convenience methods
 * const client = await AlkanesClient.withBrowserWallet('unisat', 'mainnet');
 * const client = await AlkanesClient.withKeystore(keystoreJson, password, 'mainnet');
 *
 * // Now use unified interface
 * const address = await client.getAddress();
 * const balance = await client.getBalance();
 * const txid = await client.sendTransaction(psbt);
 * ```
 */

import { AlkanesProvider, AlkanesProviderConfig, NETWORK_PRESETS } from '../provider';
import { AlkanesSigner, SignPsbtOptions, SignMessageOptions, SignedPsbt, SignerAccount } from './signer';
import { KeystoreSigner, KeystoreSignerConfig } from './keystore-signer';
import { BrowserWalletSigner, BrowserWalletSignerConfig, getWalletOptions } from './browser-wallet-signer';
import { NetworkType, AlkaneId, AlkaneBalance, UTXO, Keystore } from '../types';
import { AddressType } from '../wallet';
import * as bitcoin from 'bitcoinjs-lib';

/**
 * Transaction result after broadcast
 */
export interface TransactionResult {
  /** Transaction ID */
  txid: string;
  /** Raw transaction hex */
  rawTx: string;
  /** Whether the transaction was broadcast */
  broadcast: boolean;
}

/**
 * Balance summary
 */
export interface BalanceSummary {
  /** Total confirmed balance in satoshis */
  confirmed: number;
  /** Total unconfirmed balance in satoshis */
  unconfirmed: number;
  /** Total balance */
  total: number;
  /** UTXOs */
  utxos: UTXO[];
}

/**
 * Enriched balance including alkane tokens
 */
export interface EnrichedBalance extends BalanceSummary {
  /** Alkane token balances */
  alkanes: AlkaneBalance[];
}

/**
 * AlkanesClient - Combines Provider + Signer for full wallet functionality
 */
export class AlkanesClient {
  public readonly provider: AlkanesProvider;
  public readonly signer: AlkanesSigner;

  constructor(provider: AlkanesProvider, signer: AlkanesSigner) {
    this.provider = provider;
    this.signer = signer;
  }

  // ============================================================================
  // STATIC FACTORY METHODS
  // ============================================================================

  /**
   * Create client with a browser wallet signer
   *
   * @param walletId - Wallet to connect to (e.g., 'unisat', 'xverse')
   * @param network - Network to use (default: autodetect from wallet)
   */
  static async withBrowserWallet(
    walletId: string,
    network?: string,
    signerConfig?: BrowserWalletSignerConfig
  ): Promise<AlkanesClient> {
    const signer = await BrowserWalletSigner.connect(walletId, signerConfig);

    // Use network from wallet if not specified
    const networkToUse = network || signer.network;
    const provider = new AlkanesProvider({ network: networkToUse });
    await provider.initialize();

    return new AlkanesClient(provider, signer);
  }

  /**
   * Create client with any available browser wallet
   */
  static async withAnyBrowserWallet(
    network?: string,
    signerConfig?: BrowserWalletSignerConfig
  ): Promise<AlkanesClient> {
    const signer = await BrowserWalletSigner.connectAny(signerConfig);

    const networkToUse = network || signer.network;
    const provider = new AlkanesProvider({ network: networkToUse });
    await provider.initialize();

    return new AlkanesClient(provider, signer);
  }

  /**
   * Create client with an encrypted keystore
   *
   * @param keystoreJson - Encrypted keystore JSON
   * @param password - Decryption password
   * @param network - Network to use
   */
  static async withKeystore(
    keystoreJson: string,
    password: string,
    network: string = 'mainnet',
    signerConfig?: Partial<KeystoreSignerConfig>
  ): Promise<AlkanesClient> {
    const signer = await KeystoreSigner.fromEncrypted(keystoreJson, password, {
      network: network as NetworkType,
      ...signerConfig,
    });

    const provider = new AlkanesProvider({ network });
    await provider.initialize();

    return new AlkanesClient(provider, signer);
  }

  /**
   * Create client with a mnemonic phrase
   *
   * @param mnemonic - BIP39 mnemonic phrase
   * @param network - Network to use
   */
  static withMnemonic(
    mnemonic: string,
    network: string = 'mainnet',
    signerConfig?: Partial<KeystoreSignerConfig>
  ): AlkanesClient {
    const signer = KeystoreSigner.fromMnemonic(mnemonic, {
      network: network as NetworkType,
      ...signerConfig,
    });

    const provider = new AlkanesProvider({ network });

    // Note: Provider will need to be initialized before use
    return new AlkanesClient(provider, signer);
  }

  /**
   * Create client with a Keystore object
   */
  static fromKeystore(
    keystore: Keystore,
    network?: string,
    signerConfig?: Partial<KeystoreSignerConfig>
  ): AlkanesClient {
    const networkToUse = network || keystore.network;
    const signer = KeystoreSigner.fromKeystore(keystore, {
      network: networkToUse as NetworkType,
      ...signerConfig,
    });

    const provider = new AlkanesProvider({ network: networkToUse });
    return new AlkanesClient(provider, signer);
  }

  /**
   * Generate a new wallet with fresh mnemonic
   */
  static generate(
    network: string = 'mainnet',
    wordCount: 12 | 24 = 12,
    signerConfig?: Partial<KeystoreSignerConfig>
  ): AlkanesClient {
    const signer = KeystoreSigner.generate(
      {
        network: network as NetworkType,
        ...signerConfig,
      },
      wordCount
    );

    const provider = new AlkanesProvider({ network });
    return new AlkanesClient(provider, signer);
  }

  // ============================================================================
  // INITIALIZATION
  // ============================================================================

  /**
   * Initialize the provider (required before blockchain operations)
   */
  async initialize(): Promise<void> {
    await this.provider.initialize();
  }

  /**
   * Check if connected and ready
   */
  async isReady(): Promise<boolean> {
    try {
      const signerConnected = await this.signer.isConnected();
      // Try a simple provider call
      await this.provider.getBlockHeight();
      return signerConnected;
    } catch {
      return false;
    }
  }

  // ============================================================================
  // ACCOUNT METHODS (from Signer)
  // ============================================================================

  /**
   * Get the primary address
   */
  async getAddress(): Promise<string> {
    return this.signer.getAddress();
  }

  /**
   * Get the public key
   */
  async getPublicKey(): Promise<string> {
    return this.signer.getPublicKey();
  }

  /**
   * Get full account info
   */
  async getAccount(): Promise<SignerAccount> {
    return this.signer.getAccount();
  }

  /**
   * Get the signer type
   */
  getSignerType(): string {
    return this.signer.getSignerType();
  }

  /**
   * Get the network type
   */
  getNetwork(): NetworkType {
    return this.provider.networkType;
  }

  // ============================================================================
  // BALANCE METHODS (from Provider, for current address)
  // ============================================================================

  /**
   * Get BTC balance for the current address
   */
  async getBalance(address?: string): Promise<BalanceSummary> {
    const addr = address || (await this.getAddress());
    const result = await this.provider.getBalance(addr);

    return {
      confirmed: result.confirmed,
      unconfirmed: result.unconfirmed,
      total: result.confirmed + result.unconfirmed,
      utxos: result.utxos,
    };
  }

  /**
   * Get enriched balances (BTC + alkanes) for the current address
   */
  async getEnrichedBalances(address?: string): Promise<any> {
    const addr = address || (await this.getAddress());
    return this.provider.getEnrichedBalances(addr);
  }

  /**
   * Get alkane token balances for the current address
   */
  async getAlkaneBalances(address?: string): Promise<AlkaneBalance[]> {
    const addr = address || (await this.getAddress());
    return this.provider.getAlkaneBalance(addr);
  }

  /**
   * Get UTXOs for the current address
   */
  async getUtxos(address?: string): Promise<UTXO[]> {
    const addr = address || (await this.getAddress());
    const balance = await this.provider.getBalance(addr);
    return balance.utxos;
  }

  // ============================================================================
  // SIGNING METHODS (from Signer)
  // ============================================================================

  /**
   * Sign a message
   */
  async signMessage(message: string, options?: SignMessageOptions): Promise<string> {
    return this.signer.signMessage(message, options);
  }

  /**
   * Sign a PSBT
   */
  async signPsbt(psbt: string, options?: SignPsbtOptions): Promise<SignedPsbt> {
    return this.signer.signPsbt(psbt, options);
  }

  /**
   * Sign multiple PSBTs
   */
  async signPsbts(psbts: string[], options?: SignPsbtOptions): Promise<SignedPsbt[]> {
    return this.signer.signPsbts(psbts, options);
  }

  // ============================================================================
  // TRANSACTION METHODS (Signing + Broadcasting)
  // ============================================================================

  /**
   * Sign and broadcast a PSBT
   *
   * @param psbt - PSBT in hex or base64 format
   * @param options - Signing options
   * @returns Transaction result with txid
   */
  async sendTransaction(psbt: string, options?: SignPsbtOptions): Promise<TransactionResult> {
    // Sign the PSBT
    const signed = await this.signer.signPsbt(psbt, {
      ...options,
      finalize: true,
      extractTx: true,
    });

    if (!signed.txHex) {
      throw new Error('Failed to extract transaction from signed PSBT');
    }

    // Broadcast
    const txid = await this.provider.broadcastTransaction(signed.txHex);

    return {
      txid,
      rawTx: signed.txHex,
      broadcast: true,
    };
  }

  /**
   * Sign a PSBT without broadcasting (returns signed hex)
   */
  async signTransaction(psbt: string, options?: SignPsbtOptions): Promise<SignedPsbt> {
    return this.signer.signPsbt(psbt, {
      ...options,
      finalize: true,
    });
  }

  /**
   * Broadcast a raw transaction
   */
  async broadcastTransaction(txHex: string): Promise<string> {
    return this.provider.broadcastTransaction(txHex);
  }

  // ============================================================================
  // ALKANES METHODS
  // ============================================================================

  /**
   * Get current block height
   */
  async getBlockHeight(): Promise<number> {
    return this.provider.getBlockHeight();
  }

  /**
   * Get transaction history for the current address
   */
  async getTransactionHistory(address?: string): Promise<any[]> {
    const addr = address || (await this.getAddress());
    return this.provider.getAddressHistory(addr);
  }

  /**
   * Get transaction history with alkane traces
   */
  async getTransactionHistoryWithTraces(address?: string): Promise<any[]> {
    const addr = address || (await this.getAddress());
    return this.provider.getAddressHistoryWithTraces(addr);
  }

  /**
   * Get alkane token details
   */
  async getAlkaneTokenDetails(alkaneId: AlkaneId): Promise<any> {
    return this.provider.getAlkaneTokenDetails({ alkaneId });
  }

  /**
   * Simulate an alkanes contract call
   */
  async simulateAlkanes(contractId: string, calldata: number[]): Promise<any> {
    return this.provider.simulateAlkanes(contractId, calldata);
  }

  // ============================================================================
  // AMM/DEX METHODS
  // ============================================================================

  /**
   * Get all AMM pools
   */
  async getPools(factoryId: string): Promise<any[]> {
    return this.provider.getAllPools(factoryId);
  }

  /**
   * Get pool reserves
   */
  async getPoolReserves(poolId: string): Promise<any> {
    return this.provider.getPoolReserves(poolId);
  }

  /**
   * Get pool trade history
   */
  async getPoolTrades(poolId: string, limit?: number): Promise<any[]> {
    return this.provider.getPoolTrades(poolId, limit);
  }

  /**
   * Get pool candle data
   */
  async getPoolCandles(poolId: string, interval?: string, limit?: number): Promise<any[]> {
    return this.provider.getPoolCandles(poolId, interval, limit);
  }

  // ============================================================================
  // UTILITY METHODS
  // ============================================================================

  /**
   * Get Bitcoin price in USD
   */
  async getBitcoinPrice(): Promise<number> {
    return this.provider.getBitcoinPrice();
  }

  /**
   * Disconnect the signer
   */
  async disconnect(): Promise<void> {
    await this.signer.disconnect();
  }

  /**
   * Get underlying provider sub-clients
   */
  get bitcoin() {
    return this.provider.bitcoin;
  }

  get esplora() {
    return this.provider.esplora;
  }

  get alkanes() {
    return this.provider.alkanes;
  }

  get dataApi() {
    return this.provider.dataApi;
  }

  get lua() {
    return this.provider.lua;
  }

  get metashrew() {
    return this.provider.metashrew;
  }
}

// ============================================================================
// CONNECT WALLET UTILITIES
// ============================================================================

/**
 * Wallet connection options for UI
 */
export interface WalletOption {
  id: string;
  name: string;
  icon: string;
  installed: boolean;
}

/**
 * Get available wallet options for building a wallet picker UI
 */
export async function getAvailableWallets(): Promise<WalletOption[]> {
  const options = await getWalletOptions();
  return options.map((opt) => ({
    id: opt.id,
    name: opt.name,
    icon: opt.icon,
    installed: opt.installed,
  }));
}

/**
 * Connect to a wallet and create an AlkanesClient
 *
 * This is the main entry point for "Connect Wallet" button functionality.
 *
 * @param walletId - ID of wallet to connect (e.g., 'unisat', 'xverse')
 * @param network - Optional network override (autodetects from wallet if not provided)
 */
export async function connectWallet(
  walletId: string,
  network?: string
): Promise<AlkanesClient> {
  return AlkanesClient.withBrowserWallet(walletId, network);
}

/**
 * Connect to any available wallet
 */
export async function connectAnyWallet(network?: string): Promise<AlkanesClient> {
  return AlkanesClient.withAnyBrowserWallet(network);
}

/**
 * Create a read-only provider (no signer)
 *
 * Use this when you only need to read blockchain data without signing.
 */
export function createReadOnlyProvider(network: string = 'mainnet'): AlkanesProvider {
  return new AlkanesProvider({ network });
}
