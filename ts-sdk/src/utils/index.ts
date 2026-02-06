/**
 * Utility functions for Alkanes SDK
 */

import * as bitcoin from 'bitcoinjs-lib';
import { NetworkType, AlkaneId, FeeEstimation } from '../types';

/**
 * Convert network type string to bitcoinjs-lib network object
 */
export function getNetwork(networkType: NetworkType): bitcoin.networks.Network {
  switch (networkType) {
    case 'mainnet':
      return bitcoin.networks.bitcoin;
    case 'testnet':
      return bitcoin.networks.testnet;
    case 'regtest':
      return bitcoin.networks.regtest;
    case 'signet':
      return bitcoin.networks.testnet; // Signet uses testnet params
    default:
      throw new Error(`Unknown network type: ${networkType}`);
  }
}

/**
 * Validate Bitcoin address for a specific network
 */
export function validateAddress(address: string, network?: bitcoin.networks.Network): boolean {
  try {
    bitcoin.address.toOutputScript(address, network);
    return true;
  } catch {
    return false;
  }
}

/**
 * Convert satoshis to BTC
 */
export function satoshisToBTC(satoshis: number): number {
  return satoshis / 100000000;
}

/**
 * Convert BTC to satoshis
 */
export function btcToSatoshis(btc: number): number {
  return Math.round(btc * 100000000);
}

/**
 * Format AlkaneId as string
 */
export function formatAlkaneId(id: AlkaneId): string {
  return `${id.block}:${id.tx}`;
}

/**
 * Parse AlkaneId from string
 */
export function parseAlkaneId(idString: string): AlkaneId {
  const [block, tx] = idString.split(':').map(Number);
  if (isNaN(block) || isNaN(tx)) {
    throw new Error(`Invalid AlkaneId format: ${idString}`);
  }
  return { block, tx };
}

/**
 * Wait for a specific amount of time
 */
export function delay(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Retry a function with exponential backoff
 */
export async function retry<T>(
  fn: () => Promise<T>,
  maxAttempts: number = 3,
  delayMs: number = 1000
): Promise<T> {
  let lastError: Error | undefined;
  
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error as Error;
      if (attempt < maxAttempts - 1) {
        await delay(delayMs * Math.pow(2, attempt));
      }
    }
  }
  
  throw lastError || new Error('Retry failed');
}

/**
 * Calculate transaction fee for given size and fee rate
 */
export function calculateFee(vsize: number, feeRate: number): number {
  return Math.ceil(vsize * feeRate);
}

/**
 * Estimate transaction vsize
 */
export function estimateTxSize(inputCount: number, outputCount: number, inputType: 'legacy' | 'segwit' | 'taproot' = 'segwit'): number {
  const baseSize = 10; // Version (4) + locktime (4) + input count (1) + output count (1)
  const outputSize = 34; // Typical output size
  
  let inputSize: number;
  switch (inputType) {
    case 'legacy':
      inputSize = 148;
      break;
    case 'segwit':
      inputSize = 68; // Witness vsize
      break;
    case 'taproot':
      inputSize = 57.5; // Taproot witness vsize
      break;
  }
  
  return baseSize + (inputCount * inputSize) + (outputCount * outputSize);
}

/**
 * Convert hex string to Uint8Array
 */
export function hexToBytes(hex: string): Uint8Array {
  const clean = hex.replace(/^0x/, '');
  const matches = clean.match(/.{1,2}/g);
  if (!matches) {
    throw new Error('Invalid hex string');
  }
  return new Uint8Array(matches.map(byte => parseInt(byte, 16)));
}

/**
 * Convert Uint8Array to hex string
 */
export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');
}

/**
 * Reverse byte order (for block hashes, txids, etc.)
 */
export function reverseBytes(bytes: Uint8Array): Uint8Array {
  return new Uint8Array(bytes).reverse();
}

/**
 * Convert little-endian hex to big-endian
 */
export function reversedHex(hex: string): string {
  return bytesToHex(reverseBytes(hexToBytes(hex)));
}

/**
 * Check if running in browser
 */
export function isBrowser(): boolean {
  return typeof window !== 'undefined' && typeof window.document !== 'undefined';
}

/**
 * Check if running in Node.js
 */
export function isNode(): boolean {
  return typeof process !== 'undefined' && 
         process.versions != null && 
         process.versions.node != null;
}

/**
 * Safe JSON parse with error handling
 */
export function safeJsonParse<T>(json: string, defaultValue?: T): T | null {
  try {
    return JSON.parse(json);
  } catch (error) {
    if (defaultValue !== undefined) {
      return defaultValue;
    }
    return null;
  }
}

/**
 * Format timestamp to readable date
 */
export function formatTimestamp(timestamp: number): string {
  return new Date(timestamp * 1000).toISOString();
}

/**
 * Calculate transaction weight
 */
export function calculateWeight(baseSize: number, witnessSize: number): number {
  return baseSize * 4 + witnessSize;
}

/**
 * Convert weight to vsize
 */
export function weightToVsize(weight: number): number {
  return Math.ceil(weight / 4);
}

/**
 * Dust threshold in satoshis. Outputs below this value are non-standard
 * and will be rejected by most Bitcoin nodes.
 */
export const DUST_THRESHOLD = 546;

/**
 * Typical input vsize by address type (in vbytes).
 */
export const INPUT_VSIZE: Record<string, number> = {
  legacy: 148,
  segwit: 68,
  taproot: 57.5,
};

/**
 * Typical output vsize by address type (in vbytes).
 */
export const OUTPUT_VSIZE: Record<string, number> = {
  legacy: 34,
  segwit: 31,
  taproot: 43,
};

/**
 * Transaction overhead vsize (version, locktime, segwit marker/flag, varint counts).
 */
export const TX_OVERHEAD_VSIZE = 10.5;

/**
 * Compute accurate BTC send fee accounting for dust threshold on the change output.
 *
 * When change would be below the dust threshold, it is absorbed into the miner fee,
 * which raises the effective fee rate above the requested rate. This function handles
 * both cases (2-output with change, 1-output with dust absorbed) and returns full
 * details for UI display.
 */
export function computeSendFee(params: {
  inputCount: number;
  sendAmount: number;
  totalInputValue: number;
  feeRate: number;
  inputType?: 'legacy' | 'segwit' | 'taproot';
  recipientType?: 'legacy' | 'segwit' | 'taproot';
  changeType?: 'legacy' | 'segwit' | 'taproot';
  dustThreshold?: number;
}): FeeEstimation {
  const {
    inputCount,
    sendAmount,
    totalInputValue,
    feeRate,
    inputType = 'segwit',
    recipientType = 'segwit',
    dustThreshold = DUST_THRESHOLD,
  } = params;
  const changeType = params.changeType ?? inputType;

  const inVsize = INPUT_VSIZE[inputType] ?? INPUT_VSIZE.segwit;
  const recipientOutVsize = OUTPUT_VSIZE[recipientType] ?? OUTPUT_VSIZE.segwit;
  const changeOutVsize = OUTPUT_VSIZE[changeType] ?? OUTPUT_VSIZE.segwit;

  // Try with 2 outputs (recipient + change)
  const vsize2 = inputCount * inVsize + recipientOutVsize + changeOutVsize + TX_OVERHEAD_VSIZE;
  const fee2 = Math.ceil(vsize2 * feeRate);
  const change = totalInputValue - sendAmount - fee2;

  if (change > dustThreshold) {
    return { fee: fee2, numOutputs: 2, change, vsize: vsize2, effectiveFeeRate: feeRate };
  }

  // Change is dust or negative — use 1 output, remainder becomes fee
  const vsize1 = inputCount * inVsize + recipientOutVsize + TX_OVERHEAD_VSIZE;
  const minFee1 = Math.ceil(vsize1 * feeRate);
  const remainder = totalInputValue - sendAmount;

  if (remainder < minFee1) {
    // Not enough to cover even 1-output fee
    return { fee: minFee1, numOutputs: 1, change: 0, vsize: vsize1, effectiveFeeRate: feeRate };
  }

  // Dust absorbed into fee — effective rate is higher than selected
  return { fee: remainder, numOutputs: 1, change: 0, vsize: vsize1, effectiveFeeRate: remainder / vsize1 };
}

/**
 * Lightweight fee estimate for UTXO selection loops.
 *
 * Returns just the fee number (no dust logic). Use this while accumulating UTXOs
 * to estimate when you have enough, then call `computeSendFee` for the final result.
 */
export function estimateSelectionFee(
  inputCount: number,
  feeRate: number,
  inputType: 'legacy' | 'segwit' | 'taproot' = 'segwit',
  outputCount: number = 2,
  outputType: 'legacy' | 'segwit' | 'taproot' = 'segwit',
): number {
  const inVsize = INPUT_VSIZE[inputType] ?? INPUT_VSIZE.segwit;
  const outVsize = OUTPUT_VSIZE[outputType] ?? OUTPUT_VSIZE.segwit;
  const vsize = inputCount * inVsize + outputCount * outVsize + TX_OVERHEAD_VSIZE;
  return Math.ceil(vsize * feeRate);
}

// Re-export WASM utilities
export {
  analyzeRunestone,
  type Protostone,
  type ProtostoneEdict,
  type RunestoneAnalysisResult,
} from './wasm';
