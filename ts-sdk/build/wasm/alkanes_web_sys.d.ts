/* tslint:disable */
/* eslint-disable */
/**
 * Asynchronously encrypts data using the Web Crypto API.
 */
export function encryptMnemonic(mnemonic: string, passphrase: string): Promise<any>;
/**
 * Initialize the panic hook for better error messages in WASM
 * This should be called early in your application
 */
export function init_panic_hook(): void;
export function analyze_psbt(psbt_base64: string, network_str: string): string;
export function simulate_alkane_call(alkane_id_str: string, wasm_hex: string, cellpack_hex: string): Promise<any>;
export function get_alkane_bytecode(network: string, block: number, tx: number, block_tag: string): Promise<any>;
export function get_alkane_meta(network: string, block: number, tx: number, block_tag: string): Promise<any>;
/**
 * Analyze a transaction's runestone to extract Protostones
 *
 * This function takes a raw transaction hex string, decodes it, and extracts
 * all Protostones from the transaction's OP_RETURN output.
 *
 * # Arguments
 *
 * * `tx_hex` - Hexadecimal string of the raw transaction (with or without "0x" prefix)
 *
 * # Returns
 *
 * A JSON string containing:
 * - `protostone_count`: Number of Protostones found
 * - `protostones`: Array of Protostone objects with their details
 *
 * # Example
 *
 * ```javascript
 * const result = analyze_runestone(txHex);
 * const data = JSON.parse(result);
 * console.log(`Found ${data.protostone_count} Protostones`);
 * ```
 */
export function analyze_runestone(tx_hex: string): string;
/**
 * Decode a PSBT (Partially Signed Bitcoin Transaction) from base64
 *
 * This function decodes a PSBT from its base64 representation and returns
 * a JSON object containing detailed information about the transaction,
 * inputs, outputs, and PSBT-specific fields.
 *
 * # Arguments
 *
 * * `psbt_base64` - Base64 encoded PSBT string
 *
 * # Returns
 *
 * A JSON string containing the decoded PSBT information including:
 * - Transaction details (txid, version, locktime, inputs, outputs)
 * - Global PSBT data (xpubs)
 * - Per-input data (witness UTXOs, scripts, signatures, derivation paths)
 * - Per-output data (scripts, derivation paths)
 * - Fee information (if calculable)
 *
 * # Example
 *
 * ```javascript
 * const decodedPsbt = decode_psbt(psbtBase64);
 * const data = JSON.parse(decodedPsbt);
 * console.log(`TXID: ${data.tx.txid}`);
 * console.log(`Fee: ${data.fee} sats`);
 * ```
 */
export function decode_psbt(psbt_base64: string): string;
/**
 * Deploy a BRC20-prog contract from Foundry JSON
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `foundry_json` - Foundry build JSON as string containing contract bytecode
 * * `params_json` - JSON string with execution parameters:
 *   ```json
 *   {
 *     "from_addresses": ["address1", "address2"],  // optional
 *     "change_address": "address",                  // optional
 *     "fee_rate": 100.0,                            // optional, sat/vB
 *     "use_activation": false,                      // optional, use 3-tx pattern
 *     "use_slipstream": false,                      // optional
 *     "use_rebar": false,                           // optional
 *     "rebar_tier": 1,                              // optional (1 or 2)
 *     "resume_from_commit": "txid"                  // optional, auto-detects commit/reveal
 *   }
 *   ```
 *
 * # Returns
 *
 * A JSON string containing:
 * - `commit_txid`: Commit transaction ID
 * - `reveal_txid`: Reveal transaction ID
 * - `activation_txid`: Activation transaction ID (if use_activation=true)
 * - `commit_fee`: Commit fee in sats
 * - `reveal_fee`: Reveal fee in sats
 * - `activation_fee`: Activation fee in sats (if applicable)
 *
 * # Example
 *
 * ```javascript
 * const result = await brc20_prog_deploy_contract(
 *   "regtest",
 *   foundryJson,
 *   JSON.stringify({ fee_rate: 100, use_activation: false })
 * );
 * const data = JSON.parse(result);
 * console.log(`Deployed! Commit: ${data.commit_txid}, Reveal: ${data.reveal_txid}`);
 * ```
 */
export function brc20_prog_deploy_contract(network: string, foundry_json: string, params_json: string): Promise<any>;
/**
 * Call a BRC20-prog contract function (transact)
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `contract_address` - Contract address to call (0x-prefixed hex)
 * * `function_signature` - Function signature (e.g., "transfer(address,uint256)")
 * * `calldata` - Comma-separated calldata arguments
 * * `params_json` - JSON string with execution parameters (same as deploy_contract)
 *
 * # Returns
 *
 * A JSON string with transaction details (same format as deploy_contract)
 *
 * # Example
 *
 * ```javascript
 * const result = await brc20_prog_transact(
 *   "regtest",
 *   "0x1234567890abcdef1234567890abcdef12345678",
 *   "transfer(address,uint256)",
 *   "0xrecipient,1000",
 *   JSON.stringify({ fee_rate: 100 })
 * );
 * const data = JSON.parse(result);
 * console.log(`Transaction sent! Commit: ${data.commit_txid}`);
 * ```
 */
export function brc20_prog_transact(network: string, contract_address: string, function_signature: string, calldata: string, params_json: string): Promise<any>;
/**
 * Wrap BTC into frBTC and execute a contract call in one transaction
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `amount` - Amount of BTC to wrap (in satoshis)
 * * `target_contract` - Target contract address for wrapAndExecute2
 * * `function_signature` - Function signature for the target contract call
 * * `calldata` - Comma-separated calldata arguments for the target function
 * * `params_json` - JSON string with execution parameters:
 *   ```json
 *   {
 *     "from_addresses": ["address1", "address2"],  // optional
 *     "change_address": "address",                  // optional
 *     "fee_rate": 100.0                             // optional, sat/vB
 *   }
 *   ```
 *
 * # Returns
 *
 * A JSON string with transaction details
 *
 * # Example
 *
 * ```javascript
 * const result = await brc20_prog_wrap_btc(
 *   "regtest",
 *   100000,  // 100k sats
 *   "0xtargetContract",
 *   "someFunction(uint256)",
 *   "42",
 *   JSON.stringify({ fee_rate: 100 })
 * );
 * const data = JSON.parse(result);
 * console.log(`frBTC wrapped! Reveal: ${data.reveal_txid}`);
 * ```
 */
export function brc20_prog_wrap_btc(network: string, amount: bigint, target_contract: string, function_signature: string, calldata: string, params_json: string): Promise<any>;
/**
 * Simple wrap: convert BTC to frBTC without executing any contract
 *
 * This calls the wrap() function on the FrBTC contract.
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `amount` - Amount of BTC to wrap (in satoshis)
 * * `params_json` - JSON string with execution parameters:
 *   ```json
 *   {
 *     "from_addresses": ["address1", "address2"],  // optional
 *     "change_address": "address",                  // optional
 *     "fee_rate": 100.0                             // optional, sat/vB
 *   }
 *   ```
 *
 * # Returns
 *
 * A JSON string with transaction details
 */
export function frbtc_wrap(network: string, amount: bigint, params_json: string): Promise<any>;
/**
 * Unwrap frBTC to BTC
 *
 * This calls unwrap2() on the FrBTC contract to burn frBTC and queue a BTC payment.
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `amount` - Amount of frBTC to unwrap (in satoshis)
 * * `vout` - Vout index for the inscription output
 * * `recipient_address` - Bitcoin address to receive the unwrapped BTC
 * * `params_json` - JSON string with execution parameters
 *
 * # Returns
 *
 * A JSON string with transaction details
 */
export function frbtc_unwrap(network: string, amount: bigint, vout: bigint, recipient_address: string, params_json: string): Promise<any>;
/**
 * Wrap BTC and deploy+execute a script (wrapAndExecute)
 *
 * This calls wrapAndExecute() on the FrBTC contract.
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `amount` - Amount of BTC to wrap (in satoshis)
 * * `script_bytecode` - Script bytecode to deploy and execute (hex-encoded)
 * * `params_json` - JSON string with execution parameters
 *
 * # Returns
 *
 * A JSON string with transaction details
 */
export function frbtc_wrap_and_execute(network: string, amount: bigint, script_bytecode: string, params_json: string): Promise<any>;
/**
 * Wrap BTC and call an existing contract (wrapAndExecute2)
 *
 * This calls wrapAndExecute2() on the FrBTC contract.
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 * * `amount` - Amount of BTC to wrap (in satoshis)
 * * `target_address` - Target contract address
 * * `function_signature` - Function signature (e.g., "deposit()")
 * * `calldata_args` - Comma-separated calldata arguments
 * * `params_json` - JSON string with execution parameters
 *
 * # Returns
 *
 * A JSON string with transaction details
 */
export function frbtc_wrap_and_execute2(network: string, amount: bigint, target_address: string, function_signature: string, calldata_args: string, params_json: string): Promise<any>;
/**
 * Get the FrBTC signer address for a network
 *
 * This calls getSignerAddress() on the FrBTC contract to get the p2tr address
 * where BTC should be sent for wrapping.
 *
 * # Arguments
 *
 * * `network` - Network to use ("mainnet", "testnet", "signet", "regtest")
 *
 * # Returns
 *
 * A JSON string containing:
 * - `network`: The network name
 * - `frbtc_contract`: The FrBTC contract address
 * - `signer_address`: The Bitcoin p2tr address for the signer
 */
export function frbtc_get_signer_address(network: string): Promise<any>;
export interface PoolWithDetails {
    pool_id_block: number;
    pool_id_tx: number;
    details: PoolDetails | null;
}

export interface BatchPoolsResponse {
    pool_count: number;
    pools: PoolWithDetails[];
}

/**
 * Represents the entire JSON keystore, compatible with wasm-bindgen.
 */
export class Keystore {
  free(): void;
  [Symbol.dispose](): void;
  constructor(val: any);
  to_js(): any;
  accountXpub(): string;
  hdPaths(): any;
  masterFingerprint(): string;
  decryptMnemonic(passphrase: string): Promise<any>;
}
/**
 * Parameters for the PBKDF2/S2K key derivation function.
 */
export class PbkdfParams {
  free(): void;
  [Symbol.dispose](): void;
  constructor(val: any);
  to_js(): any;
}
/**
 * WASM-exported BrowserWalletProvider that can be created from JavaScript
 */
export class WasmBrowserWalletProvider {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new BrowserWalletProvider from a JavaScript wallet adapter
   *
   * @param adapter - A JavaScript object implementing the JsWalletAdapter interface
   * @param network - Network string ("mainnet", "testnet", "signet", "regtest")
   * @returns Promise<WasmBrowserWalletProvider>
   */
  constructor(adapter: JsWalletAdapter, network: string);
  /**
   * Get the connected wallet address
   */
  getAddress(): string | undefined;
  /**
   * Get the wallet public key
   */
  getPublicKey(): Promise<string>;
  /**
   * Sign a PSBT (hex encoded)
   */
  signPsbt(psbt_hex: string, options: any): Promise<string>;
  /**
   * Sign a message
   */
  signMessage(message: string, address?: string | null): Promise<string>;
  /**
   * Broadcast a transaction
   */
  broadcastTransaction(tx_hex: string): Promise<string>;
  /**
   * Get balance
   */
  getBalance(): Promise<any>;
  /**
   * Get UTXOs
   */
  getUtxos(include_frozen: boolean): Promise<any>;
  /**
   * Get enriched UTXOs with asset information
   */
  getEnrichedUtxos(): Promise<any>;
  /**
   * Get all balances (BTC + alkanes)
   */
  getAllBalances(): Promise<any>;
  /**
   * Get wallet info
   */
  getWalletInfo(): any;
  /**
   * Get connection status
   */
  getConnectionStatus(): string;
  /**
   * Get current network
   */
  getNetwork(): string;
  /**
   * Disconnect from the wallet
   */
  disconnect(): Promise<void>;
}
/**
 * Web-compatible provider implementation for browser environments
 *
 * The `WebProvider` is the main entry point for using deezel functionality in web browsers
 * and WASM environments. It implements all deezel-common traits using web-standard APIs,
 * providing complete Bitcoin wallet and Alkanes metaprotocol functionality.
 *
 * # Features
 *
 * - **Bitcoin Operations**: Full wallet functionality, transaction creation, and broadcasting
 * - **Alkanes Integration**: Smart contract execution, token operations, and AMM functionality
 * - **Web Standards**: Uses fetch API, localStorage, Web Crypto API, and console logging
 * - **Network Support**: Configurable for mainnet, testnet, signet, regtest, and custom networks
 * - **Privacy Features**: Rebar Labs Shield integration for private transaction broadcasting
 *
 * # Example
 *
 * ```rust,no_run
 * use deezel_web::WebProvider;
 * use alkanes_cli_common::*;
 *
 * async fn create_provider() -> Result<WebProvider> {
 *     let provider = WebProvider::new("mainnet".to_string()).await?;
 *
 *     provider.initialize().await?;
 *     Ok(provider)
 * }
 * ```
 */
export class WebProvider {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new WebProvider from provider name and optional config overrides
   * 
   * # Arguments
   * * `provider` - Network provider: "mainnet", "signet", "subfrost-regtest", "regtest"
   * * `config` - Optional JS object with RpcConfig fields to override defaults
   *
   * # Example (JavaScript)
   * ```js
   * // Simple - uses all defaults for signet
   * const provider = new WebProvider("signet");
   * 
   * // With overrides
   * const provider = new WebProvider("signet", {
   *   bitcoin_rpc_url: "https://custom-rpc.example.com",
   *   esplora_url: "https://custom-esplora.example.com"
   * });
   * ```
   */
  constructor(provider: string, config?: any | null);
  sandshrew_rpc_url(): string;
  esplora_rpc_url(): string | undefined;
  bitcoin_rpc_url(): string;
  brc20_prog_rpc_url(): string;
  /**
   * Get enriched wallet balances using the balances.lua script
   * 
   * This uses the built-in balances.lua script with automatic hash-based caching.
   * Returns comprehensive balance data including spendable UTXOs, asset UTXOs, and pending.
   */
  getEnrichedBalances(address: string, protocol_tag?: string | null): Promise<any>;
  /**
   * Get all transactions for an address from Esplora
   */
  getAddressTxs(address: string): Promise<any>;
  /**
   * Get raw transaction hex
   */
  getTransactionHex(txid: string): Promise<any>;
  /**
   * Trace alkanes execution for a protostone outpoint
   */
  traceOutpoint(outpoint: string): Promise<any>;
  /**
   * Get address UTXOs
   */
  getAddressUtxos(address: string): Promise<any>;
  /**
   * Broadcast a raw transaction
   */
  broadcastTransaction(tx_hex: string): Promise<any>;
  /**
   * Get address transactions with complete runestone traces (CLI: esplora address-txs --runestone-trace)
   */
  getAddressTxsWithTraces(address: string, exclude_coinbase?: boolean | null): Promise<any>;
  ordInscription(inscription_id: string): Promise<any>;
  ordInscriptions(page?: number | null): Promise<any>;
  ordOutputs(address: string): Promise<any>;
  ordRune(rune: string): Promise<any>;
  ordAddressInfo(address: string): Promise<any>;
  ordBlockInfo(query: string): Promise<any>;
  ordBlockCount(): Promise<any>;
  ordBlocks(): Promise<any>;
  ordChildren(inscription_id: string, page?: number | null): Promise<any>;
  ordContent(inscription_id: string): Promise<any>;
  ordParents(inscription_id: string, page?: number | null): Promise<any>;
  ordTxInfo(txid: string): Promise<any>;
  /**
   * Execute an alkanes smart contract
   */
  alkanesExecute(params_json: string): Promise<any>;
  /**
   * Execute an alkanes smart contract using CLI-style string parameters
   * This is the recommended method for executing alkanes contracts as it supports
   * the same parameter format as alkanes-cli.
   *
   * # Parameters
   * - `to_addresses`: JSON array of recipient addresses
   * - `input_requirements`: String format like "B:10000" or "2:0:1000" (alkane block:tx:amount)
   * - `protostones`: String format like "[32,0,77]:v0:v0" (cellpack:pointer:refund)
   * - `fee_rate`: Optional fee rate in sat/vB
   * - `envelope_hex`: Optional envelope data as hex string
   * - `options_json`: Optional JSON with additional options (trace_enabled, mine_enabled, auto_confirm, raw_output)
   */
  alkanesExecuteWithStrings(to_addresses_json: string, input_requirements: string, protostones: string, fee_rate?: number | null, envelope_hex?: string | null, options_json?: string | null): Promise<any>;
  /**
   * Execute an alkanes smart contract fully (handles complete flow internally)
   *
   * This method handles the complete execution flow:
   * - For deployments (with envelope): commit -> reveal -> mine -> trace
   * - For simple transactions: sign -> broadcast -> mine -> trace
   *
   * Returns the final EnhancedExecuteResult directly, avoiding serialization issues
   * with intermediate states.
   */
  alkanesExecuteFull(to_addresses_json: string, input_requirements: string, protostones: string, fee_rate?: number | null, envelope_hex?: string | null, options_json?: string | null): Promise<any>;
  /**
   * Resume execution after user confirmation (for simple transactions)
   */
  alkanesResumeExecution(state_json: string, params_json: string): Promise<any>;
  /**
   * Resume execution after commit transaction confirmation
   */
  alkanesResumeCommitExecution(state_json: string): Promise<any>;
  /**
   * Resume execution after reveal transaction confirmation
   */
  alkanesResumeRevealExecution(state_json: string): Promise<any>;
  /**
   * Simulate an alkanes contract call (read-only)
   */
  alkanesSimulate(contract_id: string, context_json: string, block_tag?: string | null): Promise<any>;
  /**
   * Wrap BTC to frBTC
   */
  alkanesWrapBtc(params_json: string): Promise<any>;
  /**
   * Initialize a new AMM liquidity pool
   */
  alkanesInitPool(params_json: string): Promise<any>;
  /**
   * Execute an AMM swap
   */
  alkanesSwap(params_json: string): Promise<any>;
  /**
   * Reflect metadata for a range of alkanes
   */
  alkanesReflectAlkaneRange(block: number, start_tx: number, end_tx: number, concurrency?: number | null): Promise<any>;
  /**
   * Execute a tx-script with WASM bytecode
   */
  alkanesTxScript(wasm_hex: string, inputs_json: string, block_tag?: string | null): Promise<any>;
  /**
   * Get pool details for a specific pool
   */
  alkanesPoolDetails(pool_id: string): Promise<any>;
  /**
   * Calculate minimum unwrap amount for subfrost frBTC unwrapping
   */
  subfrostMinimumUnwrap(fee_rate_override?: number | null, premium?: number | null, expected_inputs?: number | null, expected_outputs?: number | null, raw?: boolean | null): Promise<any>;
  /**
   * Get OPI block height
   */
  opiBlockHeight(base_url: string): Promise<any>;
  /**
   * Get OPI extras block height
   */
  opiExtrasBlockHeight(base_url: string): Promise<any>;
  /**
   * Get OPI database version
   */
  opiDbVersion(base_url: string): Promise<any>;
  /**
   * Get OPI event hash version
   */
  opiEventHashVersion(base_url: string): Promise<any>;
  /**
   * Get OPI balance on block
   */
  opiBalanceOnBlock(base_url: string, block_height: number, pkscript: string, ticker: string): Promise<any>;
  /**
   * Get OPI activity on block
   */
  opiActivityOnBlock(base_url: string, block_height: number): Promise<any>;
  /**
   * Get OPI Bitcoin RPC results on block
   */
  opiBitcoinRpcResultsOnBlock(base_url: string, block_height: number): Promise<any>;
  /**
   * Get OPI current balance
   */
  opiCurrentBalance(base_url: string, ticker: string, address?: string | null, pkscript?: string | null): Promise<any>;
  /**
   * Get OPI valid tx notes of wallet
   */
  opiValidTxNotesOfWallet(base_url: string, address?: string | null, pkscript?: string | null): Promise<any>;
  /**
   * Get OPI valid tx notes of ticker
   */
  opiValidTxNotesOfTicker(base_url: string, ticker: string): Promise<any>;
  /**
   * Get OPI holders
   */
  opiHolders(base_url: string, ticker: string): Promise<any>;
  /**
   * Get OPI hash of all activity
   */
  opiHashOfAllActivity(base_url: string, block_height: number): Promise<any>;
  /**
   * Get OPI hash of all current balances
   */
  opiHashOfAllCurrentBalances(base_url: string): Promise<any>;
  /**
   * Get OPI event
   */
  opiEvent(base_url: string, event_hash: string): Promise<any>;
  /**
   * Get OPI IP address
   */
  opiIp(base_url: string): Promise<any>;
  /**
   * Get OPI raw endpoint
   */
  opiRaw(base_url: string, endpoint: string): Promise<any>;
  /**
   * Get OPI Runes block height
   */
  opiRunesBlockHeight(base_url: string): Promise<any>;
  /**
   * Get OPI Runes balance on block
   */
  opiRunesBalanceOnBlock(base_url: string, block_height: number, pkscript: string, rune_id: string): Promise<any>;
  /**
   * Get OPI Runes activity on block
   */
  opiRunesActivityOnBlock(base_url: string, block_height: number): Promise<any>;
  /**
   * Get OPI Runes current balance
   */
  opiRunesCurrentBalance(base_url: string, address?: string | null, pkscript?: string | null): Promise<any>;
  /**
   * Get OPI Runes unspent outpoints
   */
  opiRunesUnspentOutpoints(base_url: string, address?: string | null, pkscript?: string | null): Promise<any>;
  /**
   * Get OPI Runes holders
   */
  opiRunesHolders(base_url: string, rune_id: string): Promise<any>;
  /**
   * Get OPI Runes hash of all activity
   */
  opiRunesHashOfAllActivity(base_url: string, block_height: number): Promise<any>;
  /**
   * Get OPI Runes event
   */
  opiRunesEvent(base_url: string, txid: string): Promise<any>;
  /**
   * Get OPI Bitmap block height
   */
  opiBitmapBlockHeight(base_url: string): Promise<any>;
  /**
   * Get OPI Bitmap hash of all activity
   */
  opiBitmapHashOfAllActivity(base_url: string, block_height: number): Promise<any>;
  /**
   * Get OPI Bitmap hash of all bitmaps
   */
  opiBitmapHashOfAllBitmaps(base_url: string): Promise<any>;
  /**
   * Get OPI Bitmap inscription ID
   */
  opiBitmapInscriptionId(base_url: string, bitmap: string): Promise<any>;
  /**
   * Get OPI POW20 block height
   */
  opiPow20BlockHeight(base_url: string): Promise<any>;
  /**
   * Get OPI POW20 balance on block
   */
  opiPow20BalanceOnBlock(base_url: string, block_height: number, pkscript: string, ticker: string): Promise<any>;
  /**
   * Get OPI POW20 activity on block
   */
  opiPow20ActivityOnBlock(base_url: string, block_height: number): Promise<any>;
  /**
   * Get OPI POW20 current balance
   */
  opiPow20CurrentBalance(base_url: string, ticker: string, address?: string | null, pkscript?: string | null): Promise<any>;
  /**
   * Get OPI POW20 valid tx notes of wallet
   */
  opiPow20ValidTxNotesOfWallet(base_url: string, address?: string | null, pkscript?: string | null): Promise<any>;
  /**
   * Get OPI POW20 valid tx notes of ticker
   */
  opiPow20ValidTxNotesOfTicker(base_url: string, ticker: string): Promise<any>;
  /**
   * Get OPI POW20 holders
   */
  opiPow20Holders(base_url: string, ticker: string): Promise<any>;
  /**
   * Get OPI POW20 hash of all activity
   */
  opiPow20HashOfAllActivity(base_url: string, block_height: number): Promise<any>;
  /**
   * Get OPI POW20 hash of all current balances
   */
  opiPow20HashOfAllCurrentBalances(base_url: string): Promise<any>;
  /**
   * Get OPI SNS block height
   */
  opiSnsBlockHeight(base_url: string): Promise<any>;
  /**
   * Get OPI SNS hash of all activity
   */
  opiSnsHashOfAllActivity(base_url: string, block_height: number): Promise<any>;
  /**
   * Get OPI SNS hash of all registered names
   */
  opiSnsHashOfAllRegisteredNames(base_url: string): Promise<any>;
  /**
   * Get OPI SNS info
   */
  opiSnsInfo(base_url: string, name: string): Promise<any>;
  /**
   * Get OPI SNS inscriptions of domain
   */
  opiSnsInscriptionsOfDomain(base_url: string, domain: string): Promise<any>;
  /**
   * Get OPI SNS registered namespaces
   */
  opiSnsRegisteredNamespaces(base_url: string): Promise<any>;
  /**
   * Get alkanes contract balance for an address
   */
  alkanesBalance(address?: string | null): Promise<any>;
  /**
   * Get alkanes contract bytecode
   */
  alkanesBytecode(alkane_id: string, block_tag?: string | null): Promise<any>;
  /**
   * Get metadata (ABI) for an alkanes contract
   */
  alkanesMeta(alkane_id: string, block_tag?: string | null): Promise<any>;
  /**
   * Get all pools with details from an AMM factory (parallel optimized for browser)
   */
  alkanesGetAllPoolsWithDetails(factory_id: string, chunk_size?: number | null, max_concurrent?: number | null): Promise<any>;
  /**
   * Get all pools from a factory (lightweight, IDs only)
   */
  alkanesGetAllPools(factory_id: string): Promise<any>;
  /**
   * Get pool details including reserves using simulation
   */
  ammGetPoolDetails(pool_id: string): Promise<any>;
  alkanesTrace(outpoint: string): Promise<any>;
  traceProtostones(txid: string): Promise<any>;
  traceBlock(height: number): Promise<any>;
  alkanesByAddress(address: string, block_tag?: string | null, protocol_tag?: number | null): Promise<any>;
  alkanesByOutpoint(outpoint: string, block_tag?: string | null, protocol_tag?: number | null): Promise<any>;
  esploraGetTx(txid: string): Promise<any>;
  esploraGetTxStatus(txid: string): Promise<any>;
  esploraGetAddressInfo(address: string): Promise<any>;
  esploraGetBlocksTipHeight(): Promise<any>;
  esploraGetBlocksTipHash(): Promise<any>;
  esploraGetAddressUtxo(address: string): Promise<any>;
  esploraGetAddressTxs(address: string): Promise<any>;
  esploraGetAddressTxsChain(address: string, last_seen_txid?: string | null): Promise<any>;
  getStorageAt(block: bigint, tx: bigint, path: Uint8Array): Promise<any>;
  esploraGetFeeEstimates(): Promise<any>;
  esploraBroadcastTx(tx_hex: string): Promise<any>;
  esploraGetTxHex(txid: string): Promise<any>;
  esploraGetBlocks(start_height?: number | null): Promise<any>;
  esploraGetBlockByHeight(height: number): Promise<any>;
  esploraGetBlock(hash: string): Promise<any>;
  esploraGetBlockStatus(hash: string): Promise<any>;
  esploraGetBlockTxids(hash: string): Promise<any>;
  esploraGetBlockHeader(hash: string): Promise<any>;
  esploraGetBlockRaw(hash: string): Promise<any>;
  esploraGetBlockTxid(hash: string, index: number): Promise<any>;
  esploraGetBlockTxs(hash: string, start_index?: number | null): Promise<any>;
  esploraGetAddressTxsMempool(address: string): Promise<any>;
  esploraGetAddressPrefix(prefix: string): Promise<any>;
  esploraGetTxRaw(txid: string): Promise<any>;
  esploraGetTxMerkleProof(txid: string): Promise<any>;
  esploraGetTxMerkleblockProof(txid: string): Promise<any>;
  esploraGetTxOutspend(txid: string, index: number): Promise<any>;
  esploraGetTxOutspends(txid: string): Promise<any>;
  esploraGetMempool(): Promise<any>;
  esploraGetMempoolTxids(): Promise<any>;
  esploraGetMempoolRecent(): Promise<any>;
  esploraPostTx(tx_hex: string): Promise<any>;
  bitcoindGetBlockCount(): Promise<any>;
  bitcoindSendRawTransaction(tx_hex: string): Promise<any>;
  bitcoindGenerateToAddress(nblocks: number, address: string): Promise<any>;
  bitcoindGenerateFuture(address: string): Promise<any>;
  bitcoindGetBlockchainInfo(): Promise<any>;
  bitcoindGetNetworkInfo(): Promise<any>;
  bitcoindGetRawTransaction(txid: string, block_hash?: string | null): Promise<any>;
  bitcoindGetBlock(hash: string, raw: boolean): Promise<any>;
  bitcoindGetBlockHash(height: number): Promise<any>;
  bitcoindGetBlockHeader(hash: string): Promise<any>;
  bitcoindGetBlockStats(hash: string): Promise<any>;
  bitcoindGetMempoolInfo(): Promise<any>;
  bitcoindEstimateSmartFee(target: number): Promise<any>;
  bitcoindGetChainTips(): Promise<any>;
  bitcoindGetRawMempool(): Promise<any>;
  bitcoindGetTxOut(txid: string, vout: number, include_mempool: boolean): Promise<any>;
  bitcoindDecodeRawTransaction(hex: string): Promise<any>;
  bitcoindDecodePsbt(psbt: string): Promise<any>;
  alkanesView(contract_id: string, view_fn: string, params?: Uint8Array | null, block_tag?: string | null): Promise<any>;
  alkanesInspect(target: string, config: any): Promise<any>;
  /**
   * Inspect alkanes bytecode directly from WASM bytes (hex-encoded or raw bytes)
   * This allows inspection without fetching from RPC - useful for local/offline analysis
   */
  alkanesInspectBytecode(bytecode_hex: string, alkane_id: string, config: any): Promise<any>;
  alkanesPendingUnwraps(block_tag?: string | null): Promise<any>;
  brc20progCall(to: string, data: string, block?: string | null): Promise<any>;
  brc20progGetBalance(address: string, block?: string | null): Promise<any>;
  brc20progGetCode(address: string): Promise<any>;
  brc20progGetTransactionCount(address: string, block?: string | null): Promise<any>;
  brc20progBlockNumber(): Promise<any>;
  brc20progChainId(): Promise<any>;
  brc20progGetTransactionReceipt(tx_hash: string): Promise<any>;
  brc20progGetTransactionByHash(tx_hash: string): Promise<any>;
  brc20progGetBlockByNumber(block: string, full_tx: boolean): Promise<any>;
  brc20progEstimateGas(to: string, data: string, block?: string | null): Promise<any>;
  brc20progGetLogs(filter: any): Promise<any>;
  brc20progWeb3ClientVersion(): Promise<any>;
  metashrewHeight(): Promise<any>;
  waitForIndexer(): Promise<any>;
  metashrewStateRoot(height?: number | null): Promise<any>;
  metashrewGetBlockHash(height: number): Promise<any>;
  /**
   * Generic metashrew_view call
   *
   * Calls the metashrew_view RPC method with the given view function, payload, and block tag.
   * This is the low-level method for calling any metashrew view function.
   *
   * # Arguments
   * * `view_fn` - The view function name (e.g., "simulate", "protorunesbyaddress")
   * * `payload` - The hex-encoded payload (with or without 0x prefix)
   * * `block_tag` - The block tag ("latest" or a block height as string)
   *
   * # Returns
   * The hex-encoded response string from the view function
   */
  metashrewView(view_fn: string, payload: string, block_tag: string): Promise<any>;
  luaEvalScript(script: string): Promise<any>;
  /**
   * Execute a Lua script with arguments, using scripthash caching
   *
   * This method first tries to use the cached scripthash version (lua_evalsaved),
   * and falls back to the full script (lua_evalscript) if the hash isn't cached.
   * This is the recommended way to execute Lua scripts for better performance.
   *
   * # Arguments
   * * `script` - The Lua script content
   * * `args` - JSON-serialized array of arguments to pass to the script
   */
  luaEval(script: string, args: any): Promise<any>;
  ordList(outpoint: string): Promise<any>;
  ordFind(sat: number): Promise<any>;
  runestoneDecodeTx(txid: string): Promise<any>;
  runestoneAnalyzeTx(txid: string): Promise<any>;
  protorunesDecodeTx(txid: string): Promise<any>;
  protorunesAnalyzeTx(txid: string): Promise<any>;
  /**
   * Create a new wallet with an optional mnemonic phrase
   * If no mnemonic is provided, a new one will be generated
   * Returns wallet info including address and mnemonic
   *
   * Note: This sets the keystore on self synchronously so walletIsLoaded() returns true immediately
   */
  walletCreate(mnemonic?: string | null, passphrase?: string | null): any;
  /**
   * Load an existing wallet from storage
   */
  walletLoad(passphrase?: string | null): Promise<any>;
  /**
   * Get the wallet's primary address
   */
  walletGetAddress(): Promise<any>;
  /**
   * Get the wallet's BTC balance
   * Returns { confirmed: number, pending: number }
   */
  walletGetBalance(addresses?: string[] | null): Promise<any>;
  /**
   * Load a wallet from mnemonic for signing transactions
   * This must be called before walletSend or other signing operations
   */
  walletLoadMnemonic(mnemonic_str: string, passphrase?: string | null): void;
  /**
   * Check if wallet is loaded (has keystore for signing)
   */
  walletIsLoaded(): boolean;
  /**
   * Get addresses from the loaded wallet keystore
   * Uses the Keystore.get_addresses method from alkanes-cli-common
   *
   * # Arguments
   * * `address_type` - Address type: "p2tr", "p2wpkh", "p2sh-p2wpkh", "p2pkh"
   * * `start_index` - Starting index for address derivation
   * * `count` - Number of addresses to derive
   * * `chain` - Chain index (0 for external/receiving, 1 for internal/change)
   *
   * # Returns
   * Array of address info objects with: { derivation_path, address, script_type, index, used }
   */
  walletGetAddresses(address_type: string, start_index: number, count: number, chain?: number | null): any;
  /**
   * Send BTC to an address
   * params: { address: string, amount: number (satoshis), fee_rate?: number }
   * Wallet must be loaded first via walletLoadMnemonic
   */
  walletSend(params_json: string): Promise<any>;
  /**
   * Get UTXOs for the wallet
   */
  walletGetUtxos(addresses?: string[] | null): Promise<any>;
  /**
   * Get transaction history for an address
   */
  walletGetHistory(address?: string | null): Promise<any>;
  walletCreatePsbt(params_json: string): Promise<any>;
  walletExport(): Promise<any>;
  walletBackup(): Promise<any>;
  /**
   * Get the FrBTC signer address for the current network
   */
  frbtcGetSignerAddress(): Promise<any>;
  /**
   * Wrap BTC to frBTC
   * params_json: { fee_rate?: number, from?: string[], change?: string }
   */
  frbtcWrap(amount: bigint, params_json: string): Promise<any>;
  /**
   * Unwrap frBTC to BTC
   * params_json: { fee_rate?: number, from?: string[], change?: string }
   */
  frbtcUnwrap(amount: bigint, vout: bigint, recipient_address: string, params_json: string): Promise<any>;
  /**
   * Wrap BTC and deploy+execute a script (wrapAndExecute)
   * params_json: { fee_rate?: number, from_addresses?: string[], change_address?: string, ... }
   */
  frbtcWrapAndExecute(amount: bigint, script_bytecode: string, params_json: string): Promise<any>;
  /**
   * Wrap BTC and call an existing contract (wrapAndExecute2)
   * params_json: { fee_rate?: number, from_addresses?: string[], change_address?: string, ... }
   */
  frbtcWrapAndExecute2(amount: bigint, target_address: string, signature: string, calldata_args: string, params_json: string): Promise<any>;
  dataApiGetPoolHistory(pool_id: string, category?: string | null, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetPools(factory_id: string): Promise<any>;
  dataApiGetAlkanesByAddress(address: string): Promise<any>;
  dataApiGetAllPoolsDetails(factory_id: string, limit?: bigint | null, offset?: bigint | null, sort_by?: string | null, order?: string | null): Promise<any>;
  dataApiGetPoolDetails(factory_id: string, pool_id: string): Promise<any>;
  dataApiGetAddressBalances(address: string, include_outpoints: boolean): Promise<any>;
  dataApiGetAllHistory(pool_id: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetSwapHistory(pool_id: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetMintHistory(pool_id: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetBurnHistory(pool_id: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetTrades(pool: string, start_time?: number | null, end_time?: number | null, limit?: bigint | null): Promise<any>;
  dataApiGetCandles(pool: string, interval: string, start_time?: number | null, end_time?: number | null, limit?: bigint | null): Promise<any>;
  dataApiGetReserves(pool: string): Promise<any>;
  dataApiGetHolders(alkane: string, page: bigint, limit: bigint): Promise<any>;
  dataApiGetHoldersCount(alkane: string): Promise<any>;
  dataApiGetKeys(alkane: string, prefix: string | null | undefined, limit: bigint): Promise<any>;
  dataApiGetBitcoinPrice(): Promise<any>;
  dataApiGetBitcoinMarketChart(days: string): Promise<any>;
  dataApiHealth(): Promise<any>;
  dataApiGetAlkanes(page?: bigint | null, limit?: bigint | null): Promise<any>;
  dataApiGetAlkaneDetails(alkane_id: string): Promise<any>;
  dataApiGetPoolById(pool_id: string): Promise<any>;
  dataApiGetOutpointBalances(outpoint: string): Promise<any>;
  dataApiGetBlockHeight(): Promise<any>;
  dataApiGetBlockHash(): Promise<any>;
  dataApiGetIndexerPosition(): Promise<any>;
  dataApiGetPoolCreationHistory(limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetPoolSwapHistory(pool_id?: string | null, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetTokenSwapHistory(alkane_id: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetPoolMintHistory(pool_id?: string | null, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetPoolBurnHistory(pool_id?: string | null, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAddressSwapHistoryForPool(address: string, pool_id: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAddressSwapHistoryForToken(address: string, alkane_id: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAddressWrapHistory(address: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAddressUnwrapHistory(address: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAllWrapHistory(limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAllUnwrapHistory(limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetTotalUnwrapAmount(): Promise<any>;
  dataApiGetAddressPoolCreationHistory(address: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAddressPoolMintHistory(address: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAddressPoolBurnHistory(address: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAllAddressAmmTxHistory(address: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAllAmmTxHistory(limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAddressPositions(address: string, factory_id: string): Promise<any>;
  dataApiGetTokenPairs(factory_id: string, alkane_id?: string | null, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAllTokenPairs(factory_id: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiGetAlkaneSwapPairDetails(factory_id: string, token_a_id: string, token_b_id: string): Promise<any>;
  dataApiGetAlkanesUtxo(address: string): Promise<any>;
  dataApiGetAmmUtxos(address: string): Promise<any>;
  dataApiGetAddressUtxos(address: string): Promise<any>;
  dataApiGetAddressBalance(address: string): Promise<any>;
  dataApiGetTaprootBalance(address: string): Promise<any>;
  dataApiGetAccountUtxos(account: string): Promise<any>;
  dataApiGetAccountBalance(account: string): Promise<any>;
  dataApiGetAddressOutpoints(address: string): Promise<any>;
  dataApiGlobalAlkanesSearch(search_query: string, limit?: bigint | null, offset?: bigint | null): Promise<any>;
  dataApiPathfind(token_in: string, token_out: string, amount_in: string, max_hops?: bigint | null): Promise<any>;
  dataApiGetBitcoinMarketWeekly(): Promise<any>;
  dataApiGetBitcoinMarkets(): Promise<any>;
  dataApiGetTaprootHistory(taproot_address: string, total_txs: bigint): Promise<any>;
  dataApiGetIntentHistory(address: string, total_txs?: bigint | null, last_seen_tx_id?: string | null): Promise<any>;
  /**
   * Reflect alkane token metadata by querying standard opcodes
   *
   * This method queries the alkane contract with standard opcodes to retrieve
   * token metadata like name, symbol, total supply, cap, minted, and value per mint.
   *
   * # Arguments
   * * `alkane_id` - The alkane ID in "block:tx" format (e.g., "2:1234")
   *
   * # Returns
   * An AlkaneReflection object with all available metadata
   */
  alkanesReflect(alkane_id: string): Promise<any>;
  alkanesSequence(block_tag?: string | null): Promise<any>;
  alkanesSpendables(address: string): Promise<any>;
  /**
   * Get current ESPO indexer height
   */
  espoGetHeight(): Promise<any>;
  /**
   * Ping the ESPO essentials module
   */
  espoPing(): Promise<any>;
  /**
   * Get alkanes balances for an address from ESPO
   */
  espoGetAddressBalances(address: string, include_outpoints: boolean): Promise<any>;
  /**
   * Get outpoints containing alkanes for an address from ESPO
   */
  espoGetAddressOutpoints(address: string): Promise<any>;
  /**
   * Get alkanes balances at a specific outpoint from ESPO
   */
  espoGetOutpointBalances(outpoint: string): Promise<any>;
  /**
   * Get holders of an alkane token from ESPO
   */
  espoGetHolders(alkane_id: string, page: number, limit: number): Promise<any>;
  /**
   * Get holder count for an alkane from ESPO
   */
  espoGetHoldersCount(alkane_id: string): Promise<any>;
  /**
   * Get storage keys for an alkane contract from ESPO
   */
  espoGetKeys(alkane_id: string, page: number, limit: number): Promise<any>;
  /**
   * Ping the ESPO AMM Data module
   */
  espoAmmdataPing(): Promise<any>;
  /**
   * Get OHLCV candlestick data for a pool from ESPO
   */
  espoGetCandles(pool: string, timeframe?: string | null, side?: string | null, limit?: number | null, page?: number | null): Promise<any>;
  /**
   * Get trade history for a pool from ESPO
   */
  espoGetTrades(pool: string, limit?: number | null, page?: number | null, side?: string | null, filter_side?: string | null, sort?: string | null, dir?: string | null): Promise<any>;
  /**
   * Get all pools from ESPO
   */
  espoGetPools(limit?: number | null, page?: number | null): Promise<any>;
  /**
   * Find the best swap path between two tokens using ESPO
   */
  espoFindBestSwapPath(token_in: string, token_out: string, mode?: string | null, amount_in?: string | null, amount_out?: string | null, amount_out_min?: string | null, amount_in_max?: string | null, available_in?: string | null, fee_bps?: number | null, max_hops?: number | null): Promise<any>;
  /**
   * Find the best MEV swap opportunity for a token using ESPO
   */
  espoGetBestMevSwap(token: string, fee_bps?: number | null, max_hops?: number | null): Promise<any>;
  /**
   * Get AMM factories from ESPO
   */
  espoGetAmmFactories(page?: number | null, limit?: number | null): Promise<any>;
  /**
   * Get all alkanes from ESPO
   */
  espoGetAllAlkanes(page?: number | null, limit?: number | null): Promise<any>;
  /**
   * Get alkane info from ESPO
   */
  espoGetAlkaneInfo(alkane_id: string): Promise<any>;
  /**
   * Get block summary from ESPO
   */
  espoGetBlockSummary(height: number): Promise<any>;
  /**
   * Get circulating supply of an alkane from ESPO
   */
  espoGetCirculatingSupply(alkane_id: string, height?: number | null): Promise<any>;
  /**
   * Get transfer volume for an alkane from ESPO
   */
  espoGetTransferVolume(alkane_id: string, page?: number | null, limit?: number | null): Promise<any>;
  /**
   * Get total received for an alkane from ESPO
   */
  espoGetTotalReceived(alkane_id: string, page?: number | null, limit?: number | null): Promise<any>;
  /**
   * Get address activity from ESPO
   */
  espoGetAddressActivity(address: string): Promise<any>;
  /**
   * Get all balances for an alkane (all holders) from ESPO
   */
  espoGetAlkaneBalances(alkane_id: string): Promise<any>;
  /**
   * Get alkane balance via metashrew from ESPO
   */
  espoGetAlkaneBalanceMetashrew(owner: string, target: string, height?: number | null): Promise<any>;
  /**
   * Get alkane balance transactions from ESPO
   */
  espoGetAlkaneBalanceTxs(alkane_id: string, page?: number | null, limit?: number | null): Promise<any>;
  /**
   * Get alkane balance transactions by token from ESPO
   */
  espoGetAlkaneBalanceTxsByToken(owner: string, token: string, page?: number | null, limit?: number | null): Promise<any>;
  /**
   * Get block traces from ESPO
   */
  espoGetBlockTraces(height: number): Promise<any>;
  /**
   * Get alkane transaction summary from ESPO
   */
  espoGetAlkaneTxSummary(txid: string): Promise<any>;
  /**
   * Get alkane transactions in a block from ESPO
   */
  espoGetAlkaneBlockTxs(height: number, page?: number | null, limit?: number | null): Promise<any>;
  /**
   * Get alkane transactions for an address from ESPO
   */
  espoGetAlkaneAddressTxs(address: string, page?: number | null, limit?: number | null): Promise<any>;
  /**
   * Get all transactions for an address from ESPO
   */
  espoGetAddressTransactions(address: string, page?: number | null, limit?: number | null, only_alkane_txs?: boolean | null): Promise<any>;
  /**
   * Get latest alkane traces from ESPO
   */
  espoGetAlkaneLatestTraces(): Promise<any>;
  /**
   * Get mempool traces from ESPO
   */
  espoGetMempoolTraces(page?: number | null, limit?: number | null, address?: string | null): Promise<any>;
  /**
   * Get all wrap events from ESPO (subfrost namespace)
   */
  espoGetWrapEvents(count?: number | null, offset?: number | null, successful?: boolean | null): Promise<any>;
  /**
   * Get wrap events for a specific address from ESPO (subfrost namespace)
   */
  espoGetWrapEventsByAddress(address: string, count?: number | null, offset?: number | null, successful?: boolean | null): Promise<any>;
  /**
   * Get all unwrap events from ESPO (subfrost namespace)
   */
  espoGetUnwrapEvents(count?: number | null, offset?: number | null, successful?: boolean | null): Promise<any>;
  /**
   * Get unwrap events for a specific address from ESPO (subfrost namespace)
   */
  espoGetUnwrapEventsByAddress(address: string, count?: number | null, offset?: number | null, successful?: boolean | null): Promise<any>;
  /**
   * Get series ID from alkane ID (pizzafun namespace)
   */
  espoGetSeriesIdFromAlkaneId(alkane_id: string): Promise<any>;
  /**
   * Get series IDs from multiple alkane IDs (pizzafun namespace)
   */
  espoGetSeriesIdsFromAlkaneIds(alkane_ids: string[]): Promise<any>;
  /**
   * Get alkane ID from series ID (pizzafun namespace)
   */
  espoGetAlkaneIdFromSeriesId(series_id: string): Promise<any>;
  /**
   * Get alkane IDs from multiple series IDs (pizzafun namespace)
   */
  espoGetAlkaneIdsFromSeriesIds(series_ids: string[]): Promise<any>;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_webprovider_free: (a: number, b: number) => void;
  readonly webprovider_new_js: (a: number, b: number, c: number) => [number, number, number];
  readonly webprovider_sandshrew_rpc_url: (a: number) => [number, number];
  readonly webprovider_esplora_rpc_url: (a: number) => [number, number];
  readonly webprovider_bitcoin_rpc_url: (a: number) => [number, number];
  readonly webprovider_brc20_prog_rpc_url: (a: number) => [number, number];
  readonly webprovider_getEnrichedBalances: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_getAddressTxs: (a: number, b: number, c: number) => any;
  readonly webprovider_getTransactionHex: (a: number, b: number, c: number) => any;
  readonly webprovider_traceOutpoint: (a: number, b: number, c: number) => any;
  readonly webprovider_getAddressUtxos: (a: number, b: number, c: number) => any;
  readonly webprovider_broadcastTransaction: (a: number, b: number, c: number) => any;
  readonly webprovider_getAddressTxsWithTraces: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_ordInscription: (a: number, b: number, c: number) => any;
  readonly webprovider_ordInscriptions: (a: number, b: number, c: number) => any;
  readonly webprovider_ordOutputs: (a: number, b: number, c: number) => any;
  readonly webprovider_ordRune: (a: number, b: number, c: number) => any;
  readonly webprovider_ordAddressInfo: (a: number, b: number, c: number) => any;
  readonly webprovider_ordBlockInfo: (a: number, b: number, c: number) => any;
  readonly webprovider_ordBlockCount: (a: number) => any;
  readonly webprovider_ordBlocks: (a: number) => any;
  readonly webprovider_ordChildren: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_ordContent: (a: number, b: number, c: number) => any;
  readonly webprovider_ordParents: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_ordTxInfo: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesExecute: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesExecuteWithStrings: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => any;
  readonly webprovider_alkanesExecuteFull: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => any;
  readonly webprovider_alkanesResumeExecution: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_alkanesResumeCommitExecution: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesResumeRevealExecution: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesSimulate: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_alkanesWrapBtc: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesInitPool: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesSwap: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesReflectAlkaneRange: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
  readonly webprovider_alkanesTxScript: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_alkanesPoolDetails: (a: number, b: number, c: number) => any;
  readonly webprovider_subfrostMinimumUnwrap: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => any;
  readonly webprovider_opiBlockHeight: (a: number, b: number, c: number) => any;
  readonly webprovider_opiExtrasBlockHeight: (a: number, b: number, c: number) => any;
  readonly webprovider_opiDbVersion: (a: number, b: number, c: number) => any;
  readonly webprovider_opiEventHashVersion: (a: number, b: number, c: number) => any;
  readonly webprovider_opiBalanceOnBlock: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => any;
  readonly webprovider_opiActivityOnBlock: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_opiBitcoinRpcResultsOnBlock: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_opiCurrentBalance: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => any;
  readonly webprovider_opiValidTxNotesOfWallet: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_opiValidTxNotesOfTicker: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_opiHolders: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_opiHashOfAllActivity: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_opiHashOfAllCurrentBalances: (a: number, b: number, c: number) => any;
  readonly webprovider_opiEvent: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_opiIp: (a: number, b: number, c: number) => any;
  readonly webprovider_opiRaw: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_opiRunesBlockHeight: (a: number, b: number, c: number) => any;
  readonly webprovider_opiRunesBalanceOnBlock: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => any;
  readonly webprovider_opiRunesActivityOnBlock: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_opiRunesCurrentBalance: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_opiRunesUnspentOutpoints: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_opiRunesHolders: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_opiRunesHashOfAllActivity: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_opiRunesEvent: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_opiBitmapBlockHeight: (a: number, b: number, c: number) => any;
  readonly webprovider_opiBitmapHashOfAllActivity: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_opiBitmapHashOfAllBitmaps: (a: number, b: number, c: number) => any;
  readonly webprovider_opiBitmapInscriptionId: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_opiPow20BlockHeight: (a: number, b: number, c: number) => any;
  readonly webprovider_opiPow20BalanceOnBlock: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => any;
  readonly webprovider_opiPow20ActivityOnBlock: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_opiPow20CurrentBalance: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => any;
  readonly webprovider_opiPow20ValidTxNotesOfWallet: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_opiPow20ValidTxNotesOfTicker: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_opiPow20Holders: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_opiPow20HashOfAllActivity: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_opiPow20HashOfAllCurrentBalances: (a: number, b: number, c: number) => any;
  readonly webprovider_opiSnsBlockHeight: (a: number, b: number, c: number) => any;
  readonly webprovider_opiSnsHashOfAllActivity: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_opiSnsHashOfAllRegisteredNames: (a: number, b: number, c: number) => any;
  readonly webprovider_opiSnsInfo: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_opiSnsInscriptionsOfDomain: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_opiSnsRegisteredNamespaces: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesBalance: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesBytecode: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_alkanesMeta: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_alkanesGetAllPoolsWithDetails: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_alkanesGetAllPools: (a: number, b: number, c: number) => any;
  readonly webprovider_ammGetPoolDetails: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesTrace: (a: number, b: number, c: number) => any;
  readonly webprovider_traceProtostones: (a: number, b: number, c: number) => any;
  readonly webprovider_traceBlock: (a: number, b: number) => any;
  readonly webprovider_alkanesByAddress: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_alkanesByOutpoint: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_esploraGetTx: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetTxStatus: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetAddressInfo: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetBlocksTipHeight: (a: number) => any;
  readonly webprovider_esploraGetBlocksTipHash: (a: number) => any;
  readonly webprovider_esploraGetAddressUtxo: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetAddressTxs: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetAddressTxsChain: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_getStorageAt: (a: number, b: bigint, c: bigint, d: number, e: number) => any;
  readonly webprovider_esploraGetFeeEstimates: (a: number) => any;
  readonly webprovider_esploraBroadcastTx: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetTxHex: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetBlocks: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetBlockByHeight: (a: number, b: number) => any;
  readonly webprovider_esploraGetBlock: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetBlockStatus: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetBlockTxids: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetBlockHeader: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetBlockRaw: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetBlockTxid: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_esploraGetBlockTxs: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_esploraGetAddressTxsMempool: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetAddressPrefix: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetTxRaw: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetTxMerkleProof: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetTxMerkleblockProof: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetTxOutspend: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_esploraGetTxOutspends: (a: number, b: number, c: number) => any;
  readonly webprovider_esploraGetMempool: (a: number) => any;
  readonly webprovider_esploraGetMempoolTxids: (a: number) => any;
  readonly webprovider_esploraGetMempoolRecent: (a: number) => any;
  readonly webprovider_esploraPostTx: (a: number, b: number, c: number) => any;
  readonly webprovider_bitcoindGetBlockCount: (a: number) => any;
  readonly webprovider_bitcoindSendRawTransaction: (a: number, b: number, c: number) => any;
  readonly webprovider_bitcoindGenerateToAddress: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_bitcoindGenerateFuture: (a: number, b: number, c: number) => any;
  readonly webprovider_bitcoindGetBlockchainInfo: (a: number) => any;
  readonly webprovider_bitcoindGetNetworkInfo: (a: number) => any;
  readonly webprovider_bitcoindGetRawTransaction: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_bitcoindGetBlock: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_bitcoindGetBlockHash: (a: number, b: number) => any;
  readonly webprovider_bitcoindGetBlockHeader: (a: number, b: number, c: number) => any;
  readonly webprovider_bitcoindGetBlockStats: (a: number, b: number, c: number) => any;
  readonly webprovider_bitcoindGetMempoolInfo: (a: number) => any;
  readonly webprovider_bitcoindEstimateSmartFee: (a: number, b: number) => any;
  readonly webprovider_bitcoindGetChainTips: (a: number) => any;
  readonly webprovider_bitcoindGetRawMempool: (a: number) => any;
  readonly webprovider_bitcoindGetTxOut: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_bitcoindDecodeRawTransaction: (a: number, b: number, c: number) => any;
  readonly webprovider_bitcoindDecodePsbt: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesView: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => any;
  readonly webprovider_alkanesInspect: (a: number, b: number, c: number, d: any) => any;
  readonly webprovider_alkanesInspectBytecode: (a: number, b: number, c: number, d: number, e: number, f: any) => any;
  readonly webprovider_alkanesPendingUnwraps: (a: number, b: number, c: number) => any;
  readonly webprovider_brc20progCall: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_brc20progGetBalance: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_brc20progGetCode: (a: number, b: number, c: number) => any;
  readonly webprovider_brc20progGetTransactionCount: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_brc20progBlockNumber: (a: number) => any;
  readonly webprovider_brc20progChainId: (a: number) => any;
  readonly webprovider_brc20progGetTransactionReceipt: (a: number, b: number, c: number) => any;
  readonly webprovider_brc20progGetTransactionByHash: (a: number, b: number, c: number) => any;
  readonly webprovider_brc20progGetBlockByNumber: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_brc20progEstimateGas: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_brc20progGetLogs: (a: number, b: any) => any;
  readonly webprovider_brc20progWeb3ClientVersion: (a: number) => any;
  readonly webprovider_metashrewHeight: (a: number) => any;
  readonly webprovider_waitForIndexer: (a: number) => any;
  readonly webprovider_metashrewStateRoot: (a: number, b: number, c: number) => any;
  readonly webprovider_metashrewGetBlockHash: (a: number, b: number) => any;
  readonly webprovider_metashrewView: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_luaEvalScript: (a: number, b: number, c: number) => any;
  readonly webprovider_luaEval: (a: number, b: number, c: number, d: any) => any;
  readonly webprovider_ordList: (a: number, b: number, c: number) => any;
  readonly webprovider_ordFind: (a: number, b: number) => any;
  readonly webprovider_runestoneDecodeTx: (a: number, b: number, c: number) => any;
  readonly webprovider_runestoneAnalyzeTx: (a: number, b: number, c: number) => any;
  readonly webprovider_protorunesDecodeTx: (a: number, b: number, c: number) => any;
  readonly webprovider_protorunesAnalyzeTx: (a: number, b: number, c: number) => any;
  readonly webprovider_walletCreate: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
  readonly webprovider_walletLoad: (a: number, b: number, c: number) => any;
  readonly webprovider_walletGetAddress: (a: number) => any;
  readonly webprovider_walletGetBalance: (a: number, b: number, c: number) => any;
  readonly webprovider_walletLoadMnemonic: (a: number, b: number, c: number, d: number, e: number) => [number, number];
  readonly webprovider_walletIsLoaded: (a: number) => number;
  readonly webprovider_walletGetAddresses: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
  readonly webprovider_walletSend: (a: number, b: number, c: number) => any;
  readonly webprovider_walletGetUtxos: (a: number, b: number, c: number) => any;
  readonly webprovider_walletGetHistory: (a: number, b: number, c: number) => any;
  readonly webprovider_walletCreatePsbt: (a: number, b: number, c: number) => any;
  readonly webprovider_walletExport: (a: number) => any;
  readonly webprovider_walletBackup: (a: number) => any;
  readonly webprovider_frbtcGetSignerAddress: (a: number) => any;
  readonly webprovider_frbtcWrap: (a: number, b: bigint, c: number, d: number) => any;
  readonly webprovider_frbtcUnwrap: (a: number, b: bigint, c: bigint, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_frbtcWrapAndExecute: (a: number, b: bigint, c: number, d: number, e: number, f: number) => any;
  readonly webprovider_frbtcWrapAndExecute2: (a: number, b: bigint, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => any;
  readonly webprovider_dataApiGetPoolHistory: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint, h: number, i: bigint) => any;
  readonly webprovider_dataApiGetPools: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetAlkanesByAddress: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetAllPoolsDetails: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint, h: number, i: number, j: number, k: number) => any;
  readonly webprovider_dataApiGetPoolDetails: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_dataApiGetAddressBalances: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_dataApiGetAllHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetSwapHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetMintHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetBurnHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetTrades: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: bigint) => any;
  readonly webprovider_dataApiGetCandles: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: bigint) => any;
  readonly webprovider_dataApiGetReserves: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetHolders: (a: number, b: number, c: number, d: bigint, e: bigint) => any;
  readonly webprovider_dataApiGetHoldersCount: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetKeys: (a: number, b: number, c: number, d: number, e: number, f: bigint) => any;
  readonly webprovider_dataApiGetBitcoinPrice: (a: number) => any;
  readonly webprovider_dataApiGetBitcoinMarketChart: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiHealth: (a: number) => any;
  readonly webprovider_dataApiGetAlkanes: (a: number, b: number, c: bigint, d: number, e: bigint) => any;
  readonly webprovider_dataApiGetAlkaneDetails: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetPoolById: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetOutpointBalances: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetBlockHeight: (a: number) => any;
  readonly webprovider_dataApiGetBlockHash: (a: number) => any;
  readonly webprovider_dataApiGetIndexerPosition: (a: number) => any;
  readonly webprovider_dataApiGetPoolCreationHistory: (a: number, b: number, c: bigint, d: number, e: bigint) => any;
  readonly webprovider_dataApiGetPoolSwapHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetTokenSwapHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetPoolMintHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetPoolBurnHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetAddressSwapHistoryForPool: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint, h: number, i: bigint) => any;
  readonly webprovider_dataApiGetAddressSwapHistoryForToken: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint, h: number, i: bigint) => any;
  readonly webprovider_dataApiGetAddressWrapHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetAddressUnwrapHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetAllWrapHistory: (a: number, b: number, c: bigint, d: number, e: bigint) => any;
  readonly webprovider_dataApiGetAllUnwrapHistory: (a: number, b: number, c: bigint, d: number, e: bigint) => any;
  readonly webprovider_dataApiGetTotalUnwrapAmount: (a: number) => any;
  readonly webprovider_dataApiGetAddressPoolCreationHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetAddressPoolMintHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetAddressPoolBurnHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetAllAddressAmmTxHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetAllAmmTxHistory: (a: number, b: number, c: bigint, d: number, e: bigint) => any;
  readonly webprovider_dataApiGetAddressPositions: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_dataApiGetTokenPairs: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint, h: number, i: bigint) => any;
  readonly webprovider_dataApiGetAllTokenPairs: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiGetAlkaneSwapPairDetails: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_dataApiGetAlkanesUtxo: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetAmmUtxos: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetAddressUtxos: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetAddressBalance: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetTaprootBalance: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetAccountUtxos: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetAccountBalance: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGetAddressOutpoints: (a: number, b: number, c: number) => any;
  readonly webprovider_dataApiGlobalAlkanesSearch: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: bigint) => any;
  readonly webprovider_dataApiPathfind: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: bigint) => any;
  readonly webprovider_dataApiGetBitcoinMarketWeekly: (a: number) => any;
  readonly webprovider_dataApiGetBitcoinMarkets: (a: number) => any;
  readonly webprovider_dataApiGetTaprootHistory: (a: number, b: number, c: number, d: bigint) => any;
  readonly webprovider_dataApiGetIntentHistory: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: number) => any;
  readonly webprovider_alkanesReflect: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesSequence: (a: number, b: number, c: number) => any;
  readonly webprovider_alkanesSpendables: (a: number, b: number, c: number) => any;
  readonly webprovider_espoGetHeight: (a: number) => any;
  readonly webprovider_espoPing: (a: number) => any;
  readonly webprovider_espoGetAddressBalances: (a: number, b: number, c: number, d: number) => any;
  readonly webprovider_espoGetAddressOutpoints: (a: number, b: number, c: number) => any;
  readonly webprovider_espoGetOutpointBalances: (a: number, b: number, c: number) => any;
  readonly webprovider_espoGetHolders: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_espoGetHoldersCount: (a: number, b: number, c: number) => any;
  readonly webprovider_espoGetKeys: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_espoAmmdataPing: (a: number) => any;
  readonly webprovider_espoGetCandles: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => any;
  readonly webprovider_espoGetTrades: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number) => any;
  readonly webprovider_espoGetPools: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_espoFindBestSwapPath: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number, r: number, s: number, t: number, u: number) => any;
  readonly webprovider_espoGetBestMevSwap: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_espoGetAmmFactories: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_espoGetAllAlkanes: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_espoGetAlkaneInfo: (a: number, b: number, c: number) => any;
  readonly webprovider_espoGetBlockSummary: (a: number, b: number) => any;
  readonly webprovider_espoGetCirculatingSupply: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly webprovider_espoGetTransferVolume: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_espoGetTotalReceived: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_espoGetAddressActivity: (a: number, b: number, c: number) => any;
  readonly webprovider_espoGetAlkaneBalances: (a: number, b: number, c: number) => any;
  readonly webprovider_espoGetAlkaneBalanceMetashrew: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_espoGetAlkaneBalanceTxs: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_espoGetAlkaneBalanceTxsByToken: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => any;
  readonly webprovider_espoGetBlockTraces: (a: number, b: number) => any;
  readonly webprovider_espoGetAlkaneTxSummary: (a: number, b: number, c: number) => any;
  readonly webprovider_espoGetAlkaneBlockTxs: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
  readonly webprovider_espoGetAlkaneAddressTxs: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_espoGetAddressTransactions: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => any;
  readonly webprovider_espoGetAlkaneLatestTraces: (a: number) => any;
  readonly webprovider_espoGetMempoolTraces: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
  readonly webprovider_espoGetWrapEvents: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
  readonly webprovider_espoGetWrapEventsByAddress: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => any;
  readonly webprovider_espoGetUnwrapEvents: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
  readonly webprovider_espoGetUnwrapEventsByAddress: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => any;
  readonly webprovider_espoGetSeriesIdFromAlkaneId: (a: number, b: number, c: number) => any;
  readonly webprovider_espoGetSeriesIdsFromAlkaneIds: (a: number, b: number, c: number) => any;
  readonly webprovider_espoGetAlkaneIdFromSeriesId: (a: number, b: number, c: number) => any;
  readonly webprovider_espoGetAlkaneIdsFromSeriesIds: (a: number, b: number, c: number) => any;
  readonly __wbg_wasmbrowserwalletprovider_free: (a: number, b: number) => void;
  readonly wasmbrowserwalletprovider_new: (a: any, b: number, c: number) => any;
  readonly wasmbrowserwalletprovider_getAddress: (a: number) => [number, number];
  readonly wasmbrowserwalletprovider_getPublicKey: (a: number) => any;
  readonly wasmbrowserwalletprovider_signPsbt: (a: number, b: number, c: number, d: any) => any;
  readonly wasmbrowserwalletprovider_signMessage: (a: number, b: number, c: number, d: number, e: number) => any;
  readonly wasmbrowserwalletprovider_broadcastTransaction: (a: number, b: number, c: number) => any;
  readonly wasmbrowserwalletprovider_getBalance: (a: number) => any;
  readonly wasmbrowserwalletprovider_getUtxos: (a: number, b: number) => any;
  readonly wasmbrowserwalletprovider_getEnrichedUtxos: (a: number) => any;
  readonly wasmbrowserwalletprovider_getAllBalances: (a: number) => any;
  readonly wasmbrowserwalletprovider_getWalletInfo: (a: number) => any;
  readonly wasmbrowserwalletprovider_getConnectionStatus: (a: number) => [number, number];
  readonly wasmbrowserwalletprovider_getNetwork: (a: number) => [number, number];
  readonly wasmbrowserwalletprovider_disconnect: (a: number) => any;
  readonly __wbg_keystore_free: (a: number, b: number) => void;
  readonly __wbg_pbkdfparams_free: (a: number, b: number) => void;
  readonly pbkdfparams_from_js: (a: any) => [number, number, number];
  readonly pbkdfparams_to_js: (a: number) => [number, number, number];
  readonly keystore_from_js: (a: any) => [number, number, number];
  readonly keystore_to_js: (a: number) => [number, number, number];
  readonly keystore_accountXpub: (a: number) => [number, number];
  readonly keystore_hdPaths: (a: number) => any;
  readonly keystore_masterFingerprint: (a: number) => [number, number];
  readonly keystore_decryptMnemonic: (a: number, b: number, c: number) => any;
  readonly encryptMnemonic: (a: number, b: number, c: number, d: number) => any;
  readonly analyze_psbt: (a: number, b: number, c: number, d: number) => [number, number, number, number];
  readonly simulate_alkane_call: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
  readonly get_alkane_bytecode: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
  readonly get_alkane_meta: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
  readonly analyze_runestone: (a: number, b: number) => [number, number, number, number];
  readonly decode_psbt: (a: number, b: number) => [number, number, number, number];
  readonly brc20_prog_deploy_contract: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
  readonly brc20_prog_transact: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => any;
  readonly brc20_prog_wrap_btc: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => any;
  readonly frbtc_wrap: (a: number, b: number, c: bigint, d: number, e: number) => any;
  readonly frbtc_unwrap: (a: number, b: number, c: bigint, d: bigint, e: number, f: number, g: number, h: number) => any;
  readonly frbtc_wrap_and_execute: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number) => any;
  readonly frbtc_wrap_and_execute2: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => any;
  readonly frbtc_get_signer_address: (a: number, b: number) => any;
  readonly init_panic_hook: () => void;
  readonly rustsecp256k1_v0_9_2_context_create: (a: number) => number;
  readonly rustsecp256k1_v0_9_2_context_destroy: (a: number) => void;
  readonly rustsecp256k1_v0_9_2_default_illegal_callback_fn: (a: number, b: number) => void;
  readonly rustsecp256k1_v0_9_2_default_error_callback_fn: (a: number, b: number) => void;
  readonly rustsecp256k1_v0_10_0_context_create: (a: number) => number;
  readonly rustsecp256k1_v0_10_0_context_destroy: (a: number) => void;
  readonly rustsecp256k1_v0_10_0_default_illegal_callback_fn: (a: number, b: number) => void;
  readonly rustsecp256k1_v0_10_0_default_error_callback_fn: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h01405c57635d3c55: (a: number, b: number) => void;
  readonly wasm_bindgen__closure__destroy__he41b8e2aae505aee: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h5943629905d90057: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__h3ba04b4139aaae95: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__hc67e7f9a7930d925: (a: number, b: number) => void;
  readonly wasm_bindgen__closure__destroy__hb154d7ec25b6c414: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__h95fdbac5e4c1bfb6: (a: number, b: number, c: any, d: any) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
