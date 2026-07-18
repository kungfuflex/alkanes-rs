/**
 * Browser Wallet Integration Example
 *
 * This example demonstrates how to:
 * 1. Detect and connect to browser wallet extensions
 * 2. Create a wallet adapter for WASM integration
 * 3. Use the WasmBrowserWalletProvider with the adapter
 *
 * @example
 * In a browser environment:
 * ```html
 * <script type="module">
 *   import { connectWallet, signAndBroadcast } from './browser-wallet-integration.js';
 *
 *   // Connect to first available wallet
 *   const provider = await connectWallet();
 *
 *   // Sign and broadcast a PSBT
 *   const txid = await signAndBroadcast(provider, psbtHex);
 * </script>
 * ```
 */

import {
  WalletConnector,
  ConnectedWallet,
  BrowserWalletInfo,
  createWalletAdapter,
  MockWalletAdapter,
  JsWalletAdapter,
} from '../src/browser-wallets';

// WasmBrowserWalletProvider would be imported from WASM module
// This is a placeholder type for the example
type WasmBrowserWalletProvider = any;

/**
 * Detect all installed browser wallets
 */
export async function detectWallets(): Promise<BrowserWalletInfo[]> {
  const connector = new WalletConnector();
  return connector.detectWallets();
}

/**
 * Connect to a specific wallet and create a WASM-compatible provider
 *
 * @param walletId - The wallet to connect to (e.g., 'unisat', 'xverse')
 * @param network - The network to use ('mainnet', 'testnet', 'signet', 'regtest')
 * @returns WasmBrowserWalletProvider instance
 */
export async function connectWallet(
  walletId?: string,
  network: string = 'mainnet'
): Promise<{ provider: WasmBrowserWalletProvider; wallet: ConnectedWallet }> {
  const connector = new WalletConnector();

  // Detect available wallets
  const availableWallets = await connector.detectWallets();

  if (availableWallets.length === 0) {
    throw new Error('No Bitcoin wallets detected. Please install a wallet extension.');
  }

  // Find the requested wallet or use the first available
  let walletInfo: BrowserWalletInfo;
  if (walletId) {
    const found = availableWallets.find((w) => w.id === walletId);
    if (!found) {
      throw new Error(`Wallet ${walletId} is not installed`);
    }
    walletInfo = found;
  } else {
    walletInfo = availableWallets[0];
  }

  console.log(`Connecting to ${walletInfo.name}...`);

  // Connect to the wallet
  const connectedWallet = await connector.connect(walletInfo);

  console.log(`Connected! Address: ${connectedWallet.address}`);

  // Create adapter for WASM integration
  const adapter = createWalletAdapter(connectedWallet);

  // Create the WASM provider (this would use the actual WASM module)
  // const provider = await WasmBrowserWalletProvider.new(adapter, network);

  // For the example, we'll simulate the provider
  const provider = createMockWasmProvider(adapter, network);

  return { provider, wallet: connectedWallet };
}

/**
 * Connect using a mock wallet (for testing without browser extension)
 */
export async function connectMockWallet(
  network: string = 'mainnet'
): Promise<{ provider: WasmBrowserWalletProvider; adapter: MockWalletAdapter }> {
  const adapter = new MockWalletAdapter({
    address: 'bc1qmockaddress1234567890abcdef',
    publicKey: '03' + 'a'.repeat(64),
    network,
  });

  // Create the WASM provider
  // const provider = await WasmBrowserWalletProvider.new(adapter, network);

  // For the example, we'll simulate the provider
  const provider = createMockWasmProvider(adapter, network);

  return { provider, adapter };
}

/**
 * Sign and broadcast a PSBT
 */
export async function signAndBroadcast(
  provider: WasmBrowserWalletProvider,
  psbtHex: string
): Promise<string> {
  // Sign the PSBT
  const signedPsbt = await provider.signPsbt(psbtHex);

  // Broadcast the transaction
  const txid = await provider.broadcastTransaction(signedPsbt);

  console.log(`Transaction broadcast! TXID: ${txid}`);
  return txid;
}

/**
 * Get wallet balance
 */
export async function getBalance(
  provider: WasmBrowserWalletProvider
): Promise<{ confirmed: number; unconfirmed: number; total: number }> {
  const balance = await provider.getBalance();
  return balance;
}

/**
 * Get enriched UTXOs with alkane information
 */
export async function getEnrichedUtxos(
  provider: WasmBrowserWalletProvider
): Promise<any[]> {
  const utxos = await provider.getEnrichedUtxos();
  return utxos;
}

/**
 * Get all balances including alkane tokens
 */
export async function getAllBalances(
  provider: WasmBrowserWalletProvider
): Promise<{
  btc: { confirmed: number; unconfirmed: number };
  alkanes: Array<{ id: string; balance: string }>;
}> {
  const balances = await provider.getAllBalances();
  return balances;
}

// Helper to create a mock WASM provider for the example
function createMockWasmProvider(adapter: JsWalletAdapter, network: string): any {
  return {
    async signPsbt(psbtHex: string, options?: any) {
      return adapter.signPsbt(psbtHex, options);
    },
    async signMessage(message: string, address?: string) {
      const info = await adapter.connect();
      return adapter.signMessage(message, address || info.address);
    },
    async broadcastTransaction(txHex: string) {
      return adapter.pushTx(txHex);
    },
    async getBalance() {
      const balance = await adapter.getBalance();
      return {
        confirmed: balance || 0,
        unconfirmed: 0,
        total: balance || 0,
      };
    },
    async getEnrichedUtxos() {
      return [];
    },
    async getAllBalances() {
      const btcBalance = await adapter.getBalance();
      return {
        btc: { confirmed: btcBalance || 0, unconfirmed: 0 },
        alkanes: [],
      };
    },
    getAddress() {
      return adapter.connect().then((a) => a.address);
    },
    getPublicKey() {
      return adapter.getPublicKey();
    },
    getNetwork() {
      return network;
    },
    async disconnect() {
      return adapter.disconnect();
    },
  };
}

// Example usage when run directly
if (typeof window !== 'undefined') {
  // Browser environment
  (window as any).alkanes = {
    detectWallets,
    connectWallet,
    connectMockWallet,
    signAndBroadcast,
    getBalance,
    getEnrichedUtxos,
    getAllBalances,
  };

  console.log('Alkanes browser wallet integration loaded!');
  console.log('Available functions: window.alkanes.connectWallet(), etc.');
}

// Export for module usage
export default {
  detectWallets,
  connectWallet,
  connectMockWallet,
  signAndBroadcast,
  getBalance,
  getEnrichedUtxos,
  getAllBalances,
};
