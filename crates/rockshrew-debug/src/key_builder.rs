/// Helper functions for building database keys with the correct binary format
///
/// Metashrew HEIGHT_TO_TRANSACTION_IDS structure:
/// - Height is encoded as u64 little-endian bytes (not a string!)
/// - Append-only list tracks multiple processings of the same block (for reorgs)
///
/// Key format: /txids/byheight + [height as u64 LE] + /{append_index}/{txid_index or "length"}
///
/// Example for height 880000:
/// - Base: /txids/byheight + [0x80, 0x6D, 0x0D, 0x00, 0x00, 0x00, 0x00, 0x00]
/// - Append count: + /length (how many times block was processed)
/// - First processing tx count: + /0/length
/// - Second processing tx count: + /1/length (means reorg happened!)

/// Build key to get how many times a block has been processed (append-only list length)
/// If this returns > 1, a reorg was detected!
pub fn build_txid_length_key(height: u64) -> Vec<u8> {
    let mut key = Vec::new();
    key.extend_from_slice(b"/txids/byheight");
    key.extend_from_slice(&height.to_le_bytes());  // Height as binary u64 LE!
    key.extend_from_slice(b"/length");             // Append-only list length
    key
}

/// Build key for getting the txid count for a specific processing of a block
pub fn build_txid_data_key(height: u64, append_index: u32) -> Vec<u8> {
    let mut key = Vec::new();
    key.extend_from_slice(b"/txids/byheight");
    key.extend_from_slice(&height.to_le_bytes());
    key.extend_from_slice(format!("/{}/length", append_index).as_bytes());
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_format_with_binary_height() {
        // Test height 917504 (which is 0x000e0000 or [00 00 0e 00 00 00 00 00] in LE)
        let key = build_txid_length_key(917504);
        // /txids/byheight + [00 00 0e 00 00 00 00 00] + /length
        let expected_hex = "2f74786964732f627968656967687400000e00000000002f6c656e677468";
        assert_eq!(hex::encode(&key), expected_hex);

        // Test height 880000 (0xd6d80 = [80 6d 0d 00 00 00 00 00] in LE)
        let key = build_txid_length_key(880000);
        // /txids/byheight + [80 6d 0d 00 00 00 00 00] + /length
        let expected = {
            let mut k = Vec::new();
            k.extend_from_slice(b"/txids/byheight");
            k.extend_from_slice(&880000u64.to_le_bytes());
            k.extend_from_slice(b"/length");
            k
        };
        assert_eq!(key, expected);

        // Test data key for append index 0
        let key = build_txid_data_key(917504, 0);
        // /txids/byheight + [00 00 0e 00 00 00 00 00] + /0/length
        let expected_hex = "2f74786964732f627968656967687400000e00000000002f302f6c656e677468";
        assert_eq!(hex::encode(&key), expected_hex);
    }
}
