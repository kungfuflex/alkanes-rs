/**
 * Alkanes Client Module
 *
 * Provides a unified ethers.js-style interface for interacting with Alkanes:
 *
 * - **AlkanesSigner**: Abstract signer interface
 *   - KeystoreSigner: In-memory HD wallet signing
 *   - BrowserWalletSigner: Browser extension wallet signing
 *
 * - **AlkanesClient**: Combines Provider + Signer for full functionality
 *
 * @example
 * ```typescript
 * import {
 *   AlkanesClient,
 *   BrowserWalletSigner,
 *   KeystoreSigner,
 *   connectWallet,
 *   getAvailableWallets,
 * } from '@alkanes/ts-sdk';
 *
 * // Get available wallets for UI
 * const wallets = await getAvailableWallets();
 *
 * // Connect to a specific wallet
 * const client = await connectWallet('unisat');
 *
 * // Or create manually for more control
 * const signer = await BrowserWalletSigner.connect('xverse');
 * const provider = new AlkanesProvider({ network: 'mainnet' });
 * await provider.initialize();
 * const client = new AlkanesClient(provider, signer);
 *
 * // Use the client
 * const address = await client.getAddress();
 * const balance = await client.getBalance();
 * const signed = await client.signPsbt(psbtHex);
 * const txid = await client.sendTransaction(psbtHex);
 * ```
 */

// Core signer interface
export {
  AlkanesSigner,
  EventEmittingSigner,
  SignPsbtOptions,
  SignMessageOptions,
  SignerAccount,
  SignedPsbt,
  SignerEventType,
  SignerEvents,
} from './signer';

// Keystore signer
export { KeystoreSigner, KeystoreSignerConfig } from './keystore-signer';

// Browser wallet signer
export {
  BrowserWalletSigner,
  BrowserWalletSignerConfig,
  WalletSelection,
  getWalletOptions,
} from './browser-wallet-signer';

// Main client
export {
  AlkanesClient,
  TransactionResult,
  BalanceSummary,
  EnrichedBalance,
  WalletOption,
  getAvailableWallets,
  connectWallet,
  connectAnyWallet,
  createReadOnlyProvider,
} from './client';
