# @alkanes/ts-sdk

TypeScript SDK for Alkanes - Bitcoin smart contracts powered by WebAssembly.

## Features

- 🔐 **Secure Keystore Management** - ethers.js-style encrypted keystores with PBKDF2
- 💼 **HD Wallet Support** - BIP32/44/84/86 derivation paths
- 🔑 **Address Generation** - P2PKH, P2WPKH, P2TR address types
- ✍️ **Transaction Signing** - PSBT creation and signing
- 🌐 **Provider Integration** - Compatible with @oyl/sdk
- ⚡ **WebAssembly Backend** - High-performance alkanes functionality via WASM
- 📦 **Tree-shakeable** - Import only what you need

## Installation

```bash
npm install "https://gitpkg.vercel.app/kungfuflex/alkanes-rs/ts-sdk?develop"
# or with yarn
yarn add "https://gitpkg.vercel.app/kungfuflex/alkanes-rs/ts-sdk?develop"
```

To install a specific branch or tag, replace `develop` with the branch/tag name:
```bash
npm install "https://gitpkg.vercel.app/kungfuflex/alkanes-rs/ts-sdk?main"
npm install "https://gitpkg.vercel.app/kungfuflex/alkanes-rs/ts-sdk?v1.0.0"
```

## Quick Start

### 1. Create a New Wallet

```typescript
import { createKeystore, unlockKeystore, createWallet } from '@alkanes/ts-sdk';

// Create encrypted keystore
const { keystore, mnemonic } = await createKeystore('password123', {
  network: 'mainnet',
});

console.log('🔑 Save this mnemonic securely:', mnemonic);
console.log('💾 Keystore:', keystore);

// Later, unlock the keystore
const unlockedKeystore = await unlockKeystore(keystore, 'password123');
const wallet = createWallet(unlockedKeystore);

// Get addresses
const address = wallet.getReceivingAddress(0);
console.log('📬 Address:', address);
```

### 2. Use with Existing Mnemonic

```typescript
import { createWalletFromMnemonic } from '@alkanes/ts-sdk';

const wallet = createWalletFromMnemonic(
  'your twelve word mnemonic phrase here...',
  'mainnet'
);

const address = wallet.getReceivingAddress(0);
console.log('Address:', address);
```

### 3. Create a Provider (Compatible with @oyl/sdk)

```typescript
import { createProvider } from '@alkanes/ts-sdk';
import * as bitcoin from 'bitcoinjs-lib';

const provider = createProvider({
  url: 'https://api.example.com',
  projectId: 'your-project-id',
  network: bitcoin.networks.bitcoin,
  networkType: 'mainnet',
});

// Get balance
const balance = await provider.getBalance(address);
console.log('Balance:', balance);

// Push PSBT
const result = await provider.pushPsbt({ psbtBase64: signedPsbt });
console.log('Transaction:', result.txId);
```

### 4. Interact with Alkanes Contracts

```typescript
import { parseAlkaneId } from '@alkanes/ts-sdk';
import init, * as wasm from '@alkanes/ts-sdk/wasm';

// Initialize WASM
await init();

// Create provider with WASM
const provider = createProvider(config, wasm);

// Get alkane balance
const alkaneId = parseAlkaneId('840000:1');
const balance = await provider.getAlkaneBalance(address, alkaneId);
console.log('Alkane balance:', balance);

// Simulate contract call
const result = await provider.simulateAlkaneCall({
  alkaneId,
  method: 'transfer',
  args: [recipientAddress, amount],
});
console.log('Simulation result:', result);
```

## API Reference

### Keystore Management

#### `createKeystore(password, config?, wordCount?)`

Create a new encrypted keystore.

```typescript
const { keystore, mnemonic } = await createKeystore('password123', {
  network: 'mainnet',
}, 12);
```

**Parameters:**
- `password` (string): Encryption password (min 8 characters)
- `config` (WalletConfig): Optional wallet configuration
- `wordCount` (12 | 15 | 18 | 21 | 24): Mnemonic word count (default: 12)

**Returns:** `Promise<{ keystore: string, mnemonic: string }>`

#### `unlockKeystore(json, password, options?)`

Decrypt and unlock a keystore.

```typescript
const keystore = await unlockKeystore(keystoreJson, 'password123', {
  validate: true,
  network: 'mainnet',
});
```

**Parameters:**
- `json` (string | EncryptedKeystore): Encrypted keystore JSON
- `password` (string): Decryption password
- `options` (ImportOptions): Optional import options

**Returns:** `Promise<Keystore>`

#### `KeystoreManager`

Advanced keystore management with WASM integration.

```typescript
import init, * as wasm from '@alkanes/ts-sdk/wasm';

await init();
const manager = new KeystoreManager(wasm);

const mnemonic = manager.generateMnemonic(12);
const keystore = manager.createKeystore(mnemonic, { network: 'mainnet' });
const encrypted = await manager.exportKeystore(keystore, 'password', { pretty: true });
```

### Wallet Operations

#### `createWallet(keystore)`

Create a wallet from a keystore.

```typescript
const wallet = createWallet(keystore);
```

#### `createWalletFromMnemonic(mnemonic, network?)`

Create a wallet directly from a mnemonic.

```typescript
const wallet = createWalletFromMnemonic(mnemonic, 'mainnet');
```

#### `AlkanesWallet` Class

```typescript
// Get addresses
const receivingAddr = wallet.getReceivingAddress(0);
const changeAddr = wallet.getChangeAddress(0);

// Get multiple addresses
const addresses = wallet.getAddresses(0, 20); // Get 20 addresses starting from index 0

// Derive specific address types
import { AddressType } from '@alkanes/ts-sdk';

const p2wpkh = wallet.deriveAddress(AddressType.P2WPKH, 0);
const p2tr = wallet.deriveAddress(AddressType.P2TR, 0);

// Sign message
const signature = wallet.signMessage('Hello Alkanes', 0);

// Create and sign PSBT
const psbt = await wallet.createPsbt({
  inputs: [{ txid, vout, value, address }],
  outputs: [{ address, value }],
  feeRate: 10,
});

// Sign existing PSBT
const signed = wallet.signPsbt(psbtBase64);

// Extract transaction
const txHex = wallet.extractTransaction(signedPsbt);
```

### Provider (@oyl/sdk Compatible)

#### `createProvider(config, wasmModule?)`

Create a provider instance.

```typescript
const provider = createProvider({
  url: 'https://api.example.com',
  projectId: 'your-project-id',
  network: bitcoin.networks.bitcoin,
  networkType: 'mainnet',
}, wasm); // Optional WASM module for alkanes features
```

#### `AlkanesProvider` Class

```typescript
// Bitcoin operations
const blockInfo = await provider.getBlockInfo(840000);
const balance = await provider.getBalance(address);

// Transaction broadcasting
const result = await provider.pushPsbt({ psbtBase64 });

// Alkanes operations (requires WASM)
const alkaneBalance = await provider.getAlkaneBalance(address, alkaneId);
const simulation = await provider.simulateAlkaneCall({
  alkaneId: { block: 840000, tx: 1 },
  method: 'transfer',
  args: [recipient, amount],
});
```

### Utilities

```typescript
import {
  satoshisToBTC,
  btcToSatoshis,
  formatAlkaneId,
  parseAlkaneId,
  validateAddress,
  calculateFee,
  estimateTxSize,
  hexToBytes,
  bytesToHex,
} from '@alkanes/ts-sdk';

// Unit conversion
const btc = satoshisToBTC(100000000); // 1.0
const sats = btcToSatoshis(1.5); // 150000000

// Alkane ID formatting
const idString = formatAlkaneId({ block: 840000, tx: 1 }); // "840000:1"
const id = parseAlkaneId('840000:1'); // { block: 840000, tx: 1 }

// Address validation
const isValid = validateAddress('bc1q...', bitcoin.networks.bitcoin);

// Fee calculation
const fee = calculateFee(250, 10); // 2500 sats for 250 vbytes at 10 sat/vb
const vsize = estimateTxSize(2, 2, 'segwit'); // Estimate size for 2 inputs, 2 outputs
```

## Integration with @oyl/sdk

This SDK is designed to be a drop-in replacement for @oyl/sdk's Provider:

```typescript
import { AlkanesProvider } from '@alkanes/ts-sdk';
import { Wallet } from '@oyl/sdk'; // Your existing @oyl/sdk code

// Create Alkanes provider
const provider = new AlkanesProvider({
  url: 'https://api.example.com',
  projectId: 'your-project-id',
  network: bitcoin.networks.bitcoin,
  networkType: 'mainnet',
});

// Use with @oyl/sdk Wallet
const oylWallet = new Wallet({
  provider,
  // ... other options
});

// All @oyl/sdk functionality works
await oylWallet.sync();
const balance = await oylWallet.getBalance();
```

## WASM Integration

The SDK uses WebAssembly for high-performance alkanes operations:

```typescript
import init, * as wasm from '@alkanes/ts-sdk/wasm';

// Initialize WASM (call once at app startup)
await init();

// Use with providers
const provider = createProvider(config, wasm);

// Or with keystore manager
const manager = new KeystoreManager(wasm);
```

**Note:** WASM features require the `alkanes-web-sys` module to be built first.

## Building from Source

```bash
# Clone the repository
git clone https://github.com/kungfuflex/alkanes-rs.git
cd alkanes-rs/ts-sdk

# Install dependencies
npm install

# Build WASM module
npm run build:wasm

# Build TypeScript
npm run build:ts

# Or build everything
npm run build
```

## Development

```bash
# Watch mode
npm run dev

# Run tests
npm test

# Lint
npm run lint

# Format code
npm run format
```

## Security Considerations

⚠️ **Important Security Notes:**

1. **Never expose mnemonics** - Always use encrypted keystores in production
2. **Strong passwords** - Use at least 12 characters with mixed case, numbers, and symbols
3. **Secure storage** - Store keystores in secure locations with proper access controls
4. **HTTPS only** - Always use HTTPS for API calls in production
5. **Validate inputs** - Always validate addresses and amounts before signing transactions

## License

MIT

## Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

## Support

- 📖 [Documentation](https://docs.alkanes.xyz)
- 💬 [Discord](https://discord.gg/alkanes)
- 🐛 [Issues](https://github.com/kungfuflex/alkanes-rs/issues)

## Links

- [alkanes-rs Repository](https://github.com/kungfuflex/alkanes-rs)
- [@oyl/sdk](https://github.com/oyl-wallet/oyl-sdk)
- [Bitcoin Developer Documentation](https://developer.bitcoin.org)
