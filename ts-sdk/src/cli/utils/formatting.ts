/**
 * Output formatting utilities for the CLI
 */

import chalk from 'chalk';
import Table from 'cli-table3';

export interface OutputOptions {
  raw?: boolean;  // Output raw JSON
  color?: boolean;  // Use colors (default: true)
}

/**
 * Format output for display
 */
export function formatOutput(data: any, options: OutputOptions = {}): string {
  const { raw = false, color = true } = options;

  if (raw) {
    return JSON.stringify(data, null, 2);
  }

  // For simple values, just convert to string
  if (typeof data === 'string' || typeof data === 'number' || typeof data === 'boolean') {
    return String(data);
  }

  // For objects and arrays, use JSON with optional coloring
  return JSON.stringify(data, null, 2);
}

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
      head: [],  // Don't apply default styling to headers
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
