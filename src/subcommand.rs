use crate::chain::Chain;
use crate::index::BlockData;
use crate::options::Options;
use crate::view::views;
use anyhow::Result;
use bitcoin::consensus::Decodable;
use bitcoin::Block;
use clap::Subcommand;
use metashrew_core::iterator::FileIterator;
use metashrew_core::Consensus;
use metashrew_core::{Find, Indexer};
use std::path::PathBuf;

#[derive(Subcommand, Debug, Clone)]
pub enum Subcommand {
    Index(Index),
    Find(Find),
    Views(Views),
}

#[derive(Subcommand, Debug, Clone)]
pub enum Index {
    #[command(name = "index")]
    Index,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Find {
    #[command(name = "find")]
    Find {
        #[arg(long, help = "Find by transaction id")]
        txid: String,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum Views {
    #[command(name = "views")]
    Views {
        #[arg(long, help = "View by transaction id")]
        txid: String,
    },
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
                let file_iterator = FileIterator::new(blocks_dir.clone())?;
                for (i, path) in file_iterator.enumerate() {
                    let height = (i + chain.first_block()) as u32;
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