//! # CLI Commands for `alkanes-cli`
//!
//! This module defines the `clap`-based command structure for the `alkanes-cli` CLI,
//! including subcommands for interacting with `bitcoind`. It also contains
//! the logic for pretty-printing complex JSON responses.

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

// Chad's Journal:
//
// The `clap` crate automatically converts subcommand names to kebab-case by default
// (e.g., `GetBytecode` becomes `get-bytecode`). However, to maintain consistency
// with the `metashrew_view` RPC method, which is named `getbytecode`, we need to
// override this behavior.
//
// By adding `#[command(name = "getbytecode")]` to the `GetBytecode` variant,
// we ensure the CLI accepts `getbytecode` as the subcommand, aligning the
// developer experience with the underlying RPC method. This same approach is
// applied to other subcommands like `traceblock` and `getbalance` to keep
// the naming consistent across the board.

/// Alkanes CLI is a command-line tool for interacting with Bitcoin and Alkanes
#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[command(author, version, about, long_about = None)]
pub struct DeezelCommands {
    /// Path to the wallet file
    #[arg(long)]
    pub wallet_file: Option<String>,
    /// Passphrase for the wallet (required to unlock keystore for signing)
    #[arg(long)]
    pub passphrase: Option<String>,
    /// HD path for the wallet
    #[arg(long)]
    pub hd_path: Option<String>,
    /// Wallet address (for watch-only operations without keystore)
    #[arg(long, conflicts_with_all = ["wallet_file", "wallet_key", "wallet_key_file"])]
    pub wallet_address: Option<String>,
    /// Wallet private key as hex string (for signing with a single key)
    #[arg(long, conflicts_with_all = ["wallet_file", "wallet_address"])]
    pub wallet_key: Option<String>,
    /// Wallet private key file path (for signing with a single key)
    #[arg(long, conflicts_with_all = ["wallet_file", "wallet_address", "wallet_key"])]
    pub wallet_key_file: Option<String>,
    /// Sandshrew RPC URL
    #[arg(long)]
    pub sandshrew_rpc_url: Option<String>,
    /// Titan API URL (alternative to sandshrew-rpc-url)
    #[arg(long)]
    pub titan_api_url: Option<String>,
    /// Bitcoin RPC URL
    #[arg(long)]
    pub bitcoin_rpc_url: Option<String>,
    /// Esplora API URL
    #[arg(long)]
    pub esplora_api_url: Option<String>,
    /// Ord server URL
    #[arg(long)]
    pub ord_server_url: Option<String>,
    /// Metashrew RPC URL
    #[arg(long)]
    pub metashrew_rpc_url: Option<String>,
    /// BRC20-Prog RPC URL (for querying brc20-programmable-module)
    #[arg(long)]
    pub brc20_prog_rpc_url: Option<String>,
    /// Data API URL (defaults to http://localhost:4000 for regtest, https://mainnet-api.oyl.gg for mainnet)
    #[arg(long)]
    pub data_api: Option<String>,
    /// Network provider
    #[arg(short, long, default_value = "regtest")]
    pub provider: String,
    /// Subcommands
    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum Commands {
    /// Bitcoin Core RPC commands
    #[command(subcommand)]
    Bitcoind(BitcoindCommands),
    /// Esplora API commands
    #[command(subcommand)]
    Esplora(EsploraCommands),
    /// Ord subcommands
    #[command(subcommand)]
    Ord(OrdCommands),
    /// Alkanes subcommands
    #[command(subcommand)]
    Alkanes(Alkanes),
    /// BRC20-Prog subcommands
    #[command(subcommand)]
    Brc20Prog(Brc20Prog),
    /// Runestone subcommands
    #[command(subcommand)]
    Runestone(Runestone),
    /// Protorunes subcommands
    #[command(subcommand)]
    Protorunes(Protorunes),
    /// Wallet subcommands
    #[command(subcommand)]
    Wallet(WalletCommands),
    /// Metashrew subcommands
    #[command(subcommand)]
    Metashrew(MetashrewCommands),
    /// DataAPI subcommands - Query data from alkanes-data-api
    #[command(subcommand)]
    Dataapi(DataApiCommand),
    /// Sandshrew RPC operations
    #[command(subcommand)]
    Sandshrew(SandshrewCommands),
}

/// Metashrew subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum MetashrewCommands {
    /// Get the current block height
    Height,
    /// Get the block hash for a given height
    Getblockhash {
        /// The block height
        height: u64,
    },
    /// Get the state root for a given height
    Getstateroot {
        /// The block height, or "latest"
        height: Option<String>,
    },
}

/// Bitcoin Core RPC subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BitcoindCommands {
    /// Get current block count
    Getblockcount {
        #[arg(long)]
        raw: bool,
    },
    /// Generate blocks to an address (regtest only)
    Generatetoaddress {
        /// Number of blocks to generate
        nblocks: u32,
        /// Address to generate to
        address: String,
        #[arg(long)]
        raw: bool,
    },
    /// Generate a single block with a future-claiming protostone in the coinbase (regtest only)
    /// The address is automatically derived from the frBTC signer
    Generatefuture,
    Getblockchaininfo {
        #[arg(long)]
        raw: bool,
    },
    Getnetworkinfo {
        #[arg(long)]
        raw: bool,
    },
    Getrawtransaction {
        txid: String,
        #[arg(long)]
        block_hash: Option<String>,
        #[arg(long)]
        raw: bool,
    },
    Getblock {
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    Getblockhash {
        height: u64,
        #[arg(long)]
        raw: bool,
    },
    Getblockheader {
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    Getblockstats {
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    Decoderawtransaction {
        /// Raw transaction hex
        hex: String,
        #[arg(long)]
        raw: bool,
    },
    Getchaintips {
        #[arg(long)]
        raw: bool,
    },
    Getmempoolinfo {
        #[arg(long)]
        raw: bool,
    },
    Getrawmempool {
        #[arg(long)]
        raw: bool,
    },
    Gettxout {
        txid: String,
        vout: u32,
        #[arg(long)]
        include_mempool: bool,
        #[arg(long)]
        raw: bool,
    },
    Sendrawtransaction {
        /// Transaction hex to broadcast (or use --from-file)
        #[arg(required_unless_present = "from_file")]
        tx_hex: Option<String>,
        /// Read transaction hex from file
        #[arg(long)]
        from_file: Option<String>,
        /// Use MARA Slipstream service for broadcasting (bypasses standard mempool, accepts large/non-standard txs)
        #[arg(long)]
        use_slipstream: bool,
        /// Use Rebar Shield for private transaction relay (requires payment output in tx)
        #[arg(long)]
        use_rebar: bool,
        #[arg(long)]
        raw: bool,
    },
}

/// Esplora API subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum EsploraCommands {
    BlocksTipHash {
        #[arg(long)]
        raw: bool,
    },
    BlocksTipHeight {
        #[arg(long)]
        raw: bool,
    },
    Blocks {
        start_height: Option<u64>,
        #[arg(long)]
        raw: bool,
    },
    BlockHeight {
        height: u64,
        #[arg(long)]
        raw: bool,
    },
    Block {
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    BlockStatus {
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    BlockTxids {
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    BlockHeader {
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    BlockRaw {
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    BlockTxid {
        hash: String,
        index: u32,
        #[arg(long)]
        raw: bool,
    },
    BlockTxs {
        hash: String,
        start_index: Option<u32>,
        #[arg(long)]
        raw: bool,
    },
    Address {
        params: String,
        #[arg(long)]
        raw: bool,
    },
    AddressTxs {
        params: String,
        #[arg(long)]
        raw: bool,
        #[arg(long)]
        exclude_coinbase: bool,
        #[arg(long)]
        runestone_trace: bool,
    },
    AddressTxsChain {
        params: String,
        #[arg(long)]
        raw: bool,
    },
    AddressTxsMempool {
        address: String,
        #[arg(long)]
        raw: bool,
    },
    AddressUtxo {
        address: String,
        #[arg(long)]
        raw: bool,
    },
    AddressPrefix {
        prefix: String,
        #[arg(long)]
        raw: bool,
    },
    Tx {
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    TxHex {
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    TxRaw {
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    TxStatus {
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    TxMerkleProof {
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    TxMerkleblockProof {
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    TxOutspend {
        txid: String,
        index: u32,
        #[arg(long)]
        raw: bool,
    },
    TxOutspends {
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    Broadcast {
        tx_hex: String,
        #[arg(long)]
        raw: bool,
    },
    PostTx {
        tx_hex: String,
        #[arg(long)]
        raw: bool,
    },
    Mempool {
        #[arg(long)]
        raw: bool,
    },
    MempoolTxids {
        #[arg(long)]
        raw: bool,
    },
    MempoolRecent {
        #[arg(long)]
        raw: bool,
    },
    FeeEstimates {
        #[arg(long)]
        raw: bool,
    },
}

/// Ord subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum OrdCommands {
    /// Get inscription by ID
    Inscription {
        /// The inscription ID
        id: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get inscriptions for a block
    InscriptionsInBlock {
        /// The block hash
        hash: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get address information
    AddressInfo {
        /// The address
        address: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get block information
    BlockInfo {
        /// The block hash or height
        query: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get latest block count
    BlockCount,
    /// Get latest blocks
    Blocks {
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get children of an inscription
    Children {
        /// The inscription ID
        id: String,
        /// Page number
        #[arg(long)]
        page: Option<u32>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get inscription content
    Content {
        /// The inscription ID
        id: String,
    },
    /// Get output information
    Output {
        /// The outpoint
        outpoint: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get parents of an inscription
    Parents {
        /// The inscription ID
        id: String,
        /// Page number
        #[arg(long)]
        page: Option<u32>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get rune information
    Rune {
        /// The rune name or ID
        rune: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get sat information
    Sat {
        /// The sat number
        sat: u64,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction information
    TxInfo {
        /// The transaction ID
        txid: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
}

/// BRC20-Prog subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum Brc20Prog {
    /// Deploy a BRC20-prog contract from Foundry build JSON
    DeployContract {
        /// Path to Foundry build JSON file
        foundry_json_path: String,
        /// Addresses to source UTXOs from
        #[arg(long, num_args = 1..)]
        from: Option<Vec<String>>,
        /// Change address
        #[arg(long)]
        change: Option<String>,
        /// Fee rate in sat/vB
        #[arg(long)]
        fee_rate: Option<f32>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Enable transaction tracing
        #[arg(long)]
        trace: bool,
        /// Mine a block after broadcasting (regtest only)
        #[arg(long)]
        mine: bool,
        /// Automatically confirm the transaction preview
        #[arg(long, short = 'y')]
        auto_confirm: bool,
    },
    /// Call a BRC20-prog contract function
    Transact {
        /// Contract address (0x prefixed hex)
        #[arg(long)]
        address: String,
        /// Function signature (e.g., "transfer(address,uint256)")
        #[arg(long)]
        signature: String,
        /// Calldata arguments as comma-separated values
        /// (e.g., "0x1234...,1000" for transfer(address,uint256))
        #[arg(long)]
        calldata: String,
        /// Addresses to source UTXOs from
        #[arg(long, num_args = 1..)]
        from: Option<Vec<String>>,
        /// Change address
        #[arg(long)]
        change: Option<String>,
        /// Fee rate in sat/vB
        #[arg(long)]
        fee_rate: Option<f32>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Enable transaction tracing
        #[arg(long)]
        trace: bool,
        /// Mine a block after broadcasting (regtest only)
        #[arg(long)]
        mine: bool,
        /// Automatically confirm the transaction preview
        #[arg(long, short = 'y')]
        auto_confirm: bool,
    },
    /// Wrap BTC to frBTC and execute in brc20-prog (wrapAndExecute2)
    WrapBtc {
        /// Amount of BTC to wrap (in satoshis)
        amount: u64,
        /// Target contract address for wrapAndExecute2
        #[arg(long)]
        target: String,
        /// Function signature to call on target (e.g., "deposit()")
        #[arg(long)]
        signature: String,
        /// Calldata arguments as comma-separated values
        #[arg(long)]
        calldata: String,
        /// Addresses to source UTXOs from
        #[arg(long, num_args = 1..)]
        from: Option<Vec<String>>,
        /// Change address
        #[arg(long)]
        change: Option<String>,
        /// Fee rate in sat/vB
        #[arg(long)]
        fee_rate: Option<f32>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Enable transaction tracing
        #[arg(long)]
        trace: bool,
        /// Mine a block after broadcasting (regtest only)
        #[arg(long)]
        mine: bool,
        /// Automatically confirm the transaction preview
        #[arg(long, short = 'y')]
        auto_confirm: bool,
    },
    /// Get contract deployments made by an address
    GetContractDeploys {
        /// Address or address identifier (e.g., "p2tr:0", "tb1p...")
        address: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get contract bytecode (eth_getCode)
    GetCode {
        /// Contract address (0x prefixed hex)
        address: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Call a contract function (eth_call)
    Call {
        /// Contract address (0x prefixed hex)
        #[arg(long)]
        to: String,
        /// Calldata (0x prefixed hex)
        #[arg(long)]
        data: String,
        /// From address (optional, 0x prefixed hex)
        #[arg(long)]
        from: Option<String>,
        /// Block number or "latest" (optional)
        #[arg(long)]
        block: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get frBTC balance (eth_getBalance)
    GetBalance {
        /// Address (0x prefixed hex)
        address: String,
        /// Block number or "latest" (optional)
        #[arg(long, default_value = "latest")]
        block: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Estimate gas for a transaction (eth_estimateGas)
    EstimateGas {
        /// Contract address (0x prefixed hex)
        #[arg(long)]
        to: String,
        /// Calldata (0x prefixed hex)
        #[arg(long)]
        data: String,
        /// From address (optional, 0x prefixed hex)
        #[arg(long)]
        from: Option<String>,
        /// Block number or "latest" (optional)
        #[arg(long)]
        block: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get current block number (eth_blockNumber)
    BlockNumber {
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get block by number (eth_getBlockByNumber)
    GetBlockByNumber {
        /// Block number (hex or decimal) or "latest"
        block: String,
        /// Include full transaction details
        #[arg(long)]
        full: bool,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get block by hash (eth_getBlockByHash)
    GetBlockByHash {
        /// Block hash (0x prefixed hex)
        hash: String,
        /// Include full transaction details
        #[arg(long)]
        full: bool,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction count/nonce (eth_getTransactionCount)
    GetTransactionCount {
        /// Address (0x prefixed hex)
        address: String,
        /// Block number or "latest"
        #[arg(long, default_value = "latest")]
        block: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction by hash (eth_getTransactionByHash)
    GetTransaction {
        /// Transaction hash (0x prefixed hex)
        hash: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction receipt (eth_getTransactionReceipt)
    GetTransactionReceipt {
        /// Transaction hash (0x prefixed hex)
        hash: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get storage at a specific location (eth_getStorageAt)
    GetStorageAt {
        /// Contract address (0x prefixed hex)
        #[arg(long)]
        address: String,
        /// Storage position (0x prefixed hex)
        #[arg(long)]
        position: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get logs (eth_getLogs)
    GetLogs {
        /// From block (hex or decimal)
        #[arg(long)]
        from_block: Option<String>,
        /// To block (hex or decimal)
        #[arg(long)]
        to_block: Option<String>,
        /// Filter by address (can be specified multiple times)
        #[arg(long)]
        address: Vec<String>,
        /// Filter by topics (JSON array format)
        #[arg(long)]
        topics: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get chain ID (eth_chainId)
    ChainId {
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get gas price (eth_gasPrice)
    GasPrice {
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get BRC20-Prog version (brc20_version)
    Version {
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction receipt by inscription ID (brc20_getTxReceiptByInscriptionId)
    GetReceiptByInscription {
        /// Inscription ID (e.g., "txid:i0")
        inscription_id: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get inscription ID by transaction hash (brc20_getInscriptionIdByTxHash)
    GetInscriptionByTx {
        /// Transaction hash (0x prefixed hex)
        tx_hash: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get inscription ID by contract address (brc20_getInscriptionIdByContractAddress)
    GetInscriptionByContract {
        /// Contract address (0x prefixed hex)
        address: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get BRC20 balance (brc20_balance)
    Brc20Balance {
        /// Bitcoin pkscript (hex)
        #[arg(long)]
        pkscript: String,
        /// BRC20 ticker symbol
        #[arg(long)]
        ticker: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction trace (debug_traceTransaction)
    TraceTransaction {
        /// Transaction hash (0x prefixed hex)
        hash: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get txpool content (txpool_content)
    TxpoolContent {
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get client version (web3_clientVersion)
    ClientVersion {
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
}

/// Alkanes subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum Alkanes {
    /// Execute an alkanes transaction
    Execute(AlkanesExecute),
    /// Inspect an alkanes contract
    Inspect {
        /// The outpoint of the contract
        outpoint: String,
        /// Disassemble the contract bytecode
        #[arg(long)]
        disasm: bool,
        /// Fuzz the contract with a range of opcodes
        #[arg(long)]
        fuzz: bool,
        /// The range of opcodes to fuzz
        #[arg(long)]
        fuzz_ranges: Option<String>,
        /// Show contract metadata
        #[arg(long)]
        meta: bool,
        /// Show the contract code hash
        #[arg(long)]
        codehash: bool,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Trace an alkanes transaction
    Trace {
        /// The outpoint of the transaction to trace
        outpoint: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Simulate an alkanes transaction
    Simulate {
        /// The alkane ID to simulate (format: block:tx:calldata_opcode, e.g., 4:65522:3)
        alkane_id: String,
        /// Input alkanes as comma-separated triplets (e.g., 2:1:1,2:2:100)
        #[arg(long)]
        inputs: Option<String>,
        /// Block height for simulation (defaults to current metashrew_height)
        #[arg(long)]
        height: Option<u64>,
        /// Block hex data (0x prefixed)
        #[arg(long)]
        block: Option<String>,
        /// Transaction hex data (0x prefixed)
        #[arg(long, conflicts_with = "envelope")]
        transaction: Option<String>,
        /// Path to binary file (e.g., WASM) to pack into transaction witness
        #[arg(long, conflicts_with = "transaction")]
        envelope: Option<String>,
        /// Pointer value (defaults to 0)
        #[arg(long, default_value = "0")]
        pointer: u32,
        /// Transaction index (defaults to 1)
        #[arg(long, default_value = "1")]
        txindex: u32,
        /// Refund pointer (defaults to 0)
        #[arg(long, default_value = "0")]
        refund: u32,
        /// Block tag to query (e.g., "latest" or a block height)
        #[arg(long)]
        block_tag: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get the sequence for an outpoint
    Sequence {
        /// Block tag to query (e.g., "latest" or a block height)
        #[arg(long)]
        block_tag: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get spendable outpoints for an address
    Spendables {
        /// The address to get spendables for
        address: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Trace a block
    #[command(name = "traceblock")]
    TraceBlock {
        /// The height of the block to trace
        height: u64,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get the bytecode for an alkane
    #[command(name = "getbytecode")]
    GetBytecode {
        /// The alkane ID to get the bytecode for
        alkane_id: String,
        /// Block tag to query (e.g., "latest" or a block height)
        #[arg(long)]
        block_tag: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get the balance of an address
    #[command(name = "getbalance")]
    GetBalance {
        /// The address to get the balance for
        address: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Wrap BTC to frBTC and lock in vault
    #[command(name = "wrap-btc")]
    WrapBtc {
        /// Amount of BTC to wrap (in satoshis)
        amount: u64,
        /// Address to receive the frBTC tokens
        #[arg(long)]
        to: String,
        /// Addresses to source UTXOs from
        #[arg(long, num_args = 1..)]
        from: Option<Vec<String>>,
        /// Change address
        #[arg(long)]
        change: Option<String>,
        /// Fee rate in sat/vB
        #[arg(long)]
        fee_rate: Option<f32>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Enable transaction tracing
        #[arg(long)]
        trace: bool,
        /// Mine a block after broadcasting (regtest only)
        #[arg(long)]
        mine: bool,
        /// Automatically confirm the transaction preview
        #[arg(long, short = 'y')]
        auto_confirm: bool,
    },
    /// Get pending unwraps
    Unwrap {
        /// Block tag to query (e.g., "latest" or a block height)
        #[arg(long)]
        block_tag: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Backtest a transaction by simulating it in a block
    Backtest {
        /// Transaction ID to backtest
        txid: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get all pools from an AMM factory contract (defaults to 4:65522)
    GetAllPools {
        /// Factory alkane ID (format: block:tx)
        #[arg(long, default_value = "4:65522")]
        factory: String,
        /// Also fetch detailed information for each pool
        #[arg(long)]
        pool_details: bool,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get all pools with detailed information from an AMM factory contract
    AllPoolsDetails {
        /// Factory alkane ID (format: block:tx)
        factory_id: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get details for a specific pool
    PoolDetails {
        /// Pool alkane ID (format: block:tx)
        pool_id: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Initialize a new liquidity pool
    InitPool {
        /// Token pair in format: BLOCK:TX,BLOCK:TX (e.g., 2:0,32:0)
        #[arg(long)]
        pair: String,
        /// Initial liquidity amounts in format: AMOUNT0:AMOUNT1 (e.g., 300000000:50000)
        #[arg(long)]
        liquidity: String,
        /// Recipient address identifier (e.g., p2tr:0)
        #[arg(long)]
        to: String,
        /// Sender address identifier (e.g., p2tr:0)
        #[arg(long)]
        from: String,
        /// Change address identifier (defaults to --from)
        #[arg(long)]
        change: Option<String>,
        /// Minimum LP tokens to receive (optional)
        #[arg(long)]
        minimum: Option<u128>,
        /// Fee rate in sat/vB (optional)
        #[arg(long)]
        fee_rate: Option<f64>,
        /// Show trace after transaction confirms
        #[arg(long)]
        trace: bool,
        /// Factory ID (defaults to 4:1 - the factory proxy)
        #[arg(long, default_value = "4:1")]
        factory: String,
        /// Auto-confirm transaction without prompting
        #[arg(long)]
        auto_confirm: bool,
    },
    /// Execute a swap on the AMM
    Swap {
        /// Swap path as comma-separated alkane IDs (e.g., 2:0,32:0 for DIESEL->frBTC)
        #[arg(long)]
        path: String,
        /// Input token amount
        #[arg(long)]
        input: u128,
        /// Minimum output amount (overrides slippage calculation if provided)
        #[arg(long)]
        minimum_output: Option<u128>,
        /// Slippage percentage (default: 5.0%)
        #[arg(long, default_value = "5.0")]
        slippage: f64,
        /// Expiry block height (defaults to metashrew_height + 100)
        #[arg(long)]
        expires: Option<u64>,
        /// Recipient address identifier (defaults to p2tr:0)
        #[arg(long, default_value = "p2tr:0")]
        to: String,
        /// Sender address identifier (defaults to p2tr:0)
        #[arg(long, default_value = "p2tr:0")]
        from: String,
        /// Change address identifier (defaults to --from)
        #[arg(long)]
        change: Option<String>,
        /// Fee rate in sat/vB (optional)
        #[arg(long)]
        fee_rate: Option<f64>,
        /// Show trace after transaction confirms
        #[arg(long)]
        trace: bool,
        /// Mine a block after broadcasting (regtest only)
        #[arg(long)]
        mine: bool,
        /// Factory ID for path optimization (defaults to 4:65522)
        #[arg(long, default_value = "4:65522")]
        factory: String,
        /// Skip path optimization
        #[arg(long)]
        no_optimize: bool,
        /// Auto-confirm transaction without prompting
        #[arg(long)]
        auto_confirm: bool,
    },
}

/// DataAPI subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum DataApiCommand {
    /// Get all alkanes
    GetAlkanes {
        #[arg(long)]
        limit: Option<i32>,
        #[arg(long)]
        offset: Option<i32>,
        #[arg(long)]
        sort_by: Option<String>,
        #[arg(long)]
        order: Option<String>,
        #[arg(long)]
        search: Option<String>,
        /// Output raw JSON instead of pretty print
        #[arg(long)]
        raw: bool,
    },
    /// Get alkanes for an address
    GetAlkanesByAddress {
        address: String,
        /// Output raw JSON instead of pretty print
        #[arg(long)]
        raw: bool,
    },
    /// Get alkane details
    GetAlkaneDetails {
        /// Alkane ID in format BLOCK:TX (e.g., 2:0)
        id: String,
        /// Output raw JSON instead of pretty print
        #[arg(long)]
        raw: bool,
    },
    /// Get all pools (defaults to factory 4:65522)
    GetPools {
        /// Factory ID in format BLOCK:TX
        #[arg(long, default_value = "4:65522")]
        factory: String,
        /// Output raw JSON instead of pretty print
        #[arg(long)]
        raw: bool,
    },
    /// Get pool details
    GetPoolById {
        /// Pool ID in format BLOCK:TX
        id: String,
        /// Output raw JSON instead of pretty print
        #[arg(long)]
        raw: bool,
    },
    /// Get pool history
    GetPoolHistory {
        /// Pool ID in format BLOCK:TX
        pool_id: String,
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        limit: Option<i32>,
        #[arg(long)]
        offset: Option<i32>,
        /// Output raw JSON instead of pretty print
        #[arg(long)]
        raw: bool,
    },
    /// Get swap history
    GetSwapHistory {
        #[arg(long)]
        pool_id: Option<String>,
        #[arg(long)]
        limit: Option<i32>,
        #[arg(long)]
        offset: Option<i32>,
        /// Output raw JSON instead of pretty print
        #[arg(long)]
        raw: bool,
    },
    /// Get Bitcoin price
    GetBitcoinPrice {
        /// Output raw JSON instead of pretty print
        #[arg(long)]
        raw: bool,
    },
    /// Get Bitcoin market chart
    GetMarketChart {
        /// Number of days (1, 7, 14, 30, 90, 180, 365, max)
        days: String,
        /// Output raw JSON instead of pretty print
        #[arg(long)]
        raw: bool,
    },
    /// Health check
    Health,
}

/// Runestone subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum Runestone {
    /// Analyze a runestone in a transaction
    Analyze {
        /// The transaction ID
        txid: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Trace all protostones in a runestone transaction
    Trace {
        /// The transaction ID
        txid: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
}

/// Protorunes subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum Protorunes {
    /// Get protorunes by address
    ByAddress {
        /// Address to query
        address: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Block tag to query (e.g., "latest" or a block height)
        #[arg(long)]
        block_tag: Option<String>,
        /// Protocol tag
        #[arg(long, default_value = "1")]
        protocol_tag: u128,
    },
    /// Get protorunes by outpoint
    ByOutpoint {
        /// Outpoint to query
        outpoint: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Block tag to query (e.g., "latest" or a block height)
        #[arg(long)]
        block_tag: Option<String>,
        /// Protocol tag
        #[arg(long, default_value = "1")]
        protocol_tag: u128,
    },
}

/// Wallet subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum WalletCommands {
    /// Create a new wallet
    Create {
        /// Optional mnemonic phrase to restore from (if not provided, generates a new one)
        mnemonic: Option<String>,
        /// Output file path for the wallet (default: ~/.alkanes/wallet.json)
        #[arg(short = 'o', long)]
        output: Option<String>,
    },
    /// Get an address from the wallet
    Addresses {
        /// Address range specifications (e.g., "p2tr:0-1000", "p2sh:0-500")
        /// If not provided, shows first 5 addresses of each type for current network
        #[arg(value_delimiter = ' ', num_args = 0..)]
        ranges: Option<Vec<String>>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Show addresses for all networks
        #[arg(long)]
        all_networks: bool,
    },
    /// List UTXOs in the wallet
    Utxos {
        /// Address specifications (e.g., "p2tr:0-100", "bc1q...")
        #[arg()]
        addresses: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Include frozen UTXOs
        #[arg(long)]
        include_frozen: bool,
    },
    /// Freeze a UTXO
    Freeze {
        /// The outpoint of the UTXO to freeze
        outpoint: String,
    },
    /// Unfreeze a UTXO
    Unfreeze {
        /// The outpoint of the UTXO to unfreeze
        outpoint: String,
    },
    /// Sign a PSBT
    Sign {
        /// The PSBT to sign, as a base64 string
        psbt: String,
    },
    /// Send a transaction
    Send {
        /// The address to send to
        address: String,
        /// The amount to send in BTC (e.g., 0.0001 for 10000 satoshis)
        amount: String,
        /// The fee rate in sat/vB
        #[arg(long)]
        fee_rate: Option<f32>,
        /// Send all funds
        #[arg(long)]
        send_all: bool,
        /// The addresses to send from
        #[arg(long, num_args = 1..)]
        from: Option<Vec<String>>,
        /// Skip UTXOs that have alkanes on them
        #[arg(long)]
        lock_alkanes: bool,
        /// The change address
        #[arg(long)]
        change_address: Option<String>,
        /// Use Rebar Shield for private transaction relay (adds payment output to tx)
        #[arg(long)]
        use_rebar: bool,
        /// Rebar fee tier (1 or 2, default: 1). Tier 1: ~8% hashrate, Tier 2: ~16% hashrate
        #[arg(long, default_value = "1")]
        rebar_tier: u8,
        /// Automatically confirm the transaction
        #[arg(long, short = 'y')]
        auto_confirm: bool,
    },
    /// Get the balance of the wallet
    Balance {
        /// The addresses to get the balance for
        #[arg(num_args = 0..)]
        addresses: Option<Vec<String>>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get the history of the wallet
    History {
        /// The number of transactions to get
        #[arg(long, default_value = "10")]
        count: u32,
        /// The address to get the history for
        address: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Create a transaction
    CreateTx {
        /// The address to send to
        address: String,
        /// The amount to send in satoshis
        amount: u64,
        /// The fee rate in sat/vB
        #[arg(long)]
        fee_rate: Option<f32>,
        /// Send all funds
        #[arg(long)]
        send_all: bool,
        /// The addresses to send from
        #[arg(long, num_args = 1..)]
        from: Option<Vec<String>>,
        /// The change address
        #[arg(long)]
        change_address: Option<String>,
    },
    /// Sign a transaction
    SignTx {
        /// The transaction hex to sign (or use --from-file)
        #[arg(required_unless_present = "from_file")]
        tx_hex: Option<String>,
        /// Read transaction hex from file
        #[arg(long)]
        from_file: Option<String>,
        /// Truncate excess inputs to fit within specified size limit
        /// Format: number followed by unit (b/B, k/K, m/M)
        /// Examples: 100k, 1m, 500K, 1000000b
        /// If specified without value, defaults to Bitcoin consensus limit (1m)
        #[arg(long, value_name = "SIZE")]
        truncate_excess_vsize: Option<String>,
        /// Split transaction into multiple transactions with max vsize per transaction
        /// Format: number followed by unit (b/B, k/K, m/M)
        /// Examples: 100k, 1m, 500K, 1000000b
        /// Creates multiple transactions that together achieve the same total effect
        #[arg(long, value_name = "SIZE", conflicts_with = "truncate_excess_vsize")]
        split_max_vsize: Option<String>,
    },
    /// Decode a transaction to view its details
    DecodeTx {
        /// Transaction hex to decode (or use --file to read from file)
        #[arg(required_unless_present = "file")]
        tx_hex: Option<String>,
        /// Read transaction hex from file
        #[arg(long)]
        file: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Broadcast a transaction
    BroadcastTx {
        /// The transaction hex to broadcast
        tx_hex: String,
    },
    /// Estimate the fee for a transaction
    EstimateFee {
        /// The target number of blocks for confirmation
        #[arg(long, default_value = "6")]
        target: u32,
    },
    /// Get the current fee rates
    FeeRates,
    /// Sync the wallet with the blockchain
    Sync,
    /// Backup the wallet
    Backup,
    /// Get the mnemonic for the wallet
    Mnemonic,
}

/// Arguments for the `alkanes execute` command
#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
pub struct AlkanesExecute {
    /// Input requirements for the transaction (format: "B:amount", "B:amount:vN", "block:tx:amount")
    #[arg(long)]
    pub inputs: Option<String>,
    /// Recipient addresses
    #[arg(long, num_args = 1..)]
    pub to: Vec<String>,
    /// Addresses to source UTXOs from
    #[arg(long, num_args = 1..)]
    pub from: Option<Vec<String>>,
    /// Change address for BTC
    #[arg(long)]
    pub change: Option<String>,
    /// Change address for unwanted alkanes (defaults to --change or p2tr:0)
    #[arg(long)]
    pub alkanes_change: Option<String>,
    /// Fee rate in sat/vB
    #[arg(long)]
    pub fee_rate: Option<f32>,
    /// Path to the envelope file (for contract deployment)
    #[arg(long)]
    pub envelope: Option<String>,
    /// Protostone specifications
    pub protostones: Vec<String>,
    /// Show raw JSON output
    #[arg(long)]
    pub raw: bool,
    /// Enable transaction tracing
    #[arg(long)]
    pub trace: bool,
    /// Mine a block after broadcasting (regtest only)
    #[arg(long)]
    pub mine: bool,
    /// Automatically confirm the transaction preview
    #[arg(long, short = 'y')]
    pub auto_confirm: bool,
}

impl From<WalletCommands> for alkanes_cli_common::commands::WalletCommands {
    fn from(cmd: WalletCommands) -> Self {
        match cmd {
            WalletCommands::Addresses {
                ranges,
                raw,
                all_networks,
            } => alkanes_cli_common::commands::WalletCommands::Addresses {
                ranges,
                hd_path: None,
                network: None,
                all_networks,
                magic: None,
                raw,
            },
            _ => serde_json::from_value(serde_json::to_value(cmd).unwrap()).unwrap(),
        }
    }
}

impl From<BitcoindCommands> for alkanes_cli_common::commands::BitcoindCommands {
    fn from(cmd: BitcoindCommands) -> Self {
        serde_json::from_value(serde_json::to_value(cmd).unwrap()).unwrap()
    }
}

impl From<EsploraCommands> for alkanes_cli_common::commands::EsploraCommands {
    fn from(cmd: EsploraCommands) -> Self {
        serde_json::from_value(serde_json::to_value(cmd).unwrap()).unwrap()
    }
}

impl From<OrdCommands> for alkanes_cli_common::commands::OrdCommands {
    fn from(cmd: OrdCommands) -> Self {
        serde_json::from_value(serde_json::to_value(cmd).unwrap()).unwrap()
    }
}

impl From<Runestone> for alkanes_cli_common::commands::RunestoneCommands {
    fn from(cmd: Runestone) -> Self {
        serde_json::from_value(serde_json::to_value(cmd).unwrap()).unwrap()
    }
}

impl From<&DeezelCommands> for alkanes_cli_common::commands::Args {
    fn from(args: &DeezelCommands) -> Self {
        alkanes_cli_common::commands::Args {
            wallet_file: args.wallet_file.clone(),
            passphrase: args.passphrase.clone(),
            hd_path: args.hd_path.clone(),
            wallet_address: args.wallet_address.clone(),
            wallet_key: args.wallet_key.clone(),
            wallet_key_file: args.wallet_key_file.clone(),
            brc20_prog_rpc_url: args.brc20_prog_rpc_url.clone(),
            rpc_config: alkanes_cli_common::network::RpcConfig {
                provider: args.provider.clone(),
                bitcoin_rpc_url: args.bitcoin_rpc_url.clone(),
                sandshrew_rpc_url: args.sandshrew_rpc_url.clone(),
                titan_api_url: args.titan_api_url.clone(),
                esplora_url: args.esplora_api_url.clone(),
                ord_url: args.ord_server_url.clone(),
                metashrew_rpc_url: args.metashrew_rpc_url.clone(),
                timeout_seconds: 600,
            },
            magic: None,
            log_level: "info".to_string(),
            command: alkanes_cli_common::commands::Commands::Bitcoind {
                command: alkanes_cli_common::commands::BitcoindCommands::Getblockchaininfo { raw: false },
            },
        }
    }
}

/// Sandshrew subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum SandshrewCommands {
    /// Execute a Lua script
    Evalscript {
        /// Path to Lua script file
        #[arg(long)]
        script: String,
        /// Arguments to pass to the script
        #[arg(num_args = 0..)]
        args: Vec<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
}
