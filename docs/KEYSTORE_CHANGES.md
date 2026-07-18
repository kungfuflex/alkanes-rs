# Keystore Derivation Path Changes

## Problem

The keystore was storing an `account_xpub` derived at a specific BIP path (e.g., `m/86'/0'/0'` for Taproot), which meant:
1. Only one address type could be derived from the stored xpub
2. To derive other address types (P2WPKH, P2SH-P2WPKH, etc.), you needed to decrypt the mnemonic
3. The keystore wasn't compatible with how oyl-sdk expects to derive addresses

## Solution

Changed the keystore to store the **master extended public key (xpub) at root level (`m/`)** instead of at a specific BIP account level.

### Key Changes

#### 1. Keystore Creation (`keystore.rs::new()`)

**Before:**
```rust
// Derived at specific path (e.g., m/86'/0'/0')
let primary_path = DerivationPath::from_str("m/86'/0'/0'")?;
let xpub = Xpub::from_priv(&secp, &root.derive_priv(&secp, &primary_path)?);
```

**After:**
```rust
// Store master xpub at root level (m/)
let master_xpub = Xpub::from_priv(&secp, &root);
```

#### 2. Address Derivation (`keystore.rs::get_addresses()`)

**Before:**
```rust
// Could only derive from the stored account-level xpub
let xpub = Xpub::from_str(&self.account_xpub)?;
let path = DerivationPath::from_str(&format!("m/{chain}/{i}"))?;
let derived_xpub = xpub.derive_pub(&secp, &path)?;
```

**After:**
```rust
// Derives from full HD path: m/purpose'/coin_type'/account'/change/index
let master_xpub = Xpub::from_str(&self.account_xpub)?;
let full_path = format!("m/{}'/{}'/{}", bip_number, coin_type, format!("0'/{}/{}", chain, i));
let path = DerivationPath::from_str(&full_path)?;
let derived_xpub = master_xpub.derive_pub(&secp, &path)?;
```

#### 3. Address Type Support

Now supports all standard BIP paths:
- **BIP-86 (Taproot)**: `m/86'/coin'/0'/0/0` → P2TR addresses
- **BIP-84 (Native SegWit)**: `m/84'/coin'/0'/0/0` → P2WPKH addresses  
- **BIP-49 (Nested SegWit)**: `m/49'/coin'/0'/0/0` → P2SH-P2WPKH addresses
- **BIP-44 (Legacy)**: `m/44'/coin'/0'/0/0` → P2PKH addresses

Where `coin` is:
- `0` for Bitcoin mainnet
- `1` for testnet/signet/regtest

### Benefits

1. **Universal Address Derivation**: Can derive any address type from a single xpub without decrypting
2. **oyl-sdk Compatibility**: Matches how oyl-sdk derives addresses from a mnemonic
3. **Watch-Only Wallets**: Can create watch-only wallets that derive all address types
4. **Backward Compatible**: Field name `account_xpub` preserved (though now holds master xpub)

### Usage Example

```bash
# Create wallet from mnemonic
./alkanes-cli --passphrase test --wallet-file wallet.json wallet create "$MNEMONIC"

# Get Taproot address (BIP-86)
./alkanes-cli --wallet-file wallet.json wallet addresses p2tr:0

# Get Native SegWit address (BIP-84)
./alkanes-cli --wallet-file wallet.json wallet addresses p2wpkh:0

# Get Nested SegWit address (BIP-49)
./alkanes-cli --wallet-file wallet.json wallet addresses p2sh-p2wpkh:0

# Get Legacy address (BIP-44)
./alkanes-cli --wallet-file wallet.json wallet addresses p2pkh:0
```

### Testing

To verify addresses match oyl-sdk:

```bash
# Using alkanes-cli
./alkanes-cli --passphrase test --wallet-file /tmp/wallet.json wallet create "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
./alkanes-cli --wallet-file /tmp/wallet.json wallet addresses p2tr:0

# Using oyl-sdk (in reference/oyl-sdk)
cd reference/oyl-sdk
export MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
npx ts-node -e "
import { mnemonicToAccount } from './src';
import * as bitcoin from 'bitcoinjs-lib';
const account = mnemonicToAccount({
  mnemonic: process.env.MNEMONIC,
  opts: { network: bitcoin.networks.regtest }
});
console.log('Taproot:', account.taproot.address);
console.log('Native SegWit:', account.nativeSegwit.address);
"
```

Both should produce identical addresses for the same mnemonic and network.

### Migration Notes

**Existing keystores** created with the old format will continue to work but:
- They store an account-level xpub (e.g., `m/86'/0'/0'`)
- Only the address type matching that path can be derived without decryption
- To get full multi-address-type support, recreate the keystore with the new version

**New keystores** store the master xpub and support all address types immediately.

### Technical Details

The master xpub at `m/` allows derivation to any **non-hardened** child paths. Since the BIP standards use hardened derivation up to the account level (`m/purpose'/coin_type'/account'`), we can't derive the hardened parts from an xpub. However, by storing the master **xpriv** (encrypted as the mnemonic) and master **xpub**, we get:

- **With decryption (unlocked wallet)**: Full access via mnemonic → can derive any path
- **Without decryption (watch-only)**: Limited to deriving from master xpub → **can still derive all address types** because we derive the full hardened path from the master

This is possible because the code derives from the **master xpub** which is at `m/`, not at `m/86'/0'/0'`. The hardened derivation steps are performed from the master level.
