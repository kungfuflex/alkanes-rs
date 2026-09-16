# Helper Scripts

This directory contains helper scripts for various Bitcoin transaction operations.

## Transaction Broadcasting Scripts

### `query-rebar.sh`

Query Rebar Shield API for current payment information and fee tiers.

**Usage:**
```bash
./scripts/query-rebar.sh
```

**Output:**
- Current block height
- Payment address
- Fee tier options with hashrate coverage
- Cost calculations for sample transaction

**Example:**
```
Payment Address: bc1qfelpskqcy3xmyrnhq4hz6y0rzk68ayn09juaek
Block Height: 920137
Fee Tier 1: 16 sat/vB @ 8% hashrate
Fee Tier 2: 28 sat/vB @ 16% hashrate
```

## Root Directory Scripts

The following scripts are located in the repository root for convenience:

### `cmd-build-tx-only.sh`

Build and sign a large consolidation transaction with automatic truncation.

**What it does:**
1. Builds unsigned transaction with `--send-all`
2. Signs with `--truncate-excess-vsize` (keeps under 1MB)
3. Saves to `signed_consolidation_tx.hex`
4. Shows transaction details

**Usage:**
```bash
./cmd-build-tx-only.sh
```

**Configuration:**
Edit the script to set your:
- Source address (`WALLET_ADDRESS`)
- Destination address (`DEST_ADDRESS`)
- Private key file path (`PRIVKEY_FILE`)
- Esplora URL (`ESPLORA_URL`)
- Fee rate (`FEE_RATE`)

### `cmd-broadcast-all-options.sh` ⭐

Interactive helper showing all broadcast methods with cost analysis.

**What it does:**
1. Analyzes transaction size
2. Shows all 5 broadcast options:
   - Standard Bitcoin Core RPC
   - Public RPC nodes
   - MARA Slipstream
   - Rebar Shield
   - Libre Relay (own node)
3. Displays cost estimates for each
4. Recommends best option
5. Interactive selection menu

**Usage:**
```bash
./cmd-broadcast-all-options.sh
```

**Example Output:**
```
Transaction Size: 980002 vbytes (~956 KB)

Available Broadcast Methods:

1. MARA Slipstream (RECOMMENDED)
   Status: ✅ VIABLE
   Est. cost: 2,058,004 sats

2. Rebar Shield
   Status: ⚠️  EXPENSIVE
   Est. cost T1: 15,680,032 sats

...

Enter option number (1-5) or 'q' to quit:
```

### `cmd-broadcast-libre.sh`

Attempt to broadcast via public Bitcoin RPC nodes.

**What it does:**
1. Tries multiple public endpoints
2. Automatic fallback on failure
3. Shows clear error messages
4. Recommends alternatives if all fail

**Endpoints tried:**
- `https://bitcoin-rpc.publicnode.com`
- `https://bitcoin.api.onfinality.io/public`
- `https://public-btc.nownodes.io`
- `https://bitcoin-mainnet.public.blastapi.io`

**Usage:**
```bash
./cmd-broadcast-libre.sh
```

**Note:** Public nodes enforce 100KB standard relay limit. For larger transactions, use Slipstream.

## Other Helper Scripts

### `test-external-signing.sh`

Test script for external signing workflow (if present).

### `build-consolidation-tx.sh`

Alternative transaction builder (if present).

### `enrich-address.sh`

Address enrichment script (if present).

## Quick Start Workflow

For large UTXO consolidation:

```bash
# 1. Build and sign transaction
./cmd-build-tx-only.sh

# 2. Choose broadcast method (interactive)
./cmd-broadcast-all-options.sh

# 3. Select option 1 (Slipstream) for best cost
```

Or manual workflow:

```bash
# 1. Build transaction
deezel -p mainnet \
  --wallet-address bc1q... \
  wallet send \
  --address bc1p... \
  --amount 10000 \
  --fee-rate 2.1 \
  --send-all \
  > unsigned_tx.hex

# 2. Sign with truncation
deezel wallet sign-tx \
  --wallet-key-file ./privkey.hex \
  --from-file unsigned_tx.hex \
  --truncate-excess-vsize \
  > signed_tx.hex

# 3. Broadcast via Slipstream
deezel -p mainnet bitcoind sendrawtransaction \
  --from-file signed_tx.hex \
  --use-slipstream
```

## Script Categories

### Transaction Building
- `cmd-build-tx-only.sh` - Build & sign with auto-truncation
- `build-consolidation-tx.sh` - Alternative builder

### Broadcasting
- `cmd-broadcast-all-options.sh` - Interactive helper ⭐
- `cmd-broadcast-libre.sh` - Public RPC nodes
- Via deezel CLI with `--use-slipstream` or `--use-rebar`

### Information & Testing
- `scripts/query-rebar.sh` - Query Rebar API
- `test-external-signing.sh` - Test workflow
- `enrich-address.sh` - Address utilities

## Environment Variables

Scripts may use the following environment variables:

- `WALLET_ADDRESS` - Source Bitcoin address
- `DEST_ADDRESS` - Destination address
- `PRIVKEY_FILE` - Path to private key file
- `ESPLORA_URL` - Esplora API endpoint
- `FEE_RATE` - Fee rate in sat/vB
- `PROVIDER` - Network (mainnet/testnet)

## Security Notes

**⚠️ Private Keys:**
- Never commit private key files to git
- Store keys securely (encrypted filesystem, HSM)
- Add `*.hex` to `.gitignore`
- Use environment variables instead of hardcoding

**⚠️ Transaction Review:**
- Always verify transaction with `decode-tx` before broadcasting
- Check destination address carefully
- Verify fee calculations
- Review UTXO selection

## Troubleshooting

### "Permission denied"
```bash
chmod +x ./scripts/*.sh
chmod +x ./*.sh
```

### "Command not found: deezel"
```bash
# Build first
cargo build --release

# Or use full path
./target/release/deezel ...
```

### "Argument list too long"
Use `--from-file` instead of passing hex as argument.

### Script fails to find file
Use absolute paths or run from repository root:
```bash
cd /path/to/alkanes-rs
./cmd-broadcast-all-options.sh
```

## Related Documentation

- [External Signing Workflow](../docs/features/external-signing.md)
- [Transaction Broadcasting Options](../docs/features/transaction-broadcasting.md)
- [Rebar Shield Integration](../docs/features/rebar-shield.md)
- [Main README](../README.md)
