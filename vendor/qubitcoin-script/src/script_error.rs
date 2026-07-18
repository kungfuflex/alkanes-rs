//! Script execution error types.
//!
//! Maps to: `src/script/script_error.h` in Bitcoin Core.

/// All possible script verification errors.
///
/// Port of Bitcoin Core's `ScriptError` enum. Discriminant values match 1:1
/// with the C++ implementation for cross-validation compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ScriptError {
    /// No error -- script evaluated successfully.
    Ok = 0,
    /// An unspecified or internal error occurred.
    UnknownError = 1,
    /// Script finished with a false/empty value on top of the stack.
    EvalFalse = 2,
    /// `OP_RETURN` was encountered during execution.
    OpReturn = 3,

    // -- Size / count limits --

    /// A script number exceeded the allowed byte length.
    ScriptNum = 4,
    /// The script exceeds the maximum allowed size.
    ScriptSize = 5,
    /// A data push exceeds `MAX_SCRIPT_ELEMENT_SIZE` (520 bytes).
    PushSize = 6,
    /// The number of non-push operations exceeded `MAX_OPS_PER_SCRIPT`.
    OpCount = 7,
    /// The combined stack + altstack size exceeded `MAX_STACK_SIZE`.
    StackSize = 8,
    /// Signature count is negative or exceeds the public key count.
    SigCount = 9,
    /// Public key count is negative or exceeds the limit.
    PubkeyCount = 10,

    // -- Verify operation failures --

    /// An `OP_VERIFY` operation failed (top of stack was false).
    Verify = 11,
    /// An `OP_EQUALVERIFY` operation failed.
    EqualVerify = 12,
    /// An `OP_CHECKMULTISIGVERIFY` operation failed.
    CheckMultiSigVerify = 13,
    /// An `OP_CHECKSIGVERIFY` operation failed.
    CheckSigVerify = 14,
    /// An `OP_NUMEQUALVERIFY` operation failed.
    NumEqualVerify = 15,

    // -- Logical / format errors --

    /// The opcode is missing, undefined, or not understood.
    BadOpcode = 16,
    /// A disabled opcode was encountered.
    DisabledOpcode = 17,
    /// A stack operation was attempted with insufficient stack depth.
    InvalidStackOperation = 18,
    /// An altstack operation was attempted with an empty altstack.
    InvalidAltstackOperation = 19,
    /// Unbalanced `OP_IF` / `OP_ELSE` / `OP_ENDIF` construction.
    UnbalancedConditional = 20,

    // -- `OP_CHECKLOCKTIMEVERIFY` / `OP_CHECKSEQUENCEVERIFY` --

    /// The locktime argument is negative.
    NegativeLocktime = 21,
    /// The locktime requirement was not satisfied by the transaction.
    UnsatisfiedLocktime = 22,

    // -- Malleability (BIP 62 / BIP 66 / BIP 146) --

    /// Unrecognized or undefined signature hash type.
    SigHashtype = 23,
    /// Signature is not valid strict-DER encoding.
    SigDer = 24,
    /// A data push used a larger encoding than necessary.
    MinimalData = 25,
    /// `scriptSig` contains non-push operations (violates `SIGPUSHONLY`).
    SigPushOnly = 26,
    /// The S value in a DER signature is not in the lower half of the curve order.
    SigHighS = 27,
    /// The dummy element for `OP_CHECKMULTISIG` is not the empty byte vector.
    SigNullDummy = 28,
    /// A public key is neither compressed nor uncompressed format.
    PubKeyType = 29,
    /// More than one element remains on the stack after execution.
    CleanStack = 30,
    /// `OP_IF`/`OP_NOTIF` argument is not minimal (must be exactly `0x01` or empty).
    MinimalIf = 31,
    /// A failing `CHECK(MULTI)SIG` left a non-empty signature on the stack.
    SigNullFail = 32,

    // -- Soft-fork safeness --

    /// An upgradable `NOP` opcode was used when `DISCOURAGE_UPGRADABLE_NOPS` is set.
    DiscourageUpgradableNops = 33,
    /// An upgradable witness program version was used.
    DiscourageUpgradableWitnessProgram = 34,
    /// An upgradable Taproot leaf version was used.
    DiscourageUpgradableTaprootVersion = 35,
    /// An `OP_SUCCESS` opcode was encountered in tapscript.
    DiscourageOpSuccess = 36,
    /// An unknown public key type was used in tapscript.
    DiscourageUpgradablePubkeyType = 37,

    // -- Segregated witness (BIP 141) --

    /// The witness program has an incorrect length.
    WitnessProgramWrongLength = 38,
    /// A witness program was provided an empty witness stack.
    WitnessProgramWitnessEmpty = 39,
    /// The witness program hash does not match the witness script.
    WitnessProgramMismatch = 40,
    /// A witness output requires an empty `scriptSig`.
    WitnessMalleated = 41,
    /// A P2SH-wrapped witness requires `scriptSig` to be only the redeem script push.
    WitnessMalleatedP2sh = 42,
    /// Witness data was provided for a non-witness script.
    WitnessUnexpected = 43,
    /// A public key used in segwit v0 is not compressed.
    WitnessPubKeyType = 44,

    // -- Taproot / Tapscript (BIP 341 / BIP 342) --

    /// A Schnorr signature has an invalid size (must be 64 or 65 bytes).
    SchnorrSigSize = 45,
    /// A Schnorr signature has an invalid hash type byte.
    SchnorrSigHashtype = 46,
    /// A Schnorr signature failed verification.
    SchnorrSig = 47,
    /// The Taproot control block has an invalid size.
    TaprootWrongControlSize = 48,
    /// The tapscript validation weight budget has been exceeded.
    TapscriptValidationWeight = 49,
    /// `OP_CHECKMULTISIG`/`OP_CHECKMULTISIGVERIFY` is not available in tapscript.
    TapscriptCheckMultiSig = 50,
    /// `OP_IF`/`OP_NOTIF` argument must be exactly `0x01` or empty in tapscript.
    TapscriptMinimalIf = 51,
    /// A public key in tapscript must not be empty.
    TapscriptEmptyPubkey = 52,

    // -- Additional --

    /// `OP_CODESEPARATOR` used in a non-witness script (when `CONST_SCRIPTCODE` is set).
    OpCodeSeparator = 53,
    /// A signature was found via `FindAndDelete` in the `scriptCode`.
    SigFindAndDelete = 54,

    /// Sentinel value equal to the total number of error codes.
    ErrorCount = 55,
}

impl ScriptError {
    /// Returns a human-readable description of this error.
    ///
    /// The strings match Bitcoin Core's `ScriptErrorString()`.
    pub fn description(&self) -> &'static str {
        match self {
            ScriptError::Ok => "No error",
            ScriptError::UnknownError => "Unknown error",
            ScriptError::EvalFalse => {
                "Script evaluated without error but finished with a false/empty top stack element"
            }
            ScriptError::OpReturn => "OP_RETURN was encountered",
            ScriptError::ScriptNum => "Script number overflow",
            ScriptError::ScriptSize => "Script is too big",
            ScriptError::PushSize => "Push value size limit exceeded",
            ScriptError::OpCount => "Operation limit exceeded",
            ScriptError::StackSize => "Stack size limit exceeded",
            ScriptError::SigCount => "Signature count negative or greater than pubkey count",
            ScriptError::PubkeyCount => "Pubkey count negative or limit exceeded",
            ScriptError::Verify => "Script failed an OP_VERIFY operation",
            ScriptError::EqualVerify => "Script failed an OP_EQUALVERIFY operation",
            ScriptError::CheckMultiSigVerify => "Script failed an OP_CHECKMULTISIGVERIFY operation",
            ScriptError::CheckSigVerify => "Script failed an OP_CHECKSIGVERIFY operation",
            ScriptError::NumEqualVerify => "Script failed an OP_NUMEQUALVERIFY operation",
            ScriptError::BadOpcode => "Opcode missing or not understood",
            ScriptError::DisabledOpcode => "Attempted to use a disabled opcode",
            ScriptError::InvalidStackOperation => "Operation not valid with the current stack size",
            ScriptError::InvalidAltstackOperation => {
                "Operation not valid with the current altstack size"
            }
            ScriptError::UnbalancedConditional => "Invalid OP_IF construction",
            ScriptError::NegativeLocktime => "Negative locktime",
            ScriptError::UnsatisfiedLocktime => "Locktime requirement not satisfied",
            ScriptError::SigHashtype => "Signature hash type missing or not understood",
            ScriptError::SigDer => "Non-canonical DER signature",
            ScriptError::MinimalData => "Data push larger than necessary",
            ScriptError::SigPushOnly => "Only push operators allowed in signatures",
            ScriptError::SigHighS => "Non-canonical signature: S value is unnecessarily high",
            ScriptError::SigNullDummy => "Dummy CHECKMULTISIG argument must be zero",
            ScriptError::PubKeyType => "Public key is neither compressed or uncompressed",
            ScriptError::CleanStack => "Stack size must be exactly one after execution",
            ScriptError::MinimalIf => "OP_IF/NOTIF argument must be minimal",
            ScriptError::SigNullFail => {
                "Signature must be zero for failed CHECK(MULTI)SIG operation"
            }
            ScriptError::DiscourageUpgradableNops => "NOPx reserved for soft-fork upgrades",
            ScriptError::DiscourageUpgradableWitnessProgram => {
                "Witness version reserved for soft-fork upgrades"
            }
            ScriptError::DiscourageUpgradableTaprootVersion => {
                "Taproot version reserved for soft-fork upgrades"
            }
            ScriptError::DiscourageOpSuccess => "OP_SUCCESSx reserved for soft-fork upgrades",
            ScriptError::DiscourageUpgradablePubkeyType => {
                "Public key version reserved for soft-fork upgrades"
            }
            ScriptError::WitnessProgramWrongLength => "Witness program has incorrect length",
            ScriptError::WitnessProgramWitnessEmpty => {
                "Witness program was passed an empty witness"
            }
            ScriptError::WitnessProgramMismatch => "Witness program hash mismatch",
            ScriptError::WitnessMalleated => "Witness requires empty scriptSig",
            ScriptError::WitnessMalleatedP2sh => "Witness requires only-redeemscript scriptSig",
            ScriptError::WitnessUnexpected => "Witness provided for non-witness script",
            ScriptError::WitnessPubKeyType => "Using non-compressed keys in segwit",
            ScriptError::SchnorrSigSize => "Invalid Schnorr signature size",
            ScriptError::SchnorrSigHashtype => "Invalid Schnorr signature hash type",
            ScriptError::SchnorrSig => "Invalid Schnorr signature",
            ScriptError::TaprootWrongControlSize => "Invalid control block size",
            ScriptError::TapscriptValidationWeight => {
                "Too much signature validation relative to witness weight"
            }
            ScriptError::TapscriptCheckMultiSig => {
                "OP_CHECKMULTISIG(VERIFY) is not available in tapscript"
            }
            ScriptError::TapscriptMinimalIf => "OP_IF/NOTIF argument must be minimal in tapscript",
            ScriptError::TapscriptEmptyPubkey => "Tapscript public key must not be empty",
            ScriptError::OpCodeSeparator => "Using OP_CODESEPARATOR in non-witness script",
            ScriptError::SigFindAndDelete => "Signature is found in scriptCode",
            ScriptError::ErrorCount => "(error count sentinel)",
        }
    }
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl std::error::Error for ScriptError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_values() {
        assert_eq!(ScriptError::Ok as u8, 0);
        assert_eq!(ScriptError::ErrorCount as u8, 55);
    }
}
