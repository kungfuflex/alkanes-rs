fn main() {
    let mut config = prost_build::Config::new();
    config.type_attribute("RuneId", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("ProtoruneRuneId", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("Rune", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("BalanceSheetItem", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("BalanceSheet", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("Outpoint", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("OutpointWithProtocol", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("Output", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("OutpointResponse", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("PaginationInput", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("WalletRequest", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("WalletResponse", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("ProtorunesWalletRequest", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("RunesByHeightRequest", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("ProtorunesByHeightRequest", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("RunesResponse", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("ProtoBurn", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("uint128", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("Clause", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("Predicate", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("ProtoMessage", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("RuntimeInput", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.type_attribute("Runtime", "#[derive(serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord)]");
    config.compile_protos(&["proto/protorune.proto"], &["proto/"]).unwrap();
}
