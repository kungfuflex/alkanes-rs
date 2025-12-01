// Reflect Alkane Range Tx-Script
// Fetches view opcode information for a range of alkanes with IDs in format 2:n (or m:n)
//
// Inputs (via context):
//   [0]: block - Alkane ID block component (u128, typically 2)
//   [1]: start_tx - Starting tx number in range (u128)
//   [2]: end_tx - Ending tx number in range (u128)
//
// Output format:
//   [count(16)] - Number of alkanes successfully reflected
//   For each alkane:
//     [alkane_block(16)][alkane_tx(16)]
//     [name_len(16)][name_bytes][symbol_len(16)][symbol_bytes]
//     [total_supply(16)][cap(16)][minted(16)][value_per_mint(16)]
//     [data_len(16)][data_bytes]

import { 
  AlkaneResponder, 
  AlkaneId, 
  ExtendedCallResponse,
  u128
} from "../../alkanes-asm-common/assembly";
import { __request_context, __load_context } from "../../alkanes-asm-common/assembly/runtime";

// Standard view opcodes (based on free-mint pattern)
const OPCODE_GET_NAME = u128.from(99);
const OPCODE_GET_SYMBOL = u128.from(100);
const OPCODE_GET_TOTAL_SUPPLY = u128.from(101);
const OPCODE_GET_CAP = u128.from(102);
const OPCODE_GET_MINTED = u128.from(103);
const OPCODE_GET_VALUE_PER_MINT = u128.from(104);
const OPCODE_GET_DATA = u128.from(1000);

/**
 * Enriches a single alkane by calling all standard view opcodes
 * @param responder AlkaneResponder instance
 * @param alkaneId Target alkane ID
 * @param response ExtendedCallResponse to write results to
 * @returns true if at least one opcode succeeded, false if all failed
 */
function enrichAlkane(
  responder: AlkaneResponder,
  alkaneId: AlkaneId,
  response: ExtendedCallResponse
): bool {
  let anySuccess = false;

  // Write alkane ID first
  response.writeU128(alkaneId.block);
  response.writeU128(alkaneId.tx);

  // Call GetName (opcode 99)
  const nameResponse = responder.staticcall(alkaneId, OPCODE_GET_NAME);
  if (nameResponse) {
    anySuccess = true;
    const nameData = nameResponse.data;
    response.writeU128(u128.from(nameData.byteLength));
    response.appendData(nameData);
  } else {
    response.writeU128(u128.Zero);
  }

  // Call GetSymbol (opcode 100)
  const symbolResponse = responder.staticcall(alkaneId, OPCODE_GET_SYMBOL);
  if (symbolResponse) {
    anySuccess = true;
    const symbolData = symbolResponse.data;
    response.writeU128(u128.from(symbolData.byteLength));
    response.appendData(symbolData);
  } else {
    response.writeU128(u128.Zero);
  }

  // Call GetTotalSupply (opcode 101)
  const totalSupplyResponse = responder.staticcall(alkaneId, OPCODE_GET_TOTAL_SUPPLY);
  if (totalSupplyResponse && totalSupplyResponse.data.byteLength >= 16) {
    anySuccess = true;
    const ptr = changetype<usize>(totalSupplyResponse.data);
    const totalSupply = new u128(load<u64>(ptr), load<u64>(ptr + 8));
    response.writeU128(totalSupply);
  } else {
    response.writeU128(u128.Zero);
  }

  // Call GetCap (opcode 102)
  const capResponse = responder.staticcall(alkaneId, OPCODE_GET_CAP);
  if (capResponse && capResponse.data.byteLength >= 16) {
    anySuccess = true;
    const ptr = changetype<usize>(capResponse.data);
    const cap = new u128(load<u64>(ptr), load<u64>(ptr + 8));
    response.writeU128(cap);
  } else {
    response.writeU128(u128.Zero);
  }

  // Call GetMinted (opcode 103)
  const mintedResponse = responder.staticcall(alkaneId, OPCODE_GET_MINTED);
  if (mintedResponse && mintedResponse.data.byteLength >= 16) {
    anySuccess = true;
    const ptr = changetype<usize>(mintedResponse.data);
    const minted = new u128(load<u64>(ptr), load<u64>(ptr + 8));
    response.writeU128(minted);
  } else {
    response.writeU128(u128.Zero);
  }

  // Call GetValuePerMint (opcode 104)
  const valuePerMintResponse = responder.staticcall(alkaneId, OPCODE_GET_VALUE_PER_MINT);
  if (valuePerMintResponse && valuePerMintResponse.data.byteLength >= 16) {
    anySuccess = true;
    const ptr = changetype<usize>(valuePerMintResponse.data);
    const valuePerMint = new u128(load<u64>(ptr), load<u64>(ptr + 8));
    response.writeU128(valuePerMint);
  } else {
    response.writeU128(u128.Zero);
  }

  // Call GetData (opcode 1000)
  const dataResponse = responder.staticcall(alkaneId, OPCODE_GET_DATA);
  if (dataResponse) {
    anySuccess = true;
    const data = dataResponse.data;
    response.writeU128(u128.from(data.byteLength));
    response.appendData(data);
  } else {
    response.writeU128(u128.Zero);
  }

  return anySuccess;
}

/**
 * Main entry point for tx-script execution
 * @returns Pointer to response data (ArrayBuffer with length at ptr-4)
 */
export function __execute(): i32 {
  const responder = new AlkaneResponder();
  
  // Load context to get range parameters
  // Use low-level context loading like get-all-pools-details does
  const contextSize = __request_context();
  const contextBuf = new ArrayBuffer(contextSize);
  __load_context(changetype<i32>(changetype<usize>(contextBuf)));
  
  // Context format: [myself(32)][caller(32)][vout(16)][incoming_alkanes_count(16)][inputs...]
  // Inputs start at offset 96
  const contextPtr = changetype<usize>(contextBuf);
  const inputsOffset: usize = 96;
  
  // Read inputs: [0] = block (u128), [1] = start_tx (u128), [2] = end_tx (u128)
  const blockLo = load<u64>(contextPtr + inputsOffset);
  const blockHi = load<u64>(contextPtr + inputsOffset + 8);
  const startTxLo = load<u64>(contextPtr + inputsOffset + 16);
  const startTxHi = load<u64>(contextPtr + inputsOffset + 24);
  const endTxLo = load<u64>(contextPtr + inputsOffset + 32);
  const endTxHi = load<u64>(contextPtr + inputsOffset + 40);
  
  const block = new u128(blockLo, blockHi);
  const startTx = new u128(startTxLo, startTxHi);
  const endTx = new u128(endTxLo, endTxHi);
  
  // Get current sequence to clamp range
  const sequence = responder.getSequence();
  
  // Clamp end_tx to sequence (max alkane that can exist)
  let actualEndTx = endTx;
  if (endTx.gt(sequence)) {
    actualEndTx = sequence;
  }
  
  // Build response - reserve space for count at the beginning
  const response = new ExtendedCallResponse();
  
  // We'll write count placeholder and update it later
  let count = u128.Zero;
  
  // Temporary buffer to hold all alkane data (we'll prepend count later)
  const tempResponse = new ExtendedCallResponse();
  
  // Iterate through range
  let currentTx = startTx;
  while (currentTx.lte(actualEndTx)) {
    const alkaneId = new AlkaneId(block, currentTx);
    
    // Try to enrich this alkane
    if (enrichAlkane(responder, alkaneId, tempResponse)) {
      count = count.add(u128.One);
    }
    
    // Increment currentTx
    currentTx = currentTx.add(u128.One);
    
    // Safety: break if we've gone too far (shouldn't happen with proper clamping)
    if (currentTx.lt(startTx)) {
      break; // Overflow protection
    }
  }
  
  // Write count first
  response.writeU128(count);
  
  // Append all the alkane data
  response.appendData(tempResponse.finalize());
  
  // Finalize and return
  const finalBuf = response.finalize();
  return changetype<i32>(changetype<usize>(finalBuf));
}
