use alkanes_support::view::{Balance, Outpoint, Wallet};
use bitcoin::{hashes::Hash, OutPoint, TxOut};
use protobuf::MessageField;
use protorune_support::proto::protorune;

pub trait IntoProto<T> {
    fn into_proto(self) -> T;
}

impl IntoProto<protorune::Outpoint> for OutPoint {
    fn into_proto(self) -> protorune::Outpoint {
        let mut output = protorune::Outpoint::new();
        output.txid = self.txid.as_byte_array().to_vec();
        output.vout = self.vout;
        output
    }
}

impl IntoProto<protorune::Output> for TxOut {
    fn into_proto(self) -> protorune::Output {
        let mut output = protorune::Output::new();
        output.value = self.value.to_sat();
        output.script = self.script_pubkey.to_bytes();
        output
    }
}

impl IntoProto<protorune::OutpointResponse> for Outpoint {
    fn into_proto(self) -> protorune::OutpointResponse {
        let mut output = protorune::OutpointResponse::new();
        output.outpoint = MessageField::some(self.outpoint.into_proto());
        output.output = MessageField::some(self.output.into_proto());
        output.height = self.height;
        output.txindex = self.txindex;
        let balances = self
            .balances
            .into_iter()
            .map(|balance| balance.into_proto())
            .collect();
        let mut balances_proto = alkanes_proto::alkanes::Balances::new();
        balances_proto.entries = balances;
        output.special_fields = MessageField::some(balances_proto);
        output
    }
}

impl IntoProto<protorune::WalletResponse> for Wallet {
    fn into_proto(self) -> protorune::WalletResponse {
        let mut output = protorune::WalletResponse::new();
        output.outpoints = self
            .outpoints
            .into_iter()
            .map(|outpoint| outpoint.into_proto())
            .collect();
        output
    }
}

impl IntoProto<alkanes_proto::alkanes::Balance> for Balance {
    fn into_proto(self) -> alkanes_proto::alkanes::Balance {
        let mut output = alkanes_proto::alkanes::Balance::new();
        output.rune_id = MessageField::some(self.rune_id.into());
        output.amount = MessageField::some(self.amount.into());
        output
    }
}

impl IntoProto<Vec<protorune::Rune>> for Vec<alkanes_support::view::Rune> {
    fn into_proto(self) -> Vec<protorune::Rune> {
        self.into_iter()
            .map(|rune| {
                let mut output = protorune::Rune::new();
                output.runeId = MessageField::some(rune.rune_id.into());
                output.name = rune.name;
                output.symbol = rune.symbol;
                output.spacers = rune.spacers;
                output.divisibility = rune.divisibility;
                output
            })
            .collect()
    }
}