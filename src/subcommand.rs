use crate::chain::Chain;
use crate::index::BlockData;
use crate::options::Options;
use crate::WasmHost;
use alkanes_support::host::AlkanesHost;
use anyhow::Result;
use bitcoin::consensus::Decodable;
use bitcoin::Block;
use clap::{Parser, Subcommand as ClapSubcommand};
use std::path::PathBuf;

#[derive(ClapSubcommand, Debug, Clone)]
pub enum Subcommand {
    Index(Index),
    Find(Find),
    Views(Views),
}

#[derive(Parser, Debug, Clone)]
pub struct Index {
    #[clap(long, help = "The height to index up to")]
    pub height: Option<u32>,
}

#[derive(Parser, Debug, Clone)]
pub struct Find {
    #[clap(long, help = "Find by transaction id")]
    pub txid: String,
}

#[derive(Parser, Debug, Clone)]
pub struct Views {
    #[clap(long, help = "View by transaction id")]
    pub txid: String,
}

impl Subcommand {
    pub fn run(self) -> Result<()> {
        let options = Options::parse();
        let data_dir = options.data_dir();
        let chain = options.chain();
        let host = WasmHost::default();
        match self {
            Subcommand::Index(index) => {
                let blocks_dir = options.blocks_dir.unwrap();
                for (i, entry) in std::fs::read_dir(blocks_dir)?.enumerate() {
                    let height = (i as u32 + chain.first_block()) as u32;
                    let path = entry?.path();
                    let block = Block::consensus_decode(&mut std::fs::File::open(path)?)?;
                    host.index_block(&block, height)?;
                }
                Ok(())
            }
            Subcommand::Find(find) => {
                // Implement find logic here if necessary
                println!("Find subcommand is not yet implemented.");
                Ok(())
            }
            Subcommand::Views(views) => {
                // Implement views logic here if necessary
                println!("Views subcommand is not yet implemented.");
                Ok(())
            }
        }
    }
}