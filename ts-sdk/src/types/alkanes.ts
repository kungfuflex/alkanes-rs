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
 * Strategy for handling UTXOs that contain ordinal inscriptions
 *
 * - 'exclude': (default) Fail if any selected UTXO contains inscriptions
 * - 'preserve': Split inscribed UTXOs to protect inscriptions, broadcast atomically
 * - 'burn': Allow spending inscribed UTXOs without protection (destroys inscriptions)
 */
export type OrdinalsStrategy = 'exclude' | 'preserve' | 'burn';

/**
 * Base execution parameters for Alkanes operations
 */
export interface AlkanesExecuteBaseParams {
  /** Addresses to source UTXOs from (optional, defaults to [p2wpkh:0, p2tr:0]) */
  from_addresses?: string[];
  /** Change address for BTC (optional, defaults to p2wpkh:0) */
  change_address?: string;
  /** Change address for unwanted alkanes (optional, defaults to p2tr:0) */
  alkanes_change_address?: string;
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
  /**
   * Strategy for handling UTXOs that contain ordinal inscriptions (optional)
   * - 'exclude': (default) Fail if inscribed UTXOs must be spent
   * - 'preserve': Split inscribed UTXOs to protect inscriptions
   * - 'burn': Allow spending inscribed UTXOs (destroys inscriptions)
   */
  ordinals_strategy?: OrdinalsStrategy;
  /**
   * Enable mempool indexer for tracing inscription state of pending UTXOs (optional)
   * When enabled, if spending unconfirmed UTXOs, traces inscription state
   * through parent transactions to detect inscriptions on pending outputs
   */
  mempool_indexer?: boolean;
}

// ============================================================================
// Alkane Transfer Types
// ============================================================================

/**
 * Parameters for transferring alkane tokens to another address
 *
 * Uses an edict-only protostone (no contract call) to move tokens.
 * The Rust layer supports this natively — `ProtostoneSpec.cellpack` is `Option<Cellpack>`,
 * and when `None`, the protostone carries only edicts for pure token transfers.
 *
 * Output layout:
 * - v0: Sender (receives unedicted alkane remainder via runestone pointer)
 * - v1: Recipient (receives transferred amount via edict)
 * - v2: OP_RETURN (protostone)
 * - v3: BTC change
 */
export interface AlkanesTransferParams extends AlkanesExecuteBaseParams {
  /** Alkane token to transfer */
  alkane_id: AlkaneId;
  /** Amount to transfer (in base units) */
  amount: number | bigint | string;
  /** Recipient address */
  to_address: string;
  /**
   * Protostone pointer override (optional, defaults to 'v0')
   * Controls where unedicted alkane remainder goes.
   * Use 'vN' for physical output N, 'pN' for shadow protostone output N.
   */
  pointer?: string;
  /**
   * Protostone refund override (optional, defaults to pointer value)
   * Controls where tokens go on execution failure.
   * Use 'vN' for physical output N, 'pN' for shadow protostone output N.
   */
  refund?: string;
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
  /**
   * Recipient addresses (optional, auto-generated from protostones if not provided)
   * When auto-generated: creates one p2tr:0 output for each protostone v0..vN reference
   */
  to_addresses?: string[];
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
  /** Split transaction ID (if inscribed UTXOs were protected) */
  split_txid?: string;
  /** Split transaction fee in sats (if applicable) */
  split_fee?: number;
  /** Commit transaction ID */
  commit_txid?: string;
  /** Reveal transaction ID */
  reveal_txid: string;
  /** Activation transaction ID (if applicable) */
  activation_txid?: string;
  /** Commit transaction fee in sats */
  commit_fee?: number;
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
