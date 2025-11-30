/** Exported memory */
export declare const memory: WebAssembly.Memory;
// Exported runtime interface
export declare function __new(size: number, id: number): number;
export declare function __pin(ptr: number): number;
export declare function __unpin(ptr: number): void;
export declare function __collect(): void;
export declare const __rtti_base: number;
/**
 * assembly/utils/memcpy/memcpy
 * @param dest `usize`
 * @param src `usize`
 * @param len `usize`
 * @returns `usize`
 */
export declare function memcpy(dest: number, src: number, len: number): number;
/**
 * assembly/utils/pointer/toPointer
 * @param v `usize`
 * @returns `assembly/utils/pointer/Pointer`
 */
export declare function toPointer(v: number): __Internref4;
/**
 * assembly/alkanes/runtime/abort
 * @param a `i32`
 * @param b `i32`
 * @param c `i32`
 * @param d `i32`
 */
export declare function abort(a: number, b: number, c: number, d: number): void;
/**
 * assembly/alkanes/runtime/__load_storage
 * @param k `i32`
 * @param v `i32`
 * @returns `i32`
 */
export declare function __load_storage(k: number, v: number): number;
/**
 * assembly/alkanes/runtime/__request_storage
 * @param k `i32`
 * @returns `i32`
 */
export declare function __request_storage(k: number): number;
/**
 * assembly/alkanes/runtime/__log
 * @param v `i32`
 */
export declare function __log(v: number): void;
/**
 * assembly/alkanes/runtime/__balance
 * @param who `i32`
 * @param what `i32`
 * @param output `i32`
 */
export declare function __balance(who: number, what: number, output: number): void;
/**
 * assembly/alkanes/runtime/__request_context
 * @returns `i32`
 */
export declare function __request_context(): number;
/**
 * assembly/alkanes/runtime/__load_context
 * @param output `i32`
 * @returns `i32`
 */
export declare function __load_context(output: number): number;
/**
 * assembly/alkanes/runtime/__sequence
 * @param output `i32`
 */
export declare function __sequence(output: number): void;
/**
 * assembly/alkanes/runtime/__fuel
 * @param output `i32`
 */
export declare function __fuel(output: number): void;
/**
 * assembly/alkanes/runtime/__height
 * @param output `i32`
 */
export declare function __height(output: number): void;
/**
 * assembly/alkanes/runtime/__returndatacopy
 * @param output `i32`
 */
export declare function __returndatacopy(output: number): void;
/**
 * assembly/alkanes/runtime/__request_transaction
 * @returns `i32`
 */
export declare function __request_transaction(): number;
/**
 * assembly/alkanes/runtime/__load_transaction
 * @param output `i32`
 */
export declare function __load_transaction(output: number): void;
/**
 * assembly/alkanes/runtime/__request_block
 * @returns `i32`
 */
export declare function __request_block(): number;
/**
 * assembly/alkanes/runtime/__load_block
 * @param output `i32`
 */
export declare function __load_block(output: number): void;
/**
 * assembly/alkanes/runtime/__call
 * @param cellpack `i32`
 * @param incoming_alkanes `i32`
 * @param checkpoint `i32`
 * @param start_fuel `u64`
 * @returns `i32`
 */
export declare function __call(cellpack: number, incoming_alkanes: number, checkpoint: number, start_fuel: bigint): number;
/**
 * assembly/alkanes/runtime/__staticcall
 * @param cellpack `i32`
 * @param incoming_alkanes `i32`
 * @param checkpoint `i32`
 * @param start_fuel `u64`
 * @returns `i32`
 */
export declare function __staticcall(cellpack: number, incoming_alkanes: number, checkpoint: number, start_fuel: bigint): number;
/**
 * assembly/alkanes/runtime/__delegatecall
 * @param cellpack `i32`
 * @param incoming_alkanes `i32`
 * @param checkpoint `i32`
 * @param start_fuel `u64`
 * @returns `i32`
 */
export declare function __delegatecall(cellpack: number, incoming_alkanes: number, checkpoint: number, start_fuel: bigint): number;
/** assembly/alkanes/responder/ExtcallType */
export declare enum ExtcallType {
  /** @type `i32` */
  CALL,
  /** @type `i32` */
  STATICCALL,
  /** @type `i32` */
  DELEGATECALL,
}
/** assembly/utils/pointer/Pointer */
declare class __Internref4 extends Number {
  private __nominal4: symbol;
  private __nominal0: symbol;
}
