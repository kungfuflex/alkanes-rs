# Alkanes Execute Quick Start Guide

## What Changed?

**TL;DR**: The `alkanes execute` command now **automatically creates outputs** for your alkanes and **returns BTC change** to prevent burning your assets.

---

## Before vs After

### ❌ Before (Dangerous!)
```bash
alkanes execute "[3,65520,50]:v0:v0" --envelope contract.wasm --from p2tr:0
# Result: Alkanes burned! BTC potentially lost!
```

### ✅ After (Safe!)
```bash
alkanes execute "[3,65520,50]:v0:v0" --envelope contract.wasm --from p2tr:0
# Result: Alkanes sent to p2tr:0, BTC change to p2wsh:0
```

---

## Key Concepts

### 1. Output Identifiers (v0, v1, v2, ...)

When your protostone references `v0`, `v1`, etc., these create **physical outputs** in the transaction:
- `v0` → Output 0
- `v1` → Output 1
- `v2` → Output 2
- etc.

**Example**:
```bash
# Protostone says: send results to v0
alkanes execute "[3,100]:v0:v0" --envelope contract.wasm

# Transaction creates:
# - Output 0 (v0): Receives contract alkane
# - Output 1: OP_RETURN (runestone)
# - Output 2: BTC change
```

---

## Common Patterns

### Pattern 1: Deploy Contract (Simplest)
```bash
alkanes execute "[3,100,50]:v0:v0" --envelope contract.wasm
```
**What happens**:
- Contract deployed to alkane `[4, 100]`
- Alkane sent to **p2tr:0** (default)
- BTC change sent to **p2wsh:0** (default)

---

### Pattern 2: Deploy with Custom Change Address
```bash
alkanes execute "[3,100,50]:v0:v0" \
  --envelope contract.wasm \
  --change p2tr:5
```
**What happens**:
- Contract deployed to alkane `[4, 100]`
- Alkane sent to **p2tr:5** (same as change)
- BTC change sent to **p2tr:5**

---

### Pattern 3: Deploy with Custom Recipient
```bash
alkanes execute "[3,100,50]:v0:v0" \
  --envelope contract.wasm \
  --to p2tr:10
```
**What happens**:
- Contract deployed to alkane `[4, 100]`
- Alkane sent to **p2tr:10** (explicit)
- BTC change sent to **p2wsh:0** (default)

---

### Pattern 4: Multiple Outputs
```bash
alkanes execute "[4,100,0]:v0:v0,[4,200,0]:v1:v1" \
  --to p2tr:0,p2tr:1
```
**What happens**:
- First protostone sends to **v0** (Output 0 = p2tr:0)
- Second protostone sends to **v1** (Output 1 = p2tr:1)
- BTC change sent to **p2wsh:0** (default)

---

## Flag Reference

### `--to` (Optional)
Comma-separated list of addresses for identifier outputs.

```bash
--to p2tr:0,p2tr:1,p2wsh:0
```
- First address → v0
- Second address → v1
- Third address → v2

**Default**: If not specified, uses `--change` address, or falls back to `p2tr:0`

---

### `--change` (Optional)
Address for BTC change output.

```bash
--change p2tr:5
```

**Default**: `p2wsh:0`

**Also affects**: If `--to` is not specified, `--change` is used for identifier outputs too.

---

### `--from` (Optional)
Source addresses for UTXO selection.

```bash
--from p2tr:0,p2tr:1
```

**Default**: All wallet addresses

---

### `--inputs` (Required)
Specify input requirements.

```bash
--inputs "requirement1,requirement2,requirement3"
```

**Format**:
- `B:amount` - Bitcoin requirement (satoshis)
- `block:tx:amount` - Alkanes requirement
- `B:amount:vN` - Bitcoin output assignment (not yet implemented)

**Examples**:
```bash
--inputs "B:50000000"           # Need 50M sats
--inputs "2:1:1"                # Need 1 unit of alkane [2,1]
--inputs "2:1:1,B:50000000"     # Need both
```

---

## Real-World Examples

### Example 1: Deploy OYL Beacon Proxy
```bash
alkanes execute "[3,780993,36863]:v0:v0" \
  --envelope alkanes_std_beacon_proxy.wasm \
  --from p2tr:0 \
  --inputs "B:10000000" \
  --fee-rate 1.0 \
  --mine \
  --trace \
  -y
```
**Breakdown**:
- `[3,780993,36863]:v0:v0` - Deploy to alkane [4, 780993], send to v0
- `--envelope` - Contract WASM file
- `--from p2tr:0` - Use UTXOs from p2tr:0
- `--inputs "B:10000000"` - Need 10M sats for deployment
- `--fee-rate 1.0` - 1 sat/vB fee rate
- `--mine` - Auto-mine block on regtest
- `--trace` - Show execution trace
- `-y` - Auto-confirm

**Result**:
- Output 0: p2tr:0 (receives alkane [4, 780993])
- Output 1: OP_RETURN (runestone)
- Output 2: p2wsh:0 (BTC change: ~9,989,000 sats)

---

### Example 2: Initialize Factory with Auth Token
```bash
PROTOSTONE="[2:1:1:p1]:v0:v0,[4,65522,0,780993,4,65523]:v0:v0"

alkanes execute "$PROTOSTONE" \
  --from p2tr:0 \
  --inputs "2:1:1,B:10000000" \
  --change p2tr:0 \
  --fee-rate 1.0 \
  --mine \
  --trace \
  -y
```
**Breakdown**:
- First protostone: `[2:1:1:p1]:v0:v0`
  - Send 1 unit of auth token [2,1] to **p1** (next protostone)
  - Refund remaining auth tokens to **v0**
  
- Second protostone: `[4,65522,0,780993,4,65523]:v0:v0`
  - Call factory.InitFactory() using auth token from p1
  - Send results to **v0**

- `--inputs "2:1:1,B:10000000"` - Need auth token [2,1] and 10M sats
- `--change p2tr:0` - All change (BTC and alkanes) to p2tr:0

**Result**:
- Output 0: p2tr:0 (receives auth tokens back + factory results)
- Output 1: OP_RETURN (runestone with two protostones)
- Output 2: p2tr:0 (BTC change)

---

## Troubleshooting

### Problem: "Insufficient funds"
**Cause**: Not enough BTC in selected UTXOs

**Solution**: 
1. Check balance: `alkanes-cli wallet balance`
2. Increase `--inputs` amount
3. Or omit `--from` to use all wallet UTXOs

---

### Problem: "Output v1 referenced but only 1 output exists"
**Cause**: Protostone references v1 but you only have v0

**Solution**: Add more addresses to `--to`:
```bash
--to p2tr:0,p2tr:1  # Now v0 and v1 exist
```

---

### Problem: "Bitcoin input requirement provided but no recipient addresses"
**Cause**: Specified `--inputs "B:amount"` but no outputs to receive it

**Solution**: Ensure your protostone references at least v0:
```bash
# Good: references v0
alkanes execute "[3,100]:v0:v0"

# Bad: no output reference
alkanes execute "[3,100]"  # Missing :v0:v0
```

---

## Best Practices

### 1. Always Specify --change
Even though it defaults to `p2wsh:0`, being explicit makes your intent clear:
```bash
--change p2tr:0  # Explicit is better than implicit
```

### 2. Use --trace for Debugging
See what's happening inside your protostones:
```bash
--trace  # Shows execution trace
```

### 3. Use -y for Scripts
Skip confirmation prompts in automated scripts:
```bash
-y  # Auto-confirm
```

### 4. Test on Regtest First
Always test complex transactions on regtest before mainnet:
```bash
alkanes-cli -p regtest ...
```

### 5. Check Balances Before and After
```bash
alkanes-cli wallet balance         # Before
alkanes-cli wallet utxos           # Check UTXOs
# ... run transaction ...
alkanes-cli wallet balance         # After
alkanes-cli wallet utxos           # Verify change received
```

---

## FAQ

### Q: Where do my alkanes go if I don't specify --to?
**A**: To the `--change` address if specified, otherwise `p2tr:0`

### Q: Where does BTC change go if I don't specify --change?
**A**: To `p2wsh:0` by default

### Q: Can I send alkanes to different addresses?
**A**: Yes! Use `--to` with multiple addresses:
```bash
--to p2tr:0,p2tr:1,p2wsh:5
```

### Q: What if I want ALL alkanes and BTC change to go to the same address?
**A**: Just use `--change`:
```bash
--change p2tr:5  # Everything goes here
```

### Q: Do I always need --inputs?
**A**: For `alkanes execute`, yes. It specifies what resources your transaction needs.

### Q: What's the difference between --from and --inputs?
**A**: 
- `--from`: Which wallet addresses to source UTXOs from
- `--inputs`: What resources (BTC amount, alkanes) the transaction requires

---

## Additional Resources

- **Detailed Scheme**: See `/docs/alkanes-execute-scheme.md`
- **Implementation Status**: See `/docs/alkanes-execute-implementation-status.md`
- **Examples**: See `/scripts/deploy-amm.sh`

---

## Getting Help

If you encounter issues:

1. **Check logs**: Use `--log-level debug` for detailed output
2. **Verify balances**: `alkanes-cli wallet balance`
3. **Check UTXOs**: `alkanes-cli wallet utxos`
4. **Validate transaction**: Use `--trace` to see execution
5. **Read error messages**: They now provide specific details about what's wrong

---

## Summary

**The Bottom Line**: The `alkanes execute` command is now much safer and more user-friendly. You don't need to manually create outputs for every identifier - it happens automatically. Just specify where you want your results and change via `--to` and `--change`, and the rest is handled for you.

**Safety First**: No more burned assets! 🔥❌
