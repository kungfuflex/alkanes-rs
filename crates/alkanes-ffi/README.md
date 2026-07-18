# Alkanes FFI

Foreign Function Interface (FFI) bindings for alkanes-rs using [UniFFI](https://mozilla.github.io/uniffi-rs/).

## Overview

This crate provides language bindings for alkanes-rs, enabling you to use the library from:
- **Kotlin** (JVM/Android)
- **Swift** (iOS/macOS) 
- **Python**
- Other languages supported by UniFFI

## Architecture

```
┌─────────────────────────────────────┐
│   Application (Kotlin/Swift/Python) │
├─────────────────────────────────────┤
│   Generated Bindings (UniFFI)      │
├─────────────────────────────────────┤
│   alkanes-ffi (this crate)         │
├─────────────────────────────────────┤
│   alkanes-cli-common               │
├─────────────────────────────────────┤
│   alkanes-support                   │
└─────────────────────────────────────┘
```

### Design Principles

1. **Safe FFI Boundary**: All types crossing the FFI boundary are carefully mapped to prevent memory issues
2. **Ergonomic API**: Language-idiomatic APIs generated automatically by UniFFI
3. **Async Support**: Async Rust functions are wrapped with internal runtime for synchronous FFI calls
4. **Error Handling**: Rust `Result` types are mapped to exceptions in target languages

## Building

### Prerequisites

- Rust toolchain (1.70+)
- UniFFI CLI: `cargo install uniffi-cli`
- Target-specific toolchains (see below)

### For Kotlin/JVM

```bash
# Generate Kotlin bindings
cargo run --bin uniffi-bindgen generate \
    --library target/debug/libalkanes_ffi.so \
    --language kotlin \
    --out-dir out/kotlin

# The generated Kotlin files will be in out/kotlin/
```

### For Swift (iOS/macOS)

```bash
# Add iOS targets
rustup target add aarch64-apple-ios x86_64-apple-ios

# Generate Swift bindings
cargo run --bin uniffi-bindgen generate \
    --library target/debug/libalkanes_ffi.dylib \
    --language swift \
    --out-dir out/swift
```

### For Python

```bash
# Generate Python bindings
cargo run --bin uniffi-bindgen generate \
    --library target/debug/libalkanes_ffi.so \
    --language python \
    --out-dir out/python
```

## Features

### Wallet Management
- Create wallets with BIP39 mnemonics
- Derive addresses (P2PKH, P2WPKH, P2TR, etc.)
- Check balances
- Build and sign transactions

### Alkanes Integration
- Query alkanes balances
- Interact with alkanes contracts
- Trace transaction execution
- Get contract bytecode

### Bitcoin RPC
- Query blockchain information
- Broadcast transactions
- Get blocks and transactions

## Usage Examples

### Kotlin (Android)

```kotlin
import org.alkanes.*

// Generate a new mnemonic
val mnemonic = generateMnemonic(WordCount.WORDS12)
println("Mnemonic: $mnemonic")

// Create a wallet
val config = WalletConfig(
    walletPath = getFilesDir().absolutePath + "/wallet",
    network = Network.REGTEST,
    passphrase = "my_secure_password"
)
val wallet = Wallet(config, mnemonic)

// Get a receiving address
val address = wallet.getAddress(AddressType.P2WPKH, 0u)
println("Receive at: $address")

// Check balance
wallet.sync()
val balance = wallet.getBalance()
println("Balance: ${balance.confirmed} sats")
```

### Swift (iOS)

```swift
import Alkanes

// Generate a mnemonic
let mnemonic = try generateMnemonic(wordCount: .words12)
print("Mnemonic: \(mnemonic)")

// Create a wallet
let config = WalletConfig(
    walletPath: nil,
    network: .regtest,
    passphrase: "my_secure_password"
)
let wallet = try Wallet(config: config, mnemonic: mnemonic)

// Get an address
let address = try wallet.getAddress(addressType: .p2wpkh, index: 0)
print("Address: \(address)")
```

### Python

```python
from alkanes import *

# Generate a mnemonic
mnemonic = generate_mnemonic(WordCount.WORDS_12)
print(f"Mnemonic: {mnemonic}")

# Create a wallet
config = WalletConfig(
    wallet_path="/tmp/wallet",
    network=Network.REGTEST,
    passphrase="my_secure_password"
)
wallet = Wallet(config, mnemonic)

# Get an address
address = wallet.get_address(AddressType.P2_WPKH, 0)
print(f"Address: {address}")
```

## API Reference

See the [UDL file](src/alkanes.udl) for the complete API definition.

### Core Types

- `Network`: Bitcoin network (Bitcoin, Testnet, Signet, Regtest)
- `AddressType`: Address types (P2PKH, P2WPKH, P2TR, etc.)
- `AlkaneId`: Alkanes contract identifier
- `WalletBalance`: Balance information
- `AlkaneBalance`: Alkanes token balance

### Core Interfaces

- `Wallet`: Manage Bitcoin wallets
- `RpcClient`: Interact with Bitcoin nodes
- `AlkanesClient`: Interact with alkanes indexer
- `TransactionBuilder`: Build Bitcoin transactions

### Utility Functions

- `version()`: Get library version
- `generateMnemonic()`: Generate BIP39 mnemonic
- `validateAddress()`: Validate Bitcoin addresses
- `parseAddress()`: Parse address information
- `addressToScriptPubkey()`: Convert address to script

## Error Handling

All fallible operations return `Result` in Rust, which maps to exceptions in target languages:

**Kotlin:**
```kotlin
try {
    val address = wallet.getAddress(AddressType.P2WPKH, 0u)
} catch (e: AlkanesException.InvalidAddress) {
    println("Invalid address: ${e.message}")
}
```

**Swift:**
```swift
do {
    let address = try wallet.getAddress(addressType: .p2wpkh, index: 0)
} catch AlkanesError.InvalidAddress(let message) {
    print("Invalid address: \(message)")
}
```

**Python:**
```python
try:
    address = wallet.get_address(AddressType.P2_WPKH, 0)
except AlkanesError.InvalidAddress as e:
    print(f"Invalid address: {e}")
```

## Testing

```bash
# Run Rust tests
cargo test

# Run UniFFI binding tests
cargo test --features uniffi/bindgen-tests
```

## Contributing

When adding new functionality:

1. Add the function/type to `src/alkanes.udl`
2. Implement it in `src/lib.rs`
3. Add tests
4. Regenerate bindings for your target language
5. Test in target language

## License

MIT

## See Also

- [alkanes-jni](../alkanes-jni) - Pre-configured JVM/Android bindings
- [UniFFI User Guide](https://mozilla.github.io/uniffi-rs/)
- [BDK-FFI](https://github.com/bitcoindevkit/bdk-ffi) - Inspiration for this architecture
