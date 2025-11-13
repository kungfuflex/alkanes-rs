# Rebar Shield Integration

Rebar Shield is a private transaction relay service for Bitcoin that provides MEV protection by sending transactions directly to mining pools, bypassing the public mempool.

## ⚠️ Important Warning

**Rebar Shield is NOT recommended for large UTXO consolidations!**

For a 980KB consolidation transaction:
- **Slipstream cost:** 2.06M sats (11% of input) ✅
- **Rebar Tier 1 cost:** 17.74M sats (97% of input) ❌
- **Rebar Tier 2 cost:** 29.5M sats (exceeds input!) ❌

**Use Rebar Shield only for small, high-value transactions needing MEV protection.**

## When to Use Rebar Shield

### ✅ Good Use Cases

- **NFT Mints:** Protect against sniping and frontrunning
- **Protocol Operations:** MEV-sensitive smart contract interactions
- **High-value Trades:** Transactions vulnerable to sandwich attacks
- **Privacy-critical Txs:** Small transactions needing maximum privacy

### ❌ Bad Use Cases

- **Large Consolidations:** Cost is 97% of input!
- **Regular Transfers:** Standard broadcast is cheaper
- **Low-value Txs:** Payment fee exceeds transaction value

## How Rebar Shield Works

Unlike standard broadcasting or Slipstream, Rebar requires a **payment output** in your transaction:

```
Transaction Structure:
  Inputs: [Your UTXOs]
  Outputs:
    [0] Your destination address (main funds)
    [1] Rebar payment address (service fee)
```

### Payment Calculation

```
rebar_payment = transaction_vsize × tier_feerate
```

This payment is **in addition** to the base network fee.

## Fee Tiers

Rebar offers two fee tiers with different hashrate coverage:

| Tier | Fee Rate | Hashrate Coverage | Use When |
|------|----------|------------------|----------|
| **Tier 1** | 16 sat/vB | ~8% | Standard protection |
| **Tier 2** | 28 sat/vB | ~16% | Maximum hashrate |

**Payment Address:** `bc1qfelpskqcy3xmyrnhq4hz6y0rzk68ayn09juaek`

## Cost Examples

### Small Transaction (250 vbytes)

**Tier 1:**
- Base fee: 525 sats @ 2.1 sat/vB
- Rebar payment: 4,000 sats
- **Total: 4,525 sats**

**Tier 2:**
- Base fee: 525 sats
- Rebar payment: 7,000 sats
- **Total: 7,525 sats**

### Medium Transaction (10,000 vbytes / ~10KB)

**Tier 1:**
- Base fee: 21,000 sats
- Rebar payment: 160,000 sats
- **Total: 181,000 sats**

**Tier 2:**
- Base fee: 21,000 sats
- Rebar payment: 280,000 sats
- **Total: 301,000 sats**

### Large Transaction (980,000 vbytes / ~980KB)

**Tier 1:**
- Base fee: 2,058,000 sats
- Rebar payment: 15,680,000 sats
- **Total: 17,738,000 sats (97% of 18.3M input!)** ❌

**Not viable for large transactions!**

## Using Rebar Shield

### Query Rebar Info

Get current payment address and fee tiers:

```bash
./scripts/query-rebar.sh
```

Output:
```
Payment Address: bc1qfelpskqcy3xmyrnhq4hz6y0rzk68ayn09juaek
Block Height: 920137
Fee Tier 1: 16 sat/vB @ 8% hashrate
Fee Tier 2: 28 sat/vB @ 16% hashrate
```

### Broadcasting with Rebar

**⚠️ Important:** Your transaction must already include the Rebar payment output!

```bash
deezel -p mainnet bitcoind sendrawtransaction \
  --from-file signed_tx.hex \
  --use-rebar
```

### Building Transactions with Rebar Payment

**TODO:** Automatic Rebar payment output is not yet implemented in `wallet send`.

For now, you must manually add the payment output to your transaction before signing.

## Implementation Details

### API Endpoints

**Info Endpoint:**
```
GET https://shield.rebarlabs.io/v1/info
```

Response:
```json
{
  "height": 920137,
  "payment": {
    "p2wpkh": "bc1qfelpskqcy3xmyrnhq4hz6y0rzk68ayn09juaek"
  },
  "fees": [
    {
      "estimated_hashrate": 0.08,
      "feerate": 16
    },
    {
      "estimated_hashrate": 0.16,
      "feerate": 28
    }
  ]
}
```

**RPC Endpoint:**
```
POST https://shield.rebarlabs.io/v1/rpc
```

Request:
```json
{
  "jsonrpc": "2.0",
  "id": "alkanes-cli",
  "method": "sendrawtransaction",
  "params": ["<tx_hex>"]
}
```

Response:
```json
{
  "result": "txid",
  "error": null,
  "id": "alkanes-cli"
}
```

### Command-Line Flags

**Send command:**
```bash
deezel wallet send \
  --address bc1p... \
  --amount 10000 \
  --use-rebar \
  --rebar-tier 1
```

**Sendrawtransaction command:**
```bash
deezel bitcoind sendrawtransaction \
  --from-file tx.hex \
  --use-rebar
```

### Helper Module

Located in `crates/alkanes-cli-common/src/provider.rs`:

```rust
pub mod rebar {
    // Query Rebar Shield for payment info
    pub async fn query_info() -> Result<RebarInfo>
    
    // Calculate payment amount
    pub fn calculate_payment(tx_vsize: usize, tier: &RebarFeeTier) -> u64
    
    // Get specific fee tier
    pub fn get_tier(info: &RebarInfo, tier_index: u8) -> Result<&RebarFeeTier>
    
    // Submit transaction via JSON-RPC
    pub async fn submit_transaction(tx_hex: &str) -> Result<String>
    
    // Print fee information
    pub fn print_fee_info(info: &RebarInfo, tx_vsize: usize)
}
```

## Comparison: Slipstream vs Rebar

| Feature | Slipstream | Rebar Shield |
|---------|-----------|--------------|
| **API Type** | REST | JSON-RPC |
| **Fee Structure** | Higher fee rate | Payment output |
| **Min Cost** | 2 sat/vB | 16 sat/vB |
| **MEV Protection** | Basic (private relay) | Advanced (mining pools) |
| **Max Size** | ~1MB | ~1MB |
| **Transaction Modification** | None | Requires payment output |
| **Best For** | Large consolidations | Small, MEV-sensitive txs |

## Cost Comparison (980KB Transaction)

| Method | Cost | % of 18.3M Input | You Receive |
|--------|------|-----------------|-------------|
| **Slipstream** | **2.06M** | **11%** | **16.25M** ✅ |
| Rebar Tier 1 | 17.74M | 97% | 569K ❌ |
| Rebar Tier 2 | 29.5M | 161% | -11.2M ❌ |

**Clear winner:** Slipstream for large transactions!

## When Rebar Makes Sense

Rebar Shield is economically viable when:

1. **Transaction value >> payment cost**
   - Example: 1 BTC NFT mint with 100KB tx
   - Rebar payment: 1.6M sats = 0.016 BTC
   - Acceptable for 1 BTC asset

2. **MEV attack risk is high**
   - NFT sniping bots active
   - High-value DeFi operations
   - Time-sensitive protocol interactions

3. **Privacy is paramount**
   - Direct to mining pools
   - Maximum confidentiality
   - No public mempool exposure

## Troubleshooting

### "Transaction must include Rebar payment output"

**Problem:** Transaction doesn't have payment output

**Solution:** 
1. Query current payment address: `./scripts/query-rebar.sh`
2. Add output to your transaction
3. Calculate amount: `tx_vsize × tier_feerate`

### "Rebar Shield error: insufficient fee"

**Problem:** Payment output amount is too low

**Solution:**
1. Check current tier rates
2. Recalculate payment amount
3. Rebuild transaction with correct payment

### Cost is too high

**Problem:** Rebar payment exceeds acceptable threshold

**Solution:**
1. Use Slipstream instead (much cheaper)
2. Use standard broadcast if < 100KB
3. Consider if MEV protection is really needed

## Test Cases

Test cases are available in:
```
crates/alkanes-cli-common/tests/rebar_integration_test.rs
```

These tests demonstrate:
- Payment calculation
- Transaction structure
- Cost comparisons
- Slipstream vs Rebar analysis

## Future Enhancements

**TODO:**
- [ ] Automatic Rebar payment output in `wallet send`
- [ ] Warning when Rebar cost exceeds 50% of input
- [ ] Cost comparison display before sending
- [ ] Rebar-specific transaction builder

## References

- **Rebar Shield Docs:** https://docs.rebarlabs.io/shield/intro
- **API Reference:** https://docs.rebarlabs.io/shield/api/get-info
- **Query Script:** [scripts/query-rebar.sh](../../scripts/query-rebar.sh)
- **Test Cases:** [tests/rebar_integration_test.rs](../../crates/alkanes-cli-common/tests/rebar_integration_test.rs)

## Related Documentation

- [Transaction Broadcasting Options](./transaction-broadcasting.md)
- [External Signing Workflow](./external-signing.md)
- [Main README](../../README.md)
