# Transaction Broadcasting Options

This guide covers all available methods for broadcasting Bitcoin transactions, especially useful for large consolidation transactions.

## Quick Decision Matrix

| Transaction Size | Recommended Method | Cost (980KB example) |
|-----------------|-------------------|---------------------|
| < 100KB | Standard RPC or Public RPC | Variable |
| 100KB - 1MB | **Slipstream** ⭐ | ~2.06M sats (11%) |
| > 1MB | Truncate with `--truncate-excess-vsize` | N/A |
| MEV-sensitive | Rebar Shield (if small) | 16-28 sat/vB |

## 1. Standard Bitcoin Core RPC

Use your local or configured Bitcoin node.

### Pros
- ✅ Direct control
- ✅ No additional cost
- ✅ Standard protocol

### Cons
- ❌ Limited to 100KB (standard relay policy)
- ❌ Requires running a node

### Usage

```bash
deezel -p mainnet bitcoind sendrawtransaction \
  --from-file signed_tx.hex
```

Or with custom RPC URL:
```bash
deezel -p mainnet \
  --bitcoin-rpc-url http://your-node:8332 \
  bitcoind sendrawtransaction \
  --from-file signed_tx.hex
```

### Best For
Normal transactions under 100KB

## 2. Public Bitcoin RPC Nodes

Try multiple public endpoints automatically.

### Available Endpoints
- `https://bitcoin-rpc.publicnode.com` (AllNodes)
- `https://bitcoin.api.onfinality.io/public` (OnFinality)
- `https://public-btc.nownodes.io` (NOWNodes - rate limited)
- `https://bitcoin-mainnet.public.blastapi.io` (Blast API)

### Pros
- ✅ No setup required
- ✅ Free to use
- ✅ Multiple fallback options

### Cons
- ❌ Limited to 100KB
- ❌ Rate limited
- ❌ May be unreliable
- ❌ No privacy guarantees

### Usage

```bash
./cmd-broadcast-libre.sh
```

The script automatically tries all endpoints until one succeeds.

### Best For
Small transactions when you don't have a local node

## 3. MARA Slipstream ⭐ RECOMMENDED

Private relay service that accepts large transactions up to ~1MB.

### Pros
- ✅ Accepts up to ~1MB (400KB weight)
- ✅ Bypasses 100KB relay limit
- ✅ Private relay to miners
- ✅ Relatively low cost (2 sat/vB min)
- ✅ Simple REST API

### Cons
- ❌ Minimum fee: 2 sat/vB
- ❌ Max size: ~1MB

### Cost Analysis

For a 980KB consolidation transaction:
- Fee: ~2.06M sats
- Percentage: ~11% of 18.3M input
- **Result: 16.25M sats received ✅**

### Usage

```bash
deezel -p mainnet bitcoind sendrawtransaction \
  --from-file signed_tx.hex \
  --use-slipstream
```

Or use the helper script:
```bash
./cmd-broadcast-all-options.sh
# Select option 1 (Slipstream)
```

### Technical Details

- **Endpoint:** `https://slipstream.mara.com/rest-api/submit-tx`
- **Method:** POST
- **Format:** JSON with `tx_hex` field
- **Response:** Returns txid in `message` field

### Best For
- Large transactions (100KB - 1MB)
- UTXO consolidations
- Non-standard transactions
- Bypassing mempool relay limits

## 4. Rebar Shield ⚠️ EXPENSIVE

Private relay with MEV protection, but very expensive for large transactions.

### Pros
- ✅ MEV protection (frontrunning, sniping)
- ✅ Private relay to mining pools
- ✅ Hashrate coverage options
- ✅ Bitcoin Core JSON-RPC compatible

### Cons
- ❌ VERY EXPENSIVE for large txs
- ❌ Requires payment OUTPUT in tx
- ❌ Tier 1: 16 sat/vB, Tier 2: 28 sat/vB

### Cost Analysis

For a 980KB consolidation transaction:

**Tier 1 (8% hashrate):**
- Base fee: ~2.06M sats
- Rebar payment: ~15.68M sats
- **Total: 17.74M sats (97% of input!)**
- Result: Only 569K sats received ❌

**Tier 2 (16% hashrate):**
- Rebar payment: ~27.44M sats
- **Exceeds total input!** ❌

### ⚠️ Warning

**Rebar Shield is NOT VIABLE for large consolidations!** Only use for small, high-value transactions needing MEV protection.

### Usage

```bash
# Transaction must already include Rebar payment output!
deezel -p mainnet bitcoind sendrawtransaction \
  --from-file signed_tx.hex \
  --use-rebar
```

### Technical Details

- **Endpoint:** `https://shield.rebarlabs.io/v1/rpc`
- **Method:** POST JSON-RPC
- **Payment address:** `bc1qfelpskqcy3xmyrnhq4hz6y0rzk68ayn09juaek`
- **Payment amount:** `tx_vsize × tier_feerate`

### Best For
- Small, high-value transactions (<100KB)
- NFT mints needing MEV protection
- Protocol operations vulnerable to frontrunning
- When privacy > cost

See [Rebar Shield](./rebar-shield.md) for full details.

## 5. Libre Relay (Your Own Node)

Run your own Bitcoin node with Libre Relay fork by Peter Todd.

### Pros
- ✅ No size limits (beyond consensus)
- ✅ No filtering/censorship
- ✅ Full control
- ✅ Can relay sub-sat/vB txs
- ✅ Maximum privacy

### Cons
- ❌ Requires setup
- ❌ Maintenance required
- ❌ Resources (disk, bandwidth)

### Setup

**Option A: Build from Source**
```bash
git clone https://github.com/petertodd/bitcoin -b libre-relay
cd bitcoin
./autogen.sh
./configure
make
sudo make install
```

**Option B: Umbrel App**
1. Install Umbrel OS
2. Open App Store
3. Install "Libre Relay"
4. Configure RPC access

### Usage

Once running:
```bash
deezel -p mainnet \
  --bitcoin-rpc-url http://localhost:8332 \
  bitcoind sendrawtransaction \
  --from-file signed_tx.hex
```

### Best For
- Regular large transaction broadcasting
- Maximum flexibility
- Full privacy and control
- Sub-sat/vB transactions

## Interactive Helper Script

The comprehensive broadcast helper shows all options with cost analysis:

```bash
./cmd-broadcast-all-options.sh
```

This script will:
1. ✅ Analyze your transaction size
2. ✅ Show all viable options
3. ✅ Display cost estimates
4. ✅ Provide recommendations
5. ✅ Let you choose interactively

## Cost Comparison Table

For 980KB consolidation (18.3M sats input):

| Method | Cost | % of Input | Receive | Viable? |
|--------|------|------------|---------|---------|
| Standard RPC | N/A | N/A | N/A | ❌ Too large |
| Public RPC | N/A | N/A | N/A | ❌ Too large |
| **Slipstream** | **2.06M** | **11%** | **16.25M** | **✅ YES** |
| Rebar Tier 1 | 17.74M | 97% | 569K | ❌ Too expensive |
| Rebar Tier 2 | 29.5M | 161% | -11.2M | ❌ Exceeds input |
| Libre Relay | ~2.06M | 11% | 16.25M | ✅ YES (if you run it) |

**Winner:** Slipstream (or Libre Relay if you run your own node)

## Transaction Size Limits

### Network Limits
- **Standard relay policy:** 100KB max
- **Bitcoin consensus:** 1MB max (4MB weight)
- **Slipstream:** ~400KB weight (~1MB legacy)
- **Rebar Shield:** ~1MB

### Handling Large Transactions

If your transaction exceeds 1MB, use auto-truncation:

```bash
deezel wallet sign-tx \
  --from-file unsigned_tx.hex \
  --wallet-key-file privkey.hex \
  --truncate-excess-vsize \
  > signed_tx.hex
```

This keeps transaction under 1MB by:
- Limiting inputs to 9,345 (for P2TR)
- Preserving original fee rate
- Recalculating output accordingly

## Helper Scripts

### Available Scripts

1. **`cmd-build-tx-only.sh`**
   - Build and sign transaction
   - Auto-truncate if needed
   - Save to file

2. **`cmd-broadcast-all-options.sh`** ⭐
   - Interactive broadcast helper
   - Shows all methods with costs
   - Recommends best option

3. **`cmd-broadcast-libre.sh`**
   - Try public RPC nodes
   - Automatic fallback
   - Error handling

4. **`scripts/query-rebar.sh`**
   - Query Rebar Shield info
   - Show fee tiers
   - Calculate costs

### Quick Start

```bash
# 1. Build transaction
./cmd-build-tx-only.sh

# 2. Choose broadcast method
./cmd-broadcast-all-options.sh
```

## Troubleshooting

### "tx-size" Error

**Problem:** Transaction exceeds size limits

**Solutions:**
1. Use Slipstream (up to 1MB)
2. Use `--truncate-excess-vsize` to reduce size
3. Run your own Libre Relay node

### "min relay fee not met"

**Problem:** Fee rate too low

**Solutions:**
- Slipstream: Minimum 2 sat/vB
- Check `getnetworkinfo` for network min
- Increase `--fee-rate` parameter

### "bad-txns-inputs-missingorspent"

**Problem:** Inputs don't exist or already spent

**Solutions:**
1. Verify UTXOs are unspent
2. Check correct network (mainnet/testnet)
3. Look for conflicting transactions

### Connection Refused

**Problem:** Can't reach RPC endpoint

**Solutions:**
1. Verify RPC URL is accessible
2. Check credentials
3. Check firewall rules
4. Try alternative public endpoint

## Best Practices

### For Large Consolidations

1. ✅ Use `--send-all` to consolidate all UTXOs
2. ✅ Use `--truncate-excess-vsize` to stay under 1MB
3. ✅ Broadcast via Slipstream (11% cost)
4. ✅ Verify transaction before broadcasting
5. ❌ Don't use Rebar Shield (97% cost!)

### For Privacy

1. ✅ Use Slipstream or Rebar Shield
2. ✅ Run your own Libre Relay node
3. ❌ Avoid public RPC nodes
4. ✅ Consider Tor for additional privacy

### For Cost Optimization

1. ✅ Use lowest viable fee rate
2. ✅ Consolidate during low-fee periods
3. ✅ Use Slipstream over Rebar for large txs
4. ✅ Monitor mempool before broadcasting

## References

- **Slipstream:** https://www.mara.com/slipstream
- **Rebar Shield:** https://docs.rebarlabs.io/shield/intro
- **Libre Relay:** https://github.com/petertodd/bitcoin
- **Umbrel Libre Relay:** https://apps.umbrel.com/app/libre-relay

## Related Documentation

- [External Signing Workflow](./external-signing.md)
- [Rebar Shield Integration](./rebar-shield.md)
- [Main README](../../README.md)
