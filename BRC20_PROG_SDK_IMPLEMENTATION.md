# BRC20-prog SDK Implementation Summary

## Overview

Successfully integrated BRC20-prog functionality into the `@alkanes/ts-sdk` following the ethers.js pattern with separate Provider and Signer concepts. The SDK now provides a clean, object-based TypeScript API for deploying and interacting with BRC20-prog contracts - **no JSON string parameters required**.

## Architecture

### Separation of Concerns (ethers.js pattern)

- **Provider**: Read-only blockchain access (fetching data, broadcasting transactions)
- **Signer**: Transaction signing capability (PSBT signing, message signing)
- **AlkanesClient**: Combines Provider + Signer for full wallet functionality

### BRC20-prog Methods Location

All BRC20-prog transaction methods are on the **`AlkanesClient`** class because they require both:
1. **Signer** - to sign the PSBTs
2. **Provider** - to broadcast the signed transactions

This follows the same pattern as `sendTransaction()` and other transaction methods.

---

## Changes Made

### 1. WASM Bindings (`crates/alkanes-web-sys/src/lib.rs`)

Added three low-level WASM functions that accept JSON strings:

- `brc20_prog_deploy_contract(network, foundry_json, params_json)` → Promise
- `brc20_prog_transact(network, contract_address, function_signature, calldata, params_json)` → Promise
- `brc20_prog_wrap_btc(network, amount, target_contract, function_signature, calldata, params_json)` → Promise

**Key Features in WASM Layer:**
- Smart resume: `resume_from_commit` auto-detects commit vs reveal txid
- Exact fee calculation: No frontrunning profit opportunities
- Rebar Shield support: Private transaction relay
- MARA Slipstream support: Priority broadcasting

**Lines Added**: 253 lines (226 → 479 total)

### 2. TypeScript Type Definitions (`ts-sdk/src/types/brc20-prog.ts`)

Created clean TypeScript interfaces (no JSON strings!):

```typescript
export interface Brc20ProgExecuteParams {
  from_addresses?: string[];
  change_address?: string;
  fee_rate?: number;
  use_activation?: boolean;
  use_slipstream?: boolean;
  use_rebar?: boolean;
  rebar_tier?: 1 | 2;
  resume_from_commit?: string;  // Auto-detects commit or reveal!
}

export interface Brc20ProgDeployParams extends Brc20ProgExecuteParams {
  foundry_json: string | object;  // Accepts both!
}

export interface Brc20ProgTransactParams extends Brc20ProgExecuteParams {
  contract_address: string;
  function_signature: string;
  calldata: string[] | string;  // Array or comma-separated
}

export interface Brc20ProgWrapBtcParams {
  amount: number;
  target_contract: string;
  function_signature: string;
  calldata: string[] | string;
  from_addresses?: string[];
  change_address?: string;
  fee_rate?: number;
}

export interface Brc20ProgExecuteResult {
  commit_txid: string;
  reveal_txid: string;
  activation_txid?: string;
  commit_fee: number;
  reveal_fee: number;
  activation_fee?: number;
  inputs_used: string[];
  outputs_created: string[];
  traces?: any[];
}
```

**Exported from**: `ts-sdk/src/types/index.ts` and `ts-sdk/src/index.ts`

### 3. AlkanesClient Methods (`ts-sdk/src/client/client.ts`)

Added three high-level methods to `AlkanesClient`:

#### `deployBrc20ProgContract(params)`

```typescript
const result = await client.deployBrc20ProgContract({
  foundry_json: foundryBuildOutput,  // Can be object or string
  fee_rate: 100,
  use_activation: false,
  resume_from_commit: "txid..."  // Optional: auto-detects commit/reveal
});

console.log(`Deployed! Commit: ${result.commit_txid}`);
```

#### `transactBrc20Prog(params)`

```typescript
const result = await client.transactBrc20Prog({
  contract_address: "0x1234...",
  function_signature: "transfer(address,uint256)",
  calldata: ["0xRecipient", "1000"],  // Array or string
  fee_rate: 100
});

console.log(`Transaction sent! Activation: ${result.activation_txid}`);
```

#### `wrapBtc(params)`

```typescript
const result = await client.wrapBtc({
  amount: 100000,  // sats
  target_contract: "0xDeFi...",
  function_signature: "deposit(uint256)",
  calldata: ["100000"],
  fee_rate: 100
});

console.log(`BTC wrapped! Reveal: ${result.reveal_txid}`);
```

**Lines Added**: 174 lines to AlkanesClient class

### 4. Usage Example (`ts-sdk/examples/brc20-prog-usage.ts`)

Created comprehensive usage documentation showing:
- How to connect wallets (browser wallet, keystore)
- How to deploy contracts
- How to call contract functions
- How to wrap BTC
- Error handling and retry logic
- Advanced features (Rebar, Slipstream, resume)

---

## Key Features

### ✅ Clean Object-Based API

**Before (WASM layer - internal only):**
```typescript
// Low-level WASM - JSON strings (developers don't see this)
const result = await brc20_prog_deploy_contract(
  "regtest",
  '{"bytecode":{"object":"0x..."}}',
  '{"fee_rate":100,"use_activation":false}'
);
```

**After (TypeScript SDK - what developers use):**
```typescript
// High-level TypeScript - objects!
const result = await client.deployBrc20ProgContract({
  foundry_json: { bytecode: { object: "0x..." } },
  fee_rate: 100,
  use_activation: false
});
```

### ✅ Flexible Parameter Types

- `foundry_json`: Accepts string OR object
- `calldata`: Accepts array OR comma-separated string
- All optional parameters are properly typed

### ✅ Smart Resume

```typescript
// Pass EITHER commit OR reveal txid - system auto-detects!
const result = await client.deployBrc20ProgContract({
  foundry_json: originalBuild,
  resume_from_commit: "dae7fdc71957..."  // Could be commit or reveal
});
```

### ✅ Anti-Frontrunning Features

All CLI features are now available in the SDK:

- **Exact fee calculation**: Commit output = reveal fee + postage (no profit for frontrunners)
- **Rebar Shield**: Private relay through mining pools (~8-16% hashrate)
- **MARA Slipstream**: Priority inclusion
- **Smart resume**: Recover from failed transactions

### ✅ Full Type Safety

All parameters are strongly typed with TypeScript interfaces. No magic strings, no JSON manipulation.

---

## Usage Pattern

### 1. Connect to Wallet

```typescript
import { connectWallet } from '@alkanes/ts-sdk';

// Connect to browser wallet (Unisat, Xverse, etc.)
const client = await connectWallet('unisat');

// Or use keystore
const client = await AlkanesClient.withKeystore(keystoreJson, password, 'regtest');
```

### 2. Deploy Contract

```typescript
const result = await client.deployBrc20ProgContract({
  foundry_json: foundryBuild,
  fee_rate: 100
});
```

### 3. Call Contract

```typescript
const result = await client.transactBrc20Prog({
  contract_address: "0x...",
  function_signature: "transfer(address,uint256)",
  calldata: ["0xRecipient", "1000"]
});
```

### 4. Wrap BTC

```typescript
const result = await client.wrapBtc({
  amount: 100000,
  target_contract: "0x...",
  function_signature: "deposit(uint256)",
  calldata: ["100000"]
});
```

---

## Files Modified

### Rust/WASM
- `crates/alkanes-web-sys/src/lib.rs` - Added 253 lines
- `crates/alkanes-cli-common/src/brc20_prog/*` - Already had the functionality

### TypeScript SDK
- `ts-sdk/src/types/brc20-prog.ts` - **NEW FILE** (91 lines)
- `ts-sdk/src/types/index.ts` - Added exports
- `ts-sdk/src/client/client.ts` - Added 174 lines
- `ts-sdk/src/index.ts` - Added exports
- `ts-sdk/examples/brc20-prog-usage.ts` - **NEW FILE** (204 lines)

### Generated Files
- `ts-sdk/wasm/alkanes_web_sys.d.ts` - Auto-generated TypeScript definitions
- `ts-sdk/wasm/alkanes_web_sys_bg.wasm` - Compiled WASM binary (7.1MB)
- `ts-sdk/wasm/alkanes_web_sys_bg.js` - JavaScript glue code
- `ts-sdk/build/wasm/*` - Build artifacts

---

## Verification

### TypeScript Compilation
✅ Passes `tsc --noEmit` with no errors

### WASM Build
✅ Successfully built with `wasm-pack` (1m 02s)

### Type Exports
✅ All types properly exported from main index
✅ All methods properly typed on AlkanesClient

---

## Example Output

When you import the SDK:

```typescript
import {
  AlkanesClient,
  connectWallet,
  // BRC20-prog types
  Brc20ProgDeployParams,
  Brc20ProgTransactParams,
  Brc20ProgWrapBtcParams,
  Brc20ProgExecuteResult,
} from '@alkanes/ts-sdk';

const client = await connectWallet('unisat');

// All methods are now available on the client:
await client.deployBrc20ProgContract(params);
await client.transactBrc20Prog(params);
await client.wrapBtc(params);
```

---

## Benefits

1. **Clean API**: No JSON string manipulation required
2. **Type Safety**: Full TypeScript type checking and IntelliSense
3. **Flexible**: Accepts objects or strings where appropriate
4. **Smart**: Auto-detects commit vs reveal txids for resume
5. **Feature Complete**: All CLI features available (Rebar, Slipstream, exact fees)
6. **Well Documented**: Comprehensive examples and JSDoc comments
7. **Follows Patterns**: Matches ethers.js Provider/Signer pattern

---

## Next Steps

The SDK is now ready for:
1. Publishing to npm as `@alkanes/ts-sdk`
2. Integration into frontend applications
3. Use in Node.js scripts and automation
4. Building higher-level abstractions (e.g., contract ABIs)

All BRC20-prog functionality from the CLI is now accessible through a clean, type-safe TypeScript API!
