# BRC20-Prog Deploy Address Issue - Solution

## Problem

When deploying a BRC20-Prog contract, the inscription is sent to an address that doesn't appear in the standard `p2tr:0-10` list, making it difficult to find where the contract deployment UTXO ended up.

**Example Issue:**
- Contract deployed to: `tb1pk0ptvrlw6yv5mld3k6tn6aq8khrzgp22rzwwt8ly8nen4zvm3qcqmzkflv`
- This address is NOT in `p2tr:0-10` range shown by `wallet addresses p2tr:0-10`

## Root Cause

When running `brc20-prog deploy` without specifying a `--change` address, the reveal transaction sends the inscription output to the **default wallet address** returned by `WalletProvider::get_address()`.

### Code Flow

1. **In `brc20_prog/execute.rs` (lines 177-193):**
```rust
// Get change address
let change_address_str = if let Some(ref addr) = params.change_address {
    addr.clone()  // Use --change if provided
} else {
    WalletProvider::get_address(self.provider).await?  // ← Uses default
};

// First output: inscription at first sat (of 546 sat utxo)
let inscription_output = TxOut {
    value: bitcoin::Amount::from_sat(546),
    script_pubkey: change_address.script_pubkey(),
};
```

2. **In `provider.rs` (line 895+):**
```rust
async fn get_address(&self) -> Result<String> {
    match &self.wallet_state {
        WalletState::AddressOnly { address, .. } => Ok(address.clone()),
        WalletState::ExternalKey { address, .. } => Ok(address.clone()),
        _ => {
            let addresses = self.get_addresses(1).await?;  // Gets first address
            addresses.first()...
        }
    }
}
```

3. **`get_addresses()` returns the first taproot address at index 0:**
   - Path: `m/86'/1'/0'/0/0` (for signet)
   - This SHOULD be `p2tr:0`

## Why the Address Differs

The mystery address `tb1pk0ptvrlw6yv5mld3k6tn6aq8khrzgp22rzwwt8ly8nen4zvm3qcqmzkflv` is likely created in ONE of these ways:

### 1. **Commit Transaction Taproot Address (MOST LIKELY)**

The commit transaction uses a **taproot address with the reveal script embedded**, not a standard p2tr address!

**In `execute.rs` line 395-416:**
```rust
async fn create_commit_address_for_envelope(
    &self,
    envelope: &Brc20ProgEnvelope,
    internal_key: XOnlyPublicKey,
) -> Result<Address> {
    use bitcoin::taproot::TaprootBuilder;
    
    let reveal_script = envelope.build_reveal_script();
    let network = self.provider.get_network();
    
    let taproot_builder = TaprootBuilder::new()
        .add_leaf(0, reveal_script)
        .map_err(|e| AlkanesError::Other(format!("{e:?}")))?;
    
    let taproot_spend_info = taproot_builder
        .finalize(self.provider.secp(), internal_key)
        .map_err(|e| AlkanesError::Other(format!("{e:?}")))?;
    
    let commit_address = Address::p2tr_tweaked(taproot_spend_info.output_key(), network);
    
    Ok(commit_address)
}
```

**This creates a UNIQUE taproot address for each inscription because:**
- The reveal script is embedded in the taproot tree
- Different script = different taproot tweak = different address
- This address is NOT derivable from your wallet's standard derivation path!

### 2. **The Commit UTXO is Then Spent to Reveal**

The reveal transaction:
- **Input**: Spends the commit UTXO (the mystery address)
- **Output 1**: Inscription sent to `change_address` (546 sats)
- **Output 2**: Change sent to `change_address`

So the inscription actually ends up at your **change address** (if specified) or **p2tr:0** (if not specified).

## Solution

To find your deployed contract UTXO:

### Option 1: Check the Reveal Transaction Outputs

```bash
# Get the reveal txid from the deployment output
REVEAL_TXID="<your_reveal_txid>"

# Check the transaction
./alkanes-cli -p signet bitcoind getrawtransaction $REVEAL_TXID --raw

# Or use explorer
https://mempool.space/signet/tx/$REVEAL_TXID
```

The inscription should be in **Output 0** of the reveal transaction.

### Option 2: Specify --change Explicitly

When deploying, ALWAYS specify a `--change` address:

```bash
./alkanes-cli -p signet \
    --wallet-file "$WALLET_FILE" \
    brc20-prog deploy-contract \
    --foundry-json-path contract.json \
    --change p2tr:0 \
    --from p2tr:0 \
    --yes
```

This ensures the inscription goes to a known address.

### Option 3: Check Default Wallet Address

Find out what address was used:

```bash
# Get default address
./alkanes-cli -p signet --wallet-file "$WALLET_FILE" wallet info

# Check UTXOs at that address
./alkanes-cli -p signet --wallet-file "$WALLET_FILE" wallet utxos
```

### Option 4: Search All Wallet Addresses

If you still can't find it, check a wider range:

```bash
./alkanes-cli -p signet \
    --wallet-file "$WALLET_FILE" \
    wallet addresses \
    p2tr:0-100 \
    p2wpkh:0-100
```

### Option 5: Use Transaction Tracing

Check the commit and reveal transactions:

```bash
# If you have the commit txid
COMMIT_TXID="<commit_txid>"

# Get commit transaction
./alkanes-cli -p signet bitcoind getrawtransaction $COMMIT_TXID

# Find the reveal transaction that spends it
# Look for the output address in reveal tx
```

## Recommended Workflow

### For Deployment

Always use explicit addresses:

```bash
# Deploy with explicit addresses
./alkanes-cli -p signet \
    --wallet-file "$WALLET_FILE" \
    brc20-prog deploy-contract \
    --foundry-json-path FrBTC.json \
    --from p2tr:0 \
    --change p2tr:0 \
    --fee-rate 10 \
    --yes
```

### For Finding Existing Deployments

1. Check deployment output for REVEAL_TXID
2. Look at reveal transaction outputs
3. The inscription is at output 0 (546 sats)
4. Note the address and import to your wallet

### For Interacting with Contract

Once you find the inscription UTXO:

```bash
# Use it in subsequent calls
./alkanes-cli -p signet \
    --wallet-file "$WALLET_FILE" \
    brc20-prog transact \
    --address 0x<contract_address> \
    --signature "setSignerPubkey(bytes32)" \
    --calldata "0x1234..." \
    --from p2tr:0 \
    --change p2tr:0 \
    --yes
```

## Computing Contract Address

For BRC20-Prog, the contract address is computed as:

```rust
// Deployer address = keccak256(reveal_output.script_pubkey)[12:]
// Contract address = keccak256(rlp([deployer_address, nonce]))[12:]
```

You can use the helper function:

```rust
use alkanes_cli_common::brc20_prog::compute_contract_address;

let contract_addr = compute_contract_address(deployer_eth_addr, 0)?;
```

Or via CLI (future enhancement needed).

## Key Takeaways

1. ✅ **Always use `--change p2tr:0`** when deploying
2. ✅ **Always use `--from p2tr:0`** for consistency
3. ✅ **Save the REVEAL_TXID** from deployment output
4. ✅ The commit address is EPHEMERAL and NOT in your wallet
5. ✅ The inscription ends up at the reveal transaction's first output
6. ✅ Check `wallet utxos` to see all your UTXOs including inscriptions

## Code Fix Recommendation

Add a warning when `--change` is not specified:

```rust
// In brc20_prog/execute.rs
if params.change_address.is_none() {
    log::warn!("⚠️  No --change address specified. Inscription will be sent to default wallet address.");
    log::warn!("   For better control, use: --change p2tr:0");
}
```

Or make it required:

```rust
// In commands.rs
#[arg(long, required = true)]
pub change: String,
```
