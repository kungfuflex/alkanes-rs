//! Script verification flags.
//!
//! Maps to: `src/script/interpreter.h` (`SCRIPT_VERIFY_*` constants) in Bitcoin Core.

bitflags::bitflags! {
    /// Script verification flags controlling which rules are enforced.
    ///
    /// Port of Bitcoin Core's SCRIPT_VERIFY_* flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ScriptVerifyFlags: u32 {
        /// No flags.
        const NONE = 0;

        /// Evaluate P2SH subscripts (BIP16).
        const P2SH = 1 << 0;

        /// Enforce strict DER signature encoding and undefined hashtype fails.
        const STRICTENC = 1 << 1;

        /// Enforce strict DER signature encoding (BIP62 rule 1, BIP66).
        const DERSIG = 1 << 2;

        /// Enforce low S values in signatures (BIP62 rule 5, BIP146).
        const LOW_S = 1 << 3;

        /// Verify dummy stack item consumed by CHECKMULTISIG is zero-length (BIP62 rule 7, BIP147).
        const NULLDUMMY = 1 << 4;

        /// Using a non-push operator in scriptSig causes failure (BIP62 rule 2).
        const SIGPUSHONLY = 1 << 5;

        /// Require minimal encodings for all push operations (BIP62 rule 3 & 4).
        const MINIMALDATA = 1 << 6;

        /// Discourage use of NOPs reserved for upgrades (NOP1-10).
        const DISCOURAGE_UPGRADABLE_NOPS = 1 << 7;

        /// Require only a single stack element after evaluation (BIP62 rule 6).
        const CLEANSTACK = 1 << 8;

        /// Verify CHECKLOCKTIMEVERIFY (BIP65).
        const CHECKLOCKTIMEVERIFY = 1 << 9;

        /// Support CHECKSEQUENCEVERIFY (BIP112).
        const CHECKSEQUENCEVERIFY = 1 << 10;

        /// Support segregated witness (BIP141, BIP143, BIP147).
        const WITNESS = 1 << 11;

        /// Making v1-v16 witness programs non-standard.
        const DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM = 1 << 12;

        /// Segwit script only: require OP_IF/NOTIF argument to be exactly 0x01 or empty.
        const MINIMALIF = 1 << 13;

        /// Signature(s) must be empty if CHECK(MULTI)SIG fails (BIP146).
        const NULLFAIL = 1 << 14;

        /// Public keys in segwit scripts must be compressed.
        const WITNESS_PUBKEYTYPE = 1 << 15;

        /// Making OP_CODESEPARATOR and FindAndDelete fail in non-segwit scripts.
        const CONST_SCRIPTCODE = 1 << 16;

        /// Taproot/Tapscript validation (BIPs 341 & 342).
        const TAPROOT = 1 << 17;

        /// Making unknown Taproot leaf versions non-standard.
        const DISCOURAGE_UPGRADABLE_TAPROOT_VERSION = 1 << 18;

        /// Making unknown OP_SUCCESS non-standard.
        const DISCOURAGE_OP_SUCCESS = 1 << 19;

        /// Making unknown public key versions non-standard (BIP342).
        const DISCOURAGE_UPGRADABLE_PUBKEYTYPE = 1 << 20;
    }
}

/// Mandatory script verification flags that all new blocks must comply with.
///
/// Combination: `P2SH | DERSIG | NULLDUMMY | CHECKLOCKTIMEVERIFY |
/// CHECKSEQUENCEVERIFY | WITNESS | TAPROOT`.
/// Matches Bitcoin Core's `MANDATORY_SCRIPT_VERIFY_FLAGS`.
pub const MANDATORY_SCRIPT_VERIFY_FLAGS: ScriptVerifyFlags = ScriptVerifyFlags::P2SH
    .union(ScriptVerifyFlags::DERSIG)
    .union(ScriptVerifyFlags::NULLDUMMY)
    .union(ScriptVerifyFlags::CHECKLOCKTIMEVERIFY)
    .union(ScriptVerifyFlags::CHECKSEQUENCEVERIFY)
    .union(ScriptVerifyFlags::WITNESS)
    .union(ScriptVerifyFlags::TAPROOT);

/// Standard script verification flags used for mempool relay policy.
///
/// Includes all [`MANDATORY_SCRIPT_VERIFY_FLAGS`] plus additional malleability
/// and soft-fork safeness flags. Matches Bitcoin Core's
/// `STANDARD_SCRIPT_VERIFY_FLAGS`.
pub const STANDARD_SCRIPT_VERIFY_FLAGS: ScriptVerifyFlags = MANDATORY_SCRIPT_VERIFY_FLAGS
    .union(ScriptVerifyFlags::STRICTENC)
    .union(ScriptVerifyFlags::MINIMALDATA)
    .union(ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_NOPS)
    .union(ScriptVerifyFlags::CLEANSTACK)
    .union(ScriptVerifyFlags::MINIMALIF)
    .union(ScriptVerifyFlags::NULLFAIL)
    .union(ScriptVerifyFlags::LOW_S)
    .union(ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM)
    .union(ScriptVerifyFlags::WITNESS_PUBKEYTYPE)
    .union(ScriptVerifyFlags::CONST_SCRIPTCODE)
    .union(ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_TAPROOT_VERSION)
    .union(ScriptVerifyFlags::DISCOURAGE_OP_SUCCESS)
    .union(ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_PUBKEYTYPE);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag_values() {
        assert_eq!(ScriptVerifyFlags::P2SH.bits(), 1);
        assert_eq!(ScriptVerifyFlags::WITNESS.bits(), 0x800);
        assert_eq!(ScriptVerifyFlags::TAPROOT.bits(), 0x20000);
    }

    #[test]
    fn test_flag_combinations() {
        let flags = ScriptVerifyFlags::P2SH | ScriptVerifyFlags::WITNESS;
        assert!(flags.contains(ScriptVerifyFlags::P2SH));
        assert!(flags.contains(ScriptVerifyFlags::WITNESS));
        assert!(!flags.contains(ScriptVerifyFlags::TAPROOT));
    }
}
