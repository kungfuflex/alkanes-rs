/**
 * Core type definitions for @alkanes/ts-sdk
 */

import * as bitcoin from 'bitcoinjs-lib';

/**
 * Bitcoin network types
 */
export type NetworkType = 'mainnet' | 'testnet' | 'signet' | 'regtest';

/**
 * HD wallet derivation path configuration
 */
export interface HDPath {
  purpose: number;
  coinType: number;
  account: number;
  change: number;
  index: number;
}

/**
 * Keystore encryption parameters (compatible with ethers.js style)
 */
export interface KeystoreParams {
  salt: string;
  nonce?: string;
  iterations: number;
  algorithm?: string;
}

/**
 * Encrypted keystore JSON structure (ethers.js compatible)
 */
export interface EncryptedKeystore {
  encrypted_mnemonic: string;
  master_fingerprint: string;
  created_at: number;
  version: string;
  pbkdf2_params: KeystoreParams;
  account_xpub: string;
  hd_paths: Record<string, string>;
}

/**
 * Decrypted keystore object (in-memory only)
 */
export interface Keystore {
  mnemonic: string;
  masterFingerprint: string;
  accountXpub: string;
  hdPaths: Record<string, HDPath>;
  network: NetworkType;
  createdAt: number;
}

/**
 * Wallet configuration
 */
export interface WalletConfig {
  network: NetworkType;
  derivationPath?: string;
  account?: number;
}

/**
 * Address information
 */
export interface AddressInfo {
  address: string;
  path: string;
  publicKey: string;
  index: number;
}

/**
 * Transaction input
 */
export interface TxInput {
  txid: string;
  vout: number;
  value: number;
  address: string;
}

/**
 * Transaction output
 */
export interface TxOutput {
  address: string;
  value: number;
}

/**
 * PSBT build options
 */
export interface PsbtOptions {
  inputs: TxInput[];
  outputs: TxOutput[];
  feeRate?: number;
  network?: bitcoin.networks.Network;
}

/**
 * Alkane token ID
 */
export interface AlkaneId {
  block: number;
  tx: number;
}

/**
 * Alkane balance information
 */
export interface AlkaneBalance {
  id: AlkaneId;
  amount: string;
  name?: string;
  symbol?: string;
  decimals?: number;
}

/**
 * Alkane call parameters
 */
export interface AlkaneCallParams {
  alkaneId: AlkaneId;
  method: string;
  args: any[];
  value?: number;
}

/**
 * Provider configuration for @oyl/sdk compatibility
 */
export interface ProviderConfig {
  url: string;
  projectId?: string;
  network: bitcoin.networks.Network;
  networkType: NetworkType;
  version?: string;
}

/**
 * Transaction result
 */
export interface TransactionResult {
  txId: string;
  rawTx: string;
  size: number;
  weight: number;
  fee: number;
  satsPerVByte: string;
}

/**
 * Block information
 */
export interface BlockInfo {
  hash: string;
  height: number;
  timestamp: number;
  txCount: number;
}

/**
 * UTXO information
 */
export interface UTXO {
  txid: string;
  vout: number;
  value: number;
  status: {
    confirmed: boolean;
    block_height?: number;
    block_hash?: string;
    block_time?: number;
  };
}

/**
 * Address balance
 */
export interface AddressBalance {
  address: string;
  confirmed: number;
  unconfirmed: number;
  utxos: UTXO[];
}

/**
 * Export options
 */
export interface ExportOptions {
  format?: 'json' | 'string';
  pretty?: boolean;
}

/**
 * Import options
 */
export interface ImportOptions {
  validate?: boolean;
  network?: NetworkType;
}
