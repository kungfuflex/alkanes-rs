/**
 * Provider initialization utilities for the CLI
 */

import * as path from 'path';
import { fileURLToPath } from 'url';
import { getConfig, CLIConfig } from './config.js';

// Dynamically import WASM module at runtime
// This is necessary because the CLI is bundled to dist/ but WASM is in wasm/
let WebProvider: any = null;

async function loadWasmModule() {
  if (WebProvider) return WebProvider;

  // Resolve path to WASM module relative to the CLI executable
  const wasmPath = path.join(process.cwd(), 'node_modules', '@alkanes', 'ts-sdk', 'wasm', 'alkanes_web_sys.js');

  // Try to load from global installation or local
  try {
    const wasmModule = await import(wasmPath);
    WebProvider = wasmModule.WebProvider;
    return WebProvider;
  } catch {
    // Fallback to relative path from the package
    const relativePath = path.join(__dirname, '..', '..', '..', 'wasm', 'alkanes_web_sys.js');
    const wasmModule = await import(relativePath);
    WebProvider = wasmModule.WebProvider;
    return WebProvider;
  }
}

export interface ProviderOptions {
  network?: string;
  jsonrpcUrl?: string;
  esploraUrl?: string;
  metashrewUrl?: string;
}

/**
 * Create and initialize a WebProvider instance for CLI usage
 */
export async function createProvider(options: ProviderOptions): Promise<any> {
  // Ensure WASM module is loaded
  const Provider = await loadWasmModule();
  const config = await getConfig();

  // Merge CLI options with config file
  const network = options.network || config.network || 'mainnet';
  const jsonrpcUrl = options.jsonrpcUrl || config.jsonrpcUrl;
  const esploraUrl = options.esploraUrl || config.esploraUrl;
  const metashrewUrl = options.metashrewUrl || config.metashrewUrl;

  // Build provider config
  const providerConfig: any = {
    jsonrpc_url: jsonrpcUrl,
    esplora_url: esploraUrl,
    metashrew_url: metashrewUrl,
  };

  // Create provider
  const provider = new Provider(network, JSON.stringify(providerConfig));

  return provider;
}

/**
 * Get default RPC URLs for a given network
 */
export function getDefaultRpcUrl(network: string): string {
  switch (network) {
    case 'mainnet':
      return 'https://bitcoin-mainnet.alkanes.live';
    case 'testnet':
      return 'https://bitcoin-testnet.alkanes.live';
    case 'signet':
      return 'https://bitcoin-signet.alkanes.live';
    case 'regtest':
      return 'http://localhost:18443';
    default:
      return 'https://bitcoin-mainnet.alkanes.live';
  }
}

/**
 * Get default Esplora URL for a given network
 */
export function getDefaultEsploraUrl(network: string): string {
  switch (network) {
    case 'mainnet':
      return 'https://blockstream.info/api';
    case 'testnet':
      return 'https://blockstream.info/testnet/api';
    case 'signet':
      return 'https://mempool.space/signet/api';
    case 'regtest':
      return 'http://localhost:3000';
    default:
      return 'https://blockstream.info/api';
  }
}
