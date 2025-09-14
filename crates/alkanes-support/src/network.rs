/*
 * Chadson's Journal:
 *
 * The `protorune_support::network::NetworkParams` struct has been removed.
 * I'm refactoring this file to use `bitcoin::Network` directly, which aligns
 * with the changes in the `protorune-support` crate. The `to_bitcoin_network`
 * function provides the mapping from the internal `Network` enum to the
 * `bitcoin::Network` enum.
 */
use bitcoin::Network as BitcoinNetwork;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    Bitcoin,
    Testnet,
    Regtest,
    Signet,
    Luckycoin,
    Dogecoin,
    Bellscoin,
    Fractal,
}

impl Network {
    pub fn to_bitcoin_network(&self) -> BitcoinNetwork {
        match self {
            Network::Bitcoin => BitcoinNetwork::Bitcoin,
            Network::Testnet => BitcoinNetwork::Testnet,
            Network::Regtest => BitcoinNetwork::Regtest,
            Network::Signet => BitcoinNetwork::Signet,
            Network::Luckycoin => BitcoinNetwork::Regtest,
            Network::Dogecoin => BitcoinNetwork::Regtest,
            Network::Bellscoin => BitcoinNetwork::Regtest,
            Network::Fractal => BitcoinNetwork::Regtest,
        }
    }
}