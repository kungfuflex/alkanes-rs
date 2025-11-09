// BRC20-Prog module for BRC20 programmable smart contracts
// This module provides functionality to interact with BRC20-prog contracts
// using the standard ord inscription envelope with JSON payloads

pub mod calldata;
pub mod envelope;
pub mod execute;
pub mod foundry;
pub mod types;
pub mod wrap_btc;

pub use calldata::encode_function_call;
pub use envelope::Brc20ProgEnvelope;
pub use execute::Brc20ProgExecutor;
pub use foundry::{parse_foundry_json, extract_deployment_bytecode};
pub use types::{
    Brc20ProgDeployParams, Brc20ProgTransactParams, Brc20ProgExecuteParams,
    Brc20ProgExecuteResult, Brc20ProgInscriptionType, Brc20ProgDeployInscription,
    Brc20ProgCallInscription,
};
pub use wrap_btc::{Brc20ProgWrapBtcExecutor, Brc20ProgWrapBtcParams, FRBTC_CONTRACT_ADDRESS};
