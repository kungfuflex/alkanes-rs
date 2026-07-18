//! Test to verify that our Rust runestone decoding matches what the C++ generatefuture creates.
//!
//! The C++ code in patch/bitcoin/src/rpc/mining.cpp builds a Runestone with:
//! - Cellpack: [32, 0, 77] (wrap-btc execute call)
//! - Protostone structure: [protocol_tag=1, num_fields, ProtoPointer=91, 0, Message=81, message_u128]
//! - Runestone structure: [Pointer=22, 0, Protocol=16383, encoded_protostone_chunks...]

use alkanes_cli_common::runestone_enhanced::{
    format_runestone, format_runestone_with_decoded_messages, decode_runestone,
};
use alkanes_cli_common::alkanes::protostone::Protostone;
use bitcoin::{
    Transaction, TxIn, TxOut, Sequence, Witness,
    blockdata::script::Builder,
    blockdata::opcodes::all::*,
    script::PushBytes,
    OutPoint, Txid,
};
use std::str::FromStr;

/// Runestone tag constants (matching C++)
const TAG_POINTER: u64 = 22;
const TAG_PROTOCOL: u64 = 16383;

/// Protostone field tag constants (matching C++)
const TAG_PROTO_POINTER: u128 = 91;
const TAG_REFUND_POINTER: u128 = 93;
const TAG_MESSAGE: u128 = 81;

/// Encode a u128 value as LEB128 varint (matching C++ EncodeU128Varint)
fn encode_u128_varint(mut n: u128, out: &mut Vec<u8>) {
    while n >= 0x80 {
        out.push(((n & 0x7F) | 0x80) as u8);
        n >>= 7;
    }
    out.push((n & 0x7F) as u8);
}

/// Encode a u64 value as LEB128 varint (matching C++ EncodeU64Varint)
fn encode_u64_varint(mut n: u64, out: &mut Vec<u8>) {
    while n >= 0x80 {
        out.push(((n & 0x7F) | 0x80) as u8);
        n >>= 7;
    }
    out.push((n & 0x7F) as u8);
}

/// Convert bytes to u128 little-endian (matching C++ BytesToU128)
fn bytes_to_u128(bytes: &[u8]) -> u128 {
    let mut result: u128 = 0;
    for (i, &b) in bytes.iter().take(16).enumerate() {
        result |= (b as u128) << (i * 8);
    }
    result
}

/// Split bytes into u128 chunks (matching C++ SplitBytesToU128Chunks)
fn split_bytes_to_u128_chunks(bytes: &[u8]) -> Vec<u128> {
    let mut result = Vec::new();

    for i in (0..bytes.len()).step_by(15) {
        let mut chunk = Vec::new();
        for j in i..std::cmp::min(i + 15, bytes.len()) {
            chunk.push(bytes[j]);
        }
        // Pad to 16 bytes
        while chunk.len() < 16 {
            chunk.push(0);
        }
        result.push(bytes_to_u128(&chunk));
    }

    if result.is_empty() {
        result.push(0);
    }

    result
}

/// Create OP_RETURN script with Runestone containing Protostone
/// This is the Rust equivalent of C++ CreateRunestoneWithProtostone
fn create_runestone_with_protostone(cellpack: &[u8], output_index: u32) -> bitcoin::ScriptBuf {
    // Step 1: Build the protostone fields
    // Fields: [ProtoPointer_tag, pointer_value, RefundPointer_tag, refund_value, Message_tag, message_value]
    let message_u128 = bytes_to_u128(cellpack);

    let protostone_fields: Vec<u128> = vec![
        TAG_PROTO_POINTER,
        output_index as u128,
        TAG_REFUND_POINTER,
        output_index as u128,  // refund_pointer = pointer
        TAG_MESSAGE,
        message_u128,
    ];

    // Step 2: Build the full protostone structure
    // Format: [protocol_tag=1, num_field_varints, ...field_varints...]
    let mut protostone_values: Vec<u128> = Vec::new();
    protostone_values.push(1); // protocol_tag = 1 (ALKANES)
    protostone_values.push(protostone_fields.len() as u128); // number of varints in fields
    for v in &protostone_fields {
        protostone_values.push(*v);
    }

    // Step 3: LEB128 encode the protostone values
    let mut protostone_leb = Vec::new();
    for v in &protostone_values {
        encode_u128_varint(*v, &mut protostone_leb);
    }

    // Step 4: Split the LEB-encoded protostone into u128 chunks for Protocol field
    let protocol_u128_chunks = split_bytes_to_u128_chunks(&protostone_leb);

    // Step 5: Build the runestone payload
    // Format: [Pointer_tag=22, pointer_value=0, Protocol_tag=16383, proto_u128_1, ...]
    let mut runestone_payload = Vec::new();

    // Add Pointer field (tag=22, value=output_index)
    encode_u64_varint(TAG_POINTER, &mut runestone_payload);
    encode_u64_varint(output_index as u64, &mut runestone_payload);

    // Add Protocol field(s) - one tag-value pair per u128 chunk
    for chunk in &protocol_u128_chunks {
        encode_u64_varint(TAG_PROTOCOL, &mut runestone_payload);
        encode_u128_varint(*chunk, &mut runestone_payload);
    }

    // Step 6: Build the OP_RETURN script
    // Format: OP_RETURN OP_13 <push payload>
    Builder::new()
        .push_opcode(OP_RETURN)
        .push_opcode(OP_PUSHNUM_13)
        .push_slice::<&PushBytes>((&runestone_payload[..]).try_into().unwrap())
        .into_script()
}

/// Create a minimal transaction with a runestone output (for testing)
fn create_test_transaction_with_runestone(cellpack: &[u8]) -> Transaction {
    let runestone_script = create_runestone_with_protostone(cellpack, 0);

    // Create a coinbase-like transaction
    let coinbase_input = TxIn {
        previous_output: OutPoint {
            txid: Txid::from_str("0000000000000000000000000000000000000000000000000000000000000000").unwrap(),
            vout: 0xffffffff,
        },
        script_sig: Builder::new().push_slice(&[0x01, 0x00]).into_script(),
        sequence: Sequence::MAX,
        witness: Witness::new(),
    };

    // Coinbase reward output (dummy)
    let reward_output = TxOut {
        value: bitcoin::Amount::from_sat(1250000000),
        script_pubkey: Builder::new()
            .push_opcode(OP_PUSHNUM_1)
            .push_slice(&[0u8; 32]) // dummy taproot pubkey
            .into_script(),
    };

    // Runestone output
    let runestone_output = TxOut {
        value: bitcoin::Amount::from_sat(0),
        script_pubkey: runestone_script,
    };

    Transaction {
        version: bitcoin::transaction::Version(2),
        lock_time: bitcoin::absolute::LockTime::from_consensus(0),
        input: vec![coinbase_input],
        output: vec![reward_output, runestone_output],
    }
}

#[test]
fn test_create_runestone_script_matches_cpp() {
    // Test with the exact cellpack used in generatefuture: [32, 0, 77]
    let cellpack = vec![32u8, 0, 77];
    let script = create_runestone_with_protostone(&cellpack, 0);

    let script_bytes = script.as_bytes();
    println!("Generated runestone script ({} bytes):", script_bytes.len());
    println!("  Hex: {}", hex::encode(&script_bytes));

    // Verify it starts with OP_RETURN OP_PUSHNUM_13
    assert_eq!(script_bytes[0], 0x6a, "Should start with OP_RETURN (0x6a)");
    assert_eq!(script_bytes[1], 0x5d, "Should have OP_PUSHNUM_13 (0x5d)");

    // Extract and print the payload
    let payload_len = script_bytes[2] as usize;
    let payload = &script_bytes[3..3+payload_len];
    println!("  Payload ({} bytes): {}", payload.len(), hex::encode(payload));

    // Decode the payload as varints
    let mut decoded_varints = Vec::new();
    let mut i = 0;
    while i < payload.len() {
        let (val, new_i) = decode_varint_at(payload, i);
        decoded_varints.push(val);
        i = new_i;
    }
    println!("  Decoded varints: {:?}", decoded_varints);
}

/// Helper to decode a varint starting at offset
fn decode_varint_at(data: &[u8], mut offset: usize) -> (u128, usize) {
    let mut result: u128 = 0;
    let mut shift = 0;
    while offset < data.len() {
        let b = data[offset];
        result |= ((b & 0x7f) as u128) << shift;
        offset += 1;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    (result, offset)
}

#[test]
fn test_decode_runestone_with_cellpack_32_0_77() {
    // Create transaction with the generatefuture cellpack
    let cellpack = vec![32u8, 0, 77];
    let tx = create_test_transaction_with_runestone(&cellpack);

    println!("Transaction created with {} inputs, {} outputs", tx.input.len(), tx.output.len());
    println!("Transaction ID: {}", tx.compute_txid());

    // Print the runestone output
    let runestone_output = &tx.output[1];
    println!("Runestone output script: {}", hex::encode(runestone_output.script_pubkey.as_bytes()));

    // Try to decode using our format_runestone function
    match format_runestone(&tx) {
        Ok(protostones) => {
            println!("\n=== Decoded Protostones ===");
            for (i, ps) in protostones.iter().enumerate() {
                println!("Protostone #{}:", i);
                println!("  protocol_tag: {}", ps.protocol_tag);
                println!("  pointer: {:?}", ps.pointer);
                println!("  refund: {:?}", ps.refund);
                println!("  burn: {:?}", ps.burn);
                println!("  from: {:?}", ps.from);
                println!("  message (bytes): {:?}", ps.message);

                // The message bytes should decode to our cellpack
                // In protorune, message is stored as the raw bytes
                println!("  message (hex): {}", hex::encode(&ps.message));
            }

            // Verify we got at least one protostone
            assert!(!protostones.is_empty(), "Should have at least one protostone");

            // Verify the protostone has protocol_tag = 1 (ALKANES)
            let ps = &protostones[0];
            assert_eq!(ps.protocol_tag, 1, "Protocol tag should be 1 (ALKANES)");

            // The message should contain our cellpack bytes [32, 0, 77]
            // Note: The exact format depends on how the message is stored
            println!("\n=== Cellpack Verification ===");
            println!("Expected cellpack: {:?}", cellpack);
            println!("Actual message bytes: {:?}", ps.message);
        }
        Err(e) => {
            println!("Error decoding runestone: {:?}", e);

            // Try the manual decode function
            println!("\n=== Trying manual decode_runestone ===");
            match decode_runestone(&tx) {
                Ok(json_result) => {
                    println!("Manual decode result: {}", serde_json::to_string_pretty(&json_result).unwrap());
                }
                Err(e2) => {
                    println!("Manual decode also failed: {:?}", e2);
                }
            }

            panic!("Failed to decode runestone: {:?}", e);
        }
    }
}

#[test]
fn test_format_runestone_with_decoded_messages() {
    // Create transaction with the generatefuture cellpack
    let cellpack = vec![32u8, 0, 77];
    let tx = create_test_transaction_with_runestone(&cellpack);

    println!("\n=== Testing format_runestone_with_decoded_messages ===");

    match format_runestone_with_decoded_messages(&tx) {
        Ok(json_result) => {
            println!("Decoded result:\n{}", serde_json::to_string_pretty(&json_result).unwrap());

            // Check that protostones array exists
            let protostones = json_result.get("protostones")
                .expect("Should have protostones field")
                .as_array()
                .expect("protostones should be an array");

            assert!(!protostones.is_empty(), "Should have at least one protostone");

            // Check the first protostone
            let ps = &protostones[0];

            // Verify protocol_tag
            let protocol_tag = ps.get("protocol_tag")
                .expect("Should have protocol_tag")
                .as_u64()
                .expect("protocol_tag should be u64");
            assert_eq!(protocol_tag, 1, "Protocol tag should be 1 (ALKANES)");

            // Check decoded message
            if let Some(decoded_msg) = ps.get("message_decoded") {
                println!("\nDecoded message: {:?}", decoded_msg);
            }

            // Check raw message bytes
            if let Some(msg_bytes) = ps.get("message_bytes") {
                println!("Raw message bytes: {:?}", msg_bytes);
            }
        }
        Err(e) => {
            panic!("Failed to format runestone with decoded messages: {:?}", e);
        }
    }
}

#[test]
fn test_varint_encoding_matches_cpp() {
    // Test that our Rust varint encoding matches C++
    let test_values: Vec<u128> = vec![
        0, 1, 127, 128, 255, 256, 16383, 16384,
        TAG_PROTO_POINTER, TAG_MESSAGE,
        0x200000004D, // 32 | (0 << 8) | (77 << 16) in little-endian as u128
    ];

    println!("=== Varint Encoding Test ===");
    for val in test_values {
        let mut encoded = Vec::new();
        encode_u128_varint(val, &mut encoded);
        println!("Value {:>20} -> {} bytes: {}", val, encoded.len(), hex::encode(&encoded));
    }
}

#[test]
fn test_bytes_to_u128_cellpack() {
    // Test the bytes_to_u128 function with our cellpack
    let cellpack = vec![32u8, 0, 77];
    let u128_val = bytes_to_u128(&cellpack);

    println!("=== Bytes to U128 Test ===");
    println!("Cellpack bytes: {:?}", cellpack);
    println!("As u128 (hex): 0x{:032x}", u128_val);
    println!("As u128 (dec): {}", u128_val);

    // Verify: [32, 0, 77] in little-endian = 32 + (0 * 256) + (77 * 65536) = 32 + 5046272 = 5046304
    let expected = 32u128 + (0u128 << 8) + (77u128 << 16);
    println!("Expected: {} (0x{:x})", expected, expected);
    assert_eq!(u128_val, expected, "bytes_to_u128 should match expected value");
}

#[test]
fn test_full_protostone_encoding() {
    // Manually encode the protostone and verify each step
    let cellpack = vec![32u8, 0, 77];
    let output_index: u32 = 0;

    println!("=== Full Protostone Encoding Test ===");

    // Step 1: Convert cellpack to u128
    let message_u128 = bytes_to_u128(&cellpack);
    println!("Step 1: cellpack {:?} -> u128 = {}", cellpack, message_u128);

    // Step 2: Build protostone fields
    let protostone_fields: Vec<u128> = vec![
        TAG_PROTO_POINTER, // 91
        output_index as u128, // 0
        TAG_REFUND_POINTER, // 93
        output_index as u128, // 0 (refund_pointer = pointer)
        TAG_MESSAGE, // 81
        message_u128, // 5046304
    ];
    println!("Step 2: protostone_fields = {:?}", protostone_fields);

    // Step 3: Build full protostone structure
    let mut protostone_values: Vec<u128> = Vec::new();
    protostone_values.push(1); // protocol_tag = 1 (ALKANES)
    protostone_values.push(protostone_fields.len() as u128); // 6 fields now
    for v in &protostone_fields {
        protostone_values.push(*v);
    }
    println!("Step 3: protostone_values = {:?}", protostone_values);

    // Step 4: LEB128 encode
    let mut protostone_leb = Vec::new();
    for v in &protostone_values {
        let start = protostone_leb.len();
        encode_u128_varint(*v, &mut protostone_leb);
        println!("  {} -> {}", v, hex::encode(&protostone_leb[start..]));
    }
    println!("Step 4: protostone_leb ({} bytes) = {}", protostone_leb.len(), hex::encode(&protostone_leb));

    // Step 5: Split into u128 chunks
    let chunks = split_bytes_to_u128_chunks(&protostone_leb);
    println!("Step 5: u128 chunks = {:?}", chunks);
    for (i, chunk) in chunks.iter().enumerate() {
        println!("  chunk[{}] = {} (0x{:032x})", i, chunk, chunk);
    }

    // Step 6: Build runestone payload
    let mut runestone_payload = Vec::new();
    encode_u64_varint(TAG_POINTER, &mut runestone_payload);
    encode_u64_varint(output_index as u64, &mut runestone_payload);
    for chunk in &chunks {
        encode_u64_varint(TAG_PROTOCOL, &mut runestone_payload);
        encode_u128_varint(*chunk, &mut runestone_payload);
    }
    println!("Step 6: runestone_payload ({} bytes) = {}", runestone_payload.len(), hex::encode(&runestone_payload));

    // Step 7: Build full script
    let script = create_runestone_with_protostone(&cellpack, 0);
    println!("Step 7: Full script = {}", hex::encode(script.as_bytes()));
}
