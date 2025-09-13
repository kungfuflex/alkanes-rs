use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(ValueEnum, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Chain {
    Mainnet,
    Testnet,
    Signet,
    Regtest,
}

impl Chain {
    pub fn default_data_dir(&self) -> PathBuf {
        let mut home = dirs::home_dir().unwrap();
        home.push(".protorunes");
        home.push(self.to_string());
        home
    }

    pub fn first_block(&self) -> u32 {
        match self {
            Chain::Mainnet => 824544,
            Chain::Testnet => 2573334,
            Chain::Signet => 180000,
            Chain::Regtest => 0,
        }
    }
}

impl Display for Chain {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl FromStr for Chain {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" => Ok(Chain::Mainnet),
            "testnet" => Ok(Chain::Testnet),
            "signet" => Ok(Chain::Signet),
            "regtest" => Ok(Chain::Regtest),
            _ => Err(anyhow::anyhow!("unknown chain: {}", s)),
        }
    }
}