//! # CLI Commands for `deezel`
//!
//! This module defines the `clap`-based command structure for the `deezel` CLI,
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

/// Deezel is a command-line tool for interacting with Bitcoin and Ordinals
#[derive(Parser, Debug, Clone, Serialize, Deserialize)]
#[command(author, version, about, long_about = None)]
pub struct DeezelCommands {
    /// Path to the wallet file
    #[arg(long)]
    pub wallet_file: Option<String>,
    /// Passphrase for the wallet
    #[arg(long)]
    pub passphrase: Option<String>,
    /// HD path for the wallet
    #[arg(long)]
    pub hd_path: Option<String>,
    /// Path to the keystore file
    #[arg(long)]
    pub keystore: Option<String>,
    /// Wallet address (for address-only operations without keystore)
    #[arg(long)]
    pub wallet_address: Option<String>,
    /// Wallet private key file (for signing transactions externally)
    #[arg(long)]
    pub wallet_key_file: Option<String>,
    /// Sandshrew RPC URL
    #[arg(long)]
    pub sandshrew_rpc_url: Option<String>,
    /// Bitcoin RPC URL
    #[arg(long)]
    pub bitcoin_rpc_url: Option<String>,
    /// Esplora API URL
    #[arg(long)]
    pub esplora_api_url: Option<String>,
    /// Ord server URL
    #[arg(long)]
    pub ord_server_url: Option<String>,
    /// Metashrew server URL
    #[arg(long)]
    pub metashrew_server_url: Option<String>,
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
        /// The contract ID to simulate
        contract_id: String,
        /// The parameters to pass to the contract
        params: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get the sequence for an outpoint
    Sequence {
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
        /// The passphrase for the new wallet
        passphrase: Option<String>,
        mnemonic: Option<String>,
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
        /// Truncate excess inputs if signed transaction exceeds consensus limit (1MB)
        #[arg(long)]
        truncate_excess_vsize: bool,
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
    /// Input requirements for the transaction
    #[arg(long)]
    pub inputs: Option<String>,
    /// Recipient addresses
    #[arg(long, num_args = 1..)]
    pub to: Vec<String>,
    /// Addresses to source UTXOs from
    #[arg(long, num_args = 1..)]
    pub from: Option<Vec<String>>,
    /// Change address
    #[arg(long)]
    pub change: Option<String>,
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
            keystore: args.keystore.clone(),
            wallet_file: args.wallet_file.clone(),
            passphrase: args.passphrase.clone(),
            hd_path: args.hd_path.clone(),
            wallet_address: args.wallet_address.clone(),
            wallet_key_file: args.wallet_key_file.clone(),
            rpc_config: alkanes_cli_common::network::RpcConfig {
                provider: args.provider.clone(),
                bitcoin_rpc_url: args.bitcoin_rpc_url.clone(),
                sandshrew_rpc_url: args.sandshrew_rpc_url.clone(),
                esplora_url: args.esplora_api_url.clone(),
                ord_url: args.ord_server_url.clone(),
                metashrew_rpc_url: args.metashrew_server_url.clone(),
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
