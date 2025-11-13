# Alkanes FFI/JNI Crates Summary

## What Was Built

Two new crates have been created to enable FFI bindings for alkanes-rs:

### 1. `alkanes-ffi` (Foreign Function Interface)
**Location**: `crates/alkanes-ffi/`

A core FFI crate that uses Mozilla's UniFFI framework to generate language bindings for:
- Kotlin/Java (JVM)
- Swift (iOS/macOS)
- Python
- Other UniFFI-supported languages

**Key Files Created**:
- `Cargo.toml` - Dependencies and build configuration
- `build.rs` - UniFFI scaffolding generation
- `src/alkanes.udl` - UniFFI Definition Language API specification
- `src/lib.rs` - Rust implementation of the FFI API
- `uniffi.toml` - Language-specific binding configuration
- `README.md` - Comprehensive documentation
- `GETTING_STARTED.md` - Quick start guide
- `examples/generate_bindings.sh` - Automated binding generation script

**API Coverage**:
- ✅ Module-level functions (version, validate_address, generate_mnemonic)
- ✅ Wallet management interface
- ✅ RPC client interface
- ✅ Alkanes client interface
- ✅ Transaction builder interface
- ✅ Address utilities
- ✅ Error handling with proper exception mapping
- ⚠️  Implementation stubs (need to wire up to alkanes-cli-common)

### 2. `alkanes-jni` (JVM/Android Specific)
**Location**: `crates/alkanes-jni/`

A specialized crate for JVM/Android that:
- Re-exports `alkanes-ffi` with JVM-optimized configuration
- Provides Android NDK build integration
- Includes Android-specific examples and documentation

**Key Files Created**:
- `Cargo.toml` - JVM-specific dependencies
- `build.rs` - JVM build configuration
- `src/lib.rs` - Re-exports from alkanes-ffi
- `README.md` - Android/JVM integration guide
- `examples/android-example/` - Example Android project configuration

## Architecture

```
Application (Kotlin/Swift/Python)
        ↓
Generated Bindings (UniFFI)
        ↓
alkanes-ffi (C-compatible FFI)
        ↓
alkanes-cli-common (Business Logic)
        ↓
Core Libraries (alkanes-support, etc.)
```

## Current Status

### ✅ Completed
1. **Project Structure**: Both crates properly set up with Cargo manifests
2. **UniFFI Integration**: UDL file defines complete API surface
3. **Error Handling**: Custom error types that map to exceptions
4. **Documentation**: Comprehensive READMEs and guides
5. **Build Scripts**: Automated binding generation
6. **Examples**: Android integration examples
7. **Type System**: FFI-safe types (u128 → string, etc.)
8. **Architecture Docs**: Complete FFI_ARCHITECTURE.md explaining design

### ⚠️ Needs Completion
1. **Implementation**: Function bodies are stubs (return "not implemented")
2. **Type Mapping**: Some type mismatches need resolution (e.g., Network enum)
3. **Testing**: No tests yet for generated bindings
4. **Async Runtime**: Runtime encapsulation needs refinement
5. **Memory Management**: Arc usage needs validation

### 🔧 To Fix Before Use

The crates currently don't compile due to:
1. **Network Enum Mismatch**: alkanes_cli_common::Network has more variants than the UDL defines
2. **Missing Implementations**: All interface methods are stubs
3. **Type Conversions**: Need proper From/Into implementations

## How to Complete

### Step 1: Fix Type Mappings

In `alkanes-ffi/src/lib.rs`, create a local `Network` enum that matches the UDL:

```rust
// Instead of re-exporting
// pub use alkanes_cli_common::Network;

// Define our own that matches UDL
#[derive(Debug, Clone, Copy)]
pub enum Network {
    Bitcoin,
    Testnet,
    Signet,
    Regtest,
}

// Convert to/from internal type
impl From<Network> for alkanes_cli_common::Network {
    fn from(net: Network) -> Self {
        match net {
            Network::Bitcoin => alkanes_cli_common::Network::Bitcoin,
            Network::Testnet => alkanes_cli_common::Network::Testnet,
            Network::Signet => alkanes_cli_common::Network::Signet,
            Network::Regtest => alkanes_cli_common::Network::Regtest,
        }
    }
}
```

### Step 2: Implement Wallet Functions

Wire up the `Wallet` struct to use `alkanes_cli_common::provider::ConcreteProvider`:

```rust
pub struct Wallet {
    runtime: tokio::runtime::Runtime,
    provider: alkanes_cli_common::provider::ConcreteProvider,
}

impl Wallet {
    pub fn new(config: WalletConfig, mnemonic: Option<String>) -> Result<Arc<Self>> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| AlkanesError::WalletError(e.to_string()))?;
        
        let provider = runtime.block_on(async {
            alkanes_cli_common::provider::ConcreteProvider::new(
                // Configure based on config parameter
                // ...
            ).await
        }).map_err(|e| AlkanesError::WalletError(e.to_string()))?;
        
        Ok(Arc::new(Self { runtime, provider }))
    }
    
    pub fn get_address(&self, address_type: AddressType, index: u32) -> Result<String> {
        self.runtime.block_on(async {
            self.provider.get_address(/* ... */).await
        }).map_err(|e| AlkanesError::WalletError(e.to_string()))
    }
}
```

### Step 3: Implement RpcClient

Similar pattern for RPC operations.

### Step 4: Add Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_wallet_creation() {
        let config = WalletConfig {
            wallet_path: None,
            network: Network::Regtest,
            passphrase: Some("test".to_string()),
        };
        
        let wallet = Wallet::new(config, None).unwrap();
        assert!(wallet.get_mnemonic().is_ok());
    }
}
```

### Step 5: Generate and Test Bindings

```bash
cd crates/alkanes-ffi
cargo build --release
./examples/generate_bindings.sh

# Test in target language
# Kotlin example in crates/alkanes-jni/examples/
```

## Usage Once Complete

### Android (Kotlin)

```kotlin
// build.gradle.kts
cargo {
    module = "path/to/alkanes-jni"
    libname = "alkanes_jni"
    targets = listOf("arm64", "x86_64")
}

dependencies {
    implementation("net.java.dev.jna:jna:5.14.0@aar")
}

// MainActivity.kt
val mnemonic = generateMnemonic(WordCount.WORDS12)
val config = WalletConfig(
    walletPath = "${filesDir}/wallet",
    network = Network.REGTEST,
    passphrase = "secure"
)
val wallet = Wallet(config, mnemonic)
val address = wallet.getAddress(AddressType.P2WPKH, 0u)
```

### iOS (Swift)

```swift
import Alkanes

let mnemonic = try generateMnemonic(wordCount: .words12)
let config = WalletConfig(
    walletPath: nil,
    network: .regtest,
    passphrase: "secure"
)
let wallet = try Wallet(config: config, mnemonic: mnemonic)
let address = try wallet.getAddress(addressType: .p2wpkh, index: 0)
```

### Python

```python
from alkanes import *

mnemonic = generate_mnemonic(WordCount.WORDS_12)
config = WalletConfig(
    wallet_path="/tmp/wallet",
    network=Network.REGTEST,
    passphrase="secure"
)
wallet = Wallet(config, mnemonic)
address = wallet.get_address(AddressType.P2_WPKH, 0)
```

## Benefits of This Architecture

1. **Single Source of Truth**: One API definition (UDL) generates all bindings
2. **Memory Safe**: UniFFI handles memory management across FFI boundary
3. **Type Safe**: Strong typing in all target languages
4. **Error Handling**: Rust Results map to exceptions automatically
5. **Maintainable**: Changes to UDL automatically propagate to all languages
6. **Well-Tested Pattern**: Used by major projects (BDK, Matrix, etc.)

## Next Steps

1. **Priority 1**: Fix Network enum and get crates compiling
2. **Priority 2**: Implement wallet operations (most valuable for users)
3. **Priority 3**: Add RPC client implementation
4. **Priority 4**: Add Alkanes client for contract interaction
5. **Priority 5**: Add comprehensive tests
6. **Priority 6**: Create real Android/iOS sample apps

## References

- [UniFFI User Guide](https://mozilla.github.io/uniffi-rs/)
- [BDK-FFI Repository](https://github.com/bitcoindevkit/bdk-ffi)
- [rust-android-gradle](https://github.com/mozilla/rust-android-gradle)
- [FFI_ARCHITECTURE.md](../docs/FFI_ARCHITECTURE.md)

## Maintenance Notes

When adding new functionality:

1. Update `src/alkanes.udl` with new types/functions
2. Implement in `src/lib.rs`
3. Regenerate bindings with `./examples/generate_bindings.sh`
4. Add tests
5. Update documentation

When updating dependencies:
- Keep UniFFI version consistent across `Cargo.toml` and CLI tool
- Test on all target platforms after updates

## Conclusion

The foundation for FFI/JNI bindings is complete with a solid architecture based on proven patterns. The remaining work is primarily:
1. Fixing compilation issues (type mapping)
2. Implementing the stub functions
3. Adding tests
4. Creating example applications

This provides a clear path for Kotlin/Android, Swift/iOS, and Python developers to use alkanes-rs in their applications.
