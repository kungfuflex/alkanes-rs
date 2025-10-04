use std::mem::size_of;

pub trait ByteView: Sized {
	fn from_bytes(bytes: Vec<u8>) -> Self;
	fn to_bytes(&self) -> Vec<u8>;
	fn zero() -> Self;
}

impl ByteView for u128 {
	fn from_bytes(bytes: Vec<u8>) -> Self {
		if bytes.len() == 0 {
			return 0;
		}
		let mut u128_bytes = [0u8; size_of::<u128>()];
		u128_bytes.copy_from_slice(&bytes[0..size_of::<u128>()]);
		u128::from_le_bytes(u128_bytes)
	}
	fn to_bytes(&self) -> Vec<u8> {
		self.to_le_bytes().to_vec()
	}
	fn zero() -> Self {
		0
	}
}

impl ByteView for u32 {
	fn from_bytes(bytes: Vec<u8>) -> Self {
		if bytes.len() == 0 {
			return 0;
		}
		let mut u32_bytes = [0u8; size_of::<u32>()];
		u32_bytes.copy_from_slice(&bytes[0..size_of::<u32>()]);
		u32::from_le_bytes(u32_bytes)
	}
	fn to_bytes(&self) -> Vec<u8> {
		self.to_le_bytes().to_vec()
	}
	fn zero() -> Self {
		0
	}
}

impl ByteView for usize {
	fn from_bytes(bytes: Vec<u8>) -> Self {
		if bytes.len() == 0 {
			return 0;
		}
		let mut usize_bytes = [0u8; size_of::<usize>()];
		usize_bytes.copy_from_slice(&bytes[0..size_of::<usize>()]);
		usize::from_le_bytes(usize_bytes)
	}
	fn to_bytes(&self) -> Vec<u8> {
		self.to_le_bytes().to_vec()
	}
	fn zero() -> Self {
		0
	}
}