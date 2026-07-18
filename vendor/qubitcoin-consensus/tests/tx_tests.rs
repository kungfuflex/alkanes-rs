//! Integration tests using Bitcoin Core's tx_valid.json and tx_invalid.json test vectors.
//!
//! These test vectors exercise:
//! - Transaction deserialization (including segwit/witness format)
//! - Context-free transaction validation via `check_transaction()`
//!
//! The test vectors come from Bitcoin Core's `src/test/data/` directory.
//! Format: each entry is either a comment string (ignored) or an array:
//!   [ [[prevout_hash, prevout_index, scriptPubKey, amount?], ...], serialized_tx_hex, verify_flags ]
//!
//! For tx_valid.json:
//!   - Every transaction must deserialize successfully
//!   - Every transaction must pass CheckTransaction() (context-free validation)
//!   - The verify_flags indicate which script verification flags are EXCLUDED
//!     (script verification is not tested here, as EvalScript is not yet ported)
//!
//! For tx_invalid.json:
//!   - Transactions should still deserialize (they are well-formed binary)
//!   - If verify_flags == "BADTX", the transaction must FAIL CheckTransaction()
//!   - Otherwise, CheckTransaction() should pass (the invalidity is at the script level)

use qubitcoin_consensus::check::check_transaction;
use qubitcoin_consensus::transaction::deserialize_transaction;
use qubitcoin_consensus::validation_state::TxValidationState;

/// Parse a hex string into bytes, matching Bitcoin Core's ParseHex().
fn parse_hex(hex_str: &str) -> Vec<u8> {
    let s = hex_str.trim();
    if s.len() % 2 != 0 {
        return Vec::new();
    }
    (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Load and parse test vectors from a JSON file.
/// Returns a Vec of test entries, where each entry is (test_array, raw_json_string).
fn load_test_vectors(json_str: &str) -> Vec<serde_json::Value> {
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).expect("Failed to parse test vector JSON");
    parsed
        .as_array()
        .expect("Top-level JSON should be an array")
        .clone()
}

/// Check if a JSON value is a test case (array starting with another array)
/// as opposed to a comment (array of strings).
fn is_test_case(entry: &serde_json::Value) -> bool {
    if let Some(arr) = entry.as_array() {
        if arr.is_empty() {
            return false;
        }
        // A test case has its first element as an array (the prevouts)
        arr[0].is_array()
    } else {
        false
    }
}

/// Extract the serialized transaction hex and verify flags from a test entry.
/// Returns (tx_hex, verify_flags) or None if the entry is malformed.
fn extract_test_data(entry: &serde_json::Value) -> Option<(String, String)> {
    let arr = entry.as_array()?;
    if arr.len() != 3 {
        return None;
    }
    let tx_hex = arr[1].as_str()?.to_string();
    let flags = arr[2].as_str()?.to_string();
    Some((tx_hex, flags))
}

#[test]
fn test_tx_valid_deserialization() {
    let json_str = include_str!("../src/test_data/tx_valid.json");
    let tests = load_test_vectors(json_str);

    let mut test_count = 0;
    let mut deser_pass = 0;
    let mut deser_fail = 0;
    let mut deser_failures: Vec<String> = Vec::new();

    for (idx, entry) in tests.iter().enumerate() {
        if !is_test_case(entry) {
            continue;
        }

        let (tx_hex, _flags) = match extract_test_data(entry) {
            Some(data) => data,
            None => {
                eprintln!("tx_valid[{}]: malformed test entry, skipping", idx);
                continue;
            }
        };

        test_count += 1;
        let tx_bytes = parse_hex(&tx_hex);

        // Deserialize with witness support (TX_WITH_WITNESS equivalent)
        match deserialize_transaction(&mut &tx_bytes[..], true) {
            Ok(_tx) => {
                deser_pass += 1;
            }
            Err(e) => {
                deser_fail += 1;
                let desc = format!("tx_valid[{}]: deserialization failed: {}", idx, e);
                deser_failures.push(desc);
            }
        }
    }

    eprintln!(
        "\n=== tx_valid deserialization: {}/{} passed, {} failed ===",
        deser_pass, test_count, deser_fail
    );
    for f in &deser_failures {
        eprintln!("  FAIL: {}", f);
    }

    // All valid transactions must deserialize
    assert_eq!(
        deser_fail, 0,
        "All tx_valid transactions must deserialize successfully. {} failures.",
        deser_fail
    );
}

#[test]
fn test_tx_valid_check_transaction() {
    let json_str = include_str!("../src/test_data/tx_valid.json");
    let tests = load_test_vectors(json_str);

    let mut test_count = 0;
    let mut check_pass = 0;
    let mut check_fail = 0;
    let mut check_failures: Vec<String> = Vec::new();

    for (idx, entry) in tests.iter().enumerate() {
        if !is_test_case(entry) {
            continue;
        }

        let (tx_hex, _flags) = match extract_test_data(entry) {
            Some(data) => data,
            None => continue,
        };

        let tx_bytes = parse_hex(&tx_hex);
        let tx = match deserialize_transaction(&mut &tx_bytes[..], true) {
            Ok(tx) => tx,
            Err(_) => continue, // Skip deserialization failures (tested separately)
        };

        test_count += 1;

        let mut state = TxValidationState::new();
        if check_transaction(&tx, &mut state) {
            check_pass += 1;
        } else {
            check_fail += 1;
            let desc = format!(
                "tx_valid[{}]: CheckTransaction failed: {} ({})",
                idx,
                state.get_reject_reason(),
                state.get_debug_message()
            );
            check_failures.push(desc);
        }
    }

    eprintln!(
        "\n=== tx_valid CheckTransaction: {}/{} passed, {} failed ===",
        check_pass, test_count, check_fail
    );
    for f in &check_failures {
        eprintln!("  FAIL: {}", f);
    }

    // All valid transactions must pass CheckTransaction
    assert_eq!(
        check_fail, 0,
        "All tx_valid transactions must pass CheckTransaction(). {} failures.",
        check_fail
    );
}

#[test]
fn test_tx_invalid_deserialization() {
    let json_str = include_str!("../src/test_data/tx_invalid.json");
    let tests = load_test_vectors(json_str);

    let mut test_count = 0;
    let mut deser_pass = 0;
    let mut deser_fail = 0;

    for (idx, entry) in tests.iter().enumerate() {
        if !is_test_case(entry) {
            continue;
        }

        let (tx_hex, flags) = match extract_test_data(entry) {
            Some(data) => data,
            None => {
                eprintln!("tx_invalid[{}]: malformed test entry, skipping", idx);
                continue;
            }
        };

        test_count += 1;
        let tx_bytes = parse_hex(&tx_hex);

        match deserialize_transaction(&mut &tx_bytes[..], true) {
            Ok(_tx) => {
                deser_pass += 1;
            }
            Err(e) => {
                deser_fail += 1;
                // Deserialization failure is acceptable for invalid txs, but
                // typically the tx_invalid entries are structurally valid
                // (they fail at CheckTransaction or script verification level).
                eprintln!(
                    "tx_invalid[{}]: deserialization failed (flags={}): {}",
                    idx, flags, e
                );
            }
        }
    }

    eprintln!(
        "\n=== tx_invalid deserialization: {}/{} deserialized, {} failed ===",
        deser_pass, test_count, deser_fail
    );

    // Most invalid transactions should still deserialize. A few might have
    // truly malformed binary, which is acceptable. We just log them.
}

#[test]
fn test_tx_invalid_badtx_check_transaction() {
    // Tests that transactions marked with "BADTX" flag fail CheckTransaction().
    // These have structural problems like no outputs, negative values, overflow, etc.
    let json_str = include_str!("../src/test_data/tx_invalid.json");
    let tests = load_test_vectors(json_str);

    let mut badtx_count = 0;
    let mut badtx_correctly_rejected = 0;
    let mut badtx_incorrectly_accepted = 0;
    let mut accept_failures: Vec<String> = Vec::new();

    for (idx, entry) in tests.iter().enumerate() {
        if !is_test_case(entry) {
            continue;
        }

        let (tx_hex, flags) = match extract_test_data(entry) {
            Some(data) => data,
            None => continue,
        };

        // Only test entries with BADTX flag
        if flags != "BADTX" {
            continue;
        }

        let tx_bytes = parse_hex(&tx_hex);
        let tx = match deserialize_transaction(&mut &tx_bytes[..], true) {
            Ok(tx) => tx,
            Err(_) => {
                // Deserialization failure counts as a successful rejection
                badtx_count += 1;
                badtx_correctly_rejected += 1;
                continue;
            }
        };

        badtx_count += 1;

        let mut state = TxValidationState::new();
        if check_transaction(&tx, &mut state) {
            // This transaction should NOT have passed CheckTransaction
            badtx_incorrectly_accepted += 1;
            let desc = format!(
                "tx_invalid[{}] (BADTX): CheckTransaction should have failed but passed",
                idx
            );
            accept_failures.push(desc);
        } else {
            badtx_correctly_rejected += 1;
            eprintln!(
                "tx_invalid[{}] (BADTX): correctly rejected with: {}",
                idx,
                state.get_reject_reason()
            );
        }
    }

    eprintln!(
        "\n=== tx_invalid BADTX: {}/{} correctly rejected, {} incorrectly accepted ===",
        badtx_correctly_rejected, badtx_count, badtx_incorrectly_accepted
    );
    for f in &accept_failures {
        eprintln!("  FAIL: {}", f);
    }

    // All BADTX transactions must fail CheckTransaction
    assert_eq!(
        badtx_incorrectly_accepted, 0,
        "All BADTX transactions must fail CheckTransaction(). {} incorrectly accepted.",
        badtx_incorrectly_accepted
    );
}

#[test]
fn test_tx_invalid_non_badtx_passes_check_transaction() {
    // Tests that non-BADTX invalid transactions still pass CheckTransaction().
    // These are invalid at the script verification level, not at the structural level.
    let json_str = include_str!("../src/test_data/tx_invalid.json");
    let tests = load_test_vectors(json_str);

    let mut non_badtx_count = 0;
    let mut check_pass = 0;
    let mut check_fail = 0;
    let mut deser_fail = 0;
    let mut check_failures: Vec<String> = Vec::new();

    for (idx, entry) in tests.iter().enumerate() {
        if !is_test_case(entry) {
            continue;
        }

        let (tx_hex, flags) = match extract_test_data(entry) {
            Some(data) => data,
            None => continue,
        };

        // Skip BADTX entries (tested separately)
        if flags == "BADTX" {
            continue;
        }

        let tx_bytes = parse_hex(&tx_hex);
        let tx = match deserialize_transaction(&mut &tx_bytes[..], true) {
            Ok(tx) => tx,
            Err(_) => {
                deser_fail += 1;
                continue;
            }
        };

        non_badtx_count += 1;

        let mut state = TxValidationState::new();
        if check_transaction(&tx, &mut state) {
            check_pass += 1;
        } else {
            // For non-BADTX entries, CheckTransaction SHOULD pass
            // (the invalidity is in the script verification, not structure)
            check_fail += 1;
            let desc = format!(
                "tx_invalid[{}] (flags={}): CheckTransaction unexpectedly failed: {}",
                idx,
                flags,
                state.get_reject_reason()
            );
            check_failures.push(desc);
        }
    }

    eprintln!(
        "\n=== tx_invalid non-BADTX CheckTransaction: {}/{} passed, {} failed, {} deser_fail ===",
        check_pass, non_badtx_count, check_fail, deser_fail
    );
    for f in &check_failures {
        eprintln!("  FAIL: {}", f);
    }

    // Non-BADTX invalid transactions should pass CheckTransaction
    // (they only fail at script verification level)
    assert_eq!(
        check_fail, 0,
        "Non-BADTX invalid transactions should pass CheckTransaction(). {} failures.",
        check_fail
    );
}

#[test]
fn test_tx_valid_roundtrip_serialization() {
    // Tests that valid transactions can be serialized and deserialized back
    // to produce the same transaction (hash stability).
    let json_str = include_str!("../src/test_data/tx_valid.json");
    let tests = load_test_vectors(json_str);

    let mut test_count = 0;
    let mut roundtrip_pass = 0;
    let mut roundtrip_fail = 0;
    let mut failures: Vec<String> = Vec::new();

    for (idx, entry) in tests.iter().enumerate() {
        if !is_test_case(entry) {
            continue;
        }

        let (tx_hex, _flags) = match extract_test_data(entry) {
            Some(data) => data,
            None => continue,
        };

        let tx_bytes = parse_hex(&tx_hex);
        let tx = match deserialize_transaction(&mut &tx_bytes[..], true) {
            Ok(tx) => tx,
            Err(_) => continue,
        };

        test_count += 1;

        // Re-serialize with witness
        let reserialized = qubitcoin_consensus::transaction::serialize_transaction(&tx, true);

        // The re-serialized bytes should match the original
        if reserialized == tx_bytes {
            roundtrip_pass += 1;
        } else {
            roundtrip_fail += 1;
            let desc = format!(
                "tx_valid[{}]: roundtrip mismatch: orig_len={}, reser_len={}, txid={:?}",
                idx,
                tx_bytes.len(),
                reserialized.len(),
                tx.txid()
            );
            failures.push(desc);
        }
    }

    eprintln!(
        "\n=== tx_valid roundtrip: {}/{} passed, {} failed ===",
        roundtrip_pass, test_count, roundtrip_fail
    );
    for f in &failures {
        eprintln!("  FAIL: {}", f);
    }

    // All valid transactions should survive roundtrip serialization
    assert_eq!(
        roundtrip_fail, 0,
        "All tx_valid transactions must roundtrip serialize correctly. {} failures.",
        roundtrip_fail
    );
}

#[test]
fn test_tx_vector_counts() {
    // Sanity check: make sure we're loading a reasonable number of test vectors.
    let valid_json = include_str!("../src/test_data/tx_valid.json");
    let invalid_json = include_str!("../src/test_data/tx_invalid.json");

    let valid_tests = load_test_vectors(valid_json);
    let invalid_tests = load_test_vectors(invalid_json);

    let valid_cases: usize = valid_tests.iter().filter(|e| is_test_case(e)).count();
    let invalid_cases: usize = invalid_tests.iter().filter(|e| is_test_case(e)).count();

    eprintln!(
        "tx_valid.json: {} total entries, {} test cases",
        valid_tests.len(),
        valid_cases
    );
    eprintln!(
        "tx_invalid.json: {} total entries, {} test cases",
        invalid_tests.len(),
        invalid_cases
    );

    // There should be a significant number of test cases
    assert!(
        valid_cases >= 50,
        "Expected at least 50 valid test cases, got {}",
        valid_cases
    );
    assert!(
        invalid_cases >= 20,
        "Expected at least 20 invalid test cases, got {}",
        invalid_cases
    );
}
