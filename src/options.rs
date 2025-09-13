use crate::chain::Chain;
use crate::subcommand::Subcommand;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser, Clone)]
#[clap(
    version,
    author = "Casey Rodarmor <casey@rodarmor.com>",
    about = "An index of runes and protorunes"
)]
pub struct Options {
    #[clap(
        long,
        default_value = "mainnet",
        help = "Chain to use: mainnet, testnet, signet, regtest"
    )]
    pub chain: Chain,
    #[clap(long, help = "Blocks directory")]
    pub blocks_dir: Option<PathBuf>,
    #[clap(long, help = "Data directory")]
    pub data_dir: Option<PathBuf>,
    #[clap(subcommand)]
    pub subcommand: Subcommand,
}

impl Options {
    pub fn data_dir(&self) -> PathBuf {
        self.data_dir
            .clone()
            .unwrap_or_else(|| self.chain.default_data_dir())
    }

    pub fn chain(&self) -> Chain {
        self.chain
    }
}