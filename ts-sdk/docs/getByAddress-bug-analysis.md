# @alkanes/ts-sdk `getByAddress()` Bug Analysis

## Summary

The code is correctly using the high-level SDK API, but there's a parsing issue with the response structure.

---

## The Good News

You're using the correct API method:

```typescript
alkanesData = await alkanesRpc.getByAddress(address);
```

This is the right approach and avoids the "memory access out of bounds" error that occurs when calling `metashrew.view()` directly with incorrect parameters.

---

## The Bug

### Problem: Incorrect `balance_sheet` Parsing

The actual response structure from `getByAddress()` is:

```json
{
  "balances": [
    {
      "output": {
        "value": 330,
        "script_pubkey": "5120b93270be..."
      },
      "outpoint": "e4517e26e4dc46afdbc61d02cdc2563783b1441198296865a07a26c9f1ec7fba:0",
      "balance_sheet": {
        "cached": {
          "balances": {
            // Tokens are HERE - nested two levels deep!
          }
        }
      }
    }
  ]
}
```

### Your Code Is Looking In The Wrong Place

```typescript
// Your current code looks for:
balanceSheet.entries           // ❌ Doesn't exist
Array.isArray(balanceSheet)    // ❌ It's an object
Object.entries(balanceSheet)   // ❌ Gets "cached", not the actual balances
```

### The Fix

The tokens are at `balance_sheet.cached.balances`:

```typescript
// ✅ Correct path to token balances
const tokenBalances = balance.balance_sheet?.cached?.balances || {};
```

---

## Two Approaches

### Option 1: Use `getBalance()` for Aggregated Balances (Simpler)

If you just need to know what tokens an address holds and their total amounts:

```typescript
const balances = await provider.alkanes.getBalance(address);

// Returns:
[
  {
    alkane_id: { block: 2, tx: 20 },
    name: "",
    symbol: "",
    balance: "10"
  },
  {
    alkane_id: { block: 2, tx: 24 },
    name: "",
    symbol: "",
    balance: "1"
  },
  // ... more tokens
]
```

This is simpler and gives you aggregated balances across all UTXOs.

### Option 2: Use `getByAddress()` for Per-UTXO Detail

If you need to know which specific UTXO holds which tokens:

```typescript
const alkanesData = await provider.alkanes.getByAddress(address);

// Then parse correctly:
const balances = alkanesData.balances || [];

balances.forEach(balance => {
  const outpoint = balance.outpoint;  // "txid:vout"
  const output = balance.output;       // { value, script_pubkey }

  // ✅ Correct path to token balances
  const tokenBalances = balance.balance_sheet?.cached?.balances || {};

  // tokenBalances is an object like:
  // { "2:20": "10", "2:24": "1" }  // alkane_id -> balance

  Object.entries(tokenBalances).forEach(([alkaneId, amount]) => {
    console.log(`Alkane ${alkaneId}: ${amount}`);
  });
});
```

---

## Important Note

Many UTXOs returned by `getByAddress()` will have **empty** `balance_sheet.cached.balances`. These are regular BTC UTXOs without alkane tokens. This is normal - only some UTXOs hold tokens.

---

## Recommended Code Fix

Replace your `balance_sheet` parsing logic with:

```typescript
// Extract runes from balance_sheet - CORRECT PATH
const balanceSheet = balance.balance_sheet || {};
const cachedBalances = balanceSheet.cached?.balances || {};
const runes: any[] = [];

// cachedBalances is an object keyed by alkane_id
// Format: { "block:tx": "amount", ... }
Object.entries(cachedBalances).forEach(([alkaneIdStr, amount]) => {
  // Parse alkane_id string like "2:20" into { block: 2, tx: 20 }
  const [block, tx] = alkaneIdStr.split(':').map(n => parseInt(n, 10));

  runes.push({
    alkane_id: { block, tx },
    amount: String(amount),
  });
});
```

---

## Quick Test

You can verify the structure by logging:

```typescript
console.log('Full balance_sheet:', JSON.stringify(balance.balance_sheet, null, 2));
console.log('Cached balances:', balance.balance_sheet?.cached?.balances);
```

---

## Reference

- SDK Source: `@alkanes/ts-sdk`
- Correct Methods:
  - `provider.alkanes.getBalance(address)` - aggregated token balances
  - `provider.alkanes.getByAddress(address)` - per-UTXO token balances
- Integration Tests: `tests/diesel-mint.integration.test.ts`
