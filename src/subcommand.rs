use crate::chain::Chain;
use crate::index::BlockData;
use crate::options::Options;
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
        let indexer = Indexer::new(data_dir.clone())?;
        match self {
            Subcommand::Index(index) => {
                let blocks_dir = options.blocks_dir.unwrap();
                for (i, entry) in std::fs::read_dir(blocks_dir)?.enumerate() {
                    let height = (i + chain.first_block()) as u32;
                    let path = entry?.path();
                    let block_data = BlockData::new(height, path.to_str().unwrap())?;
                    indexer.index_block(&block_data)?;
                }
                Ok(())
            }
            Subcommand::Find(find) => {
                let result = indexer.find_by_txid(find.txid)?;
                println!("{:?}", result);
                Ok(())
            }
            Subcommand::Views(views) => {
                let result = indexer.find_by_txid(views.txid)?;
                println!("{:?}", result);
                Ok(())
            }
        }
    }
}