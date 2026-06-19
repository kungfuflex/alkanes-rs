/**
 * Unified Client Example - ethers.js-style Alkanes SDK
 *
 * This example demonstrates the unified Signer/Provider architecture
 * that separates read operations from signing, following ethers.js patterns.
 *
 * Key concepts:
 * - Provider: Read-only blockchain access (balance, UTXOs, tx history, etc.)
 * - Signer: Signs transactions (Keystore or Browser Wallet)
 * - Client: Combines both for full functionality
 */

import {
  // Core client
  AlkanesClient,
  // Signers
  AlkanesSigner,
  KeystoreSigner,
  BrowserWalletSigner,
  // Provider
  AlkanesProvider,
  // Utilities
  connectWallet,
  connectAnyWallet,
  getAvailableWallets,
  createReadOnlyProvider,
  // Types
  SignPsbtOptions,
  BalanceSummary,
  NetworkType,
} from '../src';

// ============================================================================
// EXAMPLE 1: Connect Wallet Button Flow
// ============================================================================

/**
 * Typical "Connect Wallet" button implementation
 */
async function connectWalletButtonExample() {
  console.log('=== Connect Wallet Button Flow ===\n');

  // 1. Get available wallets for the UI picker
  const wallets = await getAvailableWallets();

  console.log('Available wallets:');
  for (const wallet of wallets) {
    console.log(`  - ${wallet.name}: ${wallet.installed ? '✓ Installed' : '✗ Not installed'}`);
  }

  // 2. User selects a wallet (simulated here)
  const selectedWallet = wallets.find((w) => w.installed);
  if (!selectedWallet) {
    console.log('\nNo wallets installed!');
    return null;
  }

  console.log(`\nConnecting to ${selectedWallet.name}...`);

  // 3. Create client with the selected wallet
  const client = await connectWallet(selectedWallet.id);

  // 4. Client is ready to use
  const address = await client.getAddress();
  console.log(`Connected! Address: ${address}`);

  return client;
}

// ============================================================================
// EXAMPLE 2: Keystore Wallet (In-Memory Signing)
// ============================================================================

/**
 * Using a keystore for in-memory signing (like ethers.js Wallet)
 */
async function keystoreWalletExample() {
  console.log('\n=== Keystore Wallet Example ===\n');

  // Option A: Generate a new wallet
  console.log('Generating new wallet...');
  const newClient = AlkanesClient.generate('mainnet');
  await newClient.initialize();

  const address = await newClient.getAddress();
  console.log(`New wallet address: ${address}`);

  // Export the mnemonic (KEEP THIS SAFE!)
  const signer = newClient.signer as KeystoreSigner;
  const mnemonic = signer.exportMnemonic();
  console.log(`Mnemonic (BACKUP THIS): ${mnemonic}`);

  // Option B: Restore from mnemonic
  console.log('\nRestoring from mnemonic...');
  const restoredClient = AlkanesClient.withMnemonic(mnemonic, 'mainnet');
  await restoredClient.initialize();

  const restoredAddress = await restoredClient.getAddress();
  console.log(`Restored address: ${restoredAddress}`);
  console.log(`Addresses match: ${address === restoredAddress}`);

  // Option C: Load from encrypted keystore
  console.log('\nEncrypting keystore...');
  const password = 'super-secure-password';
  const keystoreJson = await signer.exportToKeystore(password);

  console.log('Loading from encrypted keystore...');
  const loadedClient = await AlkanesClient.withKeystore(keystoreJson, password, 'mainnet');
  await loadedClient.initialize();

  const loadedAddress = await loadedClient.getAddress();
  console.log(`Loaded address: ${loadedAddress}`);

  return newClient;
}

// ============================================================================
// EXAMPLE 3: Using the Client for Common Operations
// ============================================================================

/**
 * Common wallet operations with the unified client
 */
async function clientOperationsExample(client: AlkanesClient) {
  console.log('\n=== Client Operations ===\n');

  // Get address
  const address = await client.getAddress();
  console.log(`Address: ${address}`);

  // Get public key
  const publicKey = await client.getPublicKey();
  console.log(`Public Key: ${publicKey.slice(0, 20)}...`);

  // Get signer type
  const signerType = client.getSignerType();
  console.log(`Signer Type: ${signerType}`);

  // Get network
  const network = client.getNetwork();
  console.log(`Network: ${network}`);

  // Get BTC balance
  console.log('\nFetching balance...');
  try {
    const balance = await client.getBalance();
    console.log(`Balance: ${balance.total} sats (${balance.confirmed} confirmed)`);
    console.log(`UTXOs: ${balance.utxos.length}`);
  } catch (e) {
    console.log(`Balance fetch failed (expected if not on network): ${e}`);
  }

  // Get alkane balances
  console.log('\nFetching alkane balances...');
  try {
    const alkanes = await client.getAlkaneBalances();
    console.log(`Alkane tokens: ${alkanes.length}`);
    for (const token of alkanes.slice(0, 3)) {
      console.log(`  - ${token.id}: ${token.balance}`);
    }
  } catch (e) {
    console.log(`Alkane fetch failed: ${e}`);
  }

  // Sign a message
  console.log('\nSigning message...');
  const signature = await client.signMessage('Hello, Alkanes!');
  console.log(`Signature: ${signature.slice(0, 30)}...`);
}

// ============================================================================
// EXAMPLE 4: Different Signers, Same Provider
// ============================================================================

/**
 * Demonstrates swapping signers while keeping the same provider
 */
async function swappableSignersExample() {
  console.log('\n=== Swappable Signers ===\n');

  // Create a provider (read-only access)
  const provider = createReadOnlyProvider('mainnet');
  await provider.initialize();

  console.log('Provider initialized (read-only)\n');

  // Get block height (doesn't need signer)
  const height = await provider.getBlockHeight();
  console.log(`Current block height: ${height}`);

  // Create different signers
  console.log('\nCreating signers...');

  // Keystore signer
  const keystoreSigner = KeystoreSigner.generate({ network: 'mainnet' });
  console.log(`Keystore signer address: ${await keystoreSigner.getAddress()}`);

  // Browser wallet signer (would connect in real app)
  // const browserSigner = await BrowserWalletSigner.connect('unisat');
  // console.log(`Browser signer address: ${await browserSigner.getAddress()}`);

  // Create clients with different signers, same provider
  const keystoreClient = new AlkanesClient(provider, keystoreSigner);
  // const browserClient = new AlkanesClient(provider, browserSigner);

  // Both clients share the same provider but sign differently
  console.log(`\nKeystore client address: ${await keystoreClient.getAddress()}`);
  // console.log(`Browser client address: ${await browserClient.getAddress()}`);
}

// ============================================================================
// EXAMPLE 5: Transaction Signing Flow
// ============================================================================

/**
 * Shows the transaction signing flow
 */
async function transactionSigningExample(client: AlkanesClient) {
  console.log('\n=== Transaction Signing Flow ===\n');

  // Example PSBT (you would get this from your transaction builder)
  const mockPsbtHex =
    '70736274ff01009a020000000258e87a21b56daf0c23be8e7070456c336f7cbaa5c8757924f545887bb2abdd7501000000' +
    '00ffffffff838d0427d0ec650a68aa46bb0b098aea4422c071b2ca78352a077959d07cea1d0100000000ffffffff0270aa' +
    '0a00000000001976a914389ffce9cd9ae88dcc0631e88a821ffdbe9bfe2688ac9093510d000000001976a9147480a33f95' +
    '0689af511e6e84c138dbbd3c3ee41588ac00000000';

  // Option 1: Sign without broadcasting
  console.log('Signing PSBT (without broadcast)...');
  try {
    const signed = await client.signTransaction(mockPsbtHex, {
      finalize: true,
    });
    console.log(`Signed PSBT hex: ${signed.psbtHex.slice(0, 50)}...`);
    console.log(`Signed PSBT base64: ${signed.psbtBase64.slice(0, 50)}...`);
    if (signed.txHex) {
      console.log(`Extracted TX: ${signed.txHex.slice(0, 50)}...`);
    }
  } catch (e) {
    console.log(`Signing failed (expected with mock PSBT): ${e}`);
  }

  // Option 2: Sign and broadcast (sendTransaction)
  // const result = await client.sendTransaction(psbtHex);
  // console.log(`Broadcast txid: ${result.txid}`);
}

// ============================================================================
// EXAMPLE 6: Multiple Address Types (Keystore)
// ============================================================================

/**
 * Demonstrates deriving different address types from keystore
 */
async function multipleAddressTypesExample() {
  console.log('\n=== Multiple Address Types ===\n');

  const signer = KeystoreSigner.generate({ network: 'mainnet' });

  // Derive different address types
  const addresses = {
    p2wpkh: signer.deriveAddress('p2wpkh' as any, 0),
    p2tr: signer.deriveAddress('p2tr' as any, 0),
    p2pkh: signer.deriveAddress('p2pkh' as any, 0),
  };

  console.log('Address types from same mnemonic:');
  console.log(`  P2WPKH (Native SegWit): ${addresses.p2wpkh}`);
  console.log(`  P2TR (Taproot): ${addresses.p2tr}`);
  console.log(`  P2PKH (Legacy): ${addresses.p2pkh}`);

  // Get multiple addresses (for receiving)
  const receivingAddresses = signer.getAddresses(5);
  console.log('\nReceiving addresses (P2WPKH):');
  for (const addr of receivingAddresses) {
    console.log(`  [${addr.index}] ${addr.address}`);
  }
}

// ============================================================================
// MAIN
// ============================================================================

async function main() {
  console.log('╔════════════════════════════════════════════════════════════╗');
  console.log('║   Alkanes SDK - Unified Client Examples                     ║');
  console.log('╚════════════════════════════════════════════════════════════╝\n');

  // Run examples
  // await connectWalletButtonExample(); // Requires browser
  const client = await keystoreWalletExample();
  if (client) {
    await clientOperationsExample(client);
    await transactionSigningExample(client);
  }
  await swappableSignersExample();
  await multipleAddressTypesExample();

  console.log('\n✓ All examples completed!');
}

// Run if executed directly
main().catch(console.error);

// Export for use in other examples
export {
  connectWalletButtonExample,
  keystoreWalletExample,
  clientOperationsExample,
  swappableSignersExample,
  transactionSigningExample,
  multipleAddressTypesExample,
};
