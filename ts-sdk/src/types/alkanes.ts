/**
 * Alkanes Types for TypeScript SDK
 *
 * These types provide a clean object-based API for Alkanes operations,
 * wrapping the underlying WASM functions that accept JSON strings.
 */

/**
 * Alkane token identifier
 */
export interface AlkaneId {
  block: number;
  tx: number;
}

/**
 * Base execution parameters for Alkanes operations
 */
export interface AlkanesExecuteBaseParams {
  /** Addresses to source UTXOs from (optional) */
  from_addresses?: string[];
  /** Change address (optional, defaults to signer address) */
  change_address?: string;
  /** Fee rate in sat/vB (optional, defaults to 100) */
  fee_rate?: number;
  /** Use MARA Slipstream service for broadcasting (optional) */
  use_slipstream?: boolean;
  /** Use Rebar Shield for private transaction relay (optional) */
  use_rebar?: boolean;
  /** Rebar fee tier: 1 (~8% hashrate) or 2 (~16% hashrate) (optional) */
  rebar_tier?: 1 | 2;
  /** Resume from existing commit transaction (txid) (optional) */
  resume_from_commit?: string;
  /** Auto-confirm transaction without prompting (optional) */
  auto_confirm?: boolean;
}

// ============================================================================
// frBTC Wrap/Unwrap Types
// ============================================================================

/**
 * Parameters for simple BTC to frBTC wrap
 */
export interface FrbtcWrapParams extends AlkanesExecuteBaseParams {
  /** Amount of BTC to wrap (in satoshis) */
  amount: number | bigint;
}

/**
 * Parameters for frBTC to BTC unwrap
 */
export interface FrbtcUnwrapParams extends AlkanesExecuteBaseParams {
  /** Amount of frBTC to unwrap (in satoshis) */
  amount: number | bigint;
  /** Recipient address for the unwrapped BTC */
  recipient_address: string;
  /** Vout index for the inscription output (defaults to 0) */
  vout?: number;
}

/**
 * Parameters for wrap and execute script
 */
export interface FrbtcWrapAndExecuteParams extends AlkanesExecuteBaseParams {
  /** Amount of BTC to wrap (in satoshis) */
  amount: number | bigint;
  /** Script bytecode to deploy and execute (hex-encoded) */
  script_bytecode: string;
}

/**
 * Parameters for wrap and execute contract call
 */
export interface FrbtcWrapAndExecute2Params extends AlkanesExecuteBaseParams {
  /** Amount of BTC to wrap (in satoshis) */
  amount: number | bigint;
  /** Target contract address */
  target_address: string;
  /** Function signature (e.g., "deposit(uint256)") */
  function_signature: string;
  /** Calldata arguments as array or comma-separated string */
  calldata: string[] | string;
}

// ============================================================================
// AMM/Swap Types
// ============================================================================

/**
 * Parameters for AMM token swap
 */
export interface AlkanesSwapParams extends AlkanesExecuteBaseParams {
  /** Factory contract ID */
  factory_id: AlkaneId;
  /** Token path - array of token IDs [input_token, ..., output_token] */
  path: AlkaneId[];
  /** Amount of input token to swap */
  input_amount: number | bigint | string;
  /** Minimum amount of output token (slippage protection) */
  minimum_output: number | bigint | string;
  /** Block number when swap expires */
  expires: number;
  /** Address to receive output tokens */
  to_address: string;
}

/**
 * Parameters for initializing a new AMM pool
 */
export interface AlkanesInitPoolParams extends AlkanesExecuteBaseParams {
  /** Factory contract ID */
  factory_id: AlkaneId;
  /** First token in the pool */
  token0: AlkaneId;
  /** Second token in the pool */
  token1: AlkaneId;
  /** Amount of token0 to deposit */
  amount0: number | bigint | string;
  /** Amount of token1 to deposit */
  amount1: number | bigint | string;
  /** Minimum LP tokens to receive (optional) */
  minimum_lp?: number | bigint | string;
  /** Address to receive LP tokens */
  to_address: string;
}

// ============================================================================
// Raw Alkanes Execute Types
// ============================================================================

/**
 * Parameters for raw Alkanes execute
 *
 * This provides full control over the execute call using the same
 * string formats as the CLI.
 */
export interface AlkanesExecuteParams extends AlkanesExecuteBaseParams {
  /** Recipient addresses */
  to_addresses: string[];
  /**
   * Input requirements string
   * Format: "B:10000" for BTC, "2:0:1000" for alkanes (block:tx:amount)
   * Multiple inputs separated by comma: "B:10000,2:0:500"
   */
  input_requirements: string;
  /**
   * Protostone specification string
   * Format: "[block,tx,opcode]:pointer:refund"
   * Example: "[2,0,77]:v0:v0" calls opcode 77 on contract 2:0
   */
  protostones: string;
  /** Optional envelope data as hex string */
  envelope_hex?: string;
  /** Enable execution tracing (optional) */
  trace_enabled?: boolean;
  /** Enable auto-mining after broadcast - regtest only (optional) */
  mine_enabled?: boolean;
  /** Return raw JSON output (optional) */
  raw_output?: boolean;
}

// ============================================================================
// Result Types
// ============================================================================

/**
 * Result of an Alkanes execution
 */
export interface AlkanesExecuteResult {
  /** Commit transaction ID */
  commit_txid: string;
  /** Reveal transaction ID */
  reveal_txid: string;
  /** Activation transaction ID (if applicable) */
  activation_txid?: string;
  /** Commit transaction fee in sats */
  commit_fee: number;
  /** Reveal transaction fee in sats */
  reveal_fee: number;
  /** Activation transaction fee in sats (if applicable) */
  activation_fee?: number;
  /** Inputs used in the transactions */
  inputs_used?: string[];
  /** Outputs created in the transactions */
  outputs_created?: string[];
  /** Trace results if tracing was enabled */
  traces?: any[];
}

/**
 * Pending unwrap information
 */
export interface PendingUnwrap {
  txid: string;
  vout: number;
  amount: string;
  recipient: string;
}

/**
 * Result of pending unwraps query
 */
export interface PendingUnwrapsResult {
  unwraps: PendingUnwrap[];
}

/**
 * Pool details result
 */
export interface PoolDetailsResult {
  token0: AlkaneId;
  token1: AlkaneId;
  token0_amount: string;
  token1_amount: string;
  token_supply: string;
  pool_name: string;
}

/**
 * Signer address result
 */
export interface SignerAddressResult {
  signer_address: string;
}
