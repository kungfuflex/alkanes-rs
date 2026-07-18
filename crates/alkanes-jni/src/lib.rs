//! # Alkanes JNI
//!
//! JVM/Android bindings for alkanes-rs.
//! This crate re-exports alkanes-ffi with additional JVM-specific configuration.
//!
//! ## Usage
//!
//! ### In your Android project's build.gradle.kts:
//!
//! ```kotlin
//! plugins {
//!     id("org.mozilla.rust-android-gradle.rust-android")
//! }
//!
//! cargo {
//!     module = "../alkanes-rs/crates/alkanes-jni"
//!     libname = "alkanes_jni"
//!     targets = listOf("arm64", "x86_64")
//! }
//!
//! dependencies {
//!     implementation("net.java.dev.jna:jna:5.14.0@aar")
//! }
//! ```
//!
//! ### In your Kotlin code:
//!
//! ```kotlin
//! import org.alkanes.*
//!
//! fun main() {
//!     // Generate a mnemonic
//!     val mnemonic = generateMnemonic(WordCount.WORDS12)
//!     println("Mnemonic: $mnemonic")
//!     
//!     // Create a wallet
//!     val config = WalletConfig(
//!         walletPath = "/data/wallet",
//!         network = Network.REGTEST,
//!         passphrase = "secure_password"
//!     )
//!     val wallet = Wallet(config, mnemonic)
//!     
//!     // Get an address
//!     val address = wallet.getAddress(AddressType.P2WPKH, 0u)
//!     println("Address: $address")
//! }
//! ```

// Re-export everything from alkanes-ffi
pub use alkanes_ffi::*;

// UniFFI will generate the JNI bindings
uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jni_version() {
        assert!(!version().is_empty());
    }
}
