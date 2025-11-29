// Get All Pools - Simple tx-script to fetch pool list from factory
// 
// This is a simplified version that just calls the factory's GetAllPools (opcode 3)
// and returns the list of pools in ExtendedCallResponse format
//
// Output format:
//   ExtendedCallResponse with pool data in the data field

import { u128 } from "as-bignum/assembly";
import { 
  AlkaneResponder, 
  AlkaneId, 
  ExtendedCallResponse 
} from "../../alkanes-asm-common/assembly";

// Factory contract (mainnet)
const FACTORY = new AlkaneId(u128.from(4), u128.from(65522));
const GET_ALL_POOLS_OPCODE = u128.from(3);

/**
 * Main entry point for tx-script execution
 * @returns Pointer to response data (ArrayBuffer with length at ptr-4)
 */
export function __execute(): i32 {
  const responder = new AlkaneResponder();
  responder.log(">>> __execute: Starting get-all-pools");
  
  responder.log(">>> Creating ExtendedCallResponse");
  const response = new ExtendedCallResponse();
  
  responder.log(">>> About to call factory staticcall");
  responder.log(">>> Factory ID: " + FACTORY.block.toString() + ":" + FACTORY.tx.toString());
  responder.log(">>> Opcode: " + GET_ALL_POOLS_OPCODE.toString());
  
  // Call factory to get all pools (opcode 3 = GET_ALL_POOLS)
  const factoryResult = responder.staticcall(FACTORY, GET_ALL_POOLS_OPCODE);
  
  responder.log(">>> staticcall returned");
  
  // Check if call succeeded
  if (factoryResult != null) {
    responder.log(">>> factoryResult is NOT null");
    responder.log(">>> factoryResult.data.byteLength: " + factoryResult.data.byteLength.toString());
    
    // Factory returns: [AlkaneTransferParcel][pool_count(u128)][pool0_block(u128)][pool0_tx(u128)]...
    // We want to return this data in our ExtendedCallResponse
    response.setData(factoryResult.data);
    responder.log(">>> setData complete");
  } else {
    responder.log(">>> factoryResult is NULL - call failed!");
  }
  
  responder.log(">>> About to finalize response");
  // Finalize and return
  const result = response.finalize();
  responder.log(">>> finalize complete, result.byteLength: " + result.byteLength.toString());
  
  const ptr = changetype<i32>(changetype<usize>(result));
  responder.log(">>> Returning pointer: " + ptr.toString());
  
  return ptr;
}
