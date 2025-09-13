use anyhow::Result;
use clap::Parser;
use options::Options;
use subcommand::Subcommand;
mod chain;
mod index;
mod logging;
mod message;
mod options;
mod proto;
mod subcommand;
mod unwrap;
mod view;
mod vm;

fn main() -> Result<()> {
    let options = Options::parse();
    let subcommand = options.subcommand.clone();
    subcommand.run()
}