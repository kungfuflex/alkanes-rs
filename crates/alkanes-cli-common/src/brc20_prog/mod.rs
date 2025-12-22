// BRC20-Prog module for BRC20 programmable smart contracts
// This module provides functionality to interact with BRC20-prog contracts
// using the standard ord inscription envelope with JSON payloads

pub mod calldata;
pub mod contract_address;
pub mod envelope;
pub mod eth_call;
pub mod execute;
pub mod foundry;
pub mod frbtc;
pub mod types;
pub mod wrap_btc;

pub mod batch_payment_bytecode;
pub mod sol_query_bytecode;

pub use calldata::encode_function_call;
pub use eth_call::{eth_call, get_payments_length, get_signer_address, get_payment, Payment};
pub use contract_address::{compute_contract_address, pkscript_to_eth_address};
pub use envelope::Brc20ProgEnvelope;
pub use execute::Brc20ProgExecutor;
pub use foundry::{parse_foundry_json, extract_deployment_bytecode};
pub use types::{
    Brc20ProgDeployParams, Brc20ProgTransactParams, Brc20ProgExecuteParams,
    Brc20ProgExecuteResult, Brc20ProgInscriptionType, Brc20ProgDeployInscription,
    Brc20ProgCallInscription, AdditionalOutput,
};
pub use wrap_btc::{
    Brc20ProgWrapBtcExecutor, Brc20ProgWrapBtcParams,
    FRBTC_CONTRACT_ADDRESS,
    DEFAULT_FRBTC_ADDRESS_MAINNET, DEFAULT_FRBTC_ADDRESS_SIGNET, DEFAULT_FRBTC_ADDRESS_REGTEST,
    get_frbtc_address,
};

pub use batch_payment_bytecode::generate_batch_payment_fetcher_bytecode;
pub use sol_query_bytecode::generate_frbtc_query_bytecode;

// FrBTC operations
pub use frbtc::{
    FrBtcExecutor,
    FrBtcWrapParams,
    FrBtcUnwrapParams,
    FrBtcWrapAndExecuteParams,
    FrBtcWrapAndExecute2Params,
    FRBTC_ADDRESS_MAINNET,
    FRBTC_ADDRESS_SIGNET,
    FRBTC_ADDRESS_REGTEST,
    get_frbtc_contract_address,
};
