# BRC20-Prog CLI Guide

The `brc20-prog` subcommand provides tools for interacting with BRC20 programmable smart contracts using the BRC20-prog protocol.

## Overview

BRC20-prog enables EVM-compatible smart contracts on Bitcoin using the standard ord inscription format with JSON payloads. It provides three main operations:

1. **deploy-contract** - Deploy smart contracts from Foundry build artifacts
2. **transact** - Call functions on deployed contracts
3. **wrap-btc** - Wrap BTC to frBTC and execute in one transaction

## Installation

BRC20-prog commands are included in the main alkanes CLI:

```bash
cargo build --release -p alkanes-cli
```

## Commands

### Deploy Contract

Deploy a smart contract from a Foundry build artifact.

**Usage:**
```bash
alkanes brc20-prog deploy-contract <FOUNDRY_JSON_PATH> [OPTIONS]
```

**Arguments:**
- `<FOUNDRY_JSON_PATH>` - Path to Foundry build JSON file (e.g., `./out/MyContract.sol/MyContract.json`)

**Options:**
- `--from <ADDRESSES>...` - Source addresses for UTXOs
- `--change <ADDRESS>` - Change address  
- `--fee-rate <RATE>` - Fee rate in sat/vB (default: 600)
- `--raw` - Show raw JSON output
- `--trace` - Enable transaction tracing
- `--mine` - Mine block after broadcast (regtest only)
- `-y, --yes` - Auto-confirm without prompt

**Example:**
```bash
# Deploy a token contract
alkanes brc20-prog deploy-contract ./out/MyToken.sol/MyToken.json \
  --from "p2tr:0" \
  --change "p2tr:0" \
  --fee-rate 600 \
  --mine \
  -y
```

**How it works:**
1. Parses Foundry JSON and extracts bytecode
2. Creates JSON inscription: `{"p":"brc20-prog","op":"deploy","d":"0x..."}`
3. Uses ord inscription format with `text/plain` content-type
4. Commit-reveal pattern with taproot script-path spending
5. Reveals to `OP_RETURN "BRC20PROG"` for indexer recognition

**Output:**
```
Commit TXID: abc123...
Reveal TXID: def456...
Contract Address: 0x1234567890abcdef...
```

---

### Transact

Call a function on a deployed BRC20-prog contract.

**Usage:**
```bash
alkanes brc20-prog transact \
  --address <CONTRACT_ADDRESS> \
  --signature <FUNCTION_SIGNATURE> \
  --calldata <ARGUMENTS> \
  [OPTIONS]
```

**Required Options:**
- `--address <ADDRESS>` - Contract address (0x-prefixed hex)
- `--signature <SIG>` - Function signature (e.g., "transfer(address,uint256)")
- `--calldata <ARGS>` - Comma-separated argument values

**Optional Options:**
- `--from <ADDRESSES>...` - Source addresses for UTXOs
- `--change <ADDRESS>` - Change address
- `--fee-rate <RATE>` - Fee rate in sat/vB
- `--raw` - Show raw JSON output
- `--trace` - Enable transaction tracing
- `--mine` - Mine block after broadcast (regtest)
- `-y, --yes` - Auto-confirm without prompt

**Examples:**

Transfer tokens:
```bash
alkanes brc20-prog transact \
  --address 0x1234567890abcdef1234567890abcdef12345678 \
  --signature "transfer(address,uint256)" \
  --calldata "0x9876543210fedcba9876543210fedcba98765432,1000000000000000000" \
  --from "p2tr:0" \
  --mine -y
```

Approve spending:
```bash
alkanes brc20-prog transact \
  --address 0x1234567890abcdef1234567890abcdef12345678 \
  --signature "approve(address,uint256)" \
  --calldata "0x9876543210fedcba9876543210fedcba98765432,115792089237316195423570985008687907853269984665640564039457584007913129639935" \
  --from "p2tr:0" \
  -y
```

Call with no arguments:
```bash
alkanes brc20-prog transact \
  --address 0x1234567890abcdef1234567890abcdef12345678 \
  --signature "claim()" \
  --calldata "" \
  --from "p2tr:0" \
  -y
```

**Supported Argument Types:**
- **address**: `0x` + 40 hex chars (e.g., `0x1234567890abcdef1234567890abcdef12345678`)
- **uint256**: Decimal number (e.g., `1000`, `1000000000000000000`)
- **bool**: `true` or `false`
- **bytes**: `0x`-prefixed hex data

**How it works:**
1. Encodes function call: selector (keccak256 first 4 bytes) + ABI-encoded args
2. Creates JSON inscription: `{"p":"brc20-prog","op":"call","c":"0x...","d":"0x..."}`
3. Commit-reveal with ord inscription format
4. Reveals to `OP_RETURN "BRC20PROG"`

---

### Wrap-BTC

Wrap BTC to frBTC and execute a target contract in one transaction.

**Usage:**
```bash
alkanes brc20-prog wrap-btc <AMOUNT> \
  --target <TARGET_CONTRACT> \
  --signature <FUNCTION_SIGNATURE> \
  --calldata <ARGUMENTS> \
  [OPTIONS]
```

**Arguments:**
- `<AMOUNT>` - Amount of BTC to wrap (in satoshis)

**Required Options:**
- `--target <ADDRESS>` - Target contract for wrapAndExecute2
- `--signature <SIG>` - Function signature to call on target
- `--calldata <ARGS>` - Comma-separated arguments

**Optional Options:**
- `--from <ADDRESSES>...` - Source addresses for UTXOs
- `--change <ADDRESS>` - Change address
- `--fee-rate <RATE>` - Fee rate in sat/vB
- `--raw` - Show raw JSON output
- `--trace` - Enable transaction tracing
- `--mine` - Mine block after broadcast (regtest)
- `-y, --yes` - Auto-confirm without prompt

**Examples:**

Wrap and deposit to liquidity pool:
```bash
alkanes brc20-prog wrap-btc 100000 \
  --target 0xABCDEF1234567890ABCDEF1234567890ABCDEF12 \
  --signature "deposit()" \
  --calldata "" \
  --from "p2tr:0" \
  --mine -y
```

Wrap and stake:
```bash
alkanes brc20-prog wrap-btc 50000 \
  --target 0xABCDEF1234567890ABCDEF1234567890ABCDEF12 \
  --signature "stake(uint256,address)" \
  --calldata "50000,0x1234567890abcdef1234567890abcdef12345678" \
  --from "p2tr:0" \
  -y
```

**How it works:**
1. Calls `FrBTC.wrapAndExecute2(target, data)` on frBTC contract
2. FrBTC mints tokens based on BTC sent to subfrost signer
3. Approves target contract with minted amount
4. Calls `target.execute(msg.sender, amount, data)`
5. Returns leftover frBTC to user

**Note:** Currently requires updating `FRBTC_CONTRACT_ADDRESS` constant with deployed contract address.

---

## Inscription Format

BRC20-prog uses the standard ord inscription format:

### Deploy Inscription
```json
{
  "p": "brc20-prog",
  "op": "deploy",
  "d": "0x<bytecode-hex>"
}
```

### Call Inscription
```json
{
  "p": "brc20-prog",
  "op": "call",
  "c": "0x<contract-address>",
  "d": "0x<calldata-hex>"
}
```

### Envelope Structure
```
OP_FALSE OP_IF
  "ord"                         # Protocol ID
  0x01                          # Content-Type tag
  "text/plain;charset=utf-8"    # MIME type
  0x00                          # Body tag
  <JSON in ≤520 byte chunks>    # Payload
OP_ENDIF
```

## Commit-Reveal Pattern

All BRC20-prog operations use a two-transaction commit-reveal pattern:

**Commit Transaction:**
- Creates P2TR output with taproot script containing reveal script
- Funds output with enough sats for reveal transaction
- Uses key-path spending for funding inputs

**Reveal Transaction:**
- Spends commit output via script-path spending
- Witness: `[signature, script, control_block]`
- Outputs to `OP_RETURN "BRC20PROG"` for indexer
- Returns change to specified address

## Integration with BRC20-Prog Indexer

For full functionality, the BRC20-prog indexer must be running and processing inscriptions. See [BRC20-Prog Module](https://github.com/bestinslot-xyz/brc20-programmable-module).

The indexer recognizes inscriptions by:
1. Detecting `OP_RETURN "BRC20PROG"` in transaction
2. Parsing inscription from witness data
3. Extracting JSON payload
4. Calling corresponding RPC method (`brc20_deploy`, `brc20_call`, etc.)

## Error Handling

Common errors and solutions:

**"Insufficient funds"**
- Check wallet balance: `alkanes wallet balance`
- Verify UTXO availability: `alkanes wallet utxos`

**"Invalid function signature"**
- Use exact Solidity signature: `transfer(address,uint256)` not `transfer(address, uint256)`
- No spaces in signature

**"Failed to encode arguments"**
- Verify argument types match signature
- Use 0x prefix for addresses and hex data
- Use decimal numbers for uint256

**"Contract not found"**
- Ensure contract was deployed successfully
- Verify contract address is correct
- Check indexer is synced

## See Also

- [Wrap-BTC Guide](../features/wrap-btc.md) - Detailed wrap-btc documentation
- [Smart Contracts](../features/smart-contracts.md) - Contract development guide
- [RPC API](../dev/rpc-api.md) - Indexer RPC reference
- [Examples](../examples/deploy.md) - More usage examples
