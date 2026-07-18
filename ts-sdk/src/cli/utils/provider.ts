/**
 * Provider initialization utilities for the CLI
 *
 * The CLI uses the SDK's AlkanesProvider which wraps the WASM WebProvider.
 * This ensures the CLI and SDK share the same typed interface.
 *
 * Uses dynamic imports to avoid bundling WASM files during CLI build.
 */

import { getConfig } from './config.js';

export interface ProviderOptions {
  network?: string;
  jsonrpcUrl?: string;
  esploraUrl?: string;
  metashrewUrl?: string;
}

// Cache the provider instance for reuse within the same CLI session
let cachedProvider: any = null;
let cachedNetwork: string | null = null;

/**
 * Create and initialize an AlkanesProvider instance for CLI usage.
 * This uses the SDK's high-level provider which wraps the WASM WebProvider.
 *
 * Uses dynamic imports to avoid bundling issues with WASM.
 */
export async function createProvider(options: ProviderOptions): Promise<any> {
  const config = await getConfig();

  // Merge CLI options with config file
  const network = options.network || config.network || 'mainnet';
  const rpcUrl = options.jsonrpcUrl || config.jsonrpcUrl;

  // Return cached provider if network matches
  if (cachedProvider && cachedNetwork === network) {
    return cachedProvider;
  }

  // Dynamic import to avoid bundling WASM during CLI build
  const { AlkanesProvider } = await import('../../provider/index.js');

  // Build provider config
  const providerConfig = {
    network,
    rpcUrl,
  };

  // Create and initialize the SDK provider
  const provider = new AlkanesProvider(providerConfig);
  await provider.initialize();

  // Cache for reuse
  cachedProvider = provider;
  cachedNetwork = network;

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
