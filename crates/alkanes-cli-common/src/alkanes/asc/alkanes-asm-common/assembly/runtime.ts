// Alkanes runtime host function imports
// These are provided by the alkanes-runtime when executing WASM

/**
 * Request the size of the execution context
 * @returns Size of the context in bytes
 */
// @ts-ignore: decorator
@external("env", "__request_context")
export declare function __request_context(): i32;

/**
 * Load the execution context into memory
 * @param ptr Pointer to write context data (must have length prefix at ptr-4)
 * @returns The pointer (for chaining)
 */
// @ts-ignore: decorator
@external("env", "__load_context")
export declare function __load_context(ptr: i32): i32;

/**
 * Make a staticcall to another alkane
 * @param cellpack Pointer to cellpack data (length at ptr-4)
 * @param incoming_alkanes Pointer to AlkaneTransferParcel (length at ptr-4)
 * @param checkpoint Pointer to StorageMap (length at ptr-4)
 * @param fuel Maximum fuel to use
 * @returns Length of return data if >= 0, error code if < 0
 */
// @ts-ignore: decorator
@external("env", "__staticcall")
export declare function __staticcall(
  cellpack: i32,
  incoming_alkanes: i32,
  checkpoint: i32,
  fuel: u64
): i32;

/**
 * Copy return data from last staticcall into memory
 * @param ptr Pointer to write return data (length prefix at ptr-4)
 */
// @ts-ignore: decorator
@external("env", "__returndatacopy")
export declare function __returndatacopy(ptr: i32): void;

/**
 * Log a message (for debugging)
 * @param ptr Pointer to message string
 * @param len Length of message
 */
// @ts-ignore: decorator
@external("env", "__log")
export declare function __log(ptr: i32, len: i32): void;

/**
 * Abort execution with an error
 * @param ptr Pointer to error message
 * @param len Length of message
 */
// @ts-ignore: decorator  
@external("env", "__abort")
export declare function __abort(ptr: i32, len: i32): void;

/**
 * Helper to log a string message
 */
export function log(message: string): void {
  const buf = String.UTF8.encode(message);
  __log(changetype<usize>(buf), buf.byteLength);
}

/**
 * Helper to abort with a string message  
 */
export function abort(message: string): void {
  const buf = String.UTF8.encode(message);
  __abort(changetype<usize>(buf), buf.byteLength);
}
