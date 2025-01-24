pub fn to_ptr(v: &mut Vec<u8>) -> i32 {
    return v.as_mut_ptr() as usize as i32;
}

pub fn to_passback_ptr(v: &mut Vec<u8>) -> i32 {
    to_ptr(v) + 4
}

pub fn to_arraybuffer_layout<T: AsRef<[u8]>>(v: T) -> Vec<u8> {
    let mut buffer = Vec::<u8>::new();
    buffer.extend_from_slice(&(v.as_ref().len() as u32).to_le_bytes());
    buffer.extend_from_slice(v.as_ref());
    return buffer;
}

pub fn export_bytes(v: Vec<u8>) -> i32 {
    let response: Vec<u8> = to_arraybuffer_layout(&v);
    Box::leak(Box::new(response)).as_mut_ptr() as usize as i32 + 4
}
