# Wrap-BTC Feature

Wrap-BTC enables wrapping Bitcoin into frBTC, a synthetic Bitcoin token that can be used in DeFi applications across both alkanes and BRC20-prog protocols.

## Overview

The wrap-btc feature provides two implementations:

1. **Alkanes wrap-btc** - Pure alkanes protocol (BTC → frBTC → vault lock)
2. **BRC20-prog wrap-btc** - Dual protocol with EVM contract execution

Both create frBTC by sending BTC to a special subfrost signer address, which verifies the transaction and mints equivalent frBTC tokens.

## Alkanes Wrap-BTC

### Quick Start

```bash
# Wrap 0.001 BTC (100,000 sats)
alkanes alkanes wrap-btc 100000 \
  --from "p2tr:0" \
  --change "p2tr:0" \
  --mine -y
```

### How It Works

The alkanes wrap creates a **two-protostone transaction**:

```
┌─────────────────────────────────────────┐
│ Transaction                             │
│                                         │
│ Inputs:                                 │
│  [0] User UTXO (≥100,000 sats)         │
│                                         │
│ Outputs:                                │
│  [0] 100,000 sats → Subfrost P2TR     │
│  [1] OP_RETURN (protostones)           │
│  [2] Change                             │
└─────────────────────────────────────────┘

Protostones in OP_RETURN:

[Protostone 1: frBTC Wrap]
├─ Target: {32, 0} (frBTC alkane)
├─ Opcode: 77 (Wrap)
├─ Pointer: 3 (→ Protostone 2)
└─ Effect: Mints ~99,900 frBTC (after 0.1% fee)

[Protostone 2: Vault Lock]
├─ Target: {4, 3032615708} (BRC20 vault)
├─ Opcode: 1 (Lock)
├─ Receives: frBTC from Protostone 1
└─ Effect: Locks frBTC in vault
```

### Subfrost Signer Address

The subfrost signer address is dynamically fetched:

1. **Query frBTC alkane** - Call opcode 103 (GetSigner) on {32, 0}
2. **Receive x-only pubkey** - 32-byte secp256k1 public key
3. **Apply taproot tweak** - `tweaked = pubkey + H_taptweak(pubkey, None) * G`
4. **Create P2TR address** - bc1p... address from tweaked key

This ensures the wrap always sends to the correct current signer.

### Premium Fee

A configurable premium is deducted from wrapped amounts:

```
mint_amount = btc_sent - (btc_sent * premium / 100_000_000)
```

Default: 100,000 (0.1%)

**Example:**
- Send: 100,000 sats
- Premium: 100 sats (0.1%)
- Receive: 99,900 frBTC

### Command Options

```bash
alkanes alkanes wrap-btc <AMOUNT> [OPTIONS]
```

**Arguments:**
- `<AMOUNT>` - Amount in satoshis to wrap

**Options:**
- `--from <ADDRESSES>...` - Source addresses for UTXOs
- `--change <ADDRESS>` - Change address
- `--fee-rate <RATE>` - Fee rate in sat/vB (default: 600)
- `--raw` - Show raw JSON output
- `--trace` - Enable transaction tracing
- `--mine` - Mine block after broadcast (regtest)
- `-y, --yes` - Auto-confirm without prompt

### Examples

**Basic wrap:**
```bash
alkanes alkanes wrap-btc 50000 \
  --from "p2tr:0" \
  --mine -y
```

**With specific change address:**
```bash
alkanes alkanes wrap-btc 100000 \
  --from "p2tr:0" \
  --from "p2tr:1" \
  --change "bc1p..." \
  --fee-rate 1000 \
  -y
```

**With tracing:**
```bash
alkanes alkanes wrap-btc 75000 \
  --from "p2tr:0" \
  --trace \
  --mine -y
```

---

## BRC20-Prog Wrap-BTC

### Quick Start

```bash
# Wrap and deposit to a pool
alkanes brc20-prog wrap-btc 100000 \
  --target 0x1234567890abcdef1234567890abcdef12345678 \
  --signature "deposit()" \
  --calldata "" \
  --from "p2tr:0" \
  --mine -y
```

### How It Works

BRC20-prog wrap creates frBTC in the EVM environment and executes a target contract:

```
┌────────────────────────────────────────┐
│ BRC20-Prog Inscription                 │
│ (Commit-Reveal Pattern)                │
│                                        │
│ JSON: {                                │
│   "p": "brc20-prog",                   │
│   "op": "call",                        │
│   "c": "0x<frBTC-contract>",           │
│   "d": "0x<wrapAndExecute2-calldata>"  │
│ }                                      │
└────────────────────────────────────────┘
                  ↓
         FrBTC.wrapAndExecute2()
                  ↓
    ┌─────────────────────────────┐
    │ 1. Verify BTC sent          │
    │ 2. Mint frBTC to contract   │
    │ 3. Approve target contract  │
    │ 4. Call target.execute()    │
    │ 5. Return leftover to user  │
    └─────────────────────────────┘
```

### wrapAndExecute2 Function

From `FrBTC.sol`:

```solidity
function wrapAndExecute2(address target, bytes memory data) public {
    bytes32 txid = _getTxid();
    require(!usedTxids[txid], "txid already used");
    
    // Get BTC sent to signer
    (, , , , , bytes[] memory voutScriptPubKeys, uint256[] memory voutValues) = 
        _getTxDetails(txid);
    
    bytes memory p2trScript = signer;
    uint256 totalAmount = 0;
    for (uint256 i = 0; i < voutScriptPubKeys.length; i++) {
        if (keccak256(voutScriptPubKeys[i]) == keccak256(p2trScript)) {
            totalAmount += voutValues[i];
        }
    }
    
    // Mint frBTC
    uint256 fee = (totalAmount * premium) / 1e8;
    uint256 mintAmount = totalAmount - fee;
    _mint(address(this), mintAmount);
    
    // Approve and execute
    SafeERC20.forceApprove(IERC20(address(this)), target, mintAmount);
    try IScript2(target).execute(msg.sender, mintAmount, data) {}
    catch {
        // Return frBTC on failure
        SafeERC20.safeTransfer(IERC20(address(this)), msg.sender, balanceOf(address(this)));
        return;
    }
    
    // Return remaining frBTC
    SafeERC20.safeTransfer(IERC20(address(this)), msg.sender, balanceOf(address(this)));
}
```

### Command Options

```bash
alkanes brc20-prog wrap-btc <AMOUNT> \
  --target <CONTRACT> \
  --signature <FUNCTION> \
  --calldata <ARGS> \
  [OPTIONS]
```

**Arguments:**
- `<AMOUNT>` - Amount in satoshis to wrap

**Required Options:**
- `--target <ADDRESS>` - Target contract for execution
- `--signature <SIG>` - Function signature (e.g., "deposit()")
- `--calldata <ARGS>` - Comma-separated arguments

**Optional Options:**
- `--from <ADDRESSES>...` - Source addresses
- `--change <ADDRESS>` - Change address
- `--fee-rate <RATE>` - Fee rate in sat/vB
- `--raw` - Raw JSON output
- `--trace` - Enable tracing
- `--mine` - Mine block (regtest)
- `-y, --yes` - Auto-confirm

### Examples

**Wrap and deposit:**
```bash
alkanes brc20-prog wrap-btc 100000 \
  --target 0xABCD1234... \
  --signature "deposit()" \
  --calldata "" \
  --from "p2tr:0" \
  -y
```

**Wrap and stake with parameters:**
```bash
alkanes brc20-prog wrap-btc 50000 \
  --target 0xABCD1234... \
  --signature "stake(uint256,address)" \
  --calldata "50000,0x9876..." \
  --from "p2tr:0" \
  -y
```

**Wrap and add liquidity:**
```bash
alkanes brc20-prog wrap-btc 200000 \
  --target 0xABCD1234... \
  --signature "addLiquidity(uint256,uint256)" \
  --calldata "200000,1000000" \
  --from "p2tr:0" \
  --mine -y
```

---

## Comparison

| Feature | Alkanes | BRC20-Prog |
|---------|---------|------------|
| Protocol | Pure alkanes | EVM + alkanes |
| Minted frBTC | Locked in vault | Contract execution |
| Composability | Limited | Full EVM compatibility |
| Use Case | Long-term holding | DeFi operations |
| Transaction | Single tx | Commit-reveal |
| Fee | ~300 vbytes | ~500 vbytes |

---

## Security

### Alkanes Wrap

1. **Txid Uniqueness** - Each txid can only wrap once
2. **Signer Verification** - Only BTC to subfrost address counts
3. **Pointer Validation** - Minted frBTC goes to correct destination
4. **Vault Lock** - frBTC secured in vault prevents unauthorized moves

### BRC20-Prog Wrap

1. **Double-Spend Protection** - Txid checked by contract
2. **Approval Safety** - frBTC only approved to specified target
3. **Execution Revert** - Failed execution returns frBTC to user
4. **Amount Verification** - BTC amount independently verified

---

## Current Limitations

### BRC20-Prog Implementation

⚠️ **Note:** The BRC20-prog wrap-btc currently only creates the brc20-prog inscription side. Full dual-protocol support (simultaneous alkanes + brc20-prog) requires additional development.

**Workaround:** Execute alkanes wrap and brc20-prog operations separately:
```bash
# 1. Wrap via alkanes
alkanes alkanes wrap-btc 100000 --from "p2tr:0" -y

# 2. Use frBTC in brc20-prog
alkanes brc20-prog transact \
  --address <frBTC-contract> \
  --signature "transfer(address,uint256)" \
  --calldata "..." \
  -y
```

### Contract Deployment

The `FRBTC_CONTRACT_ADDRESS` constant needs to be updated with the deployed contract address:

```rust
// In: alkanes-cli-common/src/brc20_prog/wrap_btc.rs
pub const FRBTC_CONTRACT_ADDRESS: &str = "0x<actual-deployed-address>";
```

---

## Troubleshooting

### "Insufficient funds"
- Check balance: `alkanes wallet balance`
- Verify UTXO availability: `alkanes wallet utxos`
- Ensure enough for wrap amount + fees

### "Invalid subfrost address"
- Ensure alkanes indexer is running
- Verify frBTC alkane {32,0} is indexed
- Check opcode 103 returns valid pubkey

### "Transaction already processed"
- Each txid can only wrap once (replay protection)
- Use fresh UTXOs for new wraps
- Don't rebroadcast same transaction

### BRC20-Prog "Contract not found"
- Update FRBTC_CONTRACT_ADDRESS constant
- Ensure FrBTC contract is deployed
- Verify brc20-prog indexer is synced

### "Pointer validation failed"
- Internal error in protostone pointer calculation
- Report as bug with transaction details

---

## Advanced Usage

### Batch Wrapping

Wrap multiple amounts by using multiple UTXOs:

```bash
alkanes alkanes wrap-btc 500000 \
  --from "p2tr:0" \
  --from "p2tr:1" \
  --from "p2tr:2" \
  --change "p2tr:0" \
  -y
```

### Custom Fee Rates

Adjust fee rate based on network conditions:

```bash
# High priority
alkanes alkanes wrap-btc 100000 \
  --fee-rate 2000 \
  -y

# Low priority
alkanes alkanes wrap-btc 100000 \
  --fee-rate 300 \
  -y
```

### Trace Execution

Enable tracing to see detailed execution:

```bash
alkanes alkanes wrap-btc 100000 \
  --trace \
  --mine -y
```

---

## See Also

- [BRC20-Prog CLI](../cli/brc20-prog.md) - BRC20-prog command reference
- [Alkanes CLI](../cli/alkanes.md) - Alkanes command reference
- [Smart Contracts](./smart-contracts.md) - Contract development
- [Examples](../examples/defi.md) - DeFi integration examples
