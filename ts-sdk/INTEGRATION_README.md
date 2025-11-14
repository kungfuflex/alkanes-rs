# @alkanes/ts-sdk - Integration with @oyl/sdk

This TypeScript SDK provides a keystore backend for **@oyl/sdk** with full Bitcoin regtest support and Alkanes smart contract features.

## Status

✅ **WASM Module**: Built and ready (`wasm-pkg/`)  
📦 **TypeScript SDK**: Ready to build  
📖 **Documentation**: Complete integration guides available  
🧪 **Examples**: Working code examples provided  

## Quick Build

```bash
# Build TypeScript SDK
npx tsup src/index.ts --format cjs,esm --dts --clean

# Or use the build script
chmod +x build-and-link.sh
./build-and-link.sh
```

## Link to Your Project

```bash
# In this directory
npm link

# In your project
npm link @alkanes/ts-sdk
npm install @oyl/sdk bitcoinjs-lib
```

## Documentation

Comprehensive guides are available in the parent directory:

- **📘 Full Integration Guide**: `/Users/erickdelgado/Documents/github/ALKANES_OYL_INTEGRATION_GUIDE.md`
  - Complete architecture overview
  - Step-by-step setup instructions
  - Security considerations
  - Troubleshooting guide

- **⚡ Quick Start Guide**: `/Users/erickdelgado/Documents/github/ALKANES_OYL_QUICKSTART.md`
  - 5-minute setup
  - Ready-to-use code snippets
  - Regtest testing workflow

- **✅ Completion Summary**: `/Users/erickdelgado/Documents/github/ALKANES_BACKEND_COMPLETE.md`
  - Task summary
  - Architecture diagram
  - Testing checklist
  - Next steps

## Example Usage

See `examples/oyl-integration.ts` for a complete working example.

Quick snippet:

```typescript
import { createKeystore, createWallet, createProvider } from '@alkanes/ts-sdk';
import { Wallet as OylWallet } from '@oyl/sdk';
import * as bitcoin from 'bitcoinjs-lib';
import init, * as wasm from '@alkanes/ts-sdk/wasm';

// Initialize WASM
await init();

// Create keystore
const { keystore, mnemonic } = await createKeystore('password', { 
  network: 'regtest' 
});

// Create wallet
const unlocked = await unlockKeystore(keystore, 'password');
const alkanesWallet = createWallet(unlocked);

// Create provider
const provider = createProvider({
  url: 'http://localhost:18443',
  network: bitcoin.networks.regtest,
  networkType: 'regtest',
}, wasm);

// Integrate with @oyl/sdk
const oylWallet = new OylWallet({
  provider: provider as any,
  address: alkanesWallet.getReceivingAddress(0),
  signer: async (psbt) => alkanesWallet.signPsbt(psbt),
});
```

## Features

- ✅ **Keystore**: Encrypted BIP39 mnemonics (ethers.js compatible)
- ✅ **HD Wallet**: BIP32/44/84/86 derivation paths
- ✅ **@oyl/sdk Provider**: Drop-in compatible provider interface
- ✅ **Regtest Support**: Full Bitcoin Core regtest compatibility
- ✅ **PSBT Signing**: Native Bitcoin transaction signing
- ✅ **Alkanes**: Smart contract features via WASM

## What This Provides

### For @oyl/sdk Integration

This SDK implements the provider interface expected by @oyl/sdk, allowing you to use alkanes-rs as the backend for:

1. **Wallet Management**: Create, restore, and manage Bitcoin wallets
2. **Transaction Signing**: Sign PSBTs with HD-derived keys
3. **Address Generation**: Generate addresses for all Bitcoin script types
4. **Balance Queries**: Check balances via Bitcoin Core RPC or Esplora
5. **Transaction Broadcasting**: Send transactions to the Bitcoin network

### Alkanes-Specific Features

When used with the WASM backend:

1. **Smart Contracts**: Interact with Alkanes Bitcoin smart contracts
2. **Token Balances**: Query alkane token balances
3. **Contract Calls**: Simulate and execute contract methods
4. **Bytecode Access**: Read and analyze contract code

## Architecture

```
Your App
   ↓
@oyl/sdk Wallet
   ↓
@alkanes/ts-sdk Provider ← You are here
   ↓
alkanes-web-sys WASM
```

## Testing with Regtest

```bash
# Start Bitcoin Core
bitcoind -regtest -daemon -rpcuser=user -rpcpassword=pass -rpcport=18443

# Generate blocks
bitcoin-cli -regtest createwallet "test"
bitcoin-cli -regtest generatetoaddress 101 $(bitcoin-cli -regtest getnewaddress)

# Fund your address
bitcoin-cli -regtest sendtoaddress <your-address> 1.0
bitcoin-cli -regtest generatetoaddress 1 $(bitcoin-cli -regtest getnewaddress)
```

## Next Steps

1. Build the SDK (see commands above)
2. Link to your project
3. Follow the integration guides
4. Check out the example code in `examples/oyl-integration.ts`
5. Test with regtest
6. Deploy to testnet/mainnet

## Support

For detailed documentation and troubleshooting:
- Read the full integration guide
- Check the example implementation
- Review the quick start guide

All guides are located in `/Users/erickdelgado/Documents/github/`

## Source

This SDK is from the `kungfuflex/develop` branch of **alkanes-rs**.

Repository: https://github.com/kungfuflex/alkanes-rs
