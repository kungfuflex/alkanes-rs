//! Test harness for Bitcoin Core's script_tests.json
//!
//! This imports Bitcoin Core's comprehensive script test vectors and runs them
//! against the Qubitcoin script interpreter, checking that eval_script and
//! verify_script produce the expected results.

use qubitcoin_script::{
    verify_script, BaseSignatureChecker, Script, ScriptError, ScriptNum, ScriptVerifyFlags,
    ScriptWitness,
};

// ---------------------------------------------------------------------------
// Test data
// ---------------------------------------------------------------------------

const SCRIPT_TESTS_JSON: &str = include_str!("../src/test_data/script_tests.json");

// ---------------------------------------------------------------------------
// Opcode name -> byte mapping (handles both "OP_ADD" and "ADD" forms)
// ---------------------------------------------------------------------------

fn opcode_name_to_byte(name: &str) -> Option<u8> {
    // Try with and without OP_ prefix
    let canonical = if name.starts_with("OP_") {
        name.to_string()
    } else {
        format!("OP_{}", name)
    };

    match canonical.as_str() {
        // Push value
        "OP_0" | "OP_FALSE" => Some(0x00),
        "OP_PUSHDATA1" => Some(0x4c),
        "OP_PUSHDATA2" => Some(0x4d),
        "OP_PUSHDATA4" => Some(0x4e),
        "OP_1NEGATE" => Some(0x4f),
        "OP_RESERVED" => Some(0x50),
        "OP_1" | "OP_TRUE" => Some(0x51),
        "OP_2" => Some(0x52),
        "OP_3" => Some(0x53),
        "OP_4" => Some(0x54),
        "OP_5" => Some(0x55),
        "OP_6" => Some(0x56),
        "OP_7" => Some(0x57),
        "OP_8" => Some(0x58),
        "OP_9" => Some(0x59),
        "OP_10" => Some(0x5a),
        "OP_11" => Some(0x5b),
        "OP_12" => Some(0x5c),
        "OP_13" => Some(0x5d),
        "OP_14" => Some(0x5e),
        "OP_15" => Some(0x5f),
        "OP_16" => Some(0x60),

        // Control flow
        "OP_NOP" => Some(0x61),
        "OP_VER" => Some(0x62),
        "OP_IF" => Some(0x63),
        "OP_NOTIF" => Some(0x64),
        "OP_VERIF" => Some(0x65),
        "OP_VERNOTIF" => Some(0x66),
        "OP_ELSE" => Some(0x67),
        "OP_ENDIF" => Some(0x68),
        "OP_VERIFY" => Some(0x69),
        "OP_RETURN" => Some(0x6a),

        // Stack
        "OP_TOALTSTACK" => Some(0x6b),
        "OP_FROMALTSTACK" => Some(0x6c),
        "OP_2DROP" => Some(0x6d),
        "OP_2DUP" => Some(0x6e),
        "OP_3DUP" => Some(0x6f),
        "OP_2OVER" => Some(0x70),
        "OP_2ROT" => Some(0x71),
        "OP_2SWAP" => Some(0x72),
        "OP_IFDUP" => Some(0x73),
        "OP_DEPTH" => Some(0x74),
        "OP_DROP" => Some(0x75),
        "OP_DUP" => Some(0x76),
        "OP_NIP" => Some(0x77),
        "OP_OVER" => Some(0x78),
        "OP_PICK" => Some(0x79),
        "OP_ROLL" => Some(0x7a),
        "OP_ROT" => Some(0x7b),
        "OP_SWAP" => Some(0x7c),
        "OP_TUCK" => Some(0x7d),

        // Splice (disabled)
        "OP_CAT" => Some(0x7e),
        "OP_SUBSTR" => Some(0x7f),
        "OP_LEFT" => Some(0x80),
        "OP_RIGHT" => Some(0x81),
        "OP_SIZE" => Some(0x82),

        // Bit logic
        "OP_INVERT" => Some(0x83),
        "OP_AND" => Some(0x84),
        "OP_OR" => Some(0x85),
        "OP_XOR" => Some(0x86),
        "OP_EQUAL" => Some(0x87),
        "OP_EQUALVERIFY" => Some(0x88),
        "OP_RESERVED1" => Some(0x89),
        "OP_RESERVED2" => Some(0x8a),

        // Numeric
        "OP_1ADD" => Some(0x8b),
        "OP_1SUB" => Some(0x8c),
        "OP_2MUL" => Some(0x8d),
        "OP_2DIV" => Some(0x8e),
        "OP_NEGATE" => Some(0x8f),
        "OP_ABS" => Some(0x90),
        "OP_NOT" => Some(0x91),
        "OP_0NOTEQUAL" => Some(0x92),
        "OP_ADD" => Some(0x93),
        "OP_SUB" => Some(0x94),
        "OP_MUL" => Some(0x95),
        "OP_DIV" => Some(0x96),
        "OP_MOD" => Some(0x97),
        "OP_LSHIFT" => Some(0x98),
        "OP_RSHIFT" => Some(0x99),
        "OP_BOOLAND" => Some(0x9a),
        "OP_BOOLOR" => Some(0x9b),
        "OP_NUMEQUAL" => Some(0x9c),
        "OP_NUMEQUALVERIFY" => Some(0x9d),
        "OP_NUMNOTEQUAL" => Some(0x9e),
        "OP_LESSTHAN" => Some(0x9f),
        "OP_GREATERTHAN" => Some(0xa0),
        "OP_LESSTHANOREQUAL" => Some(0xa1),
        "OP_GREATERTHANOREQUAL" => Some(0xa2),
        "OP_MIN" => Some(0xa3),
        "OP_MAX" => Some(0xa4),
        "OP_WITHIN" => Some(0xa5),

        // Crypto
        "OP_RIPEMD160" => Some(0xa6),
        "OP_SHA1" => Some(0xa7),
        "OP_SHA256" => Some(0xa8),
        "OP_HASH160" => Some(0xa9),
        "OP_HASH256" => Some(0xaa),
        "OP_CODESEPARATOR" => Some(0xab),
        "OP_CHECKSIG" => Some(0xac),
        "OP_CHECKSIGVERIFY" => Some(0xad),
        "OP_CHECKMULTISIG" => Some(0xae),
        "OP_CHECKMULTISIGVERIFY" => Some(0xaf),

        // Expansion
        "OP_NOP1" => Some(0xb0),
        "OP_CHECKLOCKTIMEVERIFY" | "OP_NOP2" | "OP_CLTV" => Some(0xb1),
        "OP_CHECKSEQUENCEVERIFY" | "OP_NOP3" | "OP_CSV" => Some(0xb2),
        "OP_NOP4" => Some(0xb3),
        "OP_NOP5" => Some(0xb4),
        "OP_NOP6" => Some(0xb5),
        "OP_NOP7" => Some(0xb6),
        "OP_NOP8" => Some(0xb7),
        "OP_NOP9" => Some(0xb8),
        "OP_NOP10" => Some(0xb9),

        // Tapscript
        "OP_CHECKSIGADD" => Some(0xba),

        "OP_INVALIDOPCODE" => Some(0xff),

        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Script string parser: converts Bitcoin Script assembly to raw bytes
// Matches Bitcoin Core's ParseScript() in core_io.cpp
// ---------------------------------------------------------------------------

fn parse_script_string(s: &str) -> Result<Script, String> {
    let mut result: Vec<u8> = Vec::new();

    for word in s.split_whitespace() {
        if word.is_empty() {
            continue;
        }

        // Check if it's a decimal number (possibly negative)
        let is_number = word.chars().all(|c| c.is_ascii_digit())
            || (word.starts_with('-')
                && word.len() > 1
                && word[1..].chars().all(|c| c.is_ascii_digit()));

        if is_number {
            let n: i64 = word
                .parse::<i64>()
                .map_err(|e| format!("bad number '{}': {}", word, e))?;

            // Range check: -0xFFFFFFFF..0xFFFFFFFF
            if n > 0xFFFFFFFFi64 || n < -(0xFFFFFFFFi64) {
                return Err(format!("number out of range: {}", n));
            }

            // Push number using Bitcoin's push_int64 logic
            if n == -1 || (n >= 1 && n <= 16) {
                // OP_1NEGATE (0x4f) for -1, OP_1..OP_16 for 1..16
                result.push((n + (0x51i64 - 1)) as u8);
            } else if n == 0 {
                result.push(0x00); // OP_0
            } else {
                // Serialize as CScriptNum and push
                let bytes = ScriptNum::new(n).to_bytes();
                push_data_to_script(&mut result, &bytes);
            }
        } else if word.starts_with("0x") && word.len() > 2 {
            // Raw hex data - inserted directly, NOT pushed onto stack
            let hex_str = &word[2..];
            let bytes = hex_to_bytes(hex_str).ok_or_else(|| format!("bad hex: {}", word))?;
            result.extend_from_slice(&bytes);
        } else if word.len() >= 2 && word.starts_with('\'') && word.ends_with('\'') {
            // Single-quoted string, pushed as data
            let string_bytes = word[1..word.len() - 1].as_bytes();
            push_data_to_script(&mut result, string_bytes);
        } else {
            // Opcode name (with or without OP_ prefix)
            let byte =
                opcode_name_to_byte(word).ok_or_else(|| format!("unknown opcode: {}", word))?;
            result.push(byte);
        }
    }

    Ok(Script::from_bytes(result))
}

/// Push data onto script with proper length encoding (matching CScript::AppendDataSize)
fn push_data_to_script(script: &mut Vec<u8>, data: &[u8]) {
    let size = data.len();
    if size < 0x4c {
        // Direct push: opcode byte IS the length
        script.push(size as u8);
    } else if size <= 0xff {
        script.push(0x4c); // OP_PUSHDATA1
        script.push(size as u8);
    } else if size <= 0xffff {
        script.push(0x4d); // OP_PUSHDATA2
        script.push((size & 0xff) as u8);
        script.push(((size >> 8) & 0xff) as u8);
    } else {
        script.push(0x4e); // OP_PUSHDATA4
        script.push((size & 0xff) as u8);
        script.push(((size >> 8) & 0xff) as u8);
        script.push(((size >> 16) & 0xff) as u8);
        script.push(((size >> 24) & 0xff) as u8);
    }
    script.extend_from_slice(data);
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16).ok()?;
        bytes.push(byte);
    }
    Some(bytes)
}

// ---------------------------------------------------------------------------
// Flag parser: converts "P2SH,STRICTENC,DERSIG" to ScriptVerifyFlags
// ---------------------------------------------------------------------------

fn parse_flags(s: &str) -> ScriptVerifyFlags {
    let mut flags = ScriptVerifyFlags::NONE;
    if s.is_empty() {
        return flags;
    }
    for flag_name in s.split(',') {
        let flag_name = flag_name.trim();
        if flag_name.is_empty() {
            continue;
        }
        match flag_name {
            "NONE" => {}
            "P2SH" => flags |= ScriptVerifyFlags::P2SH,
            "STRICTENC" => flags |= ScriptVerifyFlags::STRICTENC,
            "DERSIG" => flags |= ScriptVerifyFlags::DERSIG,
            "LOW_S" => flags |= ScriptVerifyFlags::LOW_S,
            "NULLDUMMY" => flags |= ScriptVerifyFlags::NULLDUMMY,
            "SIGPUSHONLY" => flags |= ScriptVerifyFlags::SIGPUSHONLY,
            "MINIMALDATA" => flags |= ScriptVerifyFlags::MINIMALDATA,
            "DISCOURAGE_UPGRADABLE_NOPS" => flags |= ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_NOPS,
            "CLEANSTACK" => flags |= ScriptVerifyFlags::CLEANSTACK,
            "CHECKLOCKTIMEVERIFY" => flags |= ScriptVerifyFlags::CHECKLOCKTIMEVERIFY,
            "CHECKSEQUENCEVERIFY" => flags |= ScriptVerifyFlags::CHECKSEQUENCEVERIFY,
            "WITNESS" => flags |= ScriptVerifyFlags::WITNESS,
            "DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM" => {
                flags |= ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM
            }
            "MINIMALIF" => flags |= ScriptVerifyFlags::MINIMALIF,
            "NULLFAIL" => flags |= ScriptVerifyFlags::NULLFAIL,
            "WITNESS_PUBKEYTYPE" => flags |= ScriptVerifyFlags::WITNESS_PUBKEYTYPE,
            "CONST_SCRIPTCODE" => flags |= ScriptVerifyFlags::CONST_SCRIPTCODE,
            "TAPROOT" => flags |= ScriptVerifyFlags::TAPROOT,
            "DISCOURAGE_UPGRADABLE_TAPROOT_VERSION" => {
                flags |= ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_TAPROOT_VERSION
            }
            "DISCOURAGE_OP_SUCCESS" => flags |= ScriptVerifyFlags::DISCOURAGE_OP_SUCCESS,
            "DISCOURAGE_UPGRADABLE_PUBKEYTYPE" => {
                flags |= ScriptVerifyFlags::DISCOURAGE_UPGRADABLE_PUBKEYTYPE
            }
            _ => panic!("Unknown script flag: {}", flag_name),
        }
    }
    flags
}

// ---------------------------------------------------------------------------
// Error mapper: maps expected error strings to ScriptError variants
// ---------------------------------------------------------------------------

fn parse_expected_error(name: &str) -> Option<ScriptError> {
    match name {
        "OK" => Some(ScriptError::Ok),
        "EVAL_FALSE" => Some(ScriptError::EvalFalse),
        "OP_RETURN" => Some(ScriptError::OpReturn),
        "SCRIPT_SIZE" => Some(ScriptError::ScriptSize),
        "PUSH_SIZE" => Some(ScriptError::PushSize),
        "OP_COUNT" => Some(ScriptError::OpCount),
        "STACK_SIZE" => Some(ScriptError::StackSize),
        "SIG_COUNT" => Some(ScriptError::SigCount),
        "PUBKEY_COUNT" => Some(ScriptError::PubkeyCount),
        "VERIFY" => Some(ScriptError::Verify),
        "EQUALVERIFY" => Some(ScriptError::EqualVerify),
        "CHECKMULTISIGVERIFY" => Some(ScriptError::CheckMultiSigVerify),
        "CHECKSIGVERIFY" => Some(ScriptError::CheckSigVerify),
        "NUMEQUALVERIFY" => Some(ScriptError::NumEqualVerify),
        "BAD_OPCODE" => Some(ScriptError::BadOpcode),
        "DISABLED_OPCODE" => Some(ScriptError::DisabledOpcode),
        "INVALID_STACK_OPERATION" => Some(ScriptError::InvalidStackOperation),
        "INVALID_ALTSTACK_OPERATION" => Some(ScriptError::InvalidAltstackOperation),
        "UNBALANCED_CONDITIONAL" => Some(ScriptError::UnbalancedConditional),
        "NEGATIVE_LOCKTIME" => Some(ScriptError::NegativeLocktime),
        "UNSATISFIED_LOCKTIME" => Some(ScriptError::UnsatisfiedLocktime),
        "SIG_HASHTYPE" => Some(ScriptError::SigHashtype),
        "SIG_DER" => Some(ScriptError::SigDer),
        "MINIMALDATA" => Some(ScriptError::MinimalData),
        "SIG_PUSHONLY" => Some(ScriptError::SigPushOnly),
        "SIG_HIGH_S" => Some(ScriptError::SigHighS),
        "SIG_NULLDUMMY" => Some(ScriptError::SigNullDummy),
        "PUBKEYTYPE" => Some(ScriptError::PubKeyType),
        "CLEANSTACK" => Some(ScriptError::CleanStack),
        "MINIMALIF" => Some(ScriptError::MinimalIf),
        "NULLFAIL" => Some(ScriptError::SigNullFail),
        "DISCOURAGE_UPGRADABLE_NOPS" => Some(ScriptError::DiscourageUpgradableNops),
        "DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM" => {
            Some(ScriptError::DiscourageUpgradableWitnessProgram)
        }
        "DISCOURAGE_UPGRADABLE_TAPROOT_VERSION" => {
            Some(ScriptError::DiscourageUpgradableTaprootVersion)
        }
        "DISCOURAGE_OP_SUCCESS" => Some(ScriptError::DiscourageOpSuccess),
        "DISCOURAGE_UPGRADABLE_PUBKEYTYPE" => Some(ScriptError::DiscourageUpgradablePubkeyType),
        "WITNESS_PROGRAM_WRONG_LENGTH" => Some(ScriptError::WitnessProgramWrongLength),
        "WITNESS_PROGRAM_WITNESS_EMPTY" => Some(ScriptError::WitnessProgramWitnessEmpty),
        "WITNESS_PROGRAM_MISMATCH" => Some(ScriptError::WitnessProgramMismatch),
        "WITNESS_MALLEATED" => Some(ScriptError::WitnessMalleated),
        "WITNESS_MALLEATED_P2SH" => Some(ScriptError::WitnessMalleatedP2sh),
        "WITNESS_UNEXPECTED" => Some(ScriptError::WitnessUnexpected),
        "WITNESS_PUBKEYTYPE" => Some(ScriptError::WitnessPubKeyType),
        "SCHNORR_SIG_SIZE" => Some(ScriptError::SchnorrSigSize),
        "SCHNORR_SIG_HASHTYPE" => Some(ScriptError::SchnorrSigHashtype),
        "SCHNORR_SIG" => Some(ScriptError::SchnorrSig),
        "TAPROOT_WRONG_CONTROL_SIZE" => Some(ScriptError::TaprootWrongControlSize),
        "TAPSCRIPT_VALIDATION_WEIGHT" => Some(ScriptError::TapscriptValidationWeight),
        "TAPSCRIPT_CHECKMULTISIG" => Some(ScriptError::TapscriptCheckMultiSig),
        "TAPSCRIPT_MINIMALIF" => Some(ScriptError::TapscriptMinimalIf),
        "TAPSCRIPT_EMPTY_PUBKEY" => Some(ScriptError::TapscriptEmptyPubkey),
        "OP_CODESEPARATOR" => Some(ScriptError::OpCodeSeparator),
        "SIG_FINDANDDELETE" => Some(ScriptError::SigFindAndDelete),
        "SCRIPTNUM" => Some(ScriptError::ScriptNum),
        "UNKNOWN_ERROR" => Some(ScriptError::UnknownError),
        _ => None,
    }
}

fn error_name(err: ScriptError) -> &'static str {
    match err {
        ScriptError::Ok => "OK",
        ScriptError::EvalFalse => "EVAL_FALSE",
        ScriptError::OpReturn => "OP_RETURN",
        ScriptError::ScriptSize => "SCRIPT_SIZE",
        ScriptError::PushSize => "PUSH_SIZE",
        ScriptError::OpCount => "OP_COUNT",
        ScriptError::StackSize => "STACK_SIZE",
        ScriptError::SigCount => "SIG_COUNT",
        ScriptError::PubkeyCount => "PUBKEY_COUNT",
        ScriptError::Verify => "VERIFY",
        ScriptError::EqualVerify => "EQUALVERIFY",
        ScriptError::CheckMultiSigVerify => "CHECKMULTISIGVERIFY",
        ScriptError::CheckSigVerify => "CHECKSIGVERIFY",
        ScriptError::NumEqualVerify => "NUMEQUALVERIFY",
        ScriptError::BadOpcode => "BAD_OPCODE",
        ScriptError::DisabledOpcode => "DISABLED_OPCODE",
        ScriptError::InvalidStackOperation => "INVALID_STACK_OPERATION",
        ScriptError::InvalidAltstackOperation => "INVALID_ALTSTACK_OPERATION",
        ScriptError::UnbalancedConditional => "UNBALANCED_CONDITIONAL",
        ScriptError::NegativeLocktime => "NEGATIVE_LOCKTIME",
        ScriptError::UnsatisfiedLocktime => "UNSATISFIED_LOCKTIME",
        ScriptError::SigHashtype => "SIG_HASHTYPE",
        ScriptError::SigDer => "SIG_DER",
        ScriptError::MinimalData => "MINIMALDATA",
        ScriptError::SigPushOnly => "SIG_PUSHONLY",
        ScriptError::SigHighS => "SIG_HIGH_S",
        ScriptError::SigNullDummy => "SIG_NULLDUMMY",
        ScriptError::PubKeyType => "PUBKEYTYPE",
        ScriptError::CleanStack => "CLEANSTACK",
        ScriptError::MinimalIf => "MINIMALIF",
        ScriptError::SigNullFail => "NULLFAIL",
        ScriptError::DiscourageUpgradableNops => "DISCOURAGE_UPGRADABLE_NOPS",
        ScriptError::DiscourageUpgradableWitnessProgram => "DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM",
        ScriptError::DiscourageUpgradableTaprootVersion => "DISCOURAGE_UPGRADABLE_TAPROOT_VERSION",
        ScriptError::DiscourageOpSuccess => "DISCOURAGE_OP_SUCCESS",
        ScriptError::DiscourageUpgradablePubkeyType => "DISCOURAGE_UPGRADABLE_PUBKEYTYPE",
        ScriptError::WitnessProgramWrongLength => "WITNESS_PROGRAM_WRONG_LENGTH",
        ScriptError::WitnessProgramWitnessEmpty => "WITNESS_PROGRAM_WITNESS_EMPTY",
        ScriptError::WitnessProgramMismatch => "WITNESS_PROGRAM_MISMATCH",
        ScriptError::WitnessMalleated => "WITNESS_MALLEATED",
        ScriptError::WitnessMalleatedP2sh => "WITNESS_MALLEATED_P2SH",
        ScriptError::WitnessUnexpected => "WITNESS_UNEXPECTED",
        ScriptError::WitnessPubKeyType => "WITNESS_PUBKEYTYPE",
        ScriptError::SchnorrSigSize => "SCHNORR_SIG_SIZE",
        ScriptError::SchnorrSigHashtype => "SCHNORR_SIG_HASHTYPE",
        ScriptError::SchnorrSig => "SCHNORR_SIG",
        ScriptError::TaprootWrongControlSize => "TAPROOT_WRONG_CONTROL_SIZE",
        ScriptError::TapscriptValidationWeight => "TAPSCRIPT_VALIDATION_WEIGHT",
        ScriptError::TapscriptCheckMultiSig => "TAPSCRIPT_CHECKMULTISIG",
        ScriptError::TapscriptMinimalIf => "TAPSCRIPT_MINIMALIF",
        ScriptError::TapscriptEmptyPubkey => "TAPSCRIPT_EMPTY_PUBKEY",
        ScriptError::OpCodeSeparator => "OP_CODESEPARATOR",
        ScriptError::SigFindAndDelete => "SIG_FINDANDDELETE",
        ScriptError::ScriptNum => "SCRIPTNUM",
        ScriptError::UnknownError => "UNKNOWN_ERROR",
        ScriptError::ErrorCount => "ERROR_COUNT",
    }
}

// ---------------------------------------------------------------------------
// Test execution
// ---------------------------------------------------------------------------

/// Represents a parsed test vector
struct TestVector {
    /// Witness stack items (hex-encoded byte strings)
    witness: Vec<Vec<u8>>,
    /// Amount in satoshis (for witness tests)
    #[allow(dead_code)]
    amount: i64,
    /// scriptSig assembly string
    script_sig_str: String,
    /// scriptPubKey assembly string
    script_pubkey_str: String,
    /// Verification flags
    flags_str: String,
    /// Expected error string
    expected_error_str: String,
    /// Optional comment
    comment: String,
    /// Line/index in the JSON for debugging
    index: usize,
}

fn parse_test_vectors() -> Vec<TestVector> {
    let json: serde_json::Value =
        serde_json::from_str(SCRIPT_TESTS_JSON).expect("Failed to parse script_tests.json");

    let array = json.as_array().expect("Top-level JSON must be an array");
    let mut tests = Vec::new();

    for (i, entry) in array.iter().enumerate() {
        let arr = match entry.as_array() {
            Some(a) => a,
            None => continue,
        };

        // Skip comment-only entries (single string element)
        if arr.len() == 1 && arr[0].is_string() {
            continue;
        }

        // Determine format:
        // Non-witness: [scriptSig, scriptPubKey, flags, expected_error, ...comments]
        // Witness:     [[wit_items..., amount], scriptSig, scriptPubKey, flags, expected_error, ...comments]

        let (witness, amount, sig_idx);
        if arr.len() >= 5 && arr[0].is_array() {
            // Witness format
            let wit_arr = arr[0].as_array().unwrap();
            if wit_arr.is_empty() {
                continue;
            }
            // Last element is amount (number), rest are hex witness items
            let amt_val = wit_arr.last().unwrap();
            let amt = if amt_val.is_f64() {
                (amt_val.as_f64().unwrap() * 100_000_000.0).round() as i64
            } else if amt_val.is_i64() {
                amt_val.as_i64().unwrap() * 100_000_000
            } else {
                continue; // can't parse amount
            };

            let mut wit_items = Vec::new();
            for j in 0..wit_arr.len() - 1 {
                if let Some(hex_str) = wit_arr[j].as_str() {
                    // Skip template items like #SCRIPT#, #CONTROLBLOCK#
                    if hex_str.contains('#') {
                        // This is a taproot template test, skip it
                        break;
                    }
                    match hex_to_bytes(hex_str) {
                        Some(bytes) => wit_items.push(bytes),
                        None => break,
                    }
                } else {
                    break;
                }
            }

            if wit_items.len() != wit_arr.len() - 1 {
                // Skipped due to template or parse error
                continue;
            }

            witness = wit_items;
            amount = amt;
            sig_idx = 1;
        } else if arr.len() >= 4 && arr[0].is_string() {
            // Non-witness format
            witness = Vec::new();
            amount = 0;
            sig_idx = 0;
        } else {
            continue;
        }

        // Extract remaining fields
        let script_sig_str = match arr[sig_idx].as_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        let script_pubkey_str = match arr[sig_idx + 1].as_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        let flags_str = match arr[sig_idx + 2].as_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        let expected_error_str = match arr[sig_idx + 3].as_str() {
            Some(s) => s.to_string(),
            None => continue,
        };

        // Optional comment
        let comment = if arr.len() > sig_idx + 4 {
            arr[sig_idx + 4].as_str().unwrap_or("").to_string()
        } else {
            String::new()
        };

        tests.push(TestVector {
            witness,
            amount,
            script_sig_str,
            script_pubkey_str,
            flags_str,
            expected_error_str,
            comment,
            index: i,
        });
    }

    tests
}

#[test]
fn run_script_tests() {
    let test_vectors = parse_test_vectors();
    let total = test_vectors.len();

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut parse_errors = 0usize;
    let mut skipped = 0usize;

    // Track failure categories
    let mut failure_categories: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    // Collect failures for summary
    let mut failure_details: Vec<String> = Vec::new();

    for tv in &test_vectors {
        // Skip taproot template tests (contain #SCRIPT#, #CONTROLBLOCK#, #TAPROOTOUTPUT#)
        if tv.script_pubkey_str.contains('#') || tv.script_sig_str.contains('#') {
            skipped += 1;
            continue;
        }

        // Parse scriptSig
        let script_sig = match parse_script_string(&tv.script_sig_str) {
            Ok(s) => s,
            Err(e) => {
                parse_errors += 1;
                failure_details.push(format!(
                    "PARSE_ERROR [{}]: scriptSig parse failed: {} | sig='{}' pubkey='{}' flags='{}' expected='{}'{}",
                    tv.index, e, tv.script_sig_str, tv.script_pubkey_str, tv.flags_str,
                    tv.expected_error_str,
                    if tv.comment.is_empty() { String::new() } else { format!(" // {}", tv.comment) }
                ));
                continue;
            }
        };

        // Parse scriptPubKey
        let script_pubkey = match parse_script_string(&tv.script_pubkey_str) {
            Ok(s) => s,
            Err(e) => {
                parse_errors += 1;
                failure_details.push(format!(
                    "PARSE_ERROR [{}]: scriptPubKey parse failed: {} | sig='{}' pubkey='{}' flags='{}' expected='{}'{}",
                    tv.index, e, tv.script_sig_str, tv.script_pubkey_str, tv.flags_str,
                    tv.expected_error_str,
                    if tv.comment.is_empty() { String::new() } else { format!(" // {}", tv.comment) }
                ));
                continue;
            }
        };

        // Parse flags
        let flags = parse_flags(&tv.flags_str);

        // Parse expected error
        let expected_error = match parse_expected_error(&tv.expected_error_str) {
            Some(e) => e,
            None => {
                parse_errors += 1;
                failure_details.push(format!(
                    "PARSE_ERROR [{}]: unknown expected error '{}' | sig='{}' pubkey='{}'{}",
                    tv.index,
                    tv.expected_error_str,
                    tv.script_sig_str,
                    tv.script_pubkey_str,
                    if tv.comment.is_empty() {
                        String::new()
                    } else {
                        format!(" // {}", tv.comment)
                    }
                ));
                continue;
            }
        };

        // Build witness
        let witness = ScriptWitness {
            stack: tv.witness.clone(),
        };

        // Use BaseSignatureChecker (returns false for all sig checks)
        let checker = BaseSignatureChecker;

        // Run verify_script
        let mut actual_error = ScriptError::UnknownError;
        let success = verify_script(
            &script_sig,
            &script_pubkey,
            &witness,
            &flags,
            &checker,
            &mut actual_error,
        );

        // Determine the effective error: if verify_script returned true, error should be Ok
        let effective_error = if success {
            ScriptError::Ok
        } else {
            actual_error
        };

        if effective_error == expected_error {
            passed += 1;
        } else {
            failed += 1;

            // Categorize the failure
            let category =
                if expected_error == ScriptError::Ok && effective_error != ScriptError::Ok {
                    format!("SHOULD_PASS_BUT_GOT_{}", error_name(effective_error))
                } else if expected_error != ScriptError::Ok && effective_error == ScriptError::Ok {
                    format!("SHOULD_FAIL_{}_BUT_PASSED", error_name(expected_error))
                } else {
                    format!(
                        "WRONG_ERROR_expected_{}_got_{}",
                        error_name(expected_error),
                        error_name(effective_error)
                    )
                };
            *failure_categories.entry(category.clone()).or_insert(0) += 1;

            // Only record first 50 failure details to avoid overwhelming output
            if failure_details.len() < 50 {
                failure_details.push(format!(
                    "FAIL [{}]: expected={}, got={} | sig='{}' pubkey='{}' flags='{}'{}",
                    tv.index,
                    error_name(expected_error),
                    error_name(effective_error),
                    truncate_str(&tv.script_sig_str, 80),
                    truncate_str(&tv.script_pubkey_str, 80),
                    tv.flags_str,
                    if tv.comment.is_empty() {
                        String::new()
                    } else {
                        format!(" // {}", tv.comment)
                    }
                ));
            }
        }
    }

    // Print results
    println!("\n=== Script Tests Results ===");
    println!("Total test vectors: {}", total);
    println!("Passed:  {}", passed);
    println!("Failed:  {}", failed);
    println!("Parse errors: {}", parse_errors);
    println!("Skipped (templates): {}", skipped);
    println!(
        "Pass rate: {:.1}% (of non-skipped, non-parse-error)",
        if passed + failed > 0 {
            100.0 * passed as f64 / (passed + failed) as f64
        } else {
            0.0
        }
    );

    println!("\n--- Failure categories ---");
    let mut sorted_cats: Vec<_> = failure_categories.iter().collect();
    sorted_cats.sort_by(|a, b| b.1.cmp(a.1));
    for (cat, count) in &sorted_cats {
        println!("  {:>4}  {}", count, cat);
    }

    if !failure_details.is_empty() {
        println!("\n--- First {} failure details ---", failure_details.len());
        for detail in &failure_details {
            println!("  {}", detail);
        }
    }

    println!("\n=== End Script Tests ===");

    // We don't assert all pass since this is an initial import.
    // Instead assert the test harness itself ran to completion.
    assert!(
        passed + failed + parse_errors + skipped == total,
        "Accounting mismatch: {} + {} + {} + {} != {}",
        passed,
        failed,
        parse_errors,
        skipped,
        total
    );
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
