// Chadson's Journal (2025-09-10T14:38:11.850Z)
//
// Objective: Fix compilation error in `metashrew-support` crate.
//
// The compilation failed with the error:
// `no method named 'to_x_only_public_key' found for struct 'TweakedPublicKey' in the current scope`
//
// Research into the `rust-bitcoin` crate's source code (`reference/rust-bitcoin/bitcoin/src/crypto/key.rs`)
// revealed that `TweakedPublicKey` is a tuple struct wrapping `XOnlyPublicKey` and implements
// `From<TweakedPublicKey> for XOnlyPublicKey`.
//
// The fix is to replace the explicit method call `.to_x_only_public_key()` with the more idiomatic
// and stable `.into()`, which leverages the `From` trait implementation. This change is less
// susceptible to breaking with future updates to the `bitcoin` crate.
//
// The following changes were made:
// - Replaced `output_key.to_x_only_public_key().serialize()` with `output_key.into().serialize()`
//   in `p2tr` and `p2tr_tweaked` functions.

//! # Comprehensive Bitcoin Address Handling and Encoding
//!
//! This module provides complete support for all Bitcoin address types and encoding formats.
//! It handles the full spectrum of Bitcoin addresses from legacy P2PKH to modern Taproot,
//! including SegWit variants and nested address types. The implementation is designed for
//! robust address parsing, validation, and encoding across different Bitcoin networks.
//!
//! ## Supported Address Types
//!
//! ### Legacy Addresses
//! - **P2PKH (Pay to Public Key Hash)**: Traditional "1..." addresses
//! - **P2SH (Pay to Script Hash)**: Multi-signature and script addresses "3..."
//!
//! ### SegWit Addresses  
//! - **P2WPKH (Pay to Witness Public Key Hash)**: Native SegWit "bc1q..." addresses
//! - **P2WSH (Pay to Witness Script Hash)**: Native SegWit script addresses
//! - **P2SH-P2WPKH**: SegWit wrapped in P2SH for backward compatibility
//! - **P2SH-P2WSH**: SegWit script wrapped in P2SH
//!
//! ### Taproot Addresses
//! - **P2TR (Pay to Taproot)**: Latest "bc1p..." addresses with enhanced privacy and scripting
//!
//! ## Core Concepts
//!
//! ### Address Payload
//! The [`Payload`] enum represents the core addressing information extracted from
//! Bitcoin scripts. It abstracts the underlying script patterns into three categories:
//! - **PubkeyHash**: Direct public key hash (P2PKH)
//! - **ScriptHash**: Script hash for complex conditions (P2SH)
//! - **WitnessProgram**: SegWit and Taproot witness programs
//!
//! ### Address Encoding
//! The [`AddressEncoding`] struct handles network-specific encoding parameters:
//! - **Base58 prefixes**: Network-specific version bytes for legacy addresses
//! - **Bech32 HRP**: Human-readable part for SegWit addresses
//! - **Case formatting**: Support for both lowercase and uppercase bech32
//!
//! ## Usage Examples
//!
//! ```rust,ignore
//! use metashrew_support::address::*;
//! use bitcoin::{PublicKey, Script};
//!
//! // Create P2PKH payload from public key
//! let pubkey = PublicKey::from_str("02...").unwrap();
//! let payload = Payload::p2pkh(&pubkey);
//!
//! // Extract payload from script
//! let script = Script::from_hex("76a914...88ac").unwrap();
//! let payload = Payload::from_script(&script).unwrap();
//!
//! // Generate script from payload
//! let script_pubkey = payload.script_pubkey();
//!
//! // Check if payload matches a script
//! assert!(payload.matches_script_pubkey(&script));
//! ```
//!
//! ## Integration with Metashrew
//!
//! This module enables Metashrew indexers to:
//! - **Parse all address types**: Extract addresses from transaction outputs
//! - **Validate addresses**: Ensure address format correctness
//! - **Index by address**: Create address-based transaction indices
//! - **Support all networks**: Handle mainnet, testnet, and custom networks

use anyhow::{anyhow, Error, Result};
use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::str::FromStr;

use bech32::primitives::hrp::Hrp;
use bitcoin::hashes::Hash;
use bitcoin::secp256k1::{Secp256k1, Verification};

use bitcoin::base58;
use bitcoin::blockdata::constants::MAX_SCRIPT_ELEMENT_SIZE;
use bitcoin::blockdata::script::witness_program::WitnessProgram;
use bitcoin::blockdata::script::witness_version::WitnessVersion;
use bitcoin::blockdata::script::{self, Script, ScriptBuf, ScriptHash};
use bitcoin::key::{TapTweak, TweakedPublicKey, UntweakedPublicKey, XOnlyPublicKey};
use bitcoin::taproot::TapNodeHash;
use bitcoin::{PubkeyHash, PublicKey};

/// The different types of Bitcoin addresses.
///
/// This enum categorizes all supported Bitcoin address types, providing a
/// high-level classification that abstracts the underlying script patterns.
/// Each variant represents a different approach to Bitcoin transaction authorization.
///
/// # Address Evolution
/// Bitcoin addresses have evolved over time to provide:
/// - **Enhanced security**: Stronger cryptographic primitives
/// - **Improved privacy**: Better transaction unlinkability  
/// - **Reduced fees**: More efficient transaction formats
/// - **Advanced scripting**: More flexible spending conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum AddressType {
    /// Pay to Public Key Hash - Legacy Bitcoin addresses.
    ///
    /// The original Bitcoin address format that pays directly to the hash
    /// of a public key. These addresses start with "1" on mainnet.
    P2pkh,
    
    /// Pay to Script Hash - Multi-signature and complex script addresses.
    ///
    /// Enables complex spending conditions by paying to the hash of a script.
    /// These addresses start with "3" on mainnet and enable multi-signature
    /// wallets and other advanced functionality.
    P2sh,
    
    /// Pay to Witness Public Key Hash - Native SegWit addresses.
    ///
    /// SegWit version 0 addresses that provide transaction malleability fixes
    /// and fee reductions. These addresses start with "bc1q" on mainnet.
    P2wpkh,
    
    /// Pay to Witness Script Hash - Native SegWit script addresses.
    ///
    /// SegWit version 0 script addresses that enable complex spending conditions
    /// with SegWit benefits. Also start with "bc1q" but are longer.
    P2wsh,
    
    /// Pay to Taproot - Latest Bitcoin address format.
    ///
    /// SegWit version 1 addresses that provide enhanced privacy through
    /// key aggregation and script hiding. Start with "bc1p" on mainnet.
    P2tr,
}

impl fmt::Display for AddressType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            AddressType::P2pkh => "p2pkh",
            AddressType::P2sh => "p2sh",
            AddressType::P2wpkh => "p2wpkh",
            AddressType::P2wsh => "p2wsh",
            AddressType::P2tr => "p2tr",
        })
    }
}

impl FromStr for AddressType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "p2pkh" => Ok(AddressType::P2pkh),
            "p2sh" => Ok(AddressType::P2sh),
            "p2wpkh" => Ok(AddressType::P2wpkh),
            "p2wsh" => Ok(AddressType::P2wsh),
            "p2tr" => Ok(AddressType::P2tr),
            _ => Err(anyhow!(s.to_owned())),
        }
    }
}

/// The method used to produce an address payload.
///
/// This enum represents the core addressing information extracted from Bitcoin
/// output scripts. It abstracts the three fundamental ways Bitcoin can specify
/// spending conditions while maintaining the specific cryptographic commitments.
///
/// # Design Philosophy
/// The payload abstraction separates the addressing logic from network-specific
/// encoding, enabling the same payload to be encoded for different Bitcoin networks
/// (mainnet, testnet, regtest) with appropriate prefixes and formats.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Payload {
    /// P2PKH address payload containing a public key hash.
    ///
    /// Contains the 20-byte RIPEMD160(SHA256(pubkey)) hash that identifies
    /// the public key authorized to spend the output.
    PubkeyHash(PubkeyHash),
    
    /// P2SH address payload containing a script hash.
    ///
    /// Contains the 20-byte RIPEMD160(SHA256(script)) hash that identifies
    /// the script whose conditions must be satisfied to spend the output.
    ScriptHash(ScriptHash),
    
    /// SegWit address payload containing a witness program.
    ///
    /// Contains the witness version and program data for SegWit outputs.
    /// This covers both P2WPKH, P2WSH, and future SegWit versions including Taproot.
    WitnessProgram(WitnessProgram),
}

impl Payload {
    /// Constructs a [`Payload`] from an output script (`scriptPubkey`).
    ///
    /// This function analyzes a Bitcoin output script and extracts the addressing
    /// information into a payload. It recognizes all standard script patterns
    /// and converts them to the appropriate payload type.
    ///
    /// # Parameters
    /// - `script`: Reference to the output script to analyze
    ///
    /// # Returns
    /// Payload extracted from the script
    ///
    /// # Errors
    /// Returns error if the script doesn't match any recognized address pattern
    ///
    /// # Supported Script Patterns
    /// - **P2PKH**: `OP_DUP OP_HASH160 <pubkey_hash> OP_EQUALVERIFY OP_CHECKSIG`
    /// - **P2SH**: `OP_HASH160 <script_hash> OP_EQUAL`
    /// - **P2WPKH**: `OP_0 <20-byte-pubkey-hash>`
    /// - **P2WSH**: `OP_0 <32-byte-script-hash>`
    /// - **P2TR**: `OP_1 <32-byte-taproot-output>`
    pub fn from_script(script: &Script) -> Result<Payload> {
        Ok(if script.is_p2pkh() {
            let bytes = script.as_bytes()[3..23]
                .try_into()
                .expect("statically 20B long");
            Payload::PubkeyHash(PubkeyHash::from_byte_array(bytes))
        } else if script.is_p2sh() {
            let bytes = script.as_bytes()[2..22]
                .try_into()
                .expect("statically 20B long");
            Payload::ScriptHash(ScriptHash::from_byte_array(bytes))
        } else if script.is_witness_program() {
            let opcode = script
                .first_opcode()
                .expect("witness_version guarantees len() > 4");

            let witness_program = script.as_bytes()[2..].to_vec();

            let witness_program =
                WitnessProgram::new(WitnessVersion::try_from(opcode)?, &witness_program)?;
            Payload::WitnessProgram(witness_program)
        } else {
            return Err(anyhow!("unrecognized script"));
        })
    }

    /// Generates a script pubkey spending to this [`Payload`].
    ///
    /// This function creates the output script that corresponds to this payload.
    /// The generated script can be used in transaction outputs to create
    /// spendable outputs that require satisfaction of the payload's conditions.
    ///
    /// # Returns
    /// ScriptBuf containing the output script for this payload
    ///
    /// # Script Generation
    /// - **PubkeyHash**: Creates P2PKH script pattern
    /// - **ScriptHash**: Creates P2SH script pattern  
    /// - **WitnessProgram**: Creates appropriate SegWit script pattern
    pub fn script_pubkey(&self) -> ScriptBuf {
        match *self {
            Payload::PubkeyHash(ref hash) => ScriptBuf::new_p2pkh(hash),
            Payload::ScriptHash(ref hash) => ScriptBuf::new_p2sh(hash),
            Payload::WitnessProgram(ref prog) => ScriptBuf::new_witness_program(prog),
        }
    }

    /// Returns true if the address creates a particular script.
    ///
    /// This function efficiently checks if this payload would generate the
    /// given script without performing any allocations. It's optimized for
    /// high-performance address matching in indexing scenarios.
    ///
    /// # Parameters
    /// - `script`: Script to check against this payload
    ///
    /// # Returns
    /// `true` if this payload generates the given script, `false` otherwise
    ///
    /// # Performance
    /// This function avoids allocations by directly comparing the relevant
    /// bytes in the script rather than generating a new script for comparison.
    pub fn matches_script_pubkey(&self, script: &Script) -> bool {
        match *self {
            Payload::PubkeyHash(ref hash) if script.is_p2pkh() => {
                &script.as_bytes()[3..23] == <PubkeyHash as AsRef<[u8; 20]>>::as_ref(hash)
            }
            Payload::ScriptHash(ref hash) if script.is_p2sh() => {
                &script.as_bytes()[2..22] == <ScriptHash as AsRef<[u8; 20]>>::as_ref(hash)
            }
            Payload::WitnessProgram(ref prog) if script.is_witness_program() => {
                &script.as_bytes()[2..] == prog.program().as_bytes()
            }
            Payload::PubkeyHash(_) | Payload::ScriptHash(_) | Payload::WitnessProgram(_) => false,
        }
    }

    /// Creates a pay to (compressed) public key hash payload from a public key.
    ///
    /// This is the most common address type, creating a P2PKH payload that
    /// requires a signature from the corresponding private key to spend.
    ///
    /// # Parameters
    /// - `pk`: Public key to create address for
    ///
    /// # Returns
    /// P2PKH payload for the given public key
    #[inline]
    pub fn p2pkh(pk: &PublicKey) -> Payload {
        Payload::PubkeyHash(pk.pubkey_hash())
    }

    /// Creates a pay to script hash P2SH payload from a script.
    ///
    /// This enables complex spending conditions by requiring the spender to
    /// provide both the script and inputs that satisfy the script's conditions.
    ///
    /// # Parameters
    /// - `script`: Script to create P2SH address for
    ///
    /// # Returns
    /// P2SH payload for the given script
    ///
    /// # Errors
    /// Returns error if script exceeds maximum allowed size
    #[inline]
    pub fn p2sh(script: &Script) -> Result<Payload, Error> {
        if script.len() > MAX_SCRIPT_ELEMENT_SIZE {
            return Err(anyhow!("excessive script size"));
        }
        Ok(Payload::ScriptHash(script.script_hash()))
    }

    /// Create a witness pay to public key payload from a public key.
    ///
    /// This creates a native SegWit P2WPKH address that provides transaction
    /// malleability fixes and reduced transaction fees compared to P2PKH.
    ///
    /// # Parameters
    /// - `pk`: Public key to create SegWit address for (must be compressed)
    ///
    /// # Returns
    /// P2WPKH payload for the given public key
    ///
    /// # Errors
    /// Returns error if the public key is uncompressed (SegWit requires compressed keys)
    pub fn p2wpkh(pk: &PublicKey) -> Result<Payload> {
        let prog = WitnessProgram::new(
            WitnessVersion::V0,
            pk.wpubkey_hash()
                .map_err(|_| anyhow!("uncompressed public key"))?
                .as_ref(),
        )?;
        Ok(Payload::WitnessProgram(prog))
    }

    /// Create a pay to script payload that embeds a witness pay to public key.
    ///
    /// This creates a P2SH-wrapped P2WPKH address for backward compatibility
    /// with wallets that don't support native SegWit. The address appears as
    /// a regular P2SH address but provides SegWit benefits when spent.
    ///
    /// # Parameters
    /// - `pk`: Public key to create wrapped SegWit address for
    ///
    /// # Returns
    /// P2SH payload wrapping P2WPKH for the given public key
    ///
    /// # Errors
    /// Returns error if the public key is uncompressed
    pub fn p2shwpkh(pk: &PublicKey) -> Result<Payload> {
        let builder = script::Builder::new().push_int(0).push_slice(
            pk.wpubkey_hash()
                .map_err(|_| anyhow!("uncompressed public key"))?,
        );

        Ok(Payload::ScriptHash(builder.into_script().script_hash()))
    }

    /// Create a witness pay to script hash payload.
    ///
    /// This creates a native SegWit P2WSH address that enables complex
    /// spending conditions with SegWit benefits. The script is committed
    /// to via its SHA256 hash.
    ///
    /// # Parameters
    /// - `script`: Script to create SegWit script hash address for
    ///
    /// # Returns
    /// P2WSH payload for the given script
    pub fn p2wsh(script: &Script) -> Payload {
        let prog = WitnessProgram::new(WitnessVersion::V0, script.wscript_hash().as_ref())
            .expect("wscript_hash has len 32 compatible with segwitv0");
        Payload::WitnessProgram(prog)
    }

    /// Create a pay to script payload that embeds a witness pay to script hash address.
    ///
    /// This creates a P2SH-wrapped P2WSH address for backward compatibility.
    /// Complex scripts can benefit from SegWit while remaining compatible
    /// with older wallet software.
    ///
    /// # Parameters
    /// - `script`: Script to create wrapped SegWit script address for
    ///
    /// # Returns
    /// P2SH payload wrapping P2WSH for the given script
    pub fn p2shwsh(script: &Script) -> Payload {
        let ws = script::Builder::new()
            .push_int(0)
            .push_slice(script.wscript_hash())
            .into_script();

        Payload::ScriptHash(ws.script_hash())
    }

    /// Create a pay to taproot payload from untweaked key.
    ///
    /// This creates a Taproot address that enables enhanced privacy and
    /// advanced scripting capabilities. The internal key can be tweaked
    /// with an optional merkle root for script path spending.
    ///
    /// # Parameters
    /// - `secp`: Secp256k1 context for cryptographic operations
    /// - `internal_key`: Untweaked public key for key path spending
    /// - `merkle_root`: Optional merkle root for script path spending
    ///
    /// # Returns
    /// P2TR payload for the given key and optional script tree
    pub fn p2tr<C: Verification>(
        secp: &Secp256k1<C>,
        internal_key: UntweakedPublicKey,
        merkle_root: Option<TapNodeHash>,
    ) -> Payload {
        let (output_key, _parity) = internal_key.tap_tweak(secp, merkle_root);
        let prog = WitnessProgram::new(
            WitnessVersion::V1,
            &XOnlyPublicKey::from(output_key).serialize(),
        )
        .expect("taproot output key has len 32 <= 40");
        Payload::WitnessProgram(prog)
    }

    /// Create a pay to taproot payload from a pre-tweaked output key.
    ///
    /// This method creates a Taproot address from an already-tweaked public key.
    /// It should be used with caution as it bypasses the standard key tweaking
    /// process and may not provide the expected security properties.
    ///
    /// # Parameters
    /// - `output_key`: Pre-tweaked Taproot output key
    ///
    /// # Returns
    /// P2TR payload for the given tweaked key
    ///
    /// # Warning
    /// This method is not recommended for general use. Prefer [`Payload::p2tr()`]
    /// which properly handles key tweaking for security.
    pub fn p2tr_tweaked(output_key: TweakedPublicKey) -> Payload {
        let prog = WitnessProgram::new(
            WitnessVersion::V1,
            &XOnlyPublicKey::from(output_key).serialize(),
        )
        .expect("taproot output key has len 32 <= 40");
        Payload::WitnessProgram(prog)
    }
}

/// A utility struct to encode an address payload with the given parameters.
///
/// This struct provides low-level control over address encoding by allowing
/// specification of network-specific parameters. It handles the conversion
/// from payload to string representation according to Bitcoin address standards.
///
/// # Usage
/// This is a low-level utility. Most applications should use higher-level
/// address types that automatically handle network parameters.
///
/// # Network Parameters
/// Different Bitcoin networks use different encoding parameters:
/// - **Mainnet**: P2PKH prefix 0x00, P2SH prefix 0x05, HRP "bc"
/// - **Testnet**: P2PKH prefix 0x6f, P2SH prefix 0xc4, HRP "tb"
/// - **Regtest**: P2PKH prefix 0x6f, P2SH prefix 0xc4, HRP "bcrt"
#[derive(Debug)]
pub struct AddressEncoding<'a> {
    /// The address payload to encode.
    pub payload: &'a Payload,
    
    /// Base58 version byte for P2PKH payloads (e.g. 0x00 for "1..." addresses).
    pub p2pkh_prefix: u8,
    
    /// Base58 version byte for P2SH payloads (e.g. 0x05 for "3..." addresses).
    pub p2sh_prefix: u8,
    
    /// The bech32 human-readable part for SegWit addresses.
    pub hrp: Hrp,
}

/// Formats bech32 as upper case if alternate formatting is chosen (`{:#}`).
///
/// This implementation handles the encoding of all address types according
/// to their respective standards:
/// - **Legacy addresses**: Base58Check encoding with network prefixes
/// - **SegWit addresses**: Bech32 encoding with case sensitivity support
impl<'a> fmt::Display for AddressEncoding<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.payload {
            Payload::PubkeyHash(hash) => {
                let mut prefixed = [0; 21];
                prefixed[0] = self.p2pkh_prefix;
                prefixed[1..].copy_from_slice(&hash[..]);
                base58::encode_check_to_fmt(fmt, &prefixed[..])
            }
            Payload::ScriptHash(hash) => {
                let mut prefixed = [0; 21];
                prefixed[0] = self.p2sh_prefix;
                prefixed[1..].copy_from_slice(&hash[..]);
                base58::encode_check_to_fmt(fmt, &prefixed[..])
            }
            Payload::WitnessProgram(witness_program) => {
                let hrp = self.hrp.clone();
                let version = witness_program.version().to_fe();
                let program = witness_program.program().as_bytes();

                if fmt.alternate() {
                    bech32::segwit::encode_upper_to_fmt_unchecked(fmt, hrp, version, program)
                } else {
                    bech32::segwit::encode_lower_to_fmt_unchecked(fmt, hrp, version, program)
                }
            }
        }
    }
}
