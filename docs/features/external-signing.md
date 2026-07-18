# External Signing Workflow

This guide covers the external signing workflow for alkanes-cli, which allows you to work with wallets using address-only mode and sign transactions with an external private key file.

## Overview

The external signing workflow is useful when you need to:
- Work with a wallet address without storing keys in a keystore
- Sign transactions using a private key stored in a separate file
- Build large transactions that exceed CLI argument limits
- Consolidate large numbers of UTXOs

## Wallet Modes

### 1. Standard Mode (Keystore)
Normal operation with encrypted keystore:
```bash
deezel wallet --keystore ./wallet.json balance
```

### 2. Address-Only Mode
Read-only operations without keys:
```bash
deezel wallet --wallet-address bc1p... balance
```

### 3. External Key Mode
Operations with external private key file:
```bash
# View balance
deezel wallet --wallet-address bc1p... balance

# Sign transactions
deezel wallet --wallet-key-file ./privkey.hex sign-tx ...
```

## External Signing Workflow

### Step 1: Create Transaction

Build an unsigned transaction:

```bash
deezel -p mainnet \
  --wallet-address bc1px562ylsev9zvfeg4jh7y7hg4y935ygcqg67vjzcjupq574uxy4ws8kmg05 \
  wallet send \
  --address bc1pdestination... \
  --amount 10000000 \
  --fee-rate 2.1 \
  --send-all
```

Or use the helper script for large consolidations:
```bash
./cmd-build-tx-only.sh
```

### Step 2: Sign Transaction

Sign with external private key:

```bash
deezel wallet sign-tx \
  --wallet-key-file ./privkey.hex \
  --from-file unsigned_tx.hex \
  > signed_tx.hex
```

For large transactions that exceed 1MB, use auto-truncation:
```bash
deezel wallet sign-tx \
  --wallet-key-file ./privkey.hex \
  --from-file unsigned_tx.hex \
  --truncate-excess-vsize \
  > signed_tx.hex
```

### Step 3: Decode and Verify

Verify the transaction before broadcasting:

```bash
deezel wallet decode-tx --file signed_tx.hex
```

### Step 4: Broadcast

See [Transaction Broadcasting](./transaction-broadcasting.md) for broadcast options.

## Key Features

### Automatic UTXO Filtering

The external signing workflow automatically filters out UTXOs that contain:
- Inscriptions
- Runes
- Alkanes

This prevents accidental spending of valuable digital assets.

### Send All Mode

Use `--send-all` to consolidate all available UTXOs:
- Selects ALL clean UTXOs (not just enough for the target amount)
- Calculates output as: `output = total_input - fee`
- No change output is created

### Auto-Truncation for Large Transactions

When signing transactions that exceed 1MB (Bitcoin consensus limit), use `--truncate-excess-vsize`:

```bash
deezel wallet sign-tx \
  --wallet-key-file ./privkey.hex \
  --from-file unsigned_tx.hex \
  --truncate-excess-vsize
```

This will:
1. Detect the fee rate from the unsigned transaction
2. Calculate max inputs to stay under 1MB: `max_inputs = (980,000 - 53) / 107 = 9,345`
3. Truncate inputs to fit
4. Recalculate output with the preserved fee rate

### File-Based Operations

For large transactions that exceed CLI argument limits, use `--from-file`:

```bash
# Decode transaction from file
deezel wallet decode-tx --file tx.hex

# Sign transaction from file
deezel wallet sign-tx --from-file unsigned.hex --wallet-key-file key.hex

# Broadcast transaction from file
deezel bitcoind sendrawtransaction --from-file signed.hex
```

## Private Key Format

The private key file should contain the hex-encoded private key:

```bash
# Example format (32 bytes = 64 hex characters)
echo "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" > privkey.hex
```

**Security Warning:** 
- Never commit private key files to version control
- Store private keys securely (encrypted filesystem, hardware security module, etc.)
- Consider using `.gitignore` for `*.hex` files

## Helper Scripts

### Build and Sign Large Consolidation

```bash
./cmd-build-tx-only.sh
```

This script:
1. Builds unsigned transaction with `--send-all`
2. Signs with `--truncate-excess-vsize`
3. Saves to `signed_consolidation_tx.hex`
4. Shows transaction details

### Broadcast Options

```bash
./cmd-broadcast-all-options.sh
```

Interactive helper showing all broadcast methods with cost estimates.

## Example: Large UTXO Consolidation

Complete workflow for consolidating 9,000+ UTXOs:

```bash
# 1. Build unsigned transaction
deezel -p mainnet \
  --wallet-address bc1q79m7f72fxyrc0hd7sgly3jnsqvpcj349lae76d \
  --esplora-api-url http://localhost:50010 \
  wallet send \
  --address bc1px562ylsev9zvfeg4jh7y7hg4y935ygcqg67vjzcjupq574uxy4ws8kmg05 \
  --amount 10000 \
  --fee-rate 2.1 \
  --send-all \
  > unsigned_tx.hex

# 2. Sign with auto-truncation (keeps under 1MB)
deezel wallet sign-tx \
  --wallet-key-file ./privkey.hex \
  --from-file unsigned_tx.hex \
  --truncate-excess-vsize \
  > signed_tx.hex

# 3. Verify
deezel wallet decode-tx --file signed_tx.hex

# 4. Broadcast (see transaction-broadcasting.md)
./cmd-broadcast-all-options.sh
```

## Troubleshooting

### "Argument list too long"
Use `--from-file` instead of passing hex as argument:
```bash
deezel wallet sign-tx --from-file tx.hex --wallet-key-file key.hex
```

### "tx-size exceeds consensus limit"
Use `--truncate-excess-vsize` when signing:
```bash
deezel wallet sign-tx --truncate-excess-vsize --from-file tx.hex ...
```

### "Zero fee calculated"
This happens when `--send-all` doesn't calculate output correctly. The latest version fixes this by calculating: `output = input - fee`.

### UTXOs with inscriptions/runes selected
The automatic filtering should prevent this. If it happens:
1. Check UTXO metadata from your indexer
2. Manually exclude problematic UTXOs with `--from` flag
3. Report the issue (filtering logic may need updates)

## Related Documentation

- [Transaction Broadcasting Options](./transaction-broadcasting.md)
- [Rebar Shield Integration](./rebar-shield.md)
- [Main README](../../README.md)

## API Reference

### Command Line Flags

**Global flags:**
- `--wallet-address <ADDRESS>` - Use address-only mode
- `--wallet-key-file <FILE>` - Path to private key file for signing

**Send command:**
- `--send-all` - Send all available funds
- `--fee-rate <RATE>` - Fee rate in sat/vB
- `--from <ADDRESSES>` - Source addresses (comma-separated)
- `--change <ADDRESS>` - Change address

**Sign-tx command:**
- `--from-file <FILE>` - Read transaction from file
- `--truncate-excess-vsize` - Auto-truncate to stay under 1MB

**Decode-tx command:**
- `--file <FILE>` - Read transaction from file

**Sendrawtransaction command:**
- `--from-file <FILE>` - Read transaction from file
- `--use-slipstream` - Broadcast via MARA Slipstream
- `--use-rebar` - Broadcast via Rebar Shield

## See Also

- [Transaction Broadcasting](./transaction-broadcasting.md) - All broadcast methods
- [Rebar Shield](./rebar-shield.md) - Private relay with MEV protection
- [Scripts Documentation](../../scripts/README.md) - Helper scripts reference
