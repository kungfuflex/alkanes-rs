# Alkanes JNI

JVM/Android bindings for alkanes-rs. This crate provides a pre-configured setup for using alkanes in Kotlin/Java applications and Android apps.

## Quick Start (Android)

### 1. Add the plugin to your Android project

In your project's `build.gradle.kts`:

```kotlin
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.mozilla.rust-android-gradle.rust-android") version "0.9.4"
}

android {
    // ... your android config
    
    ndkVersion = "26.1.10909125"
}

cargo {
    module = "../../alkanes-rs/crates/alkanes-jni"  // Path to this crate
    libname = "alkanes_jni"
    targets = listOf("arm64", "x86_64")  // Android architectures
    profile = "release"  // Use release for better performance
}

dependencies {
    implementation("net.java.dev.jna:jna:5.14.0@aar")
    // ... other dependencies
}
```

### 2. Copy the generated Kotlin files

After building, copy the generated Kotlin bindings from `alkanes-ffi/out/kotlin/` to your Android project's source directory:

```bash
# Generate bindings
cd crates/alkanes-ffi
cargo build --release
uniffi-bindgen generate \
    --library ../../target/release/libalkanes_ffi.so \
    --language kotlin \
    --out-dir out/kotlin

# Copy to Android project
cp out/kotlin/org/alkanes/*.kt /path/to/your/android/app/src/main/java/org/alkanes/
```

### 3. Use in your Kotlin code

```kotlin
package com.example.myapp

import androidx.appcompat.app.AppCompatActivity
import android.os.Bundle
import android.widget.TextView
import org.alkanes.*

class MainActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        
        // Generate a mnemonic
        val mnemonic = generateMnemonic(WordCount.WORDS12)
        
        // Create a wallet
        val walletPath = "${filesDir.absolutePath}/wallet"
        val config = WalletConfig(
            walletPath = walletPath,
            network = Network.REGTEST,
            passphrase = "my_secure_password"
        )
        
        try {
            val wallet = Wallet(config, mnemonic)
            val address = wallet.getAddress(AddressType.P2WPKH, 0u)
            
            findViewById<TextView>(R.id.addressText).text = 
                "Address: $address\nMnemonic: $mnemonic"
        } catch (e: AlkanesException) {
            findViewById<TextView>(R.id.addressText).text = 
                "Error: ${e.message}"
        }
    }
}
```

## Quick Start (JVM/Server)

### 1. Add to your build.gradle.kts

```kotlin
dependencies {
    implementation("net.java.dev.jna:jna:5.14.0")
}
```

### 2. Build the shared library

```bash
cd crates/alkanes-jni
cargo build --release

# The library will be at:
# Linux: ../../target/release/libalkanes_jni.so
# macOS: ../../target/release/libalkanes_jni.dylib
# Windows: ../../target/release/alkanes_jni.dll
```

### 3. Copy bindings and use

```kotlin
import org.alkanes.*

fun main() {
    // Make sure the native library is in your library path
    System.setProperty("jna.library.path", "/path/to/target/release")
    
    // Use the API
    val version = version()
    println("Alkanes version: $version")
    
    val mnemonic = generateMnemonic(WordCount.WORDS12)
    println("Mnemonic: $mnemonic")
}
```

## Architecture

```
┌─────────────────────────────┐
│   Your Android/JVM App      │
│   (Kotlin/Java code)        │
├─────────────────────────────┤
│   Generated Kotlin Bindings │  ← UniFFI generated
│   (org.alkanes.*)           │
├─────────────────────────────┤
│   JNA (Java Native Access)  │  ← Handles JNI calls
├─────────────────────────────┤
│   alkanes-jni (this crate)  │  ← Thin wrapper
├─────────────────────────────┤
│   alkanes-ffi               │  ← Core FFI logic
├─────────────────────────────┤
│   alkanes-cli-common        │  ← Business logic
└─────────────────────────────┘
```

## Examples

### Wallet Operations

```kotlin
// Create or restore a wallet
val config = WalletConfig(
    walletPath = "/path/to/wallet.db",
    network = Network.BITCOIN,
    passphrase = "secure_passphrase"
)

val wallet = if (existingMnemonic != null) {
    Wallet(config, existingMnemonic)
} else {
    val mnemonic = generateMnemonic(WordCount.WORDS24)
    // Save this mnemonic securely!
    Wallet(config, mnemonic)
}

// Generate addresses
val addresses = (0u..4u).map { index ->
    wallet.getAddress(AddressType.P2WPKH, index)
}

// Check balance
wallet.sync()
val balance = wallet.getBalance()
println("Confirmed: ${balance.confirmed} sats")
println("Pending: ${balance.pending} sats")
```

### Alkanes Operations

```kotlin
// Connect to alkanes indexer
val client = AlkanesClient(
    metashrewUrl = "https://mainnet.sandshrew.io",
    sandshrewUrl = "https://mainnet.sandshrew.io/v2/lasereyes",
    network = Network.BITCOIN
)

// Get alkanes balances for an address
val address = "bc1q..."
val balances = client.getBalance(address)

for (balance in balances) {
    println("Token: ${balance.name ?: "Unknown"}")
    println("Amount: ${balance.amount}")
    println("Contract: ${balance.id.block}:${balance.id.tx}")
}

// Get contract bytecode
val alkaneId = AlkaneId(block = 840000u, tx = 100u)
val bytecode = client.getBytecode(alkaneId)
println("Bytecode: $bytecode")
```

### Transaction Building

```kotlin
// Build a transaction
val builder = TransactionBuilder(Network.BITCOIN)

// Add inputs (from UTXOs)
builder.addInput(
    txid = "abc123...",
    vout = 0u,
    amount = 50000u  // sats
)

// Add outputs
builder.addOutput(
    address = "bc1q...",
    amount = 45000u  // sats
)

// Set fee rate
builder.setFeeRate(10.0f)  // 10 sats/vbyte

// Build unsigned transaction
val unsignedTx = builder.build()
println("Transaction hex: $unsignedTx")
```

## Android Permissions

Add to your `AndroidManifest.xml`:

```xml
<uses-permission android:name="android.permission.INTERNET" />
<uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
```

## ProGuard/R8 Configuration

If using ProGuard or R8, add to `proguard-rules.pro`:

```proguard
# Keep JNA classes
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }

# Keep Alkanes FFI classes
-keep class org.alkanes.** { *; }
```

## Troubleshooting

### Library not found

Make sure the native library is in your library path:

**Android:** The cargo plugin should handle this automatically. If not, manually copy the .so files to `src/main/jniLibs/{abi}/`

**JVM:** Set the library path:
```kotlin
System.setProperty("jna.library.path", "/path/to/lib")
```

### UnsatisfiedLinkError

This usually means the native library couldn't be loaded. Check:
1. Library is built for the correct architecture
2. Library path is set correctly
3. All dependencies are present

### Out of memory on Android

For memory-intensive operations, consider:
1. Using a background thread
2. Increasing heap size in `android/app/build.gradle`:
   ```kotlin
   android {
       dexOptions {
           javaMaxHeapSize "4g"
       }
   }
   ```

## Building from Source

```bash
# Build for your current platform
cargo build --release

# Build for Android (requires Android NDK)
# Install targets first:
rustup target add aarch64-linux-android
rustup target add x86_64-linux-android

# Use cargo-ndk for cross-compilation
cargo install cargo-ndk
cargo ndk -t arm64-v8a build --release
cargo ndk -t x86_64 build --release
```

## Testing

```bash
# Run Rust tests
cargo test

# For Android integration testing, use the Android project's test infrastructure
```

## License

MIT

## See Also

- [alkanes-ffi](../alkanes-ffi) - Core FFI bindings
- [UniFFI Documentation](https://mozilla.github.io/uniffi-rs/)
- [Android NDK Guide](https://developer.android.com/ndk/guides)
- [rust-android-gradle Plugin](https://github.com/mozilla/rust-android-gradle)
