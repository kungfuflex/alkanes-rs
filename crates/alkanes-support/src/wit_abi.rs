use crate::id::AlkaneId;
use anyhow::{anyhow, Result};

/// Trait for types that can be encoded into Vec<u128> for cellpack wire format.
pub trait CellpackEncode {
    fn encode_cellpack(&self, output: &mut Vec<u128>);
}

/// Trait for types that can be decoded from a Vec<u128> slice.
pub trait CellpackDecode: Sized {
    fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self>;
}

// --- u128 ---

impl CellpackEncode for u128 {
    fn encode_cellpack(&self, output: &mut Vec<u128>) {
        output.push(*self);
    }
}

impl CellpackDecode for u128 {
    fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
        if *offset >= input.len() {
            return Err(anyhow!("missing u128 parameter at offset {}", *offset));
        }
        let value = input[*offset];
        *offset += 1;
        Ok(value)
    }
}

// --- u64 ---

impl CellpackEncode for u64 {
    fn encode_cellpack(&self, output: &mut Vec<u128>) {
        output.push(*self as u128);
    }
}

impl CellpackDecode for u64 {
    fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
        let value = u128::decode_cellpack(input, offset)?;
        Ok(value as u64)
    }
}

// --- u32 ---

impl CellpackEncode for u32 {
    fn encode_cellpack(&self, output: &mut Vec<u128>) {
        output.push(*self as u128);
    }
}

impl CellpackDecode for u32 {
    fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
        let value = u128::decode_cellpack(input, offset)?;
        Ok(value as u32)
    }
}

// --- u16 ---

impl CellpackEncode for u16 {
    fn encode_cellpack(&self, output: &mut Vec<u128>) {
        output.push(*self as u128);
    }
}

impl CellpackDecode for u16 {
    fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
        let value = u128::decode_cellpack(input, offset)?;
        Ok(value as u16)
    }
}

// --- u8 ---

impl CellpackEncode for u8 {
    fn encode_cellpack(&self, output: &mut Vec<u128>) {
        output.push(*self as u128);
    }
}

impl CellpackDecode for u8 {
    fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
        let value = u128::decode_cellpack(input, offset)?;
        Ok(value as u8)
    }
}

// --- bool ---

impl CellpackEncode for bool {
    fn encode_cellpack(&self, output: &mut Vec<u128>) {
        output.push(if *self { 1 } else { 0 });
    }
}

impl CellpackDecode for bool {
    fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
        let value = u128::decode_cellpack(input, offset)?;
        Ok(value != 0)
    }
}

// --- String ---
// Encoding matches `string_to_u128_list` from utils.rs:
// Null-terminated, packed into 16-byte LE chunks.

impl CellpackEncode for String {
    fn encode_cellpack(&self, output: &mut Vec<u128>) {
        let mut bytes = self.as_bytes().to_vec();
        bytes.push(0); // null terminator
        let padding = (16 - (bytes.len() % 16)) % 16;
        bytes.extend(vec![0u8; padding]);
        for chunk in bytes.chunks(16) {
            output.push(u128::from_le_bytes(chunk.try_into().unwrap()));
        }
    }
}

impl CellpackDecode for String {
    fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
        let mut string_bytes = Vec::new();
        let mut found_null = false;

        while *offset < input.len() && !found_null {
            let value = input[*offset];
            *offset += 1;
            let bytes = value.to_le_bytes();
            for byte in bytes {
                if byte == 0 {
                    found_null = true;
                    break;
                }
                string_bytes.push(byte);
            }
        }

        String::from_utf8(string_bytes)
            .map_err(|e| anyhow!("invalid UTF-8 string: {}", e))
    }
}

// --- AlkaneId ---
// Encoded as 2 consecutive u128 slots: block, tx.

impl CellpackEncode for AlkaneId {
    fn encode_cellpack(&self, output: &mut Vec<u128>) {
        output.push(self.block);
        output.push(self.tx);
    }
}

impl CellpackDecode for AlkaneId {
    fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
        if *offset + 1 >= input.len() {
            return Err(anyhow!(
                "not enough parameters for AlkaneId at offset {}",
                *offset
            ));
        }
        let block = input[*offset];
        let tx = input[*offset + 1];
        *offset += 2;
        Ok(AlkaneId::new(block, tx))
    }
}

// --- Vec<T> ---
// Encoded as: length (1 u128) + elements in sequence.

impl<T: CellpackEncode> CellpackEncode for Vec<T> {
    fn encode_cellpack(&self, output: &mut Vec<u128>) {
        output.push(self.len() as u128);
        for item in self {
            item.encode_cellpack(output);
        }
    }
}

impl<T: CellpackDecode> CellpackDecode for Vec<T> {
    fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
        let length = u128::decode_cellpack(input, offset)? as usize;
        let mut vec = Vec::with_capacity(length);
        for _ in 0..length {
            vec.push(T::decode_cellpack(input, offset)?);
        }
        Ok(vec)
    }
}

// --- Option<T> ---
// Encoded as: 0 (none) or 1 + value (some).

impl<T: CellpackEncode> CellpackEncode for Option<T> {
    fn encode_cellpack(&self, output: &mut Vec<u128>) {
        match self {
            None => output.push(0),
            Some(val) => {
                output.push(1);
                val.encode_cellpack(output);
            }
        }
    }
}

impl<T: CellpackDecode> CellpackDecode for Option<T> {
    fn decode_cellpack(input: &[u128], offset: &mut usize) -> Result<Self> {
        let discriminant = u128::decode_cellpack(input, offset)?;
        match discriminant {
            0 => Ok(None),
            1 => Ok(Some(T::decode_cellpack(input, offset)?)),
            _ => Err(anyhow!("invalid Option discriminant: {}", discriminant)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u128_roundtrip() {
        let val: u128 = 42;
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![42]);
        let decoded = u128::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, 42);
    }

    #[test]
    fn test_string_roundtrip() {
        let val = String::from("hello");
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);

        // Verify matches string_to_u128_list
        let legacy = crate::utils::string_to_u128_list(val.clone());
        assert_eq!(encoded, legacy);

        let decoded = String::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, "hello");
    }

    #[test]
    fn test_string_long_roundtrip() {
        let val = String::from("this is a longer string that spans multiple u128 chunks!");
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);

        let legacy = crate::utils::string_to_u128_list(val.clone());
        assert_eq!(encoded, legacy);

        let decoded = String::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_alkane_id_roundtrip() {
        let id = AlkaneId::new(100, 200);
        let mut encoded = Vec::new();
        id.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![100, 200]);
        let decoded = AlkaneId::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, id);
    }

    #[test]
    fn test_vec_u128_roundtrip() {
        let val: Vec<u128> = vec![1, 2, 3];
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![3, 1, 2, 3]); // length prefix + elements
        let decoded = Vec::<u128>::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_option_roundtrip() {
        let some_val: Option<u128> = Some(42);
        let none_val: Option<u128> = None;

        let mut encoded_some = Vec::new();
        some_val.encode_cellpack(&mut encoded_some);
        assert_eq!(encoded_some, vec![1, 42]);

        let mut encoded_none = Vec::new();
        none_val.encode_cellpack(&mut encoded_none);
        assert_eq!(encoded_none, vec![0]);

        let decoded_some = Option::<u128>::decode_cellpack(&encoded_some, &mut 0).unwrap();
        assert_eq!(decoded_some, Some(42));

        let decoded_none = Option::<u128>::decode_cellpack(&encoded_none, &mut 0).unwrap();
        assert_eq!(decoded_none, None);
    }

    #[test]
    fn test_bool_roundtrip() {
        let mut encoded = Vec::new();
        true.encode_cellpack(&mut encoded);
        false.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![1, 0]);

        let t = bool::decode_cellpack(&encoded, &mut 0).unwrap();
        let f = bool::decode_cellpack(&encoded, &mut 1).unwrap();
        assert!(t);
        assert!(!f);
    }

    #[test]
    fn test_empty_string_roundtrip() {
        let val = String::from("");
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);

        let legacy = crate::utils::string_to_u128_list(val.clone());
        assert_eq!(encoded, legacy);

        let decoded = String::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, "");
    }

    #[test]
    fn test_large_u128_roundtrip() {
        let val: u128 = u128::MAX;
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        let decoded = u128::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, u128::MAX);
    }

    #[test]
    fn test_empty_vec_roundtrip() {
        let val: Vec<u128> = vec![];
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![0]); // just length = 0
        let decoded = Vec::<u128>::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_nested_vec_roundtrip() {
        let val: Vec<Vec<u128>> = vec![vec![1, 2], vec![3]];
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        // outer length(2), inner1 length(2), 1, 2, inner2 length(1), 3
        assert_eq!(encoded, vec![2, 2, 1, 2, 1, 3]);
        let decoded = Vec::<Vec<u128>>::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_multiple_strings_sequential() {
        let mut encoded = Vec::new();
        String::from("hello").encode_cellpack(&mut encoded);
        String::from("world").encode_cellpack(&mut encoded);

        let mut offset = 0;
        let s1 = String::decode_cellpack(&encoded, &mut offset).unwrap();
        let s2 = String::decode_cellpack(&encoded, &mut offset).unwrap();
        assert_eq!(s1, "hello");
        assert_eq!(s2, "world");
    }

    #[test]
    fn test_u64_roundtrip() {
        let val: u64 = 12345;
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![12345u128]);
        let decoded = u64::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, 12345);
    }

    #[test]
    fn test_option_string_roundtrip() {
        let some_val: Option<String> = Some(String::from("test"));
        let mut encoded = Vec::new();
        some_val.encode_cellpack(&mut encoded);

        let decoded = Option::<String>::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, Some(String::from("test")));
    }

    #[test]
    fn test_decode_out_of_bounds() {
        let empty: Vec<u128> = vec![];
        let result = u128::decode_cellpack(&empty, &mut 0);
        assert!(result.is_err());

        let single = vec![1u128];
        let result = AlkaneId::decode_cellpack(&single, &mut 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_mixed_sequential_decode() {
        // Simulate a cellpack with: u128(42), String("hi"), AlkaneId(1,2)
        let mut encoded = Vec::new();
        42u128.encode_cellpack(&mut encoded);
        String::from("hi").encode_cellpack(&mut encoded);
        AlkaneId::new(1, 2).encode_cellpack(&mut encoded);

        let mut offset = 0;
        let v1 = u128::decode_cellpack(&encoded, &mut offset).unwrap();
        let v2 = String::decode_cellpack(&encoded, &mut offset).unwrap();
        let v3 = AlkaneId::decode_cellpack(&encoded, &mut offset).unwrap();

        assert_eq!(v1, 42);
        assert_eq!(v2, "hi");
        assert_eq!(v3, AlkaneId::new(1, 2));
    }

    // =================================================================
    // Additional edge case tests
    // =================================================================

    #[test]
    fn test_very_large_u128_boundary_values() {
        // Test u128::MAX - 1
        let near_max = u128::MAX - 1;
        let mut encoded = Vec::new();
        near_max.encode_cellpack(&mut encoded);
        let decoded = u128::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, near_max);

        // Test a value that uses all 128 bits meaningfully
        let big_val: u128 = (1u128 << 127) | (1u128 << 64) | 42;
        let mut encoded2 = Vec::new();
        big_val.encode_cellpack(&mut encoded2);
        let decoded2 = u128::decode_cellpack(&encoded2, &mut 0).unwrap();
        assert_eq!(decoded2, big_val);

        // Test zero
        let zero: u128 = 0;
        let mut encoded3 = Vec::new();
        zero.encode_cellpack(&mut encoded3);
        let decoded3 = u128::decode_cellpack(&encoded3, &mut 0).unwrap();
        assert_eq!(decoded3, 0);
    }

    #[test]
    fn test_nested_vec_with_empty_inner() {
        // Vec<Vec<u128>> with a mix of empty and non-empty inner vecs
        let val: Vec<Vec<u128>> = vec![
            vec![1, 2, 3],
            vec![],
            vec![100, 200],
        ];
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);

        // Expected: 3, 3, 1, 2, 3, 0, 2, 100, 200
        assert_eq!(encoded, vec![3, 3, 1, 2, 3, 0, 2, 100, 200]);

        let decoded = Vec::<Vec<u128>>::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_empty_vec_of_strings_roundtrip() {
        let val: Vec<String> = vec![];
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![0]);

        let decoded = Vec::<String>::decode_cellpack(&encoded, &mut 0).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_three_strings_in_sequence() {
        let s1 = String::from("hello");
        let s2 = String::from("world");
        let s3 = String::from("!");

        let mut encoded = Vec::new();
        s1.encode_cellpack(&mut encoded);
        s2.encode_cellpack(&mut encoded);
        s3.encode_cellpack(&mut encoded);

        let mut offset = 0;
        let d1 = String::decode_cellpack(&encoded, &mut offset).unwrap();
        let d2 = String::decode_cellpack(&encoded, &mut offset).unwrap();
        let d3 = String::decode_cellpack(&encoded, &mut offset).unwrap();

        assert_eq!(d1, "hello");
        assert_eq!(d2, "world");
        assert_eq!(d3, "!");
    }

    #[test]
    fn test_string_exactly_16_bytes() {
        // 16 bytes exactly; with null terminator = 17 bytes, needs 2 chunks
        let val = String::from("0123456789abcdef"); // 16 bytes
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded.len(), 2, "16-byte string + null should use 2 u128 slots");

        let decoded = String::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_string_exactly_15_bytes() {
        // 15 bytes + null = 16 = exactly one u128 chunk
        let val = String::from("0123456789abcde"); // 15 bytes
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded.len(), 1, "15-byte string + null should fit in exactly 1 u128 slot");

        let decoded = String::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_option_invalid_discriminant() {
        let data: Vec<u128> = vec![99]; // invalid: not 0 or 1
        let result = Option::<u128>::decode_cellpack(&data, &mut 0);
        assert!(result.is_err(), "Option with invalid discriminant should fail");
    }

    #[test]
    fn test_vec_of_strings_roundtrip() {
        let val: Vec<String> = vec![
            "alpha".to_string(),
            "beta".to_string(),
            "gamma".to_string(),
        ];
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);

        let decoded = Vec::<String>::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_nested_option_roundtrip() {
        let some_some: Option<Option<u128>> = Some(Some(42));
        let some_none: Option<Option<u128>> = Some(None);
        let none: Option<Option<u128>> = None;

        let mut enc1 = Vec::new();
        some_some.encode_cellpack(&mut enc1);
        assert_eq!(enc1, vec![1, 1, 42]);

        let mut enc2 = Vec::new();
        some_none.encode_cellpack(&mut enc2);
        assert_eq!(enc2, vec![1, 0]);

        let mut enc3 = Vec::new();
        none.encode_cellpack(&mut enc3);
        assert_eq!(enc3, vec![0]);

        assert_eq!(Option::<Option<u128>>::decode_cellpack(&enc1, &mut 0).unwrap(), some_some);
        assert_eq!(Option::<Option<u128>>::decode_cellpack(&enc2, &mut 0).unwrap(), some_none);
        assert_eq!(Option::<Option<u128>>::decode_cellpack(&enc3, &mut 0).unwrap(), none);
    }

    #[test]
    fn test_u64_max_roundtrip() {
        let val = u64::MAX;
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![u64::MAX as u128]);
        let decoded = u64::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, u64::MAX);
    }

    #[test]
    fn test_bool_various_nonzero_values() {
        // Any nonzero value should decode as true
        let data: Vec<u128> = vec![0, 1, 2, 255, u128::MAX];
        assert!(!bool::decode_cellpack(&data, &mut 0).unwrap());
        assert!(bool::decode_cellpack(&data, &mut 1).unwrap());
        assert!(bool::decode_cellpack(&data, &mut 2).unwrap());
        assert!(bool::decode_cellpack(&data, &mut 3).unwrap());
        assert!(bool::decode_cellpack(&data, &mut 4).unwrap());
    }

    #[test]
    fn test_u8_roundtrip() {
        let val: u8 = 255;
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![255u128]);
        let decoded = u8::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, 255);
    }

    #[test]
    fn test_u16_roundtrip() {
        let val: u16 = 65535;
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![65535u128]);
        let decoded = u16::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, 65535);
    }

    #[test]
    fn test_u32_roundtrip() {
        let val: u32 = u32::MAX;
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![u32::MAX as u128]);
        let decoded = u32::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, u32::MAX);
    }

    #[test]
    fn test_vec_of_alkane_ids_roundtrip() {
        let val: Vec<AlkaneId> = vec![
            AlkaneId::new(1, 2),
            AlkaneId::new(3, 4),
        ];
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        // length(2), block1(1), tx1(2), block2(3), tx2(4)
        assert_eq!(encoded, vec![2, 1, 2, 3, 4]);

        let decoded = Vec::<AlkaneId>::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0], AlkaneId::new(1, 2));
        assert_eq!(decoded[1], AlkaneId::new(3, 4));
    }

    #[test]
    fn test_option_string_none_roundtrip() {
        let val: Option<String> = None;
        let mut encoded = Vec::new();
        val.encode_cellpack(&mut encoded);
        assert_eq!(encoded, vec![0]);

        let decoded = Option::<String>::decode_cellpack(&encoded, &mut 0).unwrap();
        assert_eq!(decoded, None);
    }
}
