use byteorder::{ByteOrder, LittleEndian};

#[derive(Debug, Clone)]
pub struct EnvironmentInput {
    pub height: u32,
    pub data: Vec<u8>,
}

impl EnvironmentInput {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let height = LittleEndian::read_u32(&bytes[0..4]);
        let data = bytes[4..].to_vec();
        Self { height, data }
    }
}

pub trait RuntimeEnvironment: std::fmt::Debug {
    fn get(key: &[u8]) -> Option<Vec<u8>>;
    fn flush(data: &[u8]) -> Result<(), ()>;
    fn load_input() -> Result<EnvironmentInput, ()>;
    fn log(message: &str);
    fn clear();
}
