# Address Identifiers in alkanes-cli

## Overview

The alkanes-cli now supports address identifiers that allow you to specify addresses using a shorthand notation instead of full Bitcoin addresses. This makes it easier to work with HD wallets and derive addresses on-the-fly.

## Supported Address Types

The following address types are supported:

- `p2pk` - Pay to Public Key (Legacy, rarely used)
- `p2pkh` - Pay to Public Key Hash (Legacy, addresses starting with "1")
- `p2sh` - Pay to Script Hash (addresses starting with "3")
- `p2wpkh` - Pay to Witness Public Key Hash (Native SegWit, "bc1q")
- `p2wsh` - Pay to Witness Script Hash (Native SegWit)
- `p2sh-p2wpkh` - Nested SegWit (P2SH-wrapped P2WPKH, BIP49)
- `p2sh-p2wsh` - Nested SegWit (P2SH-wrapped P2WSH)
- `p2tr` - Pay to Taproot (addresses starting with "bc1p", BIP86)

## Identifier Syntax

### Single Address
```bash
# Derive address at index 0 for the given type
p2tr:0
p2wpkh:5
p2pkh:10
```

### Address Ranges
```bash
# Derive addresses from index 0 to 50 (exclusive)
p2tr:0-50

# Derive addresses from index 20 to 150 (exclusive)
p2wpkh:20-150

# Any range is supported
p2sh-p2wpkh:100-200
```

## Commands Supporting Address Identifiers

Address identifiers can be used with the following commands:

### View-Only Commands (no wallet required)
- `alkanes-cli bitcoind generatetoaddress`
- `alkanes-cli esplora address`
- `alkanes-cli esplora address-txs`
- `alkanes-cli esplora address-txs-chain`
- `alkanes-cli esplora address-txs-mempool`
- `alkanes-cli esplora address-utxo`
- `alkanes-cli esplora address-prefix`
- `alkanes-cli ord address-info`
- `alkanes-cli protorunes by-address`
- `alkanes-cli alkanes spendables`
- `alkanes-cli alkanes getbalance`

### Signing Commands (require wallet with private key)
- `alkanes-cli alkanes execute`
- `alkanes-cli alkanes wrap-btc`
- `alkanes-cli brc20-prog deploy-contract`
- `alkanes-cli brc20-prog transact`
- `alkanes-cli brc20-prog wrap-btc`
- `alkanes-cli wallet sign`
- `alkanes-cli wallet send`
- `alkanes-cli wallet create-tx`
- `alkanes-cli wallet sign-tx`
- `alkanes-cli wallet addresses`

## Wallet Modes

The address identifier system works differently depending on how the wallet is configured:

### With `--wallet-file` (HD Wallet)

When using `--wallet-file`, the keystore contains the master extended public key (xpub) which allows derivation of addresses for all supported types using standard BIP paths:

- **BIP44** (`m/44'/coin'/0'/0/index`) - P2PK, P2PKH
- **BIP49** (`m/49'/coin'/0'/0/index`) - P2SH, P2SH-P2WPKH, P2SH-P2WSH
- **BIP84** (`m/84'/coin'/0'/0/index`) - P2WPKH, P2WSH
- **BIP86** (`m/86'/coin'/0'/0/index`) - P2TR

```bash
# Derive Taproot addresses 0-10
alkanes-cli --wallet-file wallet.json wallet addresses p2tr:0-10

# Get balance for Native SegWit addresses 0-5
alkanes-cli --wallet-file wallet.json alkanes getbalance --address p2wpkh:0-5
```

### With `--wallet-address` (Single Address)

When using `--wallet-address`, you can still use identifiers, but since there's only one address, you would typically just use index 0:

```bash
alkanes-cli --wallet-address bc1p... alkanes getbalance
```

### With `--wallet-private-key` or `--wallet-private-key-file` (Single Key)

When using a single private key, derivation is limited to index 0 for each address type since there's no HD wallet:

```bash
alkanes-cli --wallet-private-key <key> wallet send --address <dest> --amount 0.001 --from p2tr:0
```

## Usage Examples

### View balances for a range of addresses
```bash
alkanes-cli --wallet-file wallet.json alkanes getbalance --address p2tr:0-10
```

### Send from specific address types
```bash
alkanes-cli --wallet-file wallet.json --passphrase test wallet send \
  --address bc1q... \
  --amount 0.001 \
  --from p2wpkh:5
```

### Execute alkanes transaction with range of UTXOs
```bash
alkanes-cli --wallet-file wallet.json --passphrase test alkanes execute \
  --to p2tr:0 \
  --from p2tr:0-10 \
  --change p2tr:1 \
  <protostone-specs>
```

### Deploy BRC20-Prog contract using nested SegWit
```bash
alkanes-cli --wallet-file wallet.json --passphrase test brc20-prog deploy-contract \
  foundry_output.json \
  --from p2sh-p2wpkh:0-5 \
  --change p2sh-p2wpkh:10
```

### Generate to an address in regtest
```bash
alkanes-cli --provider regtest bitcoind generatetoaddress 10 p2tr:0
```

## Implementation Details

### Address Resolution

The `resolve_all_identifiers` function in `address_resolver.rs` handles parsing and resolving address identifiers:

1. Parses the identifier format (e.g., `p2tr:0-10`)
2. Extracts the address type, start index, and count
3. Calls the provider's `get_address` method for each index in the range
4. Returns comma-separated addresses when multiple addresses are resolved

### HD Path Derivation

The keystore uses the master extended public key stored in the wallet file to derive addresses using standard BIP paths. The derivation happens at the provider level, which:

1. Loads the master xpub from the keystore
2. Derives the account-level xpub for the specific BIP path (44'/49'/84'/86')
3. Derives the final address at the requested index

### Private Key Mode

When using `--wallet-private-key`, the system derives the address directly from the private key for the requested address type, limited to index 0.

## Security Considerations

- **View-Only**: Commands that don't require signing can work without the wallet passphrase
- **Signing**: Commands that create transactions require either:
  - `--wallet-file` with `--passphrase`
  - `--wallet-private-key` or `--wallet-private-key-file`
- The master xpub is stored in the wallet file but cannot be used to derive private keys
- Private keys are only accessed when signing transactions

## Backward Compatibility

All commands continue to accept full Bitcoin addresses. Address identifiers are opt-in:

```bash
# Using identifiers
alkanes-cli alkanes getbalance --address p2tr:0

# Using full address (still works)
alkanes-cli alkanes getbalance --address bc1p...
```
