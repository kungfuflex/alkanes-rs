//! Script test vectors with real signature verification.
//!
//! Re-runs Bitcoin Core's script_tests.json using TransactionSignatureChecker
//! instead of BaseSignatureChecker. This constructs crediting/spending
//! transactions matching Bitcoin Core's BuildCreditingTransaction /
//! BuildSpendingTransaction so that embedded ECDSA signatures verify correctly.

use qubitcoin_consensus::sighash::PrecomputedTransactionData;
use qubitcoin_consensus::transaction::{Transaction, TxIn, TxOut, SEQUENCE_FINAL};
use qubitcoin_consensus::OutPoint;
use qubitcoin_node::script_check::TransactionSignatureChecker;
use qubitcoin_primitives::Amount;
use qubitcoin_script::{
    verify_script, Script, ScriptError, ScriptNum, ScriptVerifyFlags, ScriptWitness,
};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Test data
// ---------------------------------------------------------------------------

const SCRIPT_TESTS_JSON: &str =
    include_str!("../../qubitcoin-script/src/test_data/script_tests.json");

// ---------------------------------------------------------------------------
// BuildCreditingTransaction / BuildSpendingTransaction
// (matches Bitcoin Core's test/util/transaction_utils.cpp exactly)
// ---------------------------------------------------------------------------

/// Create a "crediting" transaction whose output has the given scriptPubKey.
///
/// Maps to Bitcoin Core's BuildCreditingTransaction().
fn build_crediting_transaction(script_pubkey: &Script, value: i64) -> Transaction {
    // CScript() << CScriptNum(0) << CScriptNum(0)
    // CScriptNum(0).getvch() = empty vec, pushed as OP_0 (0x00) by operator<<
    // Result: script = [0x00, 0x00] (OP_0, OP_0)
    let script_sig = Script::from_bytes(vec![0x00, 0x00]);

    Transaction::new(
        1, // version
        vec![TxIn::new(OutPoint::null(), script_sig, SEQUENCE_FINAL)],
        vec![TxOut::new(Amount::from_sat(value), script_pubkey.clone())],
        0, // lock_time
    )
}

/// Create a "spending" transaction that spends the crediting transaction's output.
///
/// Maps to Bitcoin Core's BuildSpendingTransaction().
fn build_spending_transaction(
    script_sig: &Script,
    witness: &ScriptWitness,
    credit_tx: &Transaction,
) -> Transaction {
    let mut input = TxIn::new(
        OutPoint::new(*credit_tx.txid(), 0),
        script_sig.clone(),
        SEQUENCE_FINAL,
    );
    input.witness.stack = witness.stack.clone();

    Transaction::new(
        1, // version
        vec![input],
        vec![TxOut::new(credit_tx.vout[0].value, Script::new())],
        0, // lock_time
    )
}

// ---------------------------------------------------------------------------
// Opcode name -> byte mapping
// ---------------------------------------------------------------------------

fn opcode_name_to_byte(name: &str) -> Option<u8> {
    let canonical = if name.starts_with("OP_") {
        name.to_string()
    } else {
        format!("OP_{}", name)
    };

    match canonical.as_str() {
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
        "OP_CAT" => Some(0x7e),
        "OP_SUBSTR" => Some(0x7f),
        "OP_LEFT" => Some(0x80),
        "OP_RIGHT" => Some(0x81),
        "OP_SIZE" => Some(0x82),
        "OP_INVERT" => Some(0x83),
        "OP_AND" => Some(0x84),
        "OP_OR" => Some(0x85),
        "OP_XOR" => Some(0x86),
        "OP_EQUAL" => Some(0x87),
        "OP_EQUALVERIFY" => Some(0x88),
        "OP_RESERVED1" => Some(0x89),
        "OP_RESERVED2" => Some(0x8a),
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
        "OP_CHECKSIGADD" => Some(0xba),
        "OP_INVALIDOPCODE" => Some(0xff),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Hex / script parsing helpers
// ---------------------------------------------------------------------------

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if hex.is_empty() {
        return Some(Vec::new());
    }
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut result = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        match u8::from_str_radix(&hex[i..i + 2], 16) {
            Ok(byte) => result.push(byte),
            Err(_) => return None,
        }
    }
    Some(result)
}

fn parse_script_string(s: &str) -> Result<Script, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(Script::new());
    }

    let mut result = Vec::new();
    let tokens = tokenize_script(s);

    for token in &tokens {
        if token.starts_with("0x") {
            // Raw hex bytes (inserted directly, not pushed)
            let hex = &token[2..];
            match hex_to_bytes(hex) {
                Some(bytes) => result.extend_from_slice(&bytes),
                None => return Err(format!("Invalid hex: {}", token)),
            }
        } else if token.starts_with('\'') && token.ends_with('\'') && token.len() >= 2 {
            // Quoted string: push as data
            let content = &token[1..token.len() - 1];
            let bytes = content.as_bytes();
            push_data(&mut result, bytes);
        } else if let Ok(num) = token.parse::<i64>() {
            // Decimal number
            if num == -1 {
                result.push(0x4f); // OP_1NEGATE
            } else if num == 0 {
                result.push(0x00); // OP_0
            } else if num >= 1 && num <= 16 {
                result.push(0x50 + num as u8); // OP_1 through OP_16
            } else {
                let encoded = ScriptNum::encode_i64(num);
                push_data(&mut result, &encoded);
            }
        } else if let Some(opcode) = opcode_name_to_byte(token) {
            result.push(opcode);
        } else {
            return Err(format!("Unknown token: '{}'", token));
        }
    }

    Ok(Script::from_bytes(result))
}

fn tokenize_script(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = s.chars().peekable();
    let mut current = String::new();

    while let Some(&ch) = chars.peek() {
        if ch == '\'' {
            // Quoted string
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            let mut quoted = String::new();
            quoted.push(chars.next().unwrap()); // opening quote
            loop {
                match chars.next() {
                    Some('\'') => {
                        quoted.push('\'');
                        break;
                    }
                    Some(c) => quoted.push(c),
                    None => break,
                }
            }
            tokens.push(quoted);
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            chars.next();
        } else {
            current.push(chars.next().unwrap());
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn push_data(result: &mut Vec<u8>, data: &[u8]) {
    let len = data.len();
    if len == 0 {
        result.push(0x00); // OP_0
    } else if len <= 75 {
        result.push(len as u8);
        result.extend_from_slice(data);
    } else if len <= 255 {
        result.push(0x4c); // OP_PUSHDATA1
        result.push(len as u8);
        result.extend_from_slice(data);
    } else if len <= 65535 {
        result.push(0x4d); // OP_PUSHDATA2
        result.extend_from_slice(&(len as u16).to_le_bytes());
        result.extend_from_slice(data);
    } else {
        result.push(0x4e); // OP_PUSHDATA4
        result.extend_from_slice(&(len as u32).to_le_bytes());
        result.extend_from_slice(data);
    }
}

// ---------------------------------------------------------------------------
// Flag parsing
// ---------------------------------------------------------------------------

fn parse_flags(flags_str: &str) -> ScriptVerifyFlags {
    let mut flags = ScriptVerifyFlags::NONE;
    if flags_str.is_empty() {
        return flags;
    }
    for flag in flags_str.split(',') {
        match flag.trim() {
            "NONE" => {}
            "P2SH" => flags |= ScriptVerifyFlags::P2SH,
            "STRICTENC" => flags |= ScriptVerifyFlags::STRICTENC,
            "DERSIG" => flags |= ScriptVerifyFlags::DERSIG,
            "LOW_S" => flags |= ScriptVerifyFlags::LOW_S,
            "SIGPUSHONLY" | "SIGPUSH" => flags |= ScriptVerifyFlags::SIGPUSHONLY,
            "MINIMALDATA" => flags |= ScriptVerifyFlags::MINIMALDATA,
            "NULLDUMMY" => flags |= ScriptVerifyFlags::NULLDUMMY,
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
            _ => {} // Skip unknown flags
        }
    }
    flags
}

// ---------------------------------------------------------------------------
// Error parsing
// ---------------------------------------------------------------------------

fn parse_expected_error(s: &str) -> Option<ScriptError> {
    match s.trim() {
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
        "PUBKEYTYPE" => Some(ScriptError::PubKeyType),
        "CLEANSTACK" => Some(ScriptError::CleanStack),
        "MINIMALIF" => Some(ScriptError::MinimalIf),
        "SIG_NULLFAIL" | "NULLFAIL" => Some(ScriptError::SigNullFail),
        "SIG_NULLDUMMY" => Some(ScriptError::SigNullDummy),
        "SCRIPTNUM" => Some(ScriptError::ScriptNum),
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
        "UNKNOWN_ERROR" => Some(ScriptError::UnknownError),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Test vector parsing
// ---------------------------------------------------------------------------

struct TestVector {
    witness: Vec<Vec<u8>>,
    amount: i64,
    script_sig_str: String,
    script_pubkey_str: String,
    flags_str: String,
    expected_error_str: String,
    comment: String,
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
        if arr.len() == 1 && arr[0].is_string() {
            continue;
        }

        let (witness, amount, sig_idx);
        if arr.len() >= 5 && arr[0].is_array() {
            let wit_arr = arr[0].as_array().unwrap();
            if wit_arr.is_empty() {
                continue;
            }
            let amt_val = wit_arr.last().unwrap();
            let amt = if amt_val.is_f64() {
                (amt_val.as_f64().unwrap() * 100_000_000.0).round() as i64
            } else if amt_val.is_i64() {
                amt_val.as_i64().unwrap() * 100_000_000
            } else {
                continue;
            };

            let mut wit_items = Vec::new();
            for j in 0..wit_arr.len() - 1 {
                if let Some(hex_str) = wit_arr[j].as_str() {
                    if hex_str.contains('#') {
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
                continue;
            }
            witness = wit_items;
            amount = amt;
            sig_idx = 1;
        } else if arr.len() >= 4 && arr[0].is_string() {
            witness = Vec::new();
            amount = 0;
            sig_idx = 0;
        } else {
            continue;
        }

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

// ---------------------------------------------------------------------------
// Main test
// ---------------------------------------------------------------------------

#[test]
fn run_script_tests_with_real_signatures() {
    let test_vectors = parse_test_vectors();
    let total = test_vectors.len();

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failure_details: Vec<String> = Vec::new();

    for tv in &test_vectors {
        if tv.script_pubkey_str.contains('#') || tv.script_sig_str.contains('#') {
            continue;
        }

        let script_sig = match parse_script_string(&tv.script_sig_str) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let script_pubkey = match parse_script_string(&tv.script_pubkey_str) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let flags = parse_flags(&tv.flags_str);
        let expected_error = match parse_expected_error(&tv.expected_error_str) {
            Some(e) => e,
            None => continue,
        };

        // CLEANSTACK implies P2SH and WITNESS (matching Bitcoin Core)
        let flags = if flags.contains(ScriptVerifyFlags::CLEANSTACK) {
            flags | ScriptVerifyFlags::P2SH | ScriptVerifyFlags::WITNESS
        } else {
            flags
        };

        let witness = ScriptWitness {
            stack: tv.witness.clone(),
        };

        // Build crediting/spending transactions so the sighash is deterministic
        let value = if tv.amount != 0 { tv.amount } else { 0 };
        let credit_tx = build_crediting_transaction(&script_pubkey, value);
        let spend_tx = build_spending_transaction(&script_sig, &witness, &credit_tx);
        let spend_tx = Arc::new(spend_tx);

        let spent_outputs = vec![credit_tx.vout[0].clone()];
        let precomputed = PrecomputedTransactionData::new(&spend_tx, &spent_outputs);
        let checker =
            TransactionSignatureChecker::new(Arc::clone(&spend_tx), 0, value, precomputed);

        let mut actual_error = ScriptError::UnknownError;
        let success = verify_script(
            &script_sig,
            &script_pubkey,
            &witness,
            &flags,
            &checker,
            &mut actual_error,
        );

        let effective_error = if success {
            ScriptError::Ok
        } else {
            actual_error
        };

        if effective_error == expected_error {
            passed += 1;
        } else {
            failed += 1;
            if failure_details.len() < 20 {
                failure_details.push(format!(
                    "[{}] expected {:?} got {:?} | sig='{}' pubkey='{}' flags='{}'{}",
                    tv.index,
                    expected_error,
                    effective_error,
                    tv.script_sig_str,
                    tv.script_pubkey_str,
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

    let executed = passed + failed;
    let skipped = total - executed;

    // Print summary
    eprintln!("\n=== Script Test Vectors (with real signatures) ===");
    eprintln!(
        "Total: {}, Executed: {}, Passed: {}, Failed: {}, Skipped: {}",
        total, executed, passed, failed, skipped
    );
    if !failure_details.is_empty() {
        eprintln!("\nFirst {} failures:", failure_details.len());
        for detail in &failure_details {
            eprintln!("  {}", detail);
        }
    }

    // All executed tests must pass (0 failures allowed).
    assert_eq!(
        failed, 0,
        "{} test(s) failed out of {} executed",
        failed, executed
    );
    // Sanity: we should execute at least 1000 tests.
    assert!(
        executed >= 1000,
        "Only {} tests executed (expected >= 1000). Too many skipped.",
        executed
    );
}
