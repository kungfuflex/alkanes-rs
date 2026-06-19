/**
 * Browser Wallet Signer Implementation
 *
 * Signs transactions using browser extension wallets (Unisat, Xverse, etc.).
 * This bridges the browser wallet APIs to the unified Signer interface.
 *
 * @example
 * ```typescript
 * // Connect to specific wallet
 * const signer = await BrowserWalletSigner.connect('unisat');
 *
 * // Connect to any available wallet (user chooses)
 * const signer = await BrowserWalletSigner.connectAny();
 *
 * // Listen for account changes
 * signer.on('accountsChanged', (accounts) => {
 *   console.log('Account changed:', accounts);
 * });
 * ```
 */

import {
  EventEmittingSigner,
  SignerAccount,
  SignPsbtOptions,
  SignMessageOptions,
  SignedPsbt,
  SignerEventType,
} from './signer';
import { NetworkType } from '../types';
import {
  WalletConnector,
  ConnectedWallet,
  BrowserWalletInfo,
  createWalletAdapter,
  JsWalletAdapter,
} from '../browser-wallets';

/**
 * Browser wallet signer configuration
 */
export interface BrowserWalletSignerConfig {
  /** Whether to auto-reconnect on page load */
  autoReconnect?: boolean;
  /** Preferred address type */
  preferredAddressType?: 'payment' | 'ordinals' | 'both';
}

/**
 * Wallet selection result
 */
export interface WalletSelection {
  walletId: string;
  walletName: string;
  walletInfo: BrowserWalletInfo;
}

/**
 * Browser wallet signer implementation
 */
export class BrowserWalletSigner extends EventEmittingSigner {
  readonly network: NetworkType;
  private wallet: ConnectedWallet;
  private adapter: JsWalletAdapter;
  private config: BrowserWalletSignerConfig;
  private eventCleanup?: () => void;

  private constructor(
    wallet: ConnectedWallet,
    adapter: JsWalletAdapter,
    network: NetworkType,
    config: BrowserWalletSignerConfig
  ) {
    super();
    this.wallet = wallet;
    this.adapter = adapter;
    this.network = network;
    this.config = config;

    // Set up wallet event listeners
    this.setupEventListeners();
  }

  /**
   * Get list of available (installed) wallets
   */
  static async getAvailableWallets(): Promise<BrowserWalletInfo[]> {
    const connector = new WalletConnector();
    return connector.detectWallets();
  }

  /**
   * Get list of all supported wallets (installed or not)
   */
  static getSupportedWallets(): BrowserWalletInfo[] {
    return WalletConnector.getSupportedWallets();
  }

  /**
   * Check if a specific wallet is installed
   */
  static isWalletInstalled(walletId: string): boolean {
    const connector = new WalletConnector();
    return connector.isWalletInstalled(walletId);
  }

  /**
   * Connect to a specific wallet by ID
   *
   * @param walletId - Wallet identifier (e.g., 'unisat', 'xverse')
   * @param config - Signer configuration
   * @returns Connected BrowserWalletSigner
   */
  static async connect(
    walletId: string,
    config: BrowserWalletSignerConfig = {}
  ): Promise<BrowserWalletSigner> {
    const connector = new WalletConnector();
    const walletInfo = connector.getWalletInfo(walletId);

    if (!walletInfo) {
      throw new Error(`Unknown wallet: ${walletId}`);
    }

    if (!connector.isWalletInstalled(walletId)) {
      throw new Error(`${walletInfo.name} is not installed`);
    }

    const wallet = await connector.connect(walletInfo);
    const adapter = createWalletAdapter(wallet);
    const network = await adapter.getNetwork();

    return new BrowserWalletSigner(
      wallet,
      adapter,
      network as NetworkType,
      config
    );
  }

  /**
   * Connect to any available wallet
   * Returns the first available wallet or throws if none found
   */
  static async connectAny(
    config: BrowserWalletSignerConfig = {}
  ): Promise<BrowserWalletSigner> {
    const availableWallets = await BrowserWalletSigner.getAvailableWallets();

    if (availableWallets.length === 0) {
      throw new Error('No Bitcoin wallets detected. Please install a wallet extension.');
    }

    return BrowserWalletSigner.connect(availableWallets[0].id, config);
  }

  /**
   * Connect using a pre-connected wallet
   */
  static fromConnectedWallet(
    wallet: ConnectedWallet,
    config: BrowserWalletSignerConfig = {}
  ): BrowserWalletSigner {
    const adapter = createWalletAdapter(wallet);
    const network = wallet.getNetwork() as NetworkType;

    return new BrowserWalletSigner(wallet, adapter, network, config);
  }

  getSignerType(): string {
    return `browser:${this.wallet.info.id}`;
  }

  /**
   * Get the underlying wallet info
   */
  getWalletInfo(): BrowserWalletInfo {
    return this.wallet.info;
  }

  /**
   * Get the underlying wallet adapter (for WASM integration)
   */
  getAdapter(): JsWalletAdapter {
    return this.adapter;
  }

  async isConnected(): Promise<boolean> {
    try {
      await this.adapter.getAccounts();
      return true;
    } catch {
      return false;
    }
  }

  async disconnect(): Promise<void> {
    // Clean up event listeners
    if (this.eventCleanup) {
      this.eventCleanup();
    }

    await this.wallet.disconnect();
    this.emit('disconnect');
  }

  async getAccount(): Promise<SignerAccount> {
    const accounts = await this.adapter.getAccounts();

    if (accounts.length === 0) {
      throw new Error('No accounts available');
    }

    const primary = accounts[0];

    return {
      address: primary.address,
      publicKey: primary.public_key || '',
      addressType: primary.address_type,
    };
  }

  async getAddress(): Promise<string> {
    const account = await this.getAccount();
    return account.address;
  }

  async getPublicKey(): Promise<string> {
    return this.adapter.getPublicKey();
  }

  async signMessage(message: string, options?: SignMessageOptions): Promise<string> {
    const address = options?.address || (await this.getAddress());
    return this.adapter.signMessage(message, address);
  }

  async signPsbt(psbt: string, options?: SignPsbtOptions): Promise<SignedPsbt> {
    // Convert to hex if base64
    let psbtHex = psbt;
    if (!this.isHex(psbt)) {
      // It's base64
      const psbtObj = this.parsePsbt(psbt);
      psbtHex = psbtObj.toHex();
    }

    // Build signing options for the adapter
    const adapterOptions = options?.inputsToSign
      ? {
          auto_finalized: options.finalize !== false,
          to_sign_inputs: options.inputsToSign.map((input) => ({
            index: input.index,
            address: input.address,
            sighash_types: input.sighashTypes,
          })),
        }
      : {
          auto_finalized: options?.finalize !== false,
        };

    const signedHex = await this.adapter.signPsbt(psbtHex, adapterOptions);

    // Parse the signed PSBT
    const signedPsbt = this.parsePsbt(signedHex);

    const result: SignedPsbt = {
      psbtHex: signedPsbt.toHex(),
      psbtBase64: signedPsbt.toBase64(),
    };

    // Extract transaction if requested
    if (options?.extractTx && options?.finalize !== false) {
      try {
        result.txHex = signedPsbt.extractTransaction().toHex();
      } catch {
        // Not fully signed/finalized
      }
    }

    return result;
  }

  async signPsbts(psbts: string[], options?: SignPsbtOptions): Promise<SignedPsbt[]> {
    // Convert all to hex
    const psbtHexs = psbts.map((psbt) => {
      if (this.isHex(psbt)) return psbt;
      return this.parsePsbt(psbt).toHex();
    });

    const adapterOptions = {
      auto_finalized: options?.finalize !== false,
      to_sign_inputs: options?.inputsToSign?.map((input) => ({
        index: input.index,
        address: input.address,
        sighash_types: input.sighashTypes,
      })),
    };

    const signedHexs = await this.adapter.signPsbts(psbtHexs, adapterOptions);

    return signedHexs.map((signedHex) => {
      const signedPsbt = this.parsePsbt(signedHex);

      const result: SignedPsbt = {
        psbtHex: signedPsbt.toHex(),
        psbtBase64: signedPsbt.toBase64(),
      };

      if (options?.extractTx && options?.finalize !== false) {
        try {
          result.txHex = signedPsbt.extractTransaction().toHex();
        } catch {
          // Not fully signed/finalized
        }
      }

      return result;
    });
  }

  /**
   * Push a transaction to the network (if wallet supports it)
   */
  async pushTransaction(txHex: string): Promise<string> {
    return this.adapter.pushTx(txHex);
  }

  /**
   * Push a signed PSBT to the network (if wallet supports it)
   */
  async pushPsbt(psbtHex: string): Promise<string> {
    return this.adapter.pushPsbt(psbtHex);
  }

  /**
   * Get wallet balance (if wallet supports it)
   */
  async getBalance(): Promise<number | null> {
    return this.adapter.getBalance();
  }

  /**
   * Get inscriptions (if wallet supports it)
   */
  async getInscriptions(cursor?: number, size?: number): Promise<any> {
    return this.adapter.getInscriptions(cursor, size);
  }

  /**
   * Switch network (if wallet supports it)
   */
  async switchNetwork(network: NetworkType): Promise<void> {
    await this.adapter.switchNetwork(network);
    // Update our network reference
    (this as any).network = network;
    this.emit('networkChanged', network);
  }

  // Private methods

  private isHex(str: string): boolean {
    return /^[0-9a-fA-F]+$/.test(str);
  }

  private setupEventListeners(): void {
    // Set up wallet-specific event listeners
    // This would be implemented per-wallet since they have different APIs
    const injectedWallet = this.getInjectedWallet();

    if (!injectedWallet) return;

    const accountsHandler = (accounts: string[]) => {
      this.emit('accountsChanged', accounts);
    };

    const networkHandler = (network: string) => {
      this.emit('networkChanged', network);
    };

    // Different wallets have different event APIs
    if (injectedWallet.on) {
      injectedWallet.on('accountsChanged', accountsHandler);
      injectedWallet.on('networkChanged', networkHandler);

      this.eventCleanup = () => {
        injectedWallet.removeListener?.('accountsChanged', accountsHandler);
        injectedWallet.removeListener?.('networkChanged', networkHandler);
      };
    }
  }

  private getInjectedWallet(): any {
    const win = typeof window !== 'undefined' ? (window as any) : undefined;
    if (!win) return undefined;

    const walletId = this.wallet.info.id;

    switch (walletId) {
      case 'unisat':
        return win.unisat;
      case 'xverse':
        return win.XverseProviders?.BitcoinProvider;
      case 'okx':
        return win.okxwallet?.bitcoin;
      case 'leather':
        return win.LeatherProvider;
      case 'phantom':
        return win.phantom?.bitcoin;
      case 'magic-eden':
        return win.magicEden?.bitcoin;
      case 'wizz':
        return win.wizz;
      default:
        return win[this.wallet.info.injectionKey];
    }
  }
}

/**
 * Helper to create a wallet selection UI
 * Returns info needed to build a wallet picker
 */
export async function getWalletOptions(): Promise<
  Array<{
    id: string;
    name: string;
    icon: string;
    installed: boolean;
    info: BrowserWalletInfo;
  }>
> {
  const supported = BrowserWalletSigner.getSupportedWallets();
  const available = await BrowserWalletSigner.getAvailableWallets();
  const availableIds = new Set(available.map((w) => w.id));

  return supported.map((wallet) => ({
    id: wallet.id,
    name: wallet.name,
    icon: wallet.icon,
    installed: availableIds.has(wallet.id),
    info: wallet,
  }));
}
