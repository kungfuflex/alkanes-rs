#[allow(dead_code)]
pub trait ByteView {
    fn from_bytes(v: Vec<u8>) -> Self;
    fn to_bytes(&self) -> Vec<u8>;
    fn maximum() -> Self;
    fn zero() -> Self;
}

#[allow(dead_code)]
pub fn shrink_back(b: Vec<u8>, v: usize) -> Vec<u8> {
    let mut _vec = b.clone();
    if b.len() >= v {
        _vec.clear()
    }
    _vec.drain(0..v);
    _vec
}

#[allow(dead_code)]
impl ByteView for u8 {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> u8 {
        u8::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }
    fn maximum() -> u8 {
        u8::MAX
    }
    fn zero() -> u8 {
        0
    }
}

#[allow(dead_code)]
impl ByteView for u16 {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> u16 {
        u16::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }
    fn maximum() -> u16 {
        u16::MAX
    }
    fn zero() -> u16 {
        0
    }
}

#[allow(dead_code)]
impl ByteView for u32 {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> u32 {
        u32::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }
    fn maximum() -> u32 {
        u32::MAX
    }
    fn zero() -> u32 {
        0
    }
}

#[allow(dead_code)]
impl ByteView for u64 {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> u64 {
        u64::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }
    fn maximum() -> u64 {
        u64::MAX
    }
    fn zero() -> u64 {
        0
    }
}

#[allow(dead_code)]
impl ByteView for u128 {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> u128 {
        u128::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }
    fn maximum() -> u128 {
        u128::MAX
    }
    fn zero() -> u128 {
        0
    }
}

#[allow(dead_code)]
impl ByteView for usize {
    fn to_bytes(&self) -> Vec<u8> {
        Vec::<u8>::from(self.to_le_bytes())
    }
    fn from_bytes(v: Vec<u8>) -> usize {
        usize::from_le_bytes(v.as_slice().try_into().expect("incorrect length"))
    }

    fn maximum() -> usize {
        usize::MAX
    }
    fn zero() -> usize {
        0
    }
}
