/**
 * WASM utility functions from alkanes-web-sys
 *
 * These functions provide low-level Bitcoin and Alkanes operations
 * powered by the Rust WASM backend.
 */

// Lazy load the WASM module
let wasmModule: any = null;

async function getWasmModule() {
  if (!wasmModule) {
    // Dynamic import to avoid bundling issues
    // Use package-relative path for proper resolution
    wasmModule = await import(/* webpackIgnore: true */ '@alkanes/ts-sdk/wasm');
  }
  return wasmModule;
}

/**
 * Protostone edict from a runestone
 */
export interface ProtostoneEdict {
  id: {
    block: number;
    tx: number;
  };
  amount: number;
  output: number;
}

/**
 * Protostone extracted from a transaction
 */
export interface Protostone {
  burn?: number;
  message: number[];
  edicts: ProtostoneEdict[];
  refund?: number;
  pointer?: number;
  from?: number;
  protocol_tag: number;
}

/**
 * Result of analyzing a runestone
 */
export interface RunestoneAnalysisResult {
  protostone_count: number;
  protostones: Protostone[];
}

/**
 * Analyze a transaction's runestone to extract Protostones
 *
 * This function takes a raw transaction hex string, decodes it, and extracts
 * all Protostones from the transaction's OP_RETURN output.
 *
 * @param txHex - Hexadecimal string of the raw transaction (with or without "0x" prefix)
 * @returns Analysis result with Protostone count and details
 * @throws Error if transaction is invalid or has no runestone
 *
 * @example
 * ```typescript
 * import { analyzeRunestone } from '@alkanes/ts-sdk';
 *
 * const txHex = "0x..."; // Raw transaction hex
 * const result = await analyzeRunestone(txHex);
 *
 * console.log(`Found ${result.protostone_count} Protostones`);
 * result.protostones.forEach((ps, i) => {
 *   console.log(`Protostone ${i}:`, ps);
 * });
 * ```
 */
export async function analyzeRunestone(txHex: string): Promise<RunestoneAnalysisResult> {
  const wasm = await getWasmModule();
  const resultJson = wasm.analyze_runestone(txHex);
  return JSON.parse(resultJson) as RunestoneAnalysisResult;
}
