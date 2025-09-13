use protorune_support::network::NetworkParams;

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
    pub fn default_params(&self) -> NetworkParams {
        match self {
            Network::Bitcoin => NetworkParams {
                bech32_prefix: String::from("bc"),
                p2sh_prefix: 0x05,
                p2pkh_prefix: 0x00,
            },
            Network::Testnet => NetworkParams {
                bech32_prefix: String::from("tb"),
                p2pkh_prefix: 0x6f,
                p2sh_prefix: 0xc4,
            },
            Network::Regtest | Network::Signet => NetworkParams {
                bech32_prefix: String::from("bcrt"),
                p2pkh_prefix: 0x64,
                p2sh_prefix: 0xc4,
            },
            Network::Luckycoin => NetworkParams {
                bech32_prefix: String::from("lky"),
                p2pkh_prefix: 0x2f,
                p2sh_prefix: 0x05,
            },
            Network::Dogecoin => NetworkParams {
                bech32_prefix: String::from("dc"),
                p2pkh_prefix: 0x1e,
                p2sh_prefix: 0x16,
            },
            Network::Bellscoin => NetworkParams {
                bech32_prefix: String::from("bel"),
                p2pkh_prefix: 0x19,
                p2sh_prefix: 0x1e,
            },
            Network::Fractal => NetworkParams {
                bech32_prefix: String::from("fractal"),
                p2pkh_prefix: 0x64,
                p2sh_prefix: 0xc4,
            },
        }
    }
}