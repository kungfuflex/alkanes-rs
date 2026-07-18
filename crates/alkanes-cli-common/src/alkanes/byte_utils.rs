pub trait ByteUtils {
    fn to_aligned_bytes(self) -> Vec<u8>;
    fn snap_to_15_bytes(self) -> Vec<u8>;
    fn to_u32(self) -> u32;
}

impl ByteUtils for u128 {
    // Removes the leading bytes in the u128 (little endian)
    // Note that this would remove the last byte of LEB encoded protostones generated under the assumption that it is only using the first 15 bytes
    // also opens up the possibility of potentially using the last 6 bits of the u128 for protostones encoding since at this step it would not have
    // been labeled as a cenotaph for exceeding the max size of a runestone varint.
    fn to_aligned_bytes(self) -> Vec<u8> {
        let mut ar: Vec<u8> = (self.to_le_bytes()).try_into().unwrap();
        while let Some(&last) = ar.last() {
            if last != 0 {
                break; // Stop if we encounter a non-zero byte
            }
            ar.pop(); // Remove the last element if it's zero
        }
        ar
    }

    // uint128s -> leb128 max needs 19 bytes, since 128/7 = 18.3, so an extra byte is needed to store the last two bits in the uint128.
    // Runes will produce cenotaph if it needs to process more than 18 bytes for any leb128, so we cannot use the upper two bits in a uint128
    // Simplest solution is to not use the upper 8 bits (upper byte) of the uint128 so the upper 2 bits can never be set.
    // Downside is we miss out on 6 bits of storage before we have to push another tag
    fn snap_to_15_bytes(self) -> Vec<u8> {
        let mut ar: Vec<u8> = (self.to_le_bytes()).try_into().unwrap();
        ar.pop();
        ar
    }

    fn to_u32(self) -> u32 {
        self as u32
    }
}
