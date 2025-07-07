#[cfg(test)]
mod tests {
    use core::fmt;

    use leb128fmt::*;

    use proptest::prelude::*;

    fn test_encode_decode_uint_slice<T, const BITS: u32>(nums: Vec<T>)
    where
        T: Copy
            + PartialEq
            + core::ops::BitAnd
            + core::ops::Shr<u32>
            + core::ops::ShrAssign<u32>
            + From<u8>
            + UInt
            + core::ops::Shl<u32, Output = T>
            + core::ops::BitOrAssign
            + fmt::Debug,
        <T as core::ops::Shr<u32>>::Output: PartialEq<T>,
        u8: TryFrom<<T as core::ops::BitAnd<T>>::Output>,
    {
        let mut buffer = Vec::new();
        buffer.resize(max_len::<BITS>() * nums.len(), 0);

        let mut pos = 0;
        for n in &nums {
            encode_uint_slice::<T, BITS>(*n, &mut buffer, &mut pos).unwrap();
        }
        let end_pos = pos;

        pos = 0;
        for n in nums {
            let decoded_n = decode_uint_slice::<T, BITS>(&buffer, &mut pos).unwrap();
            assert_eq!(n, decoded_n);
        }
        assert_eq!(end_pos, pos);
    }

    fn test_encode_fixed_decode_uint_slice<T, const BITS: u32>(nums: Vec<T>)
    where
        T: Copy
            + PartialEq
            + core::ops::BitAnd
            + core::ops::Shr<u32>
            + core::ops::ShrAssign<u32>
            + From<u8>
            + UInt
            + core::ops::Shl<u32, Output = T>
            + core::ops::BitOrAssign
            + fmt::Debug,
        <T as core::ops::Shr<u32>>::Output: PartialEq<T>,
        u8: TryFrom<<T as core::ops::BitAnd<T>>::Output>,
    {
        let mut buffer = Vec::new();
        buffer.resize(max_len::<BITS>() * nums.len(), 0);

        let mut pos = 0;
        for n in &nums {
            encode_fixed_uint_slice::<T, BITS>(*n, &mut buffer, &mut pos).unwrap();
        }
        assert_eq!(pos, buffer.len());
        let end_pos = pos;

        pos = 0;
        for n in nums {
            let decoded_n = decode_uint_slice::<T, BITS>(&buffer, &mut pos).unwrap();
            assert_eq!(n, decoded_n);
        }
        assert_eq!(end_pos, pos);
    }

    fn test_encode_decode_sint_slice<T, const BITS: u32>(nums: Vec<T>)
    where
        T: Copy
            + PartialEq
            + core::ops::BitAnd
            + core::ops::Shr<u32>
            + core::ops::ShrAssign<u32>
            + From<u8>
            + SInt
            + core::ops::Shl<u32, Output = T>
            + core::ops::BitOrAssign
            + From<i8>
            + fmt::Debug,
        <T as core::ops::Shr<u32>>::Output: PartialEq<T>,
        u8: TryFrom<<T as core::ops::BitAnd<T>>::Output>,
    {
        let mut buffer = Vec::new();
        buffer.resize(max_len::<BITS>() * nums.len(), 0);

        let mut pos = 0;
        for n in &nums {
            encode_sint_slice::<T, BITS>(*n, &mut buffer, &mut pos).unwrap();
        }
        let end_pos = pos;

        pos = 0;
        for n in nums {
            let decoded_n = decode_sint_slice::<T, BITS>(&buffer, &mut pos).unwrap();
            assert_eq!(n, decoded_n);
        }
        assert_eq!(end_pos, pos);
    }

    fn test_encode_fixed_decode_sint_slice<T, const BITS: u32>(nums: Vec<T>)
    where
        T: Copy
            + PartialEq
            + core::ops::BitAnd
            + core::ops::Shr<u32>
            + core::ops::ShrAssign<u32>
            + From<u8>
            + SInt
            + core::ops::Shl<u32, Output = T>
            + core::ops::BitOrAssign
            + From<i8>
            + fmt::Debug,
        <T as core::ops::Shr<u32>>::Output: PartialEq<T>,
        u8: TryFrom<<T as core::ops::BitAnd<T>>::Output>,
    {
        let mut buffer = Vec::new();
        buffer.resize(max_len::<BITS>() * nums.len(), 0);

        let mut pos = 0;
        for n in &nums {
            encode_fixed_sint_slice::<T, BITS>(*n, &mut buffer, &mut pos).unwrap();
        }
        assert_eq!(pos, buffer.len());
        let end_pos = pos;

        pos = 0;
        for n in nums {
            let decoded_n = decode_sint_slice::<T, BITS>(&buffer, &mut pos).unwrap();
            assert_eq!(n, decoded_n);
        }
        assert_eq!(end_pos, pos);
    }

    proptest! {
        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_decode_u32_num(n in any::<u32>()) {
            let (bytes, written_len) = encode_u32(n).unwrap();
            let (decoded_n, read_len) = decode_u32(bytes).unwrap();
            prop_assert_eq!(decoded_n, n);
            prop_assert_eq!(written_len, read_len);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_fixed_decode_u32_num(n in any::<u32>()) {
            let bytes = encode_fixed_u32(n).unwrap();
            let (decoded_n, read_len) = decode_u32(bytes).unwrap();
            prop_assert_eq!(decoded_n, n);
            prop_assert_eq!(bytes.len(), read_len);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_decode_u64_num(n in any::<u64>()) {
            let (bytes, written_len) = encode_u64(n).unwrap();
            let (decoded_n, read_len) = decode_u64(bytes).unwrap();
            prop_assert_eq!(decoded_n, n);
            prop_assert_eq!(written_len, read_len);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_fixed_decode_u64_num(n in any::<u64>()) {
            let bytes = encode_fixed_u64(n).unwrap();
            let (decoded_n, read_len) = decode_u64(bytes).unwrap();
            prop_assert_eq!(decoded_n, n);
            prop_assert_eq!(bytes.len(), read_len);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_decode_uint_32bit_slice(nums in any::<Vec<u32>>()) {
            test_encode_decode_uint_slice::<u32, 32>(nums);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_fixed_decode_uint_32bit_slice(nums in any::<Vec<u32>>()) {
            test_encode_fixed_decode_uint_slice::<u32, 32>(nums);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_decode_uint_64bit_slice(nums in any::<Vec<u64>>()) {
            test_encode_decode_uint_slice::<u64, 64>(nums);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_fixed_decode_uint_64bit_slice(nums in any::<Vec<u64>>()) {
            test_encode_fixed_decode_uint_slice::<u64, 64>(nums);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_decode_u32_bytes(bytes in prop::array::uniform5(any::<u8>())) {
             if decode_u32(bytes).is_none() {
                for b in bytes.iter().take(4) {
                    prop_assert!(b & 0x80 != 0);
                }
                prop_assert!(bytes[4] > 0x0f);
                return Ok(());
            }
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_decode_u64_bytes(bytes in prop::array::uniform10(any::<u8>())) {
            if decode_u64(bytes).is_none() {
                for b in bytes.iter().take(9) {
                    prop_assert!(b & 0x80 != 0);
                }
                prop_assert!(bytes[9] > 0x01);
                return Ok(());
            }
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_decode_s32_num(n in any::<i32>()) {
            let (bytes, written_len) = encode_s32(n).unwrap();
            let (decoded_n, read_len) = decode_s32(bytes).unwrap();
            prop_assert_eq!(decoded_n, n);
            prop_assert_eq!(written_len, read_len);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_fixed_decode_s32_num(n in any::<i32>()) {
            let bytes = encode_fixed_s32(n).unwrap();
            let (decoded_n, read_len) = decode_s32(bytes).unwrap();
            prop_assert_eq!(decoded_n, n);
            prop_assert_eq!(bytes.len(), read_len);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_decode_s64_num(n in any::<i64>()) {
            let (bytes, written_len) = encode_s64(n).unwrap();
            let (decoded_n, read_len) = decode_s64(bytes).unwrap();
            prop_assert_eq!(decoded_n, n);
            prop_assert_eq!(written_len, read_len);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_fixed_decode_s64_num(n in any::<i64>()) {
            let bytes = encode_fixed_s64(n).unwrap();
            let (decoded_n, read_len) = decode_s64(bytes).unwrap();
            prop_assert_eq!(decoded_n, n);
            prop_assert_eq!(bytes.len(), read_len);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_decode_sint_32bit_slice(nums in any::<Vec<i32>>()) {
            test_encode_decode_sint_slice::<i32, 32>(nums);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_fixed_decode_sint_32bit_slice(nums in any::<Vec<i32>>()) {
            test_encode_fixed_decode_sint_slice::<i32, 32>(nums);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_decode_sint_64bit_slice(nums in any::<Vec<i64>>()) {
            test_encode_decode_sint_slice::<i64, 64>(nums);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_encode_fixed_decode_sint_64bit_slice(nums in any::<Vec<i64>>()) {
            test_encode_fixed_decode_sint_slice::<i64, 64>(nums);
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_decode_encode_s32_bytes(bytes in prop::array::uniform5(any::<u8>())) {
            if decode_s32(bytes).is_none() {
                for b in bytes.iter().take(4) {
                    prop_assert!(b & 0x80 != 0);
                }

                if bytes[4] & 0x80  == 0 {
                    if bytes[4] & 0x40 != 0 {
                        prop_assert!(bytes[4] < 0x78);
                    } else {
                        prop_assert!(bytes[4] > 0x07);
                    }
                }
            };
        }

        #[allow(clippy::ignored_unit_patterns)]
        #[test]
        fn test_decode_encode_s64_bytes(bytes in prop::array::uniform10(any::<u8>())) {
            if decode_s64(bytes).is_none() {
                for b in bytes.iter().take(9) {
                    prop_assert!(b & 0x80 != 0);
                }

                if bytes[9] & 0x80  == 0 {
                    if bytes[9] & 0x40 != 0 {
                        prop_assert_ne!(bytes[9], 0x7F);
                    } else {
                        prop_assert_ne!(bytes[9], 0x00);
                    }
                }
            };
        }
    }
}
