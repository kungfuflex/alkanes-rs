# Getting Started with Alkanes FFI

This guide will help you quickly get started with using alkanes-rs in your Kotlin, Swift, or Python applications.

## Prerequisites

- Rust 1.70+ installed
- `uniffi-bindgen` CLI tool: `cargo install uniffi-bindgen --version 0.28`
- For Android: Android NDK 26+ and Android Studio
- For iOS: Xcode and Rust iOS targets
- For Python: Python 3.7+

## Quick Start (5 minutes)

### 1. Build the Library

```bash
cd alkanes-rs/crates/alkanes-ffi
cargo build --release
```

### 2. Generate Bindings

```bash
# Run the binding generation script
./examples/generate_bindings.sh

# Or manually:
uniffi-bindgen generate \
    --library ../../target/release/libalkanes_ffi.so \
    --language kotlin \
    --out-dir out/kotlin
```

### 3. Copy Bindings to Your Project

**For Kotlin/Android**:
```bash
cp out/kotlin/org/alkanes/*.kt /path/to/your/android/app/src/main/java/org/alkanes/
```

**For Swift/iOS**:
```bash
cp out/swift/Alkanes.swift /path/to/your/ios/project/
```

**For Python**:
```bash
cp out/python/alkanes.py /path/to/your/python/project/
```

### 4. Use in Your Application

**Kotlin**:
```kotlin
import org.alkanes.*

fun main() {
    println("Alkanes version: ${version()}")
    
    val mnemonic = generateMnemonic(WordCount.WORDS12)
    println("New mnemonic: $mnemonic")
}
```

**Swift**:
```swift
import Alkanes

print("Alkanes version: \(version())")

let mnemonic = try! generateMnemonic(wordCount: .words12)
print("New mnemonic: \(mnemonic)")
```

**Python**:
```python
from alkanes import *

print(f"Alkanes version: {version()}")

mnemonic = generate_mnemonic(WordCount.WORDS_12)
print(f"New mnemonic: {mnemonic}")
```

## Android Integration (Detailed)

### Step 1: Add Dependencies

In your `build.gradle.kts`:

```kotlin
plugins {
    id("org.mozilla.rust-android-gradle.rust-android") version "0.9.4"
}

android {
    ndkVersion = "26.1.10909125"
}

cargo {
    module = "path/to/alkanes-jni"
    libname = "alkanes_jni"
    targets = listOf("arm64", "x86_64")
    profile = "release"
}

dependencies {
    implementation("net.java.dev.jna:jna:5.14.0@aar")
}
```

### Step 2: Copy Kotlin Bindings

```bash
# Generate bindings first
cd alkanes-rs/crates/alkanes-ffi
./examples/generate_bindings.sh

# Copy to your Android project
cp -r out/kotlin/org/alkanes/ \
    /path/to/your/android/app/src/main/java/org/alkanes/
```

### Step 3: Use in Your Activity

```kotlin
class MainActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        // Generate a wallet
        val mnemonic = generateMnemonic(WordCount.WORDS12)
        
        val config = WalletConfig(
            walletPath = "${filesDir}/wallet",
            network = Network.REGTEST,
            passphrase = "secure"
        )
        
        val wallet = Wallet(config, mnemonic)
        val address = wallet.getAddress(AddressType.P2WPKH, 0u)
        
        Toast.makeText(this, "Address: $address", Toast.LENGTH_LONG).show()
    }
}
```

## iOS Integration (Detailed)

### Step 1: Add Rust Targets

```bash
rustup target add aarch64-apple-ios
rustup target add x86_64-apple-ios  # For simulator
```

### Step 2: Build for iOS

```bash
cd alkanes-rs
cargo build --target aarch64-apple-ios --release --package alkanes-ffi
```

### Step 3: Generate Swift Bindings

```bash
cd crates/alkanes-ffi
uniffi-bindgen generate \
    --library ../../target/aarch64-apple-ios/release/libalkanes_ffi.a \
    --language swift \
    --out-dir out/swift
```

### Step 4: Add to Xcode Project

1. Drag `libalkanes_ffi.a` into your Xcode project
2. Add the generated Swift files to your project
3. Update Build Settings:
   - Add library search path
   - Link against `Security.framework`

### Step 5: Use in Your Swift Code

```swift
import Alkanes

class ViewController: UIViewController {
    override func viewDidLoad() {
        super.viewDidLoad()
        
        do {
            let mnemonic = try generateMnemonic(wordCount: .words12)
            
            let config = WalletConfig(
                walletPath: nil,
                network: .regtest,
                passphrase: "secure"
            )
            
            let wallet = try Wallet(config: config, mnemonic: mnemonic)
            let address = try wallet.getAddress(addressType: .p2wpkh, index: 0)
            
            print("Address: \(address)")
        } catch {
            print("Error: \(error)")
        }
    }
}
```

## Python Integration (Detailed)

### Step 1: Build the Library

```bash
cd alkanes-rs/crates/alkanes-ffi
cargo build --release
```

### Step 2: Generate Python Bindings

```bash
uniffi-bindgen generate \
    --library ../../target/release/libalkanes_ffi.so \
    --language python \
    --out-dir out/python
```

### Step 3: Set Up Python Environment

```bash
# Copy the binding and library
cp out/python/alkanes.py /your/python/project/
cp ../../target/release/libalkanes_ffi.so /your/python/project/

# Or add to PYTHONPATH
export PYTHONPATH=/path/to/alkanes-ffi/out/python:$PYTHONPATH
export LD_LIBRARY_PATH=/path/to/alkanes-rs/target/release:$LD_LIBRARY_PATH
```

### Step 4: Use in Your Python Code

```python
#!/usr/bin/env python3
from alkanes import *

def main():
    # Generate mnemonic
    mnemonic = generate_mnemonic(WordCount.WORDS_12)
    print(f"Mnemonic: {mnemonic}")
    
    # Create wallet
    config = WalletConfig(
        wallet_path="/tmp/wallet",
        network=Network.REGTEST,
        passphrase="secure"
    )
    
    wallet = Wallet(config, mnemonic)
    
    # Get addresses
    for i in range(5):
        address = wallet.get_address(AddressType.P2_WPKH, i)
        print(f"Address {i}: {address}")

if __name__ == "__main__":
    main()
```

## Common Issues & Solutions

### Issue: "uniffi-bindgen not found"

```bash
cargo install uniffi-bindgen --version 0.28
```

### Issue: "Library not found" on Android

Make sure:
1. NDK is properly installed
2. `cargo` plugin is configured correctly
3. Library is built for the correct architecture

Check `app/build/rustJniLibs/android/` for the .so files.

### Issue: "Library not found" on Python

Set the library path:
```bash
export LD_LIBRARY_PATH=/path/to/target/release:$LD_LIBRARY_PATH
```

Or copy the .so file next to your Python script.

### Issue: Type errors in generated bindings

Ensure your UniFFI version matches:
```bash
# In Cargo.toml
uniffi = "0.28"

# CLI tool
uniffi-bindgen 0.28
```

## Next Steps

1. **Explore the API**: Check out the [UDL file](src/alkanes.udl) for all available functions
2. **Read Examples**: See language-specific examples in `examples/`
3. **Check Documentation**: Read the [FFI Architecture](../../docs/FFI_ARCHITECTURE.md)
4. **Join Community**: Ask questions and share your projects

## Resources

- [UniFFI User Guide](https://mozilla.github.io/uniffi-rs/)
- [Alkanes Documentation](https://github.com/kungfuflex/alkanes-rs)
- [BDK FFI Examples](https://github.com/bitcoindevkit/bdk-ffi)

## Support

- GitHub Issues: https://github.com/kungfuflex/alkanes-rs/issues
- Discord: [Add your Discord link]

Happy building! 🚀
