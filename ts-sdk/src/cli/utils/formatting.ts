/**
 * Output formatting utilities for the CLI
 *
 * Provides tree-based human-readable output by default, with JSON available via --raw flag.
 * Supports verbose logging levels (-v, -vv, -vvv).
 */

import chalk from 'chalk';
import Table from 'cli-table3';

export interface OutputOptions {
  raw?: boolean;      // Output raw JSON
  verbose?: number;   // Verbosity level (0-3)
  color?: boolean;    // Use colors (default: true)
}

// Global verbosity level (set by CLI)
let globalVerbosity = 0;

/**
 * Set the global verbosity level
 */
export function setVerbosity(level: number): void {
  globalVerbosity = level;
}

/**
 * Get current verbosity level
 */
export function getVerbosity(): number {
  return globalVerbosity;
}

/**
 * Log at different verbosity levels
 * - Level 1 (-v): Basic info
 * - Level 2 (-vv): Detailed info
 * - Level 3 (-vvv): Debug/trace info including RPC calls
 */
export function verbose(level: number, message: string): void {
  if (globalVerbosity >= level) {
    const prefix = level === 3 ? chalk.dim('[DEBUG]') :
                   level === 2 ? chalk.blue('[INFO]') :
                   chalk.cyan('[VERBOSE]');
    console.error(`${prefix} ${message}`);
  }
}

// ============================================================================
// Tree View Implementation
// ============================================================================

/**
 * Tree node for building tree structures
 */
export class TreeNode {
  label: string;
  children: TreeNode[] = [];

  constructor(label: string) {
    this.label = label;
  }

  /**
   * Add a child node
   */
  push(child: TreeNode | string): TreeNode {
    if (typeof child === 'string') {
      this.children.push(new TreeNode(child));
    } else {
      this.children.push(child);
    }
    return this;
  }

  /**
   * Add multiple children
   */
  withLeaves(children: (TreeNode | string)[]): TreeNode {
    for (const child of children) {
      this.push(child);
    }
    return this;
  }

  /**
   * Render the tree as a string
   */
  toString(prefix: string = '', isLast: boolean = true, isRoot: boolean = true): string {
    const lines: string[] = [];

    // Add the current node
    if (isRoot) {
      lines.push(this.label);
    } else {
      const connector = isLast ? '└── ' : '├── ';
      lines.push(prefix + connector + this.label);
    }

    // Add children
    const childPrefix = isRoot ? '' : prefix + (isLast ? '    ' : '│   ');
    for (let i = 0; i < this.children.length; i++) {
      const child = this.children[i];
      const childIsLast = i === this.children.length - 1;
      lines.push(child.toString(childPrefix, childIsLast, false));
    }

    return lines.join('\n');
  }
}

/**
 * Create a new tree node
 */
export function tree(label: string): TreeNode {
  return new TreeNode(label);
}

// ============================================================================
// JSON Helpers
// ============================================================================

/**
 * JSON replacer to handle BigInt and other special types
 */
function jsonReplacer(key: string, value: any): any {
  if (typeof value === 'bigint') {
    return value.toString();
  }
  return value;
}

// ============================================================================
// Output Formatting
// ============================================================================

/**
 * Format output for display
 *
 * By default, formats data in a human-readable tree view.
 * With --raw, outputs JSON.
 */
export function formatOutput(data: any, options: OutputOptions = {}): string {
  const { raw = false } = options;

  // Raw mode: always output JSON
  if (raw) {
    return JSON.stringify(data, jsonReplacer, 2);
  }

  // For simple values, just convert to string
  if (typeof data === 'string') {
    return data;
  }

  if (typeof data === 'number' || typeof data === 'bigint') {
    return chalk.yellow(String(data));
  }

  if (typeof data === 'boolean') {
    return data ? chalk.green('true') : chalk.red('false');
  }

  // For null/undefined
  if (data === null || data === undefined) {
    return chalk.dim('(none)');
  }

  // For arrays
  if (Array.isArray(data)) {
    if (data.length === 0) {
      return chalk.dim('(empty)');
    }
    // For simple arrays, join with newlines
    if (data.every(item => typeof item === 'string' || typeof item === 'number')) {
      return data.join('\n');
    }
    // For complex arrays, use tree view
    return formatArrayAsTree(data);
  }

  // For objects, use tree view
  return formatObjectAsTree(data);
}

/**
 * Format an object as a tree
 */
function formatObjectAsTree(obj: any, rootLabel?: string): string {
  const root = tree(rootLabel || '');

  for (const [key, value] of Object.entries(obj)) {
    const formattedKey = chalk.bold(formatKey(key));

    if (value === null || value === undefined) {
      root.push(`${formattedKey}: ${chalk.dim('(none)')}`);
    } else if (typeof value === 'boolean') {
      root.push(`${formattedKey}: ${value ? chalk.green('yes') : chalk.red('no')}`);
    } else if (typeof value === 'number' || typeof value === 'bigint') {
      root.push(`${formattedKey}: ${chalk.yellow(String(value))}`);
    } else if (typeof value === 'string') {
      root.push(`${formattedKey}: ${formatStringValue(value)}`);
    } else if (Array.isArray(value)) {
      if (value.length === 0) {
        root.push(`${formattedKey}: ${chalk.dim('[]')}`);
      } else if (value.length <= 3 && value.every(v => typeof v === 'string' || typeof v === 'number')) {
        root.push(`${formattedKey}: ${value.join(', ')}`);
      } else {
        const arrayNode = tree(`${formattedKey}: ${chalk.dim(`[${value.length} items]`)}`);
        for (const item of value.slice(0, 5)) {
          if (typeof item === 'object' && item !== null) {
            arrayNode.push(formatNestedObject(item));
          } else {
            arrayNode.push(String(item));
          }
        }
        if (value.length > 5) {
          arrayNode.push(chalk.dim(`... and ${value.length - 5} more`));
        }
        root.push(arrayNode);
      }
    } else if (typeof value === 'object') {
      const objNode = tree(formattedKey);
      for (const [k, v] of Object.entries(value)) {
        objNode.push(`${chalk.bold(formatKey(k))}: ${formatSimpleValue(v)}`);
      }
      root.push(objNode);
    }
  }

  // If no root label, return just the children
  if (!rootLabel) {
    return root.children.map(c => c.toString('', true, true)).join('\n');
  }

  return root.toString();
}

/**
 * Format a nested object for tree display
 */
function formatNestedObject(obj: any): TreeNode {
  const firstKey = Object.keys(obj)[0];
  const label = firstKey ? `${chalk.bold(formatKey(firstKey))}: ${formatSimpleValue(obj[firstKey])}` : '{}';
  const node = tree(label);

  let first = true;
  for (const [k, v] of Object.entries(obj)) {
    if (first) {
      first = false;
      continue;
    }
    node.push(`${chalk.bold(formatKey(k))}: ${formatSimpleValue(v)}`);
  }

  return node;
}

/**
 * Format an array as a tree
 */
function formatArrayAsTree(arr: any[]): string {
  const lines: string[] = [];

  for (const item of arr) {
    if (typeof item === 'object' && item !== null) {
      lines.push(formatObjectAsTree(item));
    } else {
      lines.push(String(item));
    }
  }

  return lines.join('\n\n');
}

/**
 * Format a key name (convert camelCase/snake_case to Title Case)
 */
function formatKey(key: string): string {
  return key
    .replace(/([A-Z])/g, ' $1')
    .replace(/_/g, ' ')
    .replace(/^\w/, c => c.toUpperCase())
    .trim();
}

/**
 * Format a string value with appropriate styling
 */
function formatStringValue(value: string): string {
  // Check if it's a hash or long hex string
  if (value.length > 40 && /^[0-9a-fA-F]+$/.test(value)) {
    return chalk.cyan(value);
  }
  // Check if it's a hex value (starts with 0x)
  if (value.startsWith('0x')) {
    return chalk.cyan(value);
  }
  return value;
}

/**
 * Format a simple value
 */
function formatSimpleValue(value: any): string {
  if (value === null || value === undefined) {
    return chalk.dim('(none)');
  }
  if (typeof value === 'boolean') {
    return value ? chalk.green('yes') : chalk.red('no');
  }
  if (typeof value === 'number' || typeof value === 'bigint') {
    return chalk.yellow(String(value));
  }
  if (typeof value === 'string') {
    return formatStringValue(value);
  }
  if (Array.isArray(value)) {
    if (value.length === 0) return chalk.dim('[]');
    if (value.length <= 3 && value.every(v => typeof v === 'string' || typeof v === 'number')) {
      return value.join(', ');
    }
    return chalk.dim(`[${value.length} items]`);
  }
  if (typeof value === 'object') {
    const keys = Object.keys(value);
    if (keys.length === 0) return chalk.dim('{}');
    return chalk.dim(`{${keys.length} fields}`);
  }
  return String(value);
}

// ============================================================================
// Specialized Formatters (Tree-based with emojis)
// ============================================================================

/**
 * Format blockchain info
 */
export function formatBlockchainInfo(info: any): string {
  const root = tree(`${chalk.bold('⛓️  Blockchain Info')}`);

  root.push(`${chalk.bold('Chain:')} ${info.chain}`);
  root.push(`${chalk.bold('Blocks:')} ${chalk.yellow(info.blocks)}`);
  root.push(`${chalk.bold('Headers:')} ${chalk.yellow(info.headers)}`);
  root.push(`${chalk.bold('Best Block Hash:')} ${chalk.cyan(info.bestblockhash)}`);
  root.push(`${chalk.bold('Difficulty:')} ${chalk.yellow(info.difficulty)}`);

  if (info.mediantime) {
    root.push(`${chalk.bold('Median Time:')} ${formatDate(info.mediantime)}`);
  }

  if (info.verificationprogress !== undefined) {
    const progress = (info.verificationprogress * 100).toFixed(2);
    root.push(`${chalk.bold('Verification:')} ${chalk.yellow(progress + '%')}`);
  }

  if (info.initialblockdownload !== undefined) {
    root.push(`${chalk.bold('Initial Download:')} ${info.initialblockdownload ? chalk.yellow('yes') : chalk.green('no')}`);
  }

  if (info.pruned !== undefined) {
    root.push(`${chalk.bold('Pruned:')} ${info.pruned ? chalk.yellow('yes') : chalk.green('no')}`);
  }

  if (info.size_on_disk) {
    root.push(`${chalk.bold('Size on Disk:')} ${formatBytes(info.size_on_disk)}`);
  }

  if (info.warnings) {
    root.push(`${chalk.bold('⚠️  Warnings:')} ${chalk.yellow(info.warnings)}`);
  }

  return root.toString();
}

/**
 * Format block info
 */
export function formatBlockInfo(block: any): string {
  const root = tree(`${chalk.bold('📦 Block')}`);

  if (block.hash) root.push(`${chalk.bold('Hash:')} ${chalk.cyan(block.hash)}`);
  if (block.height !== undefined) root.push(`${chalk.bold('Height:')} ${chalk.yellow(block.height)}`);
  if (block.number !== undefined) {
    const num = typeof block.number === 'string' ? parseInt(block.number, 16) : block.number;
    root.push(`${chalk.bold('Number:')} ${chalk.yellow(num)}`);
  }
  if (block.timestamp) {
    const ts = typeof block.timestamp === 'string' ? parseInt(block.timestamp, 16) : block.timestamp;
    root.push(`${chalk.bold('Timestamp:')} ${formatDate(ts)}`);
  }
  if (block.difficulty) root.push(`${chalk.bold('Difficulty:')} ${block.difficulty}`);
  if (block.nonce) root.push(`${chalk.bold('Nonce:')} ${block.nonce}`);
  if (block.size) root.push(`${chalk.bold('Size:')} ${formatBytes(parseInt(block.size, 16) || block.size)}`);
  if (block.transactions) {
    root.push(`${chalk.bold('Transactions:')} ${chalk.yellow(Array.isArray(block.transactions) ? block.transactions.length : 0)}`);
  }
  if (block.parentHash) root.push(`${chalk.bold('Parent:')} ${chalk.cyan(block.parentHash)}`);

  return root.toString();
}

/**
 * Format alkane balances
 */
export function formatAlkaneBalances(balances: any[]): string {
  if (!balances || balances.length === 0) {
    return chalk.dim('No alkane balances found');
  }

  const root = tree(`${chalk.bold('🪙 Alkane Balances')}`);

  for (const balance of balances) {
    const id = balance.alkane_id
      ? `${balance.alkane_id.block}:${balance.alkane_id.tx}`
      : `${balance.block}:${balance.tx}`;

    const balanceNode = tree(`${chalk.bold('ID:')} ${chalk.cyan(id)}`);

    if (balance.name) balanceNode.push(`${chalk.bold('Name:')} ${balance.name}`);
    if (balance.symbol) balanceNode.push(`${chalk.bold('Symbol:')} ${balance.symbol}`);
    balanceNode.push(`${chalk.bold('Balance:')} ${chalk.yellow(balance.balance || balance.value || '0')}`);

    root.push(balanceNode);
  }

  return root.toString();
}

/**
 * Format inscription list
 */
export function formatInscriptions(inscriptions: any): string {
  const ids = inscriptions.ids || inscriptions;

  if (!ids || ids.length === 0) {
    return chalk.dim('No inscriptions found');
  }

  const root = tree(`${chalk.bold('📜 Inscriptions')} ${chalk.dim(`(${ids.length} total)`)}`);

  // Show first 10
  for (const id of ids.slice(0, 10)) {
    root.push(chalk.cyan(id));
  }

  if (ids.length > 10) {
    root.push(chalk.dim(`... and ${ids.length - 10} more`));
  }

  if (inscriptions.more) {
    root.push(chalk.dim('(more available)'));
  }

  return root.toString();
}

/**
 * Format fee estimates
 */
export function formatFeeEstimates(estimates: any): string {
  const root = tree(`${chalk.bold('💰 Fee Estimates')} ${chalk.dim('(sat/vB)')}`);

  const blocks = Object.keys(estimates).map(Number).sort((a, b) => a - b);

  for (const block of blocks) {
    const fee = estimates[block];
    const label = block === 1 ? 'Next block' :
                  block <= 6 ? `~${block * 10} min` :
                  block <= 144 ? `~${Math.round(block / 6)} hours` :
                  `~${Math.round(block / 144)} days`;
    root.push(`${chalk.bold(block.toString().padStart(4))} blocks (${label}): ${chalk.yellow(fee.toFixed(3))}`);
  }

  return root.toString();
}

/**
 * Format reflect metadata
 */
export function formatReflectMetadata(metadata: any): string {
  const root = tree(`${chalk.bold('🔍 Alkane Metadata')}`);

  if (metadata.id) root.push(`${chalk.bold('ID:')} ${chalk.cyan(metadata.id)}`);
  if (metadata.name) root.push(`${chalk.bold('Name:')} ${metadata.name}`);
  if (metadata.symbol) root.push(`${chalk.bold('Symbol:')} ${metadata.symbol}`);
  if (metadata.total_supply !== undefined) root.push(`${chalk.bold('Total Supply:')} ${chalk.yellow(metadata.total_supply)}`);
  if (metadata.cap !== undefined) root.push(`${chalk.bold('Cap:')} ${chalk.yellow(metadata.cap)}`);
  if (metadata.minted !== undefined) root.push(`${chalk.bold('Minted:')} ${chalk.yellow(metadata.minted)}`);
  if (metadata.value_per_mint !== undefined) root.push(`${chalk.bold('Value Per Mint:')} ${chalk.yellow(metadata.value_per_mint)}`);
  if (metadata.premine !== undefined) root.push(`${chalk.bold('Premine:')} ${chalk.yellow(metadata.premine)}`);
  if (metadata.decimals !== undefined) root.push(`${chalk.bold('Decimals:')} ${chalk.yellow(metadata.decimals)}`);
  if (metadata.data) root.push(`${chalk.bold('Data:')} ${metadata.data}`);

  return root.toString();
}

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Print success message
 */
export function success(message: string): void {
  console.log(chalk.green('✓ ') + message);
}

/**
 * Print error message
 */
export function error(message: string): void {
  console.error(chalk.red('✗ ') + message);
}

/**
 * Print warning message
 */
export function warn(message: string): void {
  console.warn(chalk.yellow('⚠ ') + message);
}

/**
 * Print info message
 */
export function info(message: string): void {
  console.log(chalk.blue('ℹ ') + message);
}

/**
 * Create a table for displaying data
 */
export function createTable(headers: string[]): Table.Table {
  return new Table({
    head: headers.map(h => chalk.cyan(h)),
    style: {
      head: [],
      border: [],
    },
  });
}

/**
 * Format an address for display (truncate middle)
 */
export function formatAddress(address: string, maxLength: number = 20): string {
  if (address.length <= maxLength) {
    return address;
  }

  const start = Math.floor((maxLength - 3) / 2);
  const end = Math.ceil((maxLength - 3) / 2);

  return `${address.slice(0, start)}...${address.slice(-end)}`;
}

/**
 * Format a transaction ID for display (truncate middle)
 */
export function formatTxid(txid: string, maxLength: number = 20): string {
  return formatAddress(txid, maxLength);
}

/**
 * Format satoshis to BTC
 */
export function formatBTC(satoshis: number | bigint): string {
  const btc = Number(satoshis) / 100_000_000;
  return `${btc.toFixed(8)} BTC`;
}

/**
 * Format a large number with commas
 */
export function formatNumber(num: number | bigint): string {
  return num.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

/**
 * Format bytes to human readable
 */
export function formatBytes(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let unitIndex = 0;
  let size = bytes;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }

  return `${size.toFixed(2)} ${units[unitIndex]}`;
}

/**
 * Format a timestamp to a readable date
 */
export function formatDate(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleString();
}

/**
 * Print a horizontal rule
 */
export function printRule(): void {
  console.log(chalk.gray('─'.repeat(80)));
}

/**
 * Print a header
 */
export function printHeader(text: string): void {
  console.log();
  console.log(chalk.bold.cyan(text));
  printRule();
}
