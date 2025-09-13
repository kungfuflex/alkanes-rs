use crate::subcommand::Subcommand;
use anyhow::Result;
use bitcoin::consensus::Decodable;
use bitcoin::Block;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

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
    fn consensus_decode<R: Read + ?Sized>(reader: &mut R) -> Result<Self, bitcoin::consensus::Error> {
        let height = u32::consensus_decode(reader)?;
        let path = String::consensus_decode(reader)?;
        Ok(Self { height, path })
    }
}