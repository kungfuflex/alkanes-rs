/**
 * BRC20-Prog Types for TypeScript SDK
 *
 * These types provide a clean object-based API for BRC20-prog operations,
 * wrapping the underlying WASM functions that accept JSON strings.
 */

/**
 * Anti-frontrunning strategy for BRC20-Prog transactions
 *
 * - 'presign': Pre-sign all transactions and broadcast together atomically
 * - 'cpfp': Use Child-Pays-For-Parent to accelerate commit transaction
 * - 'cltv': Use CheckLockTimeVerify to timelock reveal transaction
 * - 'rbf': Monitor mempool and use RBF to bump fees if frontrunning detected
 */
export type AntiFrontrunningStrategy = 'presign' | 'cpfp' | 'cltv' | 'rbf';

/**
 * Base execution parameters for BRC20-prog operations
 */
export interface Brc20ProgExecuteParams {
  /** Addresses to source UTXOs from (optional) */
  from_addresses?: string[];
  /** Change address (optional, defaults to signer address) */
  change_address?: string;
  /** Fee rate in sat/vB (optional, defaults to 100) */
  fee_rate?: number;
  /** Use 3-transaction activation pattern instead of 2-transaction (optional) */
  use_activation?: boolean;
  /** Use MARA Slipstream service for broadcasting (optional) */
  use_slipstream?: boolean;
  /** Use Rebar Shield for private transaction relay (optional) */
  use_rebar?: boolean;
  /** Rebar fee tier: 1 (~8% hashrate) or 2 (~16% hashrate) (optional) */
  rebar_tier?: 1 | 2;
  /** Resume from existing commit or reveal transaction (txid) (optional) */
  resume_from_commit?: string;
  /** Anti-frontrunning strategy to use (optional, defaults to 'presign') */
  strategy?: AntiFrontrunningStrategy;
  /**
   * Enable mempool indexer for tracing inscription state of pending UTXOs.
   * When enabled, if we must use pending (unconfirmed) UTXOs, we'll trace back
   * through parent transactions to determine inscription state from settled UTXOs.
   */
  mempool_indexer?: boolean;
  /** Mint DIESEL tokens (contract 2:0) in commit and reveal transactions */
  mint_diesel?: boolean;
}

/**
 * Parameters for deploying a BRC20-prog contract
 */
export interface Brc20ProgDeployParams extends Brc20ProgExecuteParams {
  /** Foundry build JSON containing contract bytecode */
  foundry_json: string | object;
}

/**
 * Parameters for calling a BRC20-prog contract function
 */
export interface Brc20ProgTransactParams extends Brc20ProgExecuteParams {
  /** Contract address to call (0x-prefixed hex) */
  contract_address: string;
  /** Function signature (e.g., "transfer(address,uint256)") */
  function_signature: string;
  /** Calldata arguments as array or comma-separated string */
  calldata: string[] | string;
}

/**
 * Parameters for wrapping BTC into frBTC and executing a contract call
 */
export interface Brc20ProgWrapBtcParams {
  /** Amount of BTC to wrap (in satoshis) */
  amount: number;
  /** Target contract address for wrapAndExecute2 */
  target_contract: string;
  /** Function signature for the target contract call */
  function_signature: string;
  /** Calldata arguments as array or comma-separated string */
  calldata: string[] | string;
  /** Addresses to source UTXOs from (optional) */
  from_addresses?: string[];
  /** Change address (optional, defaults to signer address) */
  change_address?: string;
  /** Fee rate in sat/vB (optional, defaults to 100) */
  fee_rate?: number;
  /** Mint DIESEL tokens in commit and reveal transactions */
  mint_diesel?: boolean;
}

/**
 * Result of a BRC20-prog execution
 */
export interface Brc20ProgExecuteResult {
  /** Split transaction ID (if inscribed UTXOs were split to protect inscriptions) */
  split_txid?: string;
  /** Split transaction fee in sats (if split was needed) */
  split_fee?: number;
  /** Commit transaction ID */
  commit_txid: string;
  /** Reveal transaction ID */
  reveal_txid: string;
  /** Activation transaction ID (for 3-tx pattern) */
  activation_txid?: string;
  /** Commit transaction fee in sats */
  commit_fee: number;
  /** Reveal transaction fee in sats */
  reveal_fee: number;
  /** Activation transaction fee in sats (for 3-tx pattern) */
  activation_fee?: number;
  /** Inputs used in the transactions */
  inputs_used: string[];
  /** Outputs created in the transactions */
  outputs_created: string[];
  /** Trace results if tracing was enabled */
  traces?: any[];
}
