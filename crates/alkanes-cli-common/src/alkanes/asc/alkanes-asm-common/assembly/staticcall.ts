// High-level staticcall helpers

import { __staticcall, __returndatacopy } from "./runtime";
import { AlkaneId, Cellpack, EmptyAlkaneParcel, EmptyStorageMap, CallResponse } from "./types";
import { getDataPtr } from "./arraybuffer";

/**
 * Make a staticcall to an alkane and return the response
 * @param target Target alkane ID
 * @param opcode Opcode to call
 * @param maxFuel Maximum fuel (default: max u64)
 * @returns CallResponse or null if failed
 */
export function staticcall(
  target: AlkaneId,
  opcode: u128,
  maxFuel: u64 = 0xFFFFFFFFFFFFFFFF
): CallResponse | null {
  // Build cellpack
  const cellpack = new Cellpack(target, [opcode]);
  const cellpackBuf = cellpack.toArrayBuffer();
  const cellpackPtr = getDataPtr(cellpackBuf);
  
  // Empty parcels
  const alkanesPtr = EmptyAlkaneParcel.getDataPtr();
  const storagePtr = EmptyStorageMap.getDataPtr();
  
  // Make the call
  const result = __staticcall(
    cellpackPtr as i32,
    alkanesPtr as i32,
    storagePtr as i32,
    maxFuel
  );
  
  // Check for error
  if (result < 0) {
    return null;
  }
  
  // Allocate buffer for return data with length prefix
  const returnBuf = new ArrayBuffer(result + 4);
  store<u32>(changetype<usize>(returnBuf), result);
  
  // Copy return data
  __returndatacopy((changetype<usize>(returnBuf) + 4) as i32);
  
  return new CallResponse(returnBuf);
}

/**
 * Make a staticcall with multiple inputs
 */
export function staticcallWithInputs(
  target: AlkaneId,
  inputs: u128[],
  maxFuel: u64 = 0xFFFFFFFFFFFFFFFF
): CallResponse | null {
  // Build cellpack
  const cellpack = new Cellpack(target, inputs);
  const cellpackBuf = cellpack.toArrayBuffer();
  const cellpackPtr = getDataPtr(cellpackBuf);
  
  // Empty parcels
  const alkanesPtr = EmptyAlkaneParcel.getDataPtr();
  const storagePtr = EmptyStorageMap.getDataPtr();
  
  // Make the call
  const result = __staticcall(
    cellpackPtr as i32,
    alkanesPtr as i32,
    storagePtr as i32,
    maxFuel
  );
  
  // Check for error
  if (result < 0) {
    return null;
  }
  
  // Allocate buffer for return data with length prefix
  const returnBuf = new ArrayBuffer(result + 4);
  store<u32>(changetype<usize>(returnBuf), result);
  
  // Copy return data
  __returndatacopy((changetype<usize>(returnBuf) + 4) as i32);
  
  return new CallResponse(returnBuf);
}

/**
 * Factory contract opcodes
 */
export namespace FactoryOpcodes {
  export const GET_ALL_POOLS: u128 = 3;
}

/**
 * Pool contract opcodes
 */
export namespace PoolOpcodes {
  export const GET_RESERVES: u128 = 97;
  export const GET_PRICE_CUMULATIVE_LAST: u128 = 98;
  export const GET_NAME: u128 = 99;
  export const GET_POOL_DETAILS: u128 = 999;
}
