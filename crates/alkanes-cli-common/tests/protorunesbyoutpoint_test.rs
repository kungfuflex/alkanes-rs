use alkanes_cli_common::proto::protorune as protorune_pb;
use prost::Message;

#[test]
fn test_outpoint_with_protocol_encoding() {
    // Test with a known txid and protocol_tag = 1 (alkanes)
    let txid_hex = "93e93d80e9430c155d8b3ca74d364b069b27807e9744d77c2becddce849263b5";
    let txid_bytes = hex::decode(txid_hex).unwrap();
    
    let mut request = protorune_pb::OutpointWithProtocol::default();
    // txid should be in little-endian (reversed) format for Bitcoin
    let mut reversed_txid = txid_bytes.clone();
    reversed_txid.reverse();
    request.txid = reversed_txid;
    request.vout = 0;
    
    // Set protocol_tag = 1 (alkanes protocol)
    let protocol_tag: u128 = 1;
    let mut protocol = protorune_pb::Uint128::default();
    protocol.lo = (protocol_tag & 0xFFFFFFFFFFFFFFFF) as u64;
    protocol.hi = (protocol_tag >> 64) as u64;
    request.protocol = Some(protocol);
    
    // Encode to protobuf
    let encoded = request.encode_to_vec();
    let hex_encoded = format!("0x{}", hex::encode(&encoded));
    
    println!("Encoded OutpointWithProtocol:");
    println!("  TXID: {}", txid_hex);
    println!("  VOUT: 0");
    println!("  Protocol: 1");
    println!("  Hex: {}", hex_encoded);
    println!("  Length: {} bytes", encoded.len());
    
    // Verify we can decode it back
    let decoded = protorune_pb::OutpointWithProtocol::decode(encoded.as_slice()).unwrap();
    assert_eq!(decoded.txid, request.txid);
    assert_eq!(decoded.vout, request.vout);
    let protocol = decoded.protocol.unwrap();
    assert_eq!(protocol.lo, 1);
    assert_eq!(protocol.hi, 0);
}

#[test]
fn test_protocol_tag_encoding() {
    // Test various protocol_tag values
    let test_cases = vec![
        (1u128, 1u64, 0u64),                              // alkanes
        (2u128, 2u64, 0u64),                              // another protocol
        (0xFFFFFFFFFFFFFFFFu128, 0xFFFFFFFFFFFFFFFFu64, 0u64), // max lo value
        (0x10000000000000000u128, 0u64, 1u64),            // min hi value
    ];
    
    for (protocol_tag, expected_lo, expected_hi) in test_cases {
        let mut protocol = protorune_pb::Uint128::default();
        protocol.lo = (protocol_tag & 0xFFFFFFFFFFFFFFFF) as u64;
        protocol.hi = (protocol_tag >> 64) as u64;
        
        assert_eq!(protocol.lo, expected_lo, "Failed for protocol_tag={}", protocol_tag);
        assert_eq!(protocol.hi, expected_hi, "Failed for protocol_tag={}", protocol_tag);
        
        // Verify round-trip
        let reconstructed = (protocol.hi as u128) << 64 | (protocol.lo as u128);
        assert_eq!(reconstructed, protocol_tag, "Round-trip failed for protocol_tag={}", protocol_tag);
    }
}

#[test]
fn test_compare_with_expected_format() {
    // This test validates the exact encoding format expected by the metashrew_view call
    // Based on the logs: "0x0a2093e93d80e9430c155d8b3ca74d364b069b27807e9744d77c2becddce849263b5"
    
    let txid_hex = "93e93d80e9430c155d8b3ca74d364b069b27807e9744d77c2becddce849263b5";
    let txid_bytes = hex::decode(txid_hex).unwrap();
    
    let mut request = protorune_pb::OutpointWithProtocol::default();
    // Don't reverse - the log shows it in standard order
    request.txid = txid_bytes;
    request.vout = 0;
    
    // Protocol = 1
    let mut protocol = protorune_pb::Uint128::default();
    protocol.lo = 1;
    protocol.hi = 0;
    request.protocol = Some(protocol);
    
    let encoded = request.encode_to_vec();
    let hex_encoded = hex::encode(&encoded);
    
    println!("\nEncoded (without reversal):");
    println!("  Hex: 0x{}", hex_encoded);
    
    // The log shows: 0x0a2093e93d80e9430c155d8b3ca74d364b069b27807e9744d77c2becddce849263b5
    // Field 1 (txid): 0a 20 (field tag + length) + 32 bytes
    // Let's check if our encoding matches this pattern
    
    // Protobuf wire format:
    // - Field 1 (txid, bytes): tag=0x0a (field 1, wire type 2=length-delimited)
    // - Length: 0x20 (32 bytes)
    // - Field 2 (vout, uint32): tag=0x10 (field 2, wire type 0=varint)
    // - Value: 0x00 (vout=0)
    // - Field 3 (protocol, message): tag=0x1a (field 3, wire type 2=length-delimited)
    // - Length and nested message encoding for Uint128
    
    assert!(hex_encoded.starts_with("0a20"), "Should start with field tag 0a and length 20");
}
