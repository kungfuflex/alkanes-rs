// Chadson's Journal
// Date: 2025-08-04
//
// Task: Fix wallet connection issues in slope-frontend.
//
// Current Status:
// I've been stuck on a circular compilation error in `deezel-web`.
// The root cause is that `lib.rs` was not declaring the crate's modules correctly.
// It contained a lot of old, conflicting code.
//
// Plan:
// 1.  Overwrite `lib.rs` to properly declare all public modules.
// 2.  This should resolve the `unresolved import` errors.
// 3.  Re-compile the project.

use wasm_bindgen::prelude::*;
use bitcoin::psbt::Psbt;
use bitcoin::Transaction;
use alkanes_cli_common::runestone_enhanced::{format_runestone_with_decoded_messages, format_runestone};
use alkanes_cli_common::alkanes::inspector::analysis::perform_fuzzing_analysis;
use alkanes_cli_common::alkanes::types::AlkaneId;
use alkanes_cli_common::brc20_prog::{
    Brc20ProgExecutor, Brc20ProgExecuteParams, Brc20ProgDeployInscription,
    Brc20ProgCallInscription, parse_foundry_json, extract_deployment_bytecode,
    encode_function_call, Brc20ProgWrapBtcExecutor, Brc20ProgWrapBtcParams,
};
use js_sys::Promise;
use wasm_bindgen_futures::future_to_promise;
pub use crate::provider::WebProvider;
use alkanes_cli_common::AlkanesProvider;
use base64::{engine::general_purpose::STANDARD, Engine as _};

/// Initialize the panic hook for better error messages in WASM
/// This should be called early in your application
#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}


pub mod crypto;
pub mod keystore;
pub mod logging;
pub mod network;
pub mod platform;
pub mod provider;
pub mod storage;
pub mod time;
pub mod utils;
pub mod wallet_provider;
pub mod keystore_wallet;

// Re-export wallet provider types for easier access
pub use wallet_provider::{
    WasmBrowserWalletProvider,
    WalletInfo,
    WalletAccount,
    WalletNetworkInfo,
    PsbtSigningOptions,
    PsbtSigningInput,
    WalletConnectionStatus,
};

#[wasm_bindgen]
pub fn analyze_psbt(psbt_base64: &str, network_str: &str) -> Result<String, JsValue> {
    let psbt_bytes = STANDARD.decode(psbt_base64)
        .map_err(|e| JsValue::from_str(&format!("base64 decode error: {}", e)))?;
    let psbt: Psbt = Psbt::deserialize(&psbt_bytes)
        .map_err(|e| JsValue::from_str(&format!("PSBT deserialize error: {}", e)))?;

    let tx = psbt.extract_tx()
        .map_err(|e| JsValue::from_str(&format!("PSBT extract_tx error: {}", e)))?;

    let network = match network_str {
        "mainnet" | "bitcoin" => bitcoin::Network::Bitcoin,
        "testnet" | "testnet3" => bitcoin::Network::Testnet,
        "signet" => bitcoin::Network::Signet,
        "regtest" => bitcoin::Network::Regtest,
        _ => bitcoin::Network::Bitcoin, // default to mainnet
    };

    let analysis = format_runestone_with_decoded_messages(&tx, network)
        .map_err(|e| JsValue::from_str(&format!("Runestone analysis error: {}", e)))?;

    serde_json::to_string(&analysis)
        .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
}

#[wasm_bindgen]
pub fn simulate_alkane_call(alkane_id_str: &str, wasm_hex: &str, cellpack_hex: &str) -> Promise {
    let wasm_bytes = match hex::decode(wasm_hex.strip_prefix("0x").unwrap_or(wasm_hex)) {
        Ok(bytes) => bytes,
        Err(e) => return future_to_promise(async move { Err(JsValue::from_str(&format!("WASM hex decode error: {}", e))) }),
    };

    let _cellpack_bytes = match hex::decode(cellpack_hex.strip_prefix("0x").unwrap_or(cellpack_hex)) {
        Ok(bytes) => bytes,
        Err(e) => return future_to_promise(async move { Err(JsValue::from_str(&format!("Cellpack hex decode error: {}", e))) }),
    };

    // The inspector's fuzzing analysis function is perfect for this.
    // We can treat the cellpack as a single "opcode" to test.
    // The `perform_fuzzing_analysis` function expects opcodes as u128.
    // We need to get the opcode from the cellpack.
    // For now, let's assume the first element in the cellpack is the opcode.
    // This part needs to be more robust based on actual cellpack structure.
    let alkane_id: AlkaneId = match serde_json::from_str(alkane_id_str) {
        Ok(id) => id,
        Err(e) => return future_to_promise(async move { Err(JsValue::from_str(&format!("AlkaneId deserialize error: {}", e))) }),
    };
    
    future_to_promise(async move {
        let fuzz_ranges = "0-1"; // Placeholder
        match perform_fuzzing_analysis(&alkane_id, &wasm_bytes, Some(fuzz_ranges)).await {
            Ok(fuzz_result) => {
                let result_json = serde_json::to_string(&fuzz_result)
                    .map_err(|e| JsValue::from_str(&format!("Fuzz result serialization error: {}", e)))?;
                Ok(JsValue::from_str(&result_json))
            }
            Err(e) => Err(JsValue::from_str(&format!("Alkane simulation error: {}", e))),
        }
    })
}

#[wasm_bindgen]
pub fn get_alkane_bytecode(network: &str, block: f64, tx: f64, block_tag: &str) -> Promise {
    let network_str = network.to_string();
    let alkane_id = format!("{}:{}", block as u64, tx as u32);
    let block_tag_opt = if block_tag.is_empty() {
        None
    } else {
        Some(block_tag.to_string())
    };

    future_to_promise(async move {
        let provider = WebProvider::new(network_str).await
            .map_err(|e| JsValue::from_str(&format!("Failed to create provider: {:?}", e)))?;

        match provider.get_bytecode(&alkane_id, block_tag_opt).await {
            Ok(bytecode_hex) => {
                Ok(JsValue::from_str(&bytecode_hex))
            }
            Err(e) => Err(JsValue::from_str(&format!("get_bytecode failed: {:?}", e))),
        }
    })
}

/// Analyze a transaction's runestone to extract Protostones
///
/// This function takes a raw transaction hex string, decodes it, and extracts
/// all Protostones from the transaction's OP_RETURN output.
///
/// # Arguments
///
/// * `tx_hex` - Hexadecimal string of the raw transaction (with or without "0x" prefix)
///
/// # Returns
///
/// A JSON string containing:
/// - `protostone_count`: Number of Protostones found
/// - `protostones`: Array of Protostone objects with their details
///
/// # Example
///
/// ```javascript
/// const result = analyze_runestone(txHex);
/// const data = JSON.parse(result);
/// console.log(`Found ${data.protostone_count} Protostones`);
/// ```
#[wasm_bindgen]
pub fn analyze_runestone(tx_hex: &str) -> Result<String, JsValue> {
    // Strip "0x" prefix if present
    let hex_str = tx_hex.strip_prefix("0x").unwrap_or(tx_hex);

    // Decode hex to bytes
    let tx_bytes = hex::decode(hex_str)
        .map_err(|e| JsValue::from_str(&format!("hex decode error: {}", e)))?;

    // Deserialize transaction
    let tx: Transaction = bitcoin::consensus::deserialize(&tx_bytes)
        .map_err(|e| JsValue::from_str(&format!("transaction deserialize error: {}", e)))?;

    // Extract Protostones from the transaction
    let protostones = format_runestone(&tx)
        .map_err(|e| JsValue::from_str(&format!("runestone analysis error: {}", e)))?;

    // Build response JSON
    let response = serde_json::json!({
        "protostone_count": protostones.len(),
        "protostones": protostones,
    });

    serde_json::to_string(&response)
        .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
}

/// Decode a PSBT (Partially Signed Bitcoin Transaction) from base64
///
/// This function decodes a PSBT from its base64 representation and returns
/// a JSON object containing detailed information about the transaction,
/// inputs, outputs, and PSBT-specific fields.
///
/// # Arguments
///
/// * `psbt_base64` - Base64 encoded PSBT string
///
/// # Returns
///
/// A JSON string containing the decoded PSBT information including:
/// - Transaction details (txid, version, locktime, inputs, outputs)
/// - Global PSBT data (xpubs)
/// - Per-input data (witness UTXOs, scripts, signatures, derivation paths)
/// - Per-output data (scripts, derivation paths)
/// - Fee information (if calculable)
///
/// # Example
///
/// ```javascript
/// const decodedPsbt = decode_psbt(psbtBase64);
/// const data = JSON.parse(decodedPsbt);
/// console.log(`TXID: ${data.tx.txid}`);
/// console.log(`Fee: ${data.fee} sats`);
/// ```
#[wasm_bindgen]
pub fn decode_psbt(psbt_base64: &str) -> Result<String, JsValue> {
    use alkanes_cli_common::psbt_utils::decode_psbt_from_base64;

    decode_psbt_from_base64(psbt_base64)
        .map(|json| serde_json::to_string(&json)
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e))))
        .map_err(|e| JsValue::from_str(&format!("PSBT decode error: {}", e)))?
}
/// Deploy a BRC20-prog contract from Foundry JSON
///
/// # Arguments
///
/// * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
/// * `foundry_json` - Foundry build JSON as string containing contract bytecode
/// * `params_json` - JSON string with execution parameters:
///   ```json
///   {
///     "from_addresses": ["address1", "address2"],  // optional
///     "change_address": "address",                  // optional
///     "fee_rate": 100.0,                            // optional, sat/vB
///     "use_activation": false,                      // optional, use 3-tx pattern
///     "use_slipstream": false,                      // optional
///     "use_rebar": false,                           // optional
///     "rebar_tier": 1,                              // optional (1 or 2)
///     "resume_from_commit": "txid"                  // optional, auto-detects commit/reveal
///   }
///   ```
///
/// # Returns
///
/// A JSON string containing:
/// - `commit_txid`: Commit transaction ID
/// - `reveal_txid`: Reveal transaction ID
/// - `activation_txid`: Activation transaction ID (if use_activation=true)
/// - `commit_fee`: Commit fee in sats
/// - `reveal_fee`: Reveal fee in sats
/// - `activation_fee`: Activation fee in sats (if applicable)
///
/// # Example
///
/// ```javascript
/// const result = await brc20_prog_deploy_contract(
///   "regtest",
///   foundryJson,
///   JSON.stringify({ fee_rate: 100, use_activation: false })
/// );
/// const data = JSON.parse(result);
/// console.log(`Deployed! Commit: ${data.commit_txid}, Reveal: ${data.reveal_txid}`);
/// ```
#[wasm_bindgen]
pub fn brc20_prog_deploy_contract(
    network: &str,
    foundry_json: &str,
    params_json: &str,
) -> Promise {
    let network_str = network.to_string();
    let foundry_json = foundry_json.to_string();
    let params_json = params_json.to_string();

    future_to_promise(async move {
        // Parse the Foundry JSON to extract bytecode
        let contract_data = parse_foundry_json(&foundry_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse Foundry JSON: {:?}", e)))?;
        let bytecode = extract_deployment_bytecode(&contract_data)
            .map_err(|e| JsValue::from_str(&format!("Failed to extract bytecode: {:?}", e)))?;

        // Create the deploy inscription
        let inscription = Brc20ProgDeployInscription::new(bytecode);
        let inscription_json = serde_json::to_string(&inscription)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize inscription: {}", e)))?;

        // Parse execution parameters
        let mut params: Brc20ProgExecuteParams = serde_json::from_str(&params_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid params JSON: {}", e)))?;
        params.inscription_content = inscription_json;

        // Create provider and executor
        let mut provider = WebProvider::new(network_str).await
            .map_err(|e| JsValue::from_str(&format!("Failed to create provider: {:?}", e)))?;
        let mut executor = Brc20ProgExecutor::new(&mut provider);

        // Execute the deployment
        let result = executor.execute(params).await
            .map_err(|e| JsValue::from_str(&format!("Deployment failed: {:?}", e)))?;

        // Return result as JSON
        serde_json::to_string(&result)
            .map(|json| JsValue::from_str(&json))
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
    })
}

/// Call a BRC20-prog contract function (transact)
///
/// # Arguments
///
/// * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
/// * `contract_address` - Contract address to call (0x-prefixed hex)
/// * `function_signature` - Function signature (e.g., "transfer(address,uint256)")
/// * `calldata` - Comma-separated calldata arguments
/// * `params_json` - JSON string with execution parameters (same as deploy_contract)
///
/// # Returns
///
/// A JSON string with transaction details (same format as deploy_contract)
///
/// # Example
///
/// ```javascript
/// const result = await brc20_prog_transact(
///   "regtest",
///   "0x1234567890abcdef1234567890abcdef12345678",
///   "transfer(address,uint256)",
///   "0xrecipient,1000",
///   JSON.stringify({ fee_rate: 100 })
/// );
/// const data = JSON.parse(result);
/// console.log(`Transaction sent! Commit: ${data.commit_txid}`);
/// ```
#[wasm_bindgen]
pub fn brc20_prog_transact(
    network: &str,
    contract_address: &str,
    function_signature: &str,
    calldata: &str,
    params_json: &str,
) -> Promise {
    let network_str = network.to_string();
    let contract_address = contract_address.to_string();
    let function_signature = function_signature.to_string();
    let calldata = calldata.to_string();
    let params_json = params_json.to_string();

    future_to_promise(async move {
        // Encode the function call
        let calldata_hex = encode_function_call(&function_signature, &calldata)
            .map_err(|e| JsValue::from_str(&format!("Failed to encode function call: {:?}", e)))?;

        // Create the call inscription
        let inscription = Brc20ProgCallInscription::new(contract_address, calldata_hex);
        let inscription_json = serde_json::to_string(&inscription)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize inscription: {}", e)))?;

        // Parse execution parameters
        let mut params: Brc20ProgExecuteParams = serde_json::from_str(&params_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid params JSON: {}", e)))?;
        params.inscription_content = inscription_json;
        params.use_activation = true; // Transact requires 3-tx pattern

        // Create provider and executor
        let mut provider = WebProvider::new(network_str).await
            .map_err(|e| JsValue::from_str(&format!("Failed to create provider: {:?}", e)))?;
        let mut executor = Brc20ProgExecutor::new(&mut provider);

        // Execute the transaction
        let result = executor.execute(params).await
            .map_err(|e| JsValue::from_str(&format!("Transaction failed: {:?}", e)))?;

        // Return result as JSON
        serde_json::to_string(&result)
            .map(|json| JsValue::from_str(&json))
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
    })
}

/// Wrap BTC into frBTC and execute a contract call in one transaction
///
/// # Arguments
///
/// * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
/// * `amount` - Amount of BTC to wrap (in satoshis)
/// * `target_contract` - Target contract address for wrapAndExecute2
/// * `function_signature` - Function signature for the target contract call
/// * `calldata` - Comma-separated calldata arguments for the target function
/// * `params_json` - JSON string with execution parameters:
///   ```json
///   {
///     "from_addresses": ["address1", "address2"],  // optional
///     "change_address": "address",                  // optional
///     "fee_rate": 100.0                             // optional, sat/vB
///   }
///   ```
///
/// # Returns
///
/// A JSON string with transaction details
///
/// # Example
///
/// ```javascript
/// const result = await brc20_prog_wrap_btc(
///   "regtest",
///   100000,  // 100k sats
///   "0xtargetContract",
///   "someFunction(uint256)",
///   "42",
///   JSON.stringify({ fee_rate: 100 })
/// );
/// const data = JSON.parse(result);
/// console.log(`frBTC wrapped! Reveal: ${data.reveal_txid}`);
/// ```
#[wasm_bindgen]
pub fn brc20_prog_wrap_btc(
    network: &str,
    amount: u64,
    target_contract: &str,
    function_signature: &str,
    calldata: &str,
    params_json: &str,
) -> Promise {
    let network_str = network.to_string();
    let target_contract = target_contract.to_string();
    let function_signature = function_signature.to_string();
    let calldata = calldata.to_string();
    let params_json = params_json.to_string();

    future_to_promise(async move {
        // Encode the target function call
        let calldata_hex = encode_function_call(&function_signature, &calldata)
            .map_err(|e| JsValue::from_str(&format!("Failed to encode function call: {:?}", e)))?;
        let calldata_bytes = hex::decode(calldata_hex.trim_start_matches("0x"))
            .map_err(|e| JsValue::from_str(&format!("Failed to decode calldata hex: {}", e)))?;

        // Parse base parameters
        #[derive(serde::Deserialize)]
        struct BaseParams {
            from_addresses: Option<Vec<String>>,
            change_address: Option<String>,
            fee_rate: Option<f32>,
        }
        let base_params: BaseParams = serde_json::from_str(&params_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid params JSON: {}", e)))?;

        // Create wrap-btc parameters
        let params = Brc20ProgWrapBtcParams {
            amount,
            target_address: target_contract,
            calldata: calldata_bytes,
            from_addresses: base_params.from_addresses,
            change_address: base_params.change_address,
            fee_rate: base_params.fee_rate,
            raw_output: false,
            trace_enabled: false,
            mine_enabled: false,
            auto_confirm: true, // WASM should auto-confirm
        };

        // Create provider and executor
        let mut provider = WebProvider::new(network_str).await
            .map_err(|e| JsValue::from_str(&format!("Failed to create provider: {:?}", e)))?;
        let mut executor = Brc20ProgWrapBtcExecutor::new(&mut provider);

        // Execute the wrap-btc
        let result = executor.wrap_btc(params).await
            .map_err(|e| JsValue::from_str(&format!("Wrap-BTC failed: {:?}", e)))?;

        // Return result as JSON
        serde_json::to_string(&result)
            .map(|json| JsValue::from_str(&json))
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
    })
}

/// Simple wrap: convert BTC to frBTC without executing any contract
///
/// This calls the wrap() function on the FrBTC contract.
///
/// # Arguments
///
/// * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
/// * `amount` - Amount of BTC to wrap (in satoshis)
/// * `params_json` - JSON string with execution parameters:
///   ```json
///   {
///     "from_addresses": ["address1", "address2"],  // optional
///     "change_address": "address",                  // optional
///     "fee_rate": 100.0                             // optional, sat/vB
///   }
///   ```
///
/// # Returns
///
/// A JSON string with transaction details
#[wasm_bindgen]
pub fn frbtc_wrap(
    network: &str,
    amount: u64,
    params_json: &str,
) -> Promise {
    use alkanes_cli_common::brc20_prog::frbtc::{FrBtcExecutor, FrBtcWrapParams};

    let network_str = network.to_string();
    let params_json = params_json.to_string();

    future_to_promise(async move {
        // Parse base parameters
        #[derive(serde::Deserialize, Default)]
        struct BaseParams {
            from_addresses: Option<Vec<String>>,
            change_address: Option<String>,
            fee_rate: Option<f32>,
            use_slipstream: Option<bool>,
            use_rebar: Option<bool>,
            rebar_tier: Option<u8>,
            resume_from_commit: Option<String>,
        }
        let base_params: BaseParams = serde_json::from_str(&params_json).unwrap_or_default();

        let params = FrBtcWrapParams {
            amount,
            from_addresses: base_params.from_addresses,
            change_address: base_params.change_address,
            fee_rate: base_params.fee_rate,
            raw_output: false,
            trace_enabled: false,
            mine_enabled: false,
            auto_confirm: true,
            use_slipstream: base_params.use_slipstream.unwrap_or(false),
            use_rebar: base_params.use_rebar.unwrap_or(false),
            rebar_tier: base_params.rebar_tier,
            resume_from_commit: base_params.resume_from_commit,
        };

        let mut provider = WebProvider::new(network_str).await
            .map_err(|e| JsValue::from_str(&format!("Failed to create provider: {:?}", e)))?;
        let mut executor = FrBtcExecutor::new(&mut provider);

        let result = executor.wrap(params).await
            .map_err(|e| JsValue::from_str(&format!("FrBTC wrap failed: {:?}", e)))?;

        serde_json::to_string(&result)
            .map(|json| JsValue::from_str(&json))
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
    })
}

/// Unwrap frBTC to BTC
///
/// This calls unwrap2() on the FrBTC contract to burn frBTC and queue a BTC payment.
///
/// # Arguments
///
/// * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
/// * `amount` - Amount of frBTC to unwrap (in satoshis)
/// * `vout` - Vout index for the inscription output
/// * `recipient_address` - Bitcoin address to receive the unwrapped BTC
/// * `params_json` - JSON string with execution parameters
///
/// # Returns
///
/// A JSON string with transaction details
#[wasm_bindgen]
pub fn frbtc_unwrap(
    network: &str,
    amount: u64,
    vout: u64,
    recipient_address: &str,
    params_json: &str,
) -> Promise {
    use alkanes_cli_common::brc20_prog::frbtc::{FrBtcExecutor, FrBtcUnwrapParams};

    let network_str = network.to_string();
    let recipient_address = recipient_address.to_string();
    let params_json = params_json.to_string();

    future_to_promise(async move {
        #[derive(serde::Deserialize, Default)]
        struct BaseParams {
            from_addresses: Option<Vec<String>>,
            change_address: Option<String>,
            fee_rate: Option<f32>,
            use_slipstream: Option<bool>,
            use_rebar: Option<bool>,
            rebar_tier: Option<u8>,
            resume_from_commit: Option<String>,
        }
        let base_params: BaseParams = serde_json::from_str(&params_json).unwrap_or_default();

        let params = FrBtcUnwrapParams {
            amount,
            vout,
            recipient_address,
            from_addresses: base_params.from_addresses,
            change_address: base_params.change_address,
            fee_rate: base_params.fee_rate,
            raw_output: false,
            trace_enabled: false,
            mine_enabled: false,
            auto_confirm: true,
            use_slipstream: base_params.use_slipstream.unwrap_or(false),
            use_rebar: base_params.use_rebar.unwrap_or(false),
            rebar_tier: base_params.rebar_tier,
            resume_from_commit: base_params.resume_from_commit,
        };

        let mut provider = WebProvider::new(network_str).await
            .map_err(|e| JsValue::from_str(&format!("Failed to create provider: {:?}", e)))?;
        let mut executor = FrBtcExecutor::new(&mut provider);

        let result = executor.unwrap(params).await
            .map_err(|e| JsValue::from_str(&format!("FrBTC unwrap failed: {:?}", e)))?;

        serde_json::to_string(&result)
            .map(|json| JsValue::from_str(&json))
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
    })
}

/// Wrap BTC and deploy+execute a script (wrapAndExecute)
///
/// This calls wrapAndExecute() on the FrBTC contract.
///
/// # Arguments
///
/// * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
/// * `amount` - Amount of BTC to wrap (in satoshis)
/// * `script_bytecode` - Script bytecode to deploy and execute (hex-encoded)
/// * `params_json` - JSON string with execution parameters
///
/// # Returns
///
/// A JSON string with transaction details
#[wasm_bindgen]
pub fn frbtc_wrap_and_execute(
    network: &str,
    amount: u64,
    script_bytecode: &str,
    params_json: &str,
) -> Promise {
    use alkanes_cli_common::brc20_prog::frbtc::{FrBtcExecutor, FrBtcWrapAndExecuteParams};

    let network_str = network.to_string();
    let script_bytecode = script_bytecode.to_string();
    let params_json = params_json.to_string();

    future_to_promise(async move {
        #[derive(serde::Deserialize, Default)]
        struct BaseParams {
            from_addresses: Option<Vec<String>>,
            change_address: Option<String>,
            fee_rate: Option<f32>,
            use_slipstream: Option<bool>,
            use_rebar: Option<bool>,
            rebar_tier: Option<u8>,
            resume_from_commit: Option<String>,
        }
        let base_params: BaseParams = serde_json::from_str(&params_json).unwrap_or_default();

        let params = FrBtcWrapAndExecuteParams {
            amount,
            script_bytecode,
            from_addresses: base_params.from_addresses,
            change_address: base_params.change_address,
            fee_rate: base_params.fee_rate,
            raw_output: false,
            trace_enabled: false,
            mine_enabled: false,
            auto_confirm: true,
            use_slipstream: base_params.use_slipstream.unwrap_or(false),
            use_rebar: base_params.use_rebar.unwrap_or(false),
            rebar_tier: base_params.rebar_tier,
            resume_from_commit: base_params.resume_from_commit,
        };

        let mut provider = WebProvider::new(network_str).await
            .map_err(|e| JsValue::from_str(&format!("Failed to create provider: {:?}", e)))?;
        let mut executor = FrBtcExecutor::new(&mut provider);

        let result = executor.wrap_and_execute(params).await
            .map_err(|e| JsValue::from_str(&format!("FrBTC wrapAndExecute failed: {:?}", e)))?;

        serde_json::to_string(&result)
            .map(|json| JsValue::from_str(&json))
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
    })
}

/// Wrap BTC and call an existing contract (wrapAndExecute2)
///
/// This calls wrapAndExecute2() on the FrBTC contract.
///
/// # Arguments
///
/// * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
/// * `amount` - Amount of BTC to wrap (in satoshis)
/// * `target_address` - Target contract address
/// * `function_signature` - Function signature (e.g., "deposit()")
/// * `calldata_args` - Comma-separated calldata arguments
/// * `params_json` - JSON string with execution parameters
///
/// # Returns
///
/// A JSON string with transaction details
#[wasm_bindgen]
pub fn frbtc_wrap_and_execute2(
    network: &str,
    amount: u64,
    target_address: &str,
    function_signature: &str,
    calldata_args: &str,
    params_json: &str,
) -> Promise {
    use alkanes_cli_common::brc20_prog::frbtc::{FrBtcExecutor, FrBtcWrapAndExecute2Params};

    let network_str = network.to_string();
    let target_address = target_address.to_string();
    let signature = function_signature.to_string();
    let calldata_args = calldata_args.to_string();
    let params_json = params_json.to_string();

    future_to_promise(async move {
        #[derive(serde::Deserialize, Default)]
        struct BaseParams {
            from_addresses: Option<Vec<String>>,
            change_address: Option<String>,
            fee_rate: Option<f32>,
            use_slipstream: Option<bool>,
            use_rebar: Option<bool>,
            rebar_tier: Option<u8>,
            resume_from_commit: Option<String>,
        }
        let base_params: BaseParams = serde_json::from_str(&params_json).unwrap_or_default();

        let params = FrBtcWrapAndExecute2Params {
            amount,
            target_address,
            signature,
            calldata_args,
            from_addresses: base_params.from_addresses,
            change_address: base_params.change_address,
            fee_rate: base_params.fee_rate,
            raw_output: false,
            trace_enabled: false,
            mine_enabled: false,
            auto_confirm: true,
            use_slipstream: base_params.use_slipstream.unwrap_or(false),
            use_rebar: base_params.use_rebar.unwrap_or(false),
            rebar_tier: base_params.rebar_tier,
            resume_from_commit: base_params.resume_from_commit,
        };

        let mut provider = WebProvider::new(network_str).await
            .map_err(|e| JsValue::from_str(&format!("Failed to create provider: {:?}", e)))?;
        let mut executor = FrBtcExecutor::new(&mut provider);

        let result = executor.wrap_and_execute2(params).await
            .map_err(|e| JsValue::from_str(&format!("FrBTC wrapAndExecute2 failed: {:?}", e)))?;

        serde_json::to_string(&result)
            .map(|json| JsValue::from_str(&json))
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
    })
}

/// Get the FrBTC signer address for a network
///
/// This calls getSignerAddress() on the FrBTC contract to get the p2tr address
/// where BTC should be sent for wrapping.
///
/// # Arguments
///
/// * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
///
/// # Returns
///
/// A JSON string containing:
/// - `network`: The network name
/// - `frbtc_contract`: The FrBTC contract address
/// - `signer_address`: The Bitcoin p2tr address for the signer
#[wasm_bindgen]
pub fn frbtc_get_signer_address(network: &str) -> Promise {
    use alkanes_cli_common::brc20_prog::frbtc::{FrBtcExecutor, get_frbtc_contract_address};

    let network_str = network.to_string();

    future_to_promise(async move {
        let mut provider = WebProvider::new(network_str.clone()).await
            .map_err(|e| JsValue::from_str(&format!("Failed to create provider: {:?}", e)))?;

        let bitcoin_network = provider.network();
        let frbtc_contract = get_frbtc_contract_address(bitcoin_network);

        let mut executor = FrBtcExecutor::new(&mut provider);
        let signer_address = executor.get_signer_address().await
            .map_err(|e| JsValue::from_str(&format!("Failed to get signer address: {:?}", e)))?;

        let result = serde_json::json!({
            "network": network_str,
            "frbtc_contract": frbtc_contract,
            "signer_address": signer_address,
        });

        serde_json::to_string(&result)
            .map(|json| JsValue::from_str(&json))
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
    })
}
