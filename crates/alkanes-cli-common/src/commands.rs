//! CLI command definitions for deezel
//!
//! This module contains the clap-based command definitions, which are
//! shared between the deezel CLI crate and the deezel-sys library crate.

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::network::RpcConfig;

/// Main CLI arguments
#[derive(Parser, Debug, Clone)]
#[command(name = "deezel")]
#[command(about = "DEEZEL - Alkanes SDK")]
#[command(version = "0.1.0")]
pub struct Args {
    #[clap(flatten)]
    pub rpc_config: RpcConfig,


    /// Custom network magic (overrides provider)
    #[arg(long)]
    pub magic: Option<String>,

    /// Wallet file path
    #[arg(short = 'w', long)]
    pub wallet_file: Option<String>,

    /// Wallet passphrase for encrypted wallets
    #[arg(long)]
    pub passphrase: Option<String>,

    /// HD derivation path
    #[arg(long)]
    pub hd_path: Option<String>,

    /// Keystore file path (alternative to wallet-file and passphrase)
    #[arg(long)]
    pub keystore: Option<String>,

    /// Log level
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Command to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum Commands {
    /// Wallet operations
    Wallet {
        #[command(subcommand)]
        command: WalletCommands,
    },
    /// Legacy wallet info command (deprecated, use 'wallet info' instead)
    Walletinfo {
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Bitcoin Core RPC operations
    Bitcoind {
        #[command(subcommand)]
        command: BitcoindCommands,
    },
    /// Metashrew RPC operations
    Metashrew {
        #[command(subcommand)]
        command: MetashrewCommands,
    },
    /// Alkanes smart contract operations
    Alkanes {
        #[command(subcommand)]
        command: AlkanesCommands,
    },
    /// Runestone analysis and decoding
    Runestone {
        #[command(subcommand)]
        command: RunestoneCommands,
    },
    /// Protorunes operations
    Protorunes {
        #[command(subcommand)]
        command: ProtorunesCommands,
    },
    /// Monitor blockchain for events
    Monitor {
        #[command(subcommand)]
        command: MonitorCommands,
    },
    /// Esplora API operations
    Esplora {
        #[command(subcommand)]
        command: EsploraCommands,
    },
    /// Interact with an ord indexer
    #[command(subcommand)]
    Ord(OrdCommands),
}

impl From<RunestoneCommands> for Commands {
    fn from(command: RunestoneCommands) -> Self {
        Commands::Runestone { command }
    }
}

impl Commands {
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Commands::Wallet {
            command: WalletCommands::Info,
        }
    }
}/// Wallet subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum WalletCommands {
    /// Create a new wallet
    Create {
        /// Optional mnemonic phrase (if not provided, a new one will be generated)
        #[arg(long)]
        mnemonic: Option<String>,
    },
    /// Restore wallet from mnemonic
    Restore {
        /// Mnemonic phrase to restore from
        mnemonic: String,
    },
    /// Show wallet information
    Info,
    /// List wallet addresses with flexible range specification
    Addresses {
        /// Address range specifications (e.g., "p2tr:0-1000", "p2sh:0-500")
        /// If not provided, shows first 5 addresses of each type for current network
        #[arg(value_delimiter = ' ')]
        ranges: Option<Vec<String>>,
        /// Custom HD derivation path (overrides default paths)
        #[arg(long)]
        hd_path: Option<String>,
        /// Network to derive addresses for (overrides global -p flag)
        #[arg(short = 'n', long)]
        network: Option<String>,
        /// Show addresses for all networks (mainnet, testnet, signet, regtest)
        #[arg(long)]
        all_networks: bool,
        /// Custom magic bytes in format "p2pkh_prefix,p2sh_prefix,bech32_hrp"
        #[arg(long)]
        magic: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Show wallet balance
    Balance {
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Optional comma-separated list of addresses or identifiers to check balance for
        #[arg(long)]
        addresses: Option<String>,
    },
    /// Send Bitcoin to an address
    Send {
        /// Recipient address or identifier
        address: String,
        /// Amount in satoshis
        amount: u64,
        /// Fee rate in sat/vB
        #[arg(long)]
        fee_rate: Option<f32>,
        /// Send all available funds
        #[arg(long)]
        send_all: bool,
        /// Source addresses (comma-separated)
        #[arg(long, value_delimiter = ',')]
        from: Option<Vec<String>>,
        /// Change address (optional)
        #[arg(long)]
        change: Option<String>,
        /// Auto-confirm without user prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Send all Bitcoin to an address
    SendAll {
        /// Recipient address or identifier
        address: String,
        /// Fee rate in sat/vB
        #[arg(long)]
        fee_rate: Option<f32>,
        /// Auto-confirm without user prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Create a transaction (without broadcasting)
    CreateTx {
        /// Recipient address or identifier
        address: String,
        /// Amount in satoshis
        amount: u64,
        /// Fee rate in sat/vB
        #[arg(long)]
        fee_rate: Option<f32>,
        /// Send all available funds
        #[arg(long)]
        send_all: bool,
        /// Auto-confirm without user prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Sign a transaction
    SignTx {
        /// Transaction hex to sign
        tx_hex: String,
    },
    /// Broadcast a transaction
    BroadcastTx {
        /// Transaction hex to broadcast
        tx_hex: String,
        /// Auto-confirm without user prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// List UTXOs
    Utxos {
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Include frozen UTXOs
        #[arg(long)]
        include_frozen: bool,
        /// Filter UTXOs by specific addresses (comma-separated, supports identifiers like p2tr:0)
        #[arg(long)]
        addresses: Option<String>,
    },
    /// Freeze a UTXO
    FreezeUtxo {
        /// UTXO to freeze (format: txid:vout)
        utxo: String,
        /// Reason for freezing
        #[arg(long)]
        reason: Option<String>,
    },
    /// Unfreeze a UTXO
    UnfreezeUtxo {
        /// UTXO to unfreeze (format: txid:vout)
        utxo: String,
    },
    /// Show transaction history
    History {
        /// Number of transactions to show
        #[arg(short = 'n', long, default_value = "10")]
        count: u32,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Specific address to check (supports identifiers like p2tr:0)
        #[arg(long)]
        address: Option<String>,
    },
    /// Show transaction details
    TxDetails {
        /// Transaction ID
        txid: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Estimate transaction fee
    EstimateFee {
        /// Target confirmation blocks
        #[arg(default_value = "6")]
        target: u32,
    },
    /// Get current fee rates
    FeeRates,
    /// Synchronize wallet with blockchain
    Sync,
    /// Backup wallet
    Backup,
    /// List address identifiers
    ListIdentifiers,
}

impl WalletCommands {
    /// Check if the command requires signing and thus a decrypted private key
    pub fn requires_signing(&self) -> bool {
        matches!(
            self,
            WalletCommands::Send { .. } |
            WalletCommands::SendAll { .. } |
            WalletCommands::CreateTx { .. } |
            WalletCommands::SignTx { .. }
        )
    }
}

/// Bitcoin Core RPC subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BitcoindCommands {
    /// Get current block count
    Getblockcount,
    /// Generate blocks to an address (regtest only)
    Generatetoaddress {
        /// Number of blocks to generate
        nblocks: u32,
        /// Address to generate to
        address: String,
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
        tx_hex: String,
    },
}

/// Metashrew RPC subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum MetashrewCommands {
    /// Get Metashrew height
    Height,
}

/// Alkanes smart contract subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum AlkanesCommands {
    /// Get bytecode for an alkanes contract
    GetBytecode {
        /// Alkane ID (format: block:tx)
        alkane_id: String,
        /// Block tag to query (e.g., "latest" or a block height)
        #[arg(long)]
        block_tag: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Execute alkanes smart contract with commit/reveal pattern
    Execute {
        /// Input requirements (format: "B:amount" for Bitcoin, "block:tx:amount" for alkanes)
        #[arg(long)]
        inputs: String,
        /// Recipient addresses or identifiers
        #[arg(long)]
        to: String,
        /// Change address or identifier
        #[arg(long)]
        change: Option<String>,
        /// Fee rate in sat/vB
        #[arg(long)]
        fee_rate: Option<f32>,
        /// Envelope data file for commit/reveal pattern
        #[arg(long)]
        envelope: Option<String>,
        /// Protostone specifications
        protostones: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Enable transaction tracing
        #[arg(long)]
        trace: bool,
        /// Auto-mine blocks on regtest after transaction broadcast
        #[arg(long)]
        mine: bool,
        /// Auto-confirm without user prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Get alkanes balance for an address
    Balance {
        /// Address to check (defaults to wallet address)
        #[arg(long)]
        address: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Trace an alkanes transaction
    Trace {
        /// Transaction outpoint (format: txid:vout)
        outpoint: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Inspect alkanes bytecode
    Inspect {
        /// Alkane ID (format: block:tx) or bytecode file/hex string
        target: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
        /// Enable disassembly to WAT format
        #[arg(long)]
        disasm: bool,
        /// Enable fuzzing analysis
        #[arg(long)]
        fuzz: bool,
        /// Opcode ranges for fuzzing (e.g., "100-150,200-250")
        #[arg(long)]
        fuzz_ranges: Option<String>,
        /// Extract and display metadata
        #[arg(long)]
        meta: bool,
        /// Compute and display codehash
        #[arg(long)]
        codehash: bool,
    },
    /// Simulate alkanes execution
    Simulate {
        /// Contract ID (format: txid:vout)
        contract_id: String,
        /// Simulation parameters
        #[arg(long)]
        params: Option<String>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get block data
    GetBlock {
        /// Block height
        height: u64,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get sequence of an outpoint
    Sequence {
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Get spendable outpoints by address
    SpendablesByAddress {
        /// Address to query
        address: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Trace a block
    TraceBlock {
        /// Block height
        height: u64,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
}

impl AlkanesCommands {
    /// Check if the command requires signing and thus a decrypted private key
    pub fn requires_signing(&self) -> bool {
        matches!(self, AlkanesCommands::Execute { .. })
    }
}

/// Runestone analysis subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum RunestoneCommands {
    /// Decode runestone from transaction hex
    Decode {
        /// Transaction hex
        tx_hex: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
    /// Analyze runestone from transaction ID
    Analyze {
        /// Transaction ID
        txid: String,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
}

/// Protorunes subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum ProtorunesCommands {
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
        /// Transaction ID
        txid: String,
        /// Output index
        vout: u32,
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

/// Monitor subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum MonitorCommands {
    /// Monitor blocks for events
    Blocks {
        /// Starting block height
        #[arg(long)]
        start: Option<u64>,
        /// Show raw JSON output
        #[arg(long)]
        raw: bool,
    },
}

/// Esplora API subcommands
#[derive(Subcommand, Debug, Clone, Serialize, Deserialize)]
pub enum EsploraCommands {
    /// Get blocks tip hash
    BlocksTipHash {
        #[arg(long)]
        raw: bool,
    },
    /// Get blocks tip height
    BlocksTipHeight {
        #[arg(long)]
        raw: bool,
    },
    /// Get blocks starting from height
    Blocks {
        /// Starting height (optional)
        start_height: Option<u64>,
        #[arg(long)]
        raw: bool,
    },
    /// Get block by height
    BlockHeight {
        /// Block height
        height: u64,
        #[arg(long)]
        raw: bool,
    },
    /// Get block information
    Block {
        /// Block hash
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get block status
    BlockStatus {
        /// Block hash
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get block transaction IDs
    BlockTxids {
        /// Block hash
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get block header
    BlockHeader {
        /// Block hash
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get raw block data
    BlockRaw {
        /// Block hash
        hash: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction ID by block hash and index
    BlockTxid {
        /// Block hash
        hash: String,
        /// Transaction index
        index: u32,
        #[arg(long)]
        raw: bool,
    },
    /// Get block transactions
    BlockTxs {
        /// Block hash
        hash: String,
        /// Start index (optional)
        start_index: Option<u32>,
        #[arg(long)]
        raw: bool,
    },
    /// Get address information
    Address {
        /// Address or colon-separated parameters
        params: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get address transactions
    AddressTxs {
        /// Address or colon-separated parameters
        params: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get address chain transactions
    AddressTxsChain {
        /// Address or colon-separated parameters (address:last_seen_txid)
        params: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get address mempool transactions
    AddressTxsMempool {
        /// Address
        address: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get address UTXOs
    AddressUtxo {
        /// Address
        address: String,
        #[arg(long)]
        raw: bool,
    },
    /// Search addresses by prefix
    AddressPrefix {
        /// Address prefix
        prefix: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction information
    Tx {
        /// Transaction ID
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction hex
    TxHex {
        /// Transaction ID
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get raw transaction
    TxRaw {
        /// Transaction ID
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction status
    TxStatus {
        /// Transaction ID
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction merkle proof
    TxMerkleProof {
        /// Transaction ID
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction merkle block proof
    TxMerkleblockProof {
        /// Transaction ID
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction output spend status
    TxOutspend {
        /// Transaction ID
        txid: String,
        /// Output index
        index: u32,
        #[arg(long)]
        raw: bool,
    },
    /// Get transaction output spends
    TxOutspends {
        /// Transaction ID
        txid: String,
        #[arg(long)]
        raw: bool,
    },
    /// Broadcast transaction
    Broadcast {
        /// Transaction hex
        tx_hex: String,
        #[arg(long)]
        raw: bool,
    },
    /// Post transaction (alias for broadcast)
    PostTx {
        /// Transaction hex
        tx_hex: String,
        #[arg(long)]
        raw: bool,
    },
    /// Get mempool information
    Mempool {
        #[arg(long)]
        raw: bool,
    },
    /// Get mempool transaction IDs
    MempoolTxids {
        #[arg(long)]
        raw: bool,
    },
    /// Get recent mempool transactions
    MempoolRecent {
        #[arg(long)]
        raw: bool,
    },
    /// Get fee estimates
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