use anyhow::Result;
use bitcoin::consensus::Decodable;
use bitcoin::io::Read;

#[derive(Debug, Clone)]
pub struct BlockData {
    pub height: u32,
    pub path: String,
}

impl BlockData {
    pub fn new(height: u32, path: &str) -> Result<Self> {
        Ok(Self {
            height,
            path: path.to_string(),
        })
    }
}

impl Decodable for BlockData {
    fn consensus_decode<R: Read + ?Sized>(
        reader: &mut R,
    ) -> Result<Self, bitcoin::consensus::encode::Error> {
        let height = u32::consensus_decode(reader)?;
        let path = String::consensus_decode(reader)?;
        Ok(Self { height, path })
    }
}