/**
 * Browser Wallet Detection and Connection
 *
 * Provides detection and connection to injected Bitcoin browser wallets
 * (Unisat, Xverse, Phantom, OKX, Leather, Magic Eden, Wizz, etc.)
 *
 * @example
 * ```typescript
 * import { WalletConnector, BROWSER_WALLETS } from '@alkanes/ts-sdk';
 *
 * const connector = new WalletConnector();
 * const available = await connector.detectWallets();
 *
 * if (available.length > 0) {
 *   const wallet = await connector.connect(available[0]);
 *   console.log('Connected:', wallet.address);
 * }
 * ```
 */

import { WALLET_ICONS, getWalletIcon } from './icons';

/**
 * Information about a supported browser wallet
 */
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

/**
 * Connected wallet account information
 */
export interface WalletAccount {
  address: string;
  publicKey?: string;
  addressType?: string;
  /** Payment address for dual-address wallets (Xverse, Leather, Magic Eden) */
  paymentAddress?: string;
  /** Payment public key for dual-address wallets */
  paymentPublicKey?: string;
}

/**
 * PSBT signing options
 */
export interface PsbtSigningOptions {
  autoFinalized?: boolean;
  toSignInputs?: Array<{
    index: number;
    address?: string;
    sighashTypes?: number[];
    disableTweakedPublicKey?: boolean;
  }>;
}

/**
 * List of supported browser wallets
 */
export const BROWSER_WALLETS: BrowserWalletInfo[] = [
  {
    id: 'unisat',
    name: 'Unisat Wallet',
    icon: WALLET_ICONS.unisat,
    website: 'https://unisat.io/download',
    injectionKey: 'unisat',
    supportsPsbt: true,
    supportsTaproot: true,
    supportsOrdinals: true,
    mobileSupport: false,
  },
  {
    id: 'xverse',
    name: 'Xverse Wallet',
    icon: WALLET_ICONS.xverse,
    website: 'https://www.xverse.app/download',
    injectionKey: 'XverseProviders',
    supportsPsbt: true,
    supportsTaproot: true,
    supportsOrdinals: true,
    mobileSupport: true,
    deepLinkScheme: 'xverse://',
  },
  {
    id: 'phantom',
    name: 'Phantom Wallet',
    icon: WALLET_ICONS.phantom,
    website: 'https://phantom.app/download',
    injectionKey: 'phantom',
    supportsPsbt: true,
    supportsTaproot: true,
    supportsOrdinals: false,
    mobileSupport: true,
    deepLinkScheme: 'phantom://',
  },
  {
    id: 'okx',
    name: 'OKX Wallet',
    icon: WALLET_ICONS.okx,
    website: 'https://chromewebstore.google.com/detail/okx-wallet/mcohilncbfahbmgdjkbpemcciiolgcge',
    injectionKey: 'okxwallet',
    supportsPsbt: true,
    supportsTaproot: true,
    supportsOrdinals: true,
    mobileSupport: true,
    deepLinkScheme: 'okx://',
  },
  {
    id: 'leather',
    name: 'Leather Wallet',
    icon: WALLET_ICONS.leather,
    website: 'https://leather.io/install-extension',
    injectionKey: 'LeatherProvider',
    supportsPsbt: true,
    supportsTaproot: true,
    supportsOrdinals: true,
    mobileSupport: false,
  },
  {
    id: 'magic-eden',
    name: 'Magic Eden Wallet',
    icon: WALLET_ICONS['magic-eden'],
    website: 'https://wallet.magiceden.io/',
    injectionKey: 'magicEden',
    supportsPsbt: true,
    supportsTaproot: true,
    supportsOrdinals: true,
    mobileSupport: true,
    deepLinkScheme: 'magiceden://',
  },
  {
    id: 'wizz',
    name: 'Wizz Wallet',
    icon: WALLET_ICONS.wizz,
    website: 'https://wizzwallet.io/#extension',
    injectionKey: 'wizz',
    supportsPsbt: true,
    supportsTaproot: true,
    supportsOrdinals: true,
    mobileSupport: false,
  },
  {
    id: 'oyl',
    name: 'Oyl Wallet',
    icon: WALLET_ICONS.oyl,
    website: 'https://oyl.app/',
    injectionKey: 'oyl',
    supportsPsbt: true,
    supportsTaproot: true,
    supportsOrdinals: true,
    mobileSupport: false,
  },
  {
    id: 'orange',
    name: 'Orange Wallet',
    icon: WALLET_ICONS.orange,
    website: 'https://www.orangewallet.com/',
    injectionKey: 'orange',
    supportsPsbt: false,
    supportsTaproot: false,
    supportsOrdinals: false,
    mobileSupport: false,
  },
  {
    id: 'keplr',
    name: 'Keplr Wallet',
    icon: WALLET_ICONS.keplr,
    website: 'https://keplr.app/download',
    injectionKey: 'keplr',
    supportsPsbt: false,
    supportsTaproot: false,
    supportsOrdinals: false,
    mobileSupport: true,
    deepLinkScheme: 'keplr://',
  },
];

/**
 * Check if running in browser environment
 */
export function isBrowser(): boolean {
  return typeof window !== 'undefined';
}

/**
 * Check if a specific wallet is installed
 */
export function isWalletInstalled(wallet: BrowserWalletInfo): boolean {
  if (!isBrowser()) return false;

  try {
    const walletObj = (window as any)[wallet.injectionKey];
    return walletObj !== undefined && walletObj !== null;
  } catch {
    return false;
  }
}

/**
 * Get all installed wallets
 */
export function getInstalledWallets(): BrowserWalletInfo[] {
  if (!isBrowser()) return [];
  return BROWSER_WALLETS.filter(isWalletInstalled);
}

/**
 * Get wallet info by ID
 */
export function getWalletById(id: string): BrowserWalletInfo | undefined {
  return BROWSER_WALLETS.find((w) => w.id === id);
}

/**
 * Connected browser wallet instance
 */
export class ConnectedWallet {
  private provider: any;
  public readonly info: BrowserWalletInfo;
  public readonly account: WalletAccount;

  constructor(info: BrowserWalletInfo, provider: any, account: WalletAccount) {
    this.info = info;
    this.provider = provider;
    this.account = account;
  }

  /**
   * Get the wallet's address (ordinals/taproot address for dual-address wallets)
   */
  get address(): string {
    return this.account.address;
  }

  /**
   * Get the wallet's public key (if available)
   */
  get publicKey(): string | undefined {
    return this.account.publicKey;
  }

  /**
   * Get the payment address for dual-address wallets (Xverse, Leather, etc.)
   * Returns undefined for single-address wallets
   */
  get paymentAddress(): string | undefined {
    return this.account.paymentAddress;
  }

  /**
   * Get the payment public key for dual-address wallets
   */
  get paymentPublicKey(): string | undefined {
    return this.account.paymentPublicKey;
  }

  /**
   * Sign a message
   */
  async signMessage(message: string): Promise<string> {
    switch (this.info.id) {
      case 'unisat':
      case 'wizz':
        return await this.provider.signMessage(message);

      case 'xverse': {
        const response = await this.provider.BitcoinProvider.request('signMessage', {
          address: this.account.address,
          message,
        });
        return response.result.signature;
      }

      case 'phantom': {
        const bitcoinProvider = this.provider.bitcoin;
        const { signature } = await bitcoinProvider.signMessage(
          this.account.address,
          new TextEncoder().encode(message)
        );
        return signature;
      }

      case 'okx': {
        const bitcoinProvider = this.provider.bitcoin;
        return await bitcoinProvider.signMessage(message, 'ecdsa');
      }

      case 'leather': {
        const response = await this.provider.request('signMessage', {
          message,
          paymentType: 'p2wpkh',
        });
        return response.result.signature;
      }

      case 'magic-eden': {
        const bitcoinProvider = this.provider.bitcoin;
        return await bitcoinProvider.signMessage(message);
      }

      case 'oyl': {
        const result = await this.provider.signMessage({
          address: this.account.address,
          message,
        });
        return result.signature;
      }

      default:
        throw new Error(`signMessage not supported for ${this.info.name}`);
    }
  }

  /**
   * Sign a PSBT
   */
  async signPsbt(psbtHex: string, options?: PsbtSigningOptions): Promise<string> {
    if (!this.info.supportsPsbt) {
      throw new Error(`${this.info.name} does not support PSBT signing`);
    }

    switch (this.info.id) {
      case 'unisat':
      case 'wizz':
        return await this.provider.signPsbt(psbtHex, options);

      case 'xverse': {
        const response = await this.provider.BitcoinProvider.request('signPsbt', {
          psbt: psbtHex,
          signInputs: options?.toSignInputs,
          broadcast: false,
        });
        return response.result.psbt;
      }

      case 'phantom': {
        const bitcoinProvider = this.provider.bitcoin;
        const psbtBytes = hexToBytes(psbtHex);
        const { signedPsbt } = await bitcoinProvider.signPSBT(psbtBytes, {
          inputsToSign: options?.toSignInputs?.map((i) => ({
            sigHash: i.sighashTypes?.[0],
            address: i.address || this.account.address,
            signingIndexes: [i.index],
          })),
        });
        return bytesToHex(signedPsbt);
      }

      case 'okx': {
        const bitcoinProvider = this.provider.bitcoin;
        return await bitcoinProvider.signPsbt(psbtHex, {
          autoFinalized: options?.autoFinalized ?? true,
          toSignInputs: options?.toSignInputs,
        });
      }

      case 'leather': {
        const response = await this.provider.request('signPsbt', {
          hex: psbtHex,
          signAtIndex: options?.toSignInputs?.map((i) => i.index),
          broadcast: false,
        });
        return response.result.hex;
      }

      case 'magic-eden': {
        const bitcoinProvider = this.provider.bitcoin;
        return await bitcoinProvider.signPsbt(psbtHex, options);
      }

      case 'oyl': {
        const result = await this.provider.signPsbt({
          psbt: psbtHex,
          finalize: options?.autoFinalized,
          broadcast: false,
        });
        return result.psbt;
      }

      default:
        throw new Error(`signPsbt not supported for ${this.info.name}`);
    }
  }

  /**
   * Get current network
   */
  async getNetwork(): Promise<string> {
    try {
      switch (this.info.id) {
        case 'unisat':
        case 'wizz':
          return await this.provider.getNetwork();

        case 'xverse': {
          const response = await this.provider.BitcoinProvider.request('getNetwork');
          return response.result;
        }

        default:
          return 'mainnet';
      }
    } catch {
      return 'mainnet';
    }
  }

  /**
   * Disconnect from the wallet
   */
  async disconnect(): Promise<void> {
    try {
      if (typeof this.provider.disconnect === 'function') {
        await this.provider.disconnect();
      }
    } catch {
      // Disconnect not supported or failed - that's okay
    }
  }
}

/**
 * Wallet connector for detecting and connecting to browser wallets
 */
export class WalletConnector {
  private connectedWallet: ConnectedWallet | null = null;

  /**
   * Get all supported wallets (static)
   */
  static getSupportedWallets(): BrowserWalletInfo[] {
    return BROWSER_WALLETS;
  }

  /**
   * Get wallet info by ID
   */
  getWalletInfo(walletId: string): BrowserWalletInfo | undefined {
    return BROWSER_WALLETS.find((w) => w.id === walletId);
  }

  /**
   * Check if a specific wallet is installed
   */
  isWalletInstalled(walletId: string): boolean {
    const wallet = this.getWalletInfo(walletId);
    if (!wallet) return false;
    return isWalletInstalled(wallet);
  }

  /**
   * Detect all installed wallets
   */
  async detectWallets(): Promise<BrowserWalletInfo[]> {
    if (!isBrowser()) {
      return [];
    }

    // Give wallets time to inject
    await new Promise((resolve) => setTimeout(resolve, 100));

    return getInstalledWallets();
  }

  /**
   * Connect to a specific wallet
   */
  async connect(wallet: BrowserWalletInfo): Promise<ConnectedWallet> {
    if (!isBrowser()) {
      throw new Error('Not in browser environment');
    }

    const provider = (window as any)[wallet.injectionKey];
    if (!provider) {
      throw new Error(`${wallet.name} is not installed`);
    }

    let account: WalletAccount;

    switch (wallet.id) {
      case 'unisat':
      case 'wizz': {
        const accounts = await provider.requestAccounts();
        const publicKey = await provider.getPublicKey();
        account = {
          address: accounts[0],
          publicKey,
          addressType: 'unknown',
        };
        break;
      }

      case 'xverse': {
        const response = await provider.BitcoinProvider.request('getAccounts', {
          purposes: ['ordinals', 'payment'],
        });
        // Find ordinals (taproot) and payment (segwit) addresses
        const ordinalsAccount = response.result.find(
          (acc: any) => acc.purpose === 'ordinals'
        ) || response.result[0];
        const paymentAccount = response.result.find(
          (acc: any) => acc.purpose === 'payment'
        );
        account = {
          address: ordinalsAccount.address,
          publicKey: ordinalsAccount.publicKey,
          addressType: ordinalsAccount.addressType,
          paymentAddress: paymentAccount?.address,
          paymentPublicKey: paymentAccount?.publicKey,
        };
        break;
      }

      case 'phantom': {
        const bitcoinProvider = provider.bitcoin;
        if (!bitcoinProvider) {
          throw new Error('Phantom Bitcoin provider not available');
        }
        const accounts = await bitcoinProvider.requestAccounts();
        account = {
          address: accounts[0].address,
          publicKey: accounts[0].publicKey,
          addressType: accounts[0].addressType,
        };
        break;
      }

      case 'okx': {
        const bitcoinProvider = provider.bitcoin;
        if (!bitcoinProvider) {
          throw new Error('OKX Bitcoin provider not available');
        }
        const result = await bitcoinProvider.connect();
        account = {
          address: result.address,
          publicKey: result.publicKey,
        };
        break;
      }

      case 'leather': {
        const response = await provider.request('getAddresses');
        // Find taproot (ordinals) and native segwit (payment) addresses
        const taprootAddress = response.result.addresses.find(
          (addr: any) => addr.symbol === 'BTC' && addr.type === 'p2tr'
        );
        const segwitAddress = response.result.addresses.find(
          (addr: any) => addr.symbol === 'BTC' && addr.type === 'p2wpkh'
        );
        // Fall back to first BTC address if specific types not found
        const fallbackAddress = response.result.addresses.find(
          (addr: any) => addr.symbol === 'BTC'
        );
        const ordinalsAddr = taprootAddress || fallbackAddress;
        account = {
          address: ordinalsAddr.address,
          publicKey: ordinalsAddr.publicKey,
          addressType: ordinalsAddr.type,
          paymentAddress: segwitAddress?.address,
          paymentPublicKey: segwitAddress?.publicKey,
        };
        break;
      }

      case 'magic-eden': {
        const bitcoinProvider = provider.bitcoin;
        if (!bitcoinProvider) {
          throw new Error('Magic Eden Bitcoin provider not available');
        }
        const accounts = await bitcoinProvider.connect();
        // Magic Eden may return multiple accounts (ordinals and payment)
        const ordinalsAccount = accounts.find(
          (acc: any) => acc.purpose === 'ordinals' || acc.addressType === 'p2tr'
        ) || accounts[0];
        const paymentAccount = accounts.find(
          (acc: any) => acc.purpose === 'payment' || acc.addressType === 'p2wpkh'
        );
        account = {
          address: ordinalsAccount.address,
          publicKey: ordinalsAccount.publicKey,
          addressType: ordinalsAccount.addressType,
          paymentAddress: paymentAccount?.address,
          paymentPublicKey: paymentAccount?.publicKey,
        };
        break;
      }

      case 'oyl': {
        const addresses = await provider.getAddresses();
        if (!addresses?.taproot) throw new Error('No addresses returned from OYL');
        account = {
          address: addresses.taproot.address,
          publicKey: addresses.taproot.publicKey,
          addressType: 'p2tr',
          paymentAddress: addresses.nativeSegwit?.address,
          paymentPublicKey: addresses.nativeSegwit?.publicKey,
        };
        break;
      }

      default:
        throw new Error(`Connection not implemented for ${wallet.name}`);
    }

    this.connectedWallet = new ConnectedWallet(wallet, provider, account);
    return this.connectedWallet;
  }

  /**
   * Get currently connected wallet
   */
  getConnectedWallet(): ConnectedWallet | null {
    return this.connectedWallet;
  }

  /**
   * Disconnect current wallet
   */
  async disconnect(): Promise<void> {
    if (this.connectedWallet) {
      await this.connectedWallet.disconnect();
      this.connectedWallet = null;
    }
  }

  /**
   * Check if a wallet is connected
   */
  isConnected(): boolean {
    return this.connectedWallet !== null;
  }
}

// Utility functions
function hexToBytes(hex: string): Uint8Array {
  const cleanHex = hex.startsWith('0x') ? hex.slice(2) : hex;
  const bytes = new Uint8Array(cleanHex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(cleanHex.substr(i * 2, 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

// Re-export wallet adapters
export {
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
  OylAdapter,
} from './adapter';

export type {
  JsWalletAdapter,
  WalletInfoForWasm,
  WalletAccountForWasm,
  PsbtSigningOptionsForWasm,
} from './adapter';

// Re-export wallet icons
export { WALLET_ICONS, getWalletIcon } from './icons';
