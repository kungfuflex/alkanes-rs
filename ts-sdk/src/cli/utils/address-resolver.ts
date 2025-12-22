/**
 * Address resolution system for handling address identifiers
 *
 * This module provides functionality to resolve address identifiers like:
 * - [self:p2tr:0] - Full format with wallet reference
 * - p2tr:0 - Shorthand format (most common)
 * - p2tr:0-10 - Range format
 * - [external:bc1q...] - External address reference
 * - Raw Bitcoin addresses (bc1q..., 1..., 3..., etc.)
 */

import { existsSync, readFileSync } from 'fs';
import { expandPath } from './config.js';
import { loadWalletFile, walletExists } from './wallet.js';

// Valid address types that can be resolved from wallet
const VALID_ADDRESS_TYPES = ['p2tr', 'p2wpkh', 'p2sh-p2wpkh', 'p2pkh'] as const;
type AddressType = typeof VALID_ADDRESS_TYPES[number];

export interface ResolvedAddress {
  original: string;
  resolved: string;
  type: 'wallet' | 'raw' | 'external';
}

export interface AddressResolverConfig {
  walletFile?: string;
  passphrase?: string;
  network?: string;
  jsonrpcUrl?: string;
}

/**
 * Check if a string is a valid address type
 */
export function isValidAddressType(type: string): type is AddressType {
  return VALID_ADDRESS_TYPES.includes(type as AddressType);
}

/**
 * Check if a string looks like a raw Bitcoin address
 */
export function isRawBitcoinAddress(address: string): boolean {
  // P2PKH (legacy) - starts with 1
  if (/^1[a-km-zA-HJ-NP-Z1-9]{25,34}$/.test(address)) return true;
  // P2SH - starts with 3
  if (/^3[a-km-zA-HJ-NP-Z1-9]{25,34}$/.test(address)) return true;
  // Bech32 (P2WPKH/P2WSH) - starts with bc1q or tb1q or bcrt1q
  if (/^(bc|tb|bcrt)1q[a-z0-9]{38,}$/.test(address)) return true;
  // Bech32m (P2TR) - starts with bc1p or tb1p or bcrt1p
  if (/^(bc|tb|bcrt)1p[a-z0-9]{38,}$/.test(address)) return true;
  return false;
}

/**
 * Check if a string is a shorthand wallet identifier (e.g., p2tr:0, p2wpkh:5)
 */
export function isShorthandIdentifier(input: string): boolean {
  const parts = input.split(':');
  if (parts.length !== 2) return false;

  const [type, indexPart] = parts;
  if (!isValidAddressType(type)) return false;

  // Check if indexPart is a number or range (e.g., "0", "0-10")
  if (/^\d+$/.test(indexPart)) return true;
  if (/^\d+-\d+$/.test(indexPart)) return true;

  return false;
}

/**
 * Check if a string is a full identifier (e.g., [self:p2tr:0], [external:bc1q...])
 */
export function isFullIdentifier(input: string): boolean {
  return /^\[.+\]$/.test(input);
}

/**
 * Check if a string contains any address identifiers that need resolution
 */
export function containsIdentifiers(input: string): boolean {
  if (isShorthandIdentifier(input)) return true;
  if (isFullIdentifier(input)) return true;
  return false;
}

/**
 * Parse a shorthand identifier into type and index/range
 */
export function parseShorthandIdentifier(input: string): { type: AddressType; indices: number[] } | null {
  const parts = input.split(':');
  if (parts.length !== 2) return null;

  const [type, indexPart] = parts;
  if (!isValidAddressType(type)) return null;

  const indices: number[] = [];

  // Handle range (e.g., "0-10")
  if (indexPart.includes('-')) {
    const [start, end] = indexPart.split('-').map(Number);
    if (isNaN(start) || isNaN(end) || start > end) return null;
    for (let i = start; i <= end; i++) {
      indices.push(i);
    }
  } else {
    // Handle single index
    const index = parseInt(indexPart, 10);
    if (isNaN(index)) return null;
    indices.push(index);
  }

  return { type, indices };
}

/**
 * Address resolver class that handles wallet-based address resolution
 */
export class AddressResolver {
  private rawProvider: any = null;
  private config: AddressResolverConfig;
  private initialized = false;

  constructor(config: AddressResolverConfig = {}) {
    this.config = config;
  }

  /**
   * Initialize the resolver by loading the wallet
   */
  async initialize(createProvider: (config: any) => Promise<any>): Promise<boolean> {
    if (this.initialized) return true;

    const walletPath = expandPath(this.config.walletFile || '~/.alkanes/wallet.json');

    if (!walletExists(walletPath)) {
      return false;
    }

    try {
      // Create provider
      const provider = await createProvider({
        network: this.config.network,
        jsonrpcUrl: this.config.jsonrpcUrl,
      });

      this.rawProvider = provider.rawProvider;

      // Load wallet with mnemonic
      const walletData = loadWalletFile(walletPath);
      if (!walletData || !walletData.mnemonic) {
        return false;
      }

      // Load the mnemonic into the provider
      this.rawProvider.walletLoadMnemonic(walletData.mnemonic, this.config.passphrase || '');
      this.initialized = true;
      return true;
    } catch (err) {
      return false;
    }
  }

  /**
   * Get a single address from the wallet
   */
  getAddress(addressType: AddressType, index: number): string | null {
    if (!this.initialized || !this.rawProvider) return null;

    try {
      const addresses = this.rawProvider.walletGetAddresses(addressType, index, 1);
      if (addresses && addresses.length > 0) {
        return addresses[0].address;
      }
    } catch (err) {
      // Ignore errors
    }

    return null;
  }

  /**
   * Get multiple addresses from the wallet
   */
  getAddresses(addressType: AddressType, startIndex: number, count: number): string[] {
    if (!this.initialized || !this.rawProvider) return [];

    try {
      const addresses = this.rawProvider.walletGetAddresses(addressType, startIndex, count);
      return addresses.map((a: any) => a.address);
    } catch (err) {
      return [];
    }
  }

  /**
   * Resolve a single address identifier to an actual Bitcoin address
   *
   * Handles:
   * - Raw addresses (returned as-is)
   * - Shorthand identifiers (p2tr:0)
   * - Full identifiers ([self:p2tr:0], [external:bc1q...])
   */
  async resolve(input: string): Promise<string> {
    // Raw Bitcoin address - return as-is
    if (isRawBitcoinAddress(input)) {
      return input;
    }

    // Full identifier format: [self:p2tr:0] or [external:bc1q...]
    if (isFullIdentifier(input)) {
      const inner = input.slice(1, -1); // Remove brackets
      const parts = inner.split(':');

      if (parts[0] === 'external' && parts.length === 2) {
        // [external:bc1q...] - return the address part
        return parts[1];
      }

      if (parts[0] === 'self' && parts.length === 3) {
        // [self:p2tr:0] - resolve from wallet
        const type = parts[1];
        const index = parseInt(parts[2], 10);

        if (isValidAddressType(type) && !isNaN(index)) {
          const address = this.getAddress(type, index);
          if (address) return address;
        }
      }

      throw new Error(`Cannot resolve identifier: ${input}`);
    }

    // Shorthand identifier format: p2tr:0
    if (isShorthandIdentifier(input)) {
      const parsed = parseShorthandIdentifier(input);
      if (!parsed) {
        throw new Error(`Invalid address identifier: ${input}`);
      }

      // For single index, return single address
      if (parsed.indices.length === 1) {
        const address = this.getAddress(parsed.type, parsed.indices[0]);
        if (address) return address;
        throw new Error(`Cannot resolve address for ${input} - wallet not loaded or address not found`);
      }

      // For range, return comma-separated addresses
      const addresses = this.getAddresses(parsed.type, parsed.indices[0], parsed.indices.length);
      if (addresses.length > 0) {
        return addresses.join(',');
      }

      throw new Error(`Cannot resolve addresses for ${input}`);
    }

    // Unknown format - return as-is (might be a valid address we don't recognize)
    return input;
  }

  /**
   * Resolve all identifiers in a string
   * Useful for resolving addresses in complex strings
   */
  async resolveAll(input: string): Promise<string> {
    // If it's a simple identifier, resolve it directly
    if (isShorthandIdentifier(input) || isFullIdentifier(input)) {
      return this.resolve(input);
    }

    // For complex strings, find and replace all identifiers
    // This is a simplified version - full implementation would use regex
    return input;
  }
}

/**
 * Create a pre-initialized address resolver
 */
export async function createAddressResolver(
  config: AddressResolverConfig,
  createProvider: (config: any) => Promise<any>
): Promise<AddressResolver> {
  const resolver = new AddressResolver(config);
  await resolver.initialize(createProvider);
  return resolver;
}

/**
 * Quick helper to resolve an address, handling both raw addresses and identifiers
 *
 * @param address - The address or identifier to resolve
 * @param resolver - Optional pre-initialized resolver
 * @returns The resolved Bitcoin address
 */
export async function resolveAddress(
  address: string,
  resolver?: AddressResolver
): Promise<string> {
  // If it's already a raw address, return it
  if (isRawBitcoinAddress(address)) {
    return address;
  }

  // If no resolver provided and it's an identifier, throw an error
  if (!resolver && containsIdentifiers(address)) {
    throw new Error(
      `Address identifier "${address}" requires a loaded wallet. ` +
      `Please provide --wallet-file and --passphrase options.`
    );
  }

  // If we have a resolver, use it
  if (resolver) {
    return resolver.resolve(address);
  }

  // Otherwise return as-is
  return address;
}

/**
 * Options for quick address resolution
 */
export interface QuickResolveOptions {
  walletFile?: string;
  passphrase?: string;
  network?: string;
  jsonrpcUrl?: string;
}

/**
 * Quick address resolution helper for CLI commands.
 * Loads wallet only if the address contains identifiers.
 *
 * @param address - Address or identifier to resolve (e.g., "p2tr:0" or "bc1q...")
 * @param provider - The AlkanesProvider instance
 * @param opts - Global options containing wallet info
 * @returns Resolved Bitcoin address
 */
export async function resolveAddressWithProvider(
  address: string,
  provider: any,
  opts: QuickResolveOptions
): Promise<string> {
  // Raw address - return as-is
  if (isRawBitcoinAddress(address)) {
    return address;
  }

  // Not an identifier - return as-is
  if (!containsIdentifiers(address)) {
    return address;
  }

  // Need to resolve from wallet
  const walletPath = expandPath(opts.walletFile || '~/.alkanes/wallet.json');

  if (!walletExists(walletPath)) {
    throw new Error(
      `Wallet not found at ${walletPath}. ` +
      `Address identifier "${address}" requires a loaded wallet.`
    );
  }

  const walletData = loadWalletFile(walletPath);
  if (!walletData || !walletData.mnemonic) {
    throw new Error('Failed to load wallet or wallet has no mnemonic');
  }

  // Load mnemonic into provider
  const rawProvider = provider.rawProvider;
  rawProvider.walletLoadMnemonic(walletData.mnemonic, opts.passphrase || '');

  // Parse and resolve
  const parsed = parseShorthandIdentifier(address);
  if (!parsed) {
    throw new Error(`Invalid address identifier: ${address}`);
  }

  // Get addresses
  if (parsed.indices.length === 1) {
    const addresses = rawProvider.walletGetAddresses(parsed.type, parsed.indices[0], 1);
    if (addresses && addresses.length > 0) {
      return addresses[0].address;
    }
    throw new Error(`Could not resolve address for ${address}`);
  }

  // Multiple addresses (range)
  const addresses = rawProvider.walletGetAddresses(parsed.type, parsed.indices[0], parsed.indices.length);
  if (addresses && addresses.length > 0) {
    return addresses.map((a: any) => a.address).join(',');
  }

  throw new Error(`Could not resolve addresses for ${address}`);
}

/**
 * Resolve multiple addresses (e.g., from --from option which may be an array)
 */
export async function resolveAddressesWithProvider(
  addresses: string | string[] | undefined,
  provider: any,
  opts: QuickResolveOptions
): Promise<string[] | undefined> {
  if (!addresses) return undefined;

  const addrList = Array.isArray(addresses) ? addresses : [addresses];
  const resolved: string[] = [];

  for (const addr of addrList) {
    const resolvedAddr = await resolveAddressWithProvider(addr, provider, opts);
    // Handle comma-separated addresses from range resolution
    resolved.push(...resolvedAddr.split(','));
  }

  return resolved;
}
