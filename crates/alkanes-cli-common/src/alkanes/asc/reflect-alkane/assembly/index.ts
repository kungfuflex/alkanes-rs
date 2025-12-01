// Reflect Alkane Tx-Script
// Fetches view opcode information for a single alkane
//
// Inputs (via context):
//   [0]: block - Alkane ID block component (u128)
//   [1]: tx - Alkane ID tx component (u128)
//
// Output format:
//   Serialized AlkaneReflection structure:
//   [name_len(16)][name_bytes][symbol_len(16)][symbol_bytes]
//   [total_supply(16)][cap(16)][minted(16)][value_per_mint(16)]
//   [data_len(16)][data_bytes]

import { 
  AlkaneResponder, 
  AlkaneId, 
  ExtendedCallResponse,
  CallResponse,
  AlkaneTransferParcel,
  StorageMap,
  u128
} from "../../alkanes-asm-common/assembly";
import { __request_context, __load_context, __staticcall, __returndatacopy } from "../../alkanes-asm-common/assembly/runtime";

// Standard view opcodes (based on free-mint pattern)
const OPCODE_GET_NAME = u128.from(99);
const OPCODE_GET_SYMBOL = u128.from(100);
const OPCODE_GET_TOTAL_SUPPLY = u128.from(101);
const OPCODE_GET_CAP = u128.from(102);
const OPCODE_GET_MINTED = u128.from(103);
const OPCODE_GET_VALUE_PER_MINT = u128.from(104);
const OPCODE_GET_DATA = u128.from(1000);

/**
 * Helper to make a low-level staticcall to an alkane opcode
 * Returns the data field from the CallResponse, or null if failed
 */
function callOpcode(block: u128, tx: u128, opcode: u64): ArrayBuffer | null {
  // Build cellpack
  const cellpack = new ArrayBuffer(48);
  const cellpackPtr = changetype<usize>(cellpack);
  
  store<u64>(cellpackPtr, block.lo);
  store<u64>(cellpackPtr + 8, block.hi);
  store<u64>(cellpackPtr + 16, tx.lo);
  store<u64>(cellpackPtr + 24, tx.hi);
  store<u64>(cellpackPtr + 32, opcode);
  store<u64>(cellpackPtr + 40, 0);
  
  // Empty parcel and storage (manually serialized like get-all-pools-details)
  const emptyParcel = new ArrayBuffer(16);
  store<u64>(changetype<usize>(emptyParcel), 0);
  store<u64>(changetype<usize>(emptyParcel) + 8, 0);
  
  const emptyStorage = new ArrayBuffer(4);
  store<u32>(changetype<usize>(emptyStorage), 0);
  
  // Call __staticcall
  const result = __staticcall(
    changetype<i32>(cellpackPtr),
    changetype<i32>(changetype<usize>(emptyParcel)),
    changetype<i32>(changetype<usize>(emptyStorage)),
    0xFFFFFFFFFFFFFFFF
  );
  
  if (result < 0) {
    return null;
  }
  
  // Get return data
  const returndata = new ArrayBuffer(result);
  __returndatacopy(changetype<i32>(changetype<usize>(returndata)));
  const callResponse = CallResponse.fromBytes(returndata);
  return callResponse.data;
}

/**
 * Enriches a single alkane by calling all standard view opcodes
 */
export function enrichAlkane(block: u128, tx: u128, response: ExtendedCallResponse): void {
  // TEST: Call only ONE unimplemented opcode (102) and return the raw result
  // Empty parcel and storage
  const emptyParcel = new ArrayBuffer(16);
  store<u64>(changetype<usize>(emptyParcel), 0);
  store<u64>(changetype<usize>(emptyParcel) + 8, 0);
  
  const emptyStorage = new ArrayBuffer(4);
  store<u32>(changetype<usize>(emptyStorage), 0);
  
  // Call GetName (opcode 99) - IS implemented on 2:0
  const cellpack = new ArrayBuffer(48);
  const cellpackPtr = changetype<usize>(cellpack);
  store<u64>(cellpackPtr, block.lo);
  store<u64>(cellpackPtr + 8, block.hi);
  store<u64>(cellpackPtr + 16, tx.lo);
  store<u64>(cellpackPtr + 24, tx.hi);
  store<u64>(cellpackPtr + 32, 99); // IMPLEMENTED opcode
  store<u64>(cellpackPtr + 40, 0);
  
  const result = __staticcall(
    changetype<i32>(cellpackPtr),
    changetype<i32>(changetype<usize>(emptyParcel)),
    changetype<i32>(changetype<usize>(emptyStorage)),
    10000 // Limited fuel
  );
  
  // Pack the result (i32) into a u64 and return it
  // Don't call __returndatacopy or anything else
  const resultAsU64 = result < 0 ? (0xFFFFFFFF00000000 | (result as u64)) : (result as u64);
  const resultBytes = new ArrayBuffer(8);
  store<u64>(changetype<usize>(resultBytes), resultAsU64);
  
  response.writeU128(u128.Zero); // name_len
  response.writeU128(u128.Zero); // symbol_len
  response.writeU128(u128.Zero); // total_supply
  response.writeU128(u128.Zero); // cap
  response.writeU128(u128.Zero); // minted
  response.writeU128(u128.Zero); // value_per_mint
  response.writeU128(u128.from(8)); // data_len = 8 bytes
  response.appendData(resultBytes); // The raw __staticcall return value
}

/*
  // OLD CODE - commenting out
  // Call GetSymbol (opcode 100)
  const cellpack2 = new ArrayBuffer(48);
  const cellpack2Ptr = changetype<usize>(cellpack2);
  store<u64>(cellpack2Ptr, block.lo);
  store<u64>(cellpack2Ptr + 8, block.hi);
  store<u64>(cellpack2Ptr + 16, tx.lo);
  store<u64>(cellpack2Ptr + 24, tx.hi);
  store<u64>(cellpack2Ptr + 32, 100);
  store<u64>(cellpack2Ptr + 40, 0);
  
  const result2 = __staticcall(
    changetype<i32>(cellpack2Ptr),
    changetype<i32>(changetype<usize>(emptyParcel)),
    changetype<i32>(changetype<usize>(emptyStorage)),
    FUEL_PER_CALL
  );
  
  if (result2 < 0) {
    response.writeU128(u128.Zero);
  } else {
    const returndata2 = new ArrayBuffer(result2);
    __returndatacopy(changetype<i32>(changetype<usize>(returndata2)));
    const callResponse2 = CallResponse.fromBytes(returndata2);
    const symbolData = callResponse2.data;
    response.writeU128(u128.from(symbolData.byteLength));
    response.appendData(symbolData);
  }

  // Call GetTotalSupply (opcode 101)
  const cellpack3 = new ArrayBuffer(48);
  const cellpack3Ptr = changetype<usize>(cellpack3);
  store<u64>(cellpack3Ptr, block.lo);
  store<u64>(cellpack3Ptr + 8, block.hi);
  store<u64>(cellpack3Ptr + 16, tx.lo);
  store<u64>(cellpack3Ptr + 24, tx.hi);
  store<u64>(cellpack3Ptr + 32, 101);
  store<u64>(cellpack3Ptr + 40, 0);
  
  const result3 = __staticcall(
    changetype<i32>(cellpack3Ptr),
    changetype<i32>(changetype<usize>(emptyParcel)),
    changetype<i32>(changetype<usize>(emptyStorage)),
    FUEL_PER_CALL
  );
  
  if (result3 < 0) {
    response.writeU128(u128.Zero);
  } else {
    const returndata3 = new ArrayBuffer(result3);
    __returndatacopy(changetype<i32>(changetype<usize>(returndata3)));
    const callResponse3 = CallResponse.fromBytes(returndata3);
    if (callResponse3.data.byteLength >= 16) {
      const ptr = changetype<usize>(callResponse3.data);
      const totalSupply = new u128(load<u64>(ptr), load<u64>(ptr + 8));
      response.writeU128(totalSupply);
    } else {
      response.writeU128(u128.Zero);
    }
  }

  // Call GetCap (opcode 102) - not implemented on 2:0, will return 0
  const cellpack4 = new ArrayBuffer(48);
  const cellpack4Ptr = changetype<usize>(cellpack4);
  store<u64>(cellpack4Ptr, block.lo);
  store<u64>(cellpack4Ptr + 8, block.hi);
  store<u64>(cellpack4Ptr + 16, tx.lo);
  store<u64>(cellpack4Ptr + 24, tx.hi);
  store<u64>(cellpack4Ptr + 32, 102);
  store<u64>(cellpack4Ptr + 40, 0);
  
  const result4 = __staticcall(
    changetype<i32>(cellpack4Ptr),
    changetype<i32>(changetype<usize>(emptyParcel)),
    changetype<i32>(changetype<usize>(emptyStorage)),
    FUEL_PER_CALL
  );
  
  if (result4 < 0) {
    response.writeU128(u128.Zero);
  } else {
    const returndata4 = new ArrayBuffer(result4);
    __returndatacopy(changetype<i32>(changetype<usize>(returndata4)));
    const callResponse4 = CallResponse.fromBytes(returndata4);
    if (callResponse4.data.byteLength >= 16) {
      const ptr = changetype<usize>(callResponse4.data);
      const cap = new u128(load<u64>(ptr), load<u64>(ptr + 8));
      response.writeU128(cap);
    } else {
      response.writeU128(u128.Zero);
    }
  }

  // TEMPORARY: Skip remaining opcodes to test
  response.writeU128(u128.Zero); // minted
  response.writeU128(u128.Zero); // value_per_mint
  response.writeU128(u128.Zero); // data_len
}

/*
  // Call GetMinted (opcode 103)
  const cellpack5 = new ArrayBuffer(48);
  const cellpack5Ptr = changetype<usize>(cellpack5);
  store<u64>(cellpack5Ptr, block.lo);
  store<u64>(cellpack5Ptr + 8, block.hi);
  store<u64>(cellpack5Ptr + 16, tx.lo);
  store<u64>(cellpack5Ptr + 24, tx.hi);
  store<u64>(cellpack5Ptr + 32, 103);
  store<u64>(cellpack5Ptr + 40, 0);
  
  const result5 = __staticcall(
    changetype<i32>(cellpack5Ptr),
    changetype<i32>(changetype<usize>(emptyParcel)),
    changetype<i32>(changetype<usize>(emptyStorage)),
    FUEL_PER_CALL
  );
  
  if (result5 < 0) {
    response.writeU128(u128.Zero);
  } else {
    const returndata5 = new ArrayBuffer(result5);
    __returndatacopy(changetype<i32>(changetype<usize>(returndata5)));
    const callResponse5 = CallResponse.fromBytes(returndata5);
    if (callResponse5.data.byteLength >= 16) {
      const ptr = changetype<usize>(callResponse5.data);
      const minted = new u128(load<u64>(ptr), load<u64>(ptr + 8));
      response.writeU128(minted);
    } else {
      response.writeU128(u128.Zero);
    }
  }

  // Call GetValuePerMint (opcode 104)
  const cellpack6 = new ArrayBuffer(48);
  const cellpack6Ptr = changetype<usize>(cellpack6);
  store<u64>(cellpack6Ptr, block.lo);
  store<u64>(cellpack6Ptr + 8, block.hi);
  store<u64>(cellpack6Ptr + 16, tx.lo);
  store<u64>(cellpack6Ptr + 24, tx.hi);
  store<u64>(cellpack6Ptr + 32, 104);
  store<u64>(cellpack6Ptr + 40, 0);
  
  const result6 = __staticcall(
    changetype<i32>(cellpack6Ptr),
    changetype<i32>(changetype<usize>(emptyParcel)),
    changetype<i32>(changetype<usize>(emptyStorage)),
    FUEL_PER_CALL
  );
  
  if (result6 < 0) {
    response.writeU128(u128.Zero);
  } else {
    const returndata6 = new ArrayBuffer(result6);
    __returndatacopy(changetype<i32>(changetype<usize>(returndata6)));
    const callResponse6 = CallResponse.fromBytes(returndata6);
    if (callResponse6.data.byteLength >= 16) {
      const ptr = changetype<usize>(callResponse6.data);
      const valuePerMint = new u128(load<u64>(ptr), load<u64>(ptr + 8));
      response.writeU128(valuePerMint);
    } else {
      response.writeU128(u128.Zero);
    }
  }

  // Call GetData (opcode 1000)
  const cellpack7 = new ArrayBuffer(48);
  const cellpack7Ptr = changetype<usize>(cellpack7);
  store<u64>(cellpack7Ptr, block.lo);
  store<u64>(cellpack7Ptr + 8, block.hi);
  store<u64>(cellpack7Ptr + 16, tx.lo);
  store<u64>(cellpack7Ptr + 24, tx.hi);
  store<u64>(cellpack7Ptr + 32, 1000);
  store<u64>(cellpack7Ptr + 40, 0);
  
  const result7 = __staticcall(
    changetype<i32>(cellpack7Ptr),
    changetype<i32>(changetype<usize>(emptyParcel)),
    changetype<i32>(changetype<usize>(emptyStorage)),
    FUEL_PER_CALL
  );
  
  if (result7 < 0) {
    response.writeU128(u128.Zero);
  } else {
    const returndata7 = new ArrayBuffer(result7);
    __returndatacopy(changetype<i32>(changetype<usize>(returndata7)));
    const callResponse7 = CallResponse.fromBytes(returndata7);
    const data = callResponse7.data;
    response.writeU128(u128.from(data.byteLength));
    response.appendData(data);
  }
}
*/

/**
 * Main entry point for tx-script execution
 * @returns Pointer to response data (ArrayBuffer with length at ptr-4)
 */
export function __execute(): i32 {
  const responder = new AlkaneResponder();
  
  // Load context to get alkane ID from inputs
  // Use low-level context loading like get-all-pools-details does
  const contextSize = __request_context();
  const contextBuf = new ArrayBuffer(contextSize);
  __load_context(changetype<i32>(changetype<usize>(contextBuf)));
  
  // Context format: [myself(32)][caller(32)][vout(16)][incoming_alkanes_count(16)][inputs...]
  // Inputs start at offset 96
  const contextPtr = changetype<usize>(contextBuf);
  const inputsOffset: usize = 96;
  
  // Read inputs: [0] = block (u128), [1] = tx (u128)
  const blockLo = load<u64>(contextPtr + inputsOffset);
  const blockHi = load<u64>(contextPtr + inputsOffset + 8);
  const txLo = load<u64>(contextPtr + inputsOffset + 16);
  const txHi = load<u64>(contextPtr + inputsOffset + 24);
  
  const block = new u128(blockLo, blockHi);
  const tx = new u128(txLo, txHi);
  
  // Build response
  const response = new ExtendedCallResponse();
  
  // Enrich the alkane
  enrichAlkane(block, tx, response);
  
  // Finalize and return
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
}
