/**
 * Wallet file management utilities
 */

import * as fs from 'fs';
import * as path from 'path';
import { expandPath } from './config.js';

export interface WalletData {
  mnemonic: string;
  network: string;
  created_at?: string;
  encrypted?: boolean;
}

/**
 * Check if a wallet file exists
 */
export function walletExists(walletPath: string): boolean {
  const expandedPath = expandPath(walletPath);
  return fs.existsSync(expandedPath);
}

/**
 * Load wallet from file
 * Note: The wallet file should be encrypted and managed by the WASM provider
 */
export function loadWalletFile(walletPath: string): WalletData {
  const expandedPath = expandPath(walletPath);

  if (!fs.existsSync(expandedPath)) {
    throw new Error(`Wallet file not found: ${walletPath}`);
  }

  const content = fs.readFileSync(expandedPath, 'utf-8');
  return JSON.parse(content);
}

/**
 * Save wallet to file
 */
export function saveWalletFile(walletPath: string, walletData: WalletData): void {
  const expandedPath = expandPath(walletPath);
  const dir = path.dirname(expandedPath);

  // Ensure directory exists
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  // Write wallet file
  fs.writeFileSync(expandedPath, JSON.stringify(walletData, null, 2));

  // Set restrictive permissions (readable/writable only by owner)
  fs.chmodSync(expandedPath, 0o600);
}

/**
 * Get wallet network from file
 */
export function getWalletNetwork(walletPath: string): string {
  const wallet = loadWalletFile(walletPath);
  return wallet.network || 'mainnet';
}

/**
 * Validate mnemonic format (basic check)
 */
export function isValidMnemonic(mnemonic: string): boolean {
  const words = mnemonic.trim().split(/\s+/);
  // BIP39 mnemonics are 12, 15, 18, 21, or 24 words
  return [12, 15, 18, 21, 24].includes(words.length);
}
