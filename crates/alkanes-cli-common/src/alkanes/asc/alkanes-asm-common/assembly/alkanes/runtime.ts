// Alkanes runtime host function imports
// All pointer parameters point to ArrayBuffer data (length is at ptr-4)

/**
 * Abort execution with error
 * @param a Message pointer (or 0)
 * @param b Message length (or 0)  
 * @param c Line number (or 0)
 * @param d Column number (or 0)
 */
// @ts-ignore: decorator
@external("env", "abort")
export declare function abort(a: i32, b: i32, c: i32, d: i32): void;

/**
 * Load a value from storage
 * @param k Key pointer (ArrayBuffer)
 * @param v Output pointer (ArrayBuffer) to write value
 * @returns Length of value written
 */
// @ts-ignore: decorator
@external("env", "__load_storage")
export declare function __load_storage(k: i32, v: i32): i32;

/**
 * Request the size of a storage value
 * @param k Key pointer (ArrayBuffer)
 * @returns Size of the value in bytes
 */
// @ts-ignore: decorator
@external("env", "__request_storage")
export declare function __request_storage(k: i32): i32;

/**
 * Log a message (for debugging)
 * @param v Message pointer (ArrayBuffer)
 */
// @ts-ignore: decorator
@external("env", "__log")
export declare function __log(v: i32): void;

/**
 * Get balance of an alkane
 * @param who Address pointer (AlkaneId ArrayBuffer)
 * @param what Alkane ID pointer (AlkaneId ArrayBuffer)
 * @param output Output pointer (ArrayBuffer) to write balance (16 bytes)
 */
// @ts-ignore: decorator
@external("env", "__balance")
export declare function __balance(who: i32, what: i32, output: i32): void;

/**
 * Request the size of the execution context
 * @returns Size of the context in bytes
 */
// @ts-ignore: decorator
@external("env", "__request_context")
export declare function __request_context(): i32;

/**
 * Load the execution context into memory at ptr
 * @param output Pointer to write context data (ArrayBuffer)
 * @returns The pointer (for chaining)
 */
// @ts-ignore: decorator
@external("env", "__load_context")
export declare function __load_context(output: i32): i32;

/**
 * Get the current sequence number
 * @param output Output pointer (ArrayBuffer) to write sequence (16 bytes u128)
 */
// @ts-ignore: decorator
@external("env", "__sequence")
export declare function __sequence(output: i32): void;

/**
 * Get the remaining fuel
 * @param output Output pointer (ArrayBuffer) to write fuel (8 bytes u64)
 */
// @ts-ignore: decorator
@external("env", "__fuel")
export declare function __fuel(output: i32): void;

/**
 * Get the current block height
 * @param output Output pointer (ArrayBuffer) to write height (8 bytes u64)
 */
// @ts-ignore: decorator
@external("env", "__height")
export declare function __height(output: i32): void;

/**
 * Copy return data from last call into memory at ptr
 * @param output Pointer to write return data (ArrayBuffer)
 */
// @ts-ignore: decorator
@external("env", "__returndatacopy")
export declare function __returndatacopy(output: i32): void;

/**
 * Request the size of the current transaction
 * @returns Size of the transaction in bytes
 */
// @ts-ignore: decorator
@external("env", "__request_transaction")
export declare function __request_transaction(): i32;

/**
 * Load the current transaction into memory
 * @param output Pointer to write transaction data (ArrayBuffer)
 */
// @ts-ignore: decorator
@external("env", "__load_transaction")
export declare function __load_transaction(output: i32): void;

/**
 * Request the size of the current block
 * @returns Size of the block in bytes
 */
// @ts-ignore: decorator
@external("env", "__request_block")
export declare function __request_block(): i32;

/**
 * Load the current block into memory
 * @param output Pointer to write block data (ArrayBuffer)
 */
// @ts-ignore: decorator
@external("env", "__load_block")
export declare function __load_block(output: i32): void;

/**
 * Make a regular call to another alkane
 * @param cellpack Pointer to cellpack data (ArrayBuffer)
 * @param incoming_alkanes Pointer to AlkaneTransferParcel (ArrayBuffer)
 * @param checkpoint Pointer to StorageMap (ArrayBuffer)
 * @param start_fuel Maximum fuel to use
 * @returns Length of return data if >= 0, error code if < 0
 */
// @ts-ignore: decorator
@external("env", "__call")
export declare function __call(
  cellpack: i32,
  incoming_alkanes: i32,
  checkpoint: i32,
  start_fuel: u64
): i32;

/**
 * Make a staticcall to another alkane (read-only, cannot modify state)
 * @param cellpack Pointer to cellpack data (ArrayBuffer)
 * @param incoming_alkanes Pointer to AlkaneTransferParcel (ArrayBuffer)
 * @param checkpoint Pointer to StorageMap (ArrayBuffer)
 * @param start_fuel Maximum fuel to use
 * @returns Length of return data if >= 0, error code if < 0
 */
// @ts-ignore: decorator
@external("env", "__staticcall")
export declare function __staticcall(
  cellpack: i32,
  incoming_alkanes: i32,
  checkpoint: i32,
  start_fuel: u64
): i32;

/**
 * Make a delegatecall to another alkane (runs in current context)
 * @param cellpack Pointer to cellpack data (ArrayBuffer)
 * @param incoming_alkanes Pointer to AlkaneTransferParcel (ArrayBuffer)
 * @param checkpoint Pointer to StorageMap (ArrayBuffer)
 * @param start_fuel Maximum fuel to use
 * @returns Length of return data if >= 0, error code if < 0
 */
// @ts-ignore: decorator
@external("env", "__delegatecall")
export declare function __delegatecall(
  cellpack: i32,
  incoming_alkanes: i32,
  checkpoint: i32,
  start_fuel: u64
): i32;
