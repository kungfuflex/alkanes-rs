#!/usr/bin/env node

/**
 * Alkanes CLI - Node.js CLI with feature parity to alkanes-cli (Rust)
 *
 * This CLI leverages the shared business logic from alkanes-cli-common
 * through WASM bindings in alkanes-web-sys.
 */

import { Command } from 'commander';
import chalk from 'chalk';
import { error as logError } from './utils/formatting.js';

const program = new Command();

// Configure the main program
program
  .name('alkanes-bindgen-cli')
  .version('0.1.0')
  .description('Alkanes Bindgen CLI - Bitcoin smart contracts (WASM/TypeScript version)')
  .option('-p, --provider <network>', 'Network: mainnet/testnet/signet/regtest', 'mainnet')
  .option('--wallet-file <path>', 'Wallet file path', '~/.alkanes/wallet.json')
  .option('--passphrase <password>', 'Wallet passphrase')
  .option('--jsonrpc-url <url>', 'JSON-RPC URL')
  .option('--esplora-url <url>', 'Esplora API URL')
  .option('--metashrew-url <url>', 'Metashrew RPC URL')
  .option('--raw', 'Output raw JSON')
  .option('-y, --auto-confirm', 'Skip confirmation prompts');

// Import and register all command groups
// We'll implement these incrementally

// Import the command modules we have implemented
import { registerWalletCommands } from './commands/wallet.js';
import { registerBitcoindCommands } from './commands/bitcoind.js';

// Register implemented command groups
registerWalletCommands(program);
registerBitcoindCommands(program);

// TODO: Implement remaining command groups:
// - Alkanes
// - Esplora
// - Ord
// - Runestone
// - Protorunes
// - Metashrew
// - Lua
// - Dataapi
// - OPI
// - Subfrost
// - ESPO
// - BRC20-Prog

// Global error handler
process.on('unhandledRejection', (reason, promise) => {
  logError(`Unhandled rejection at: ${promise}, reason: ${reason}`);
  process.exit(1);
});

process.on('uncaughtException', (error) => {
  logError(`Uncaught exception: ${error.message}`);
  if (error.stack) {
    console.error(error.stack);
  }
  process.exit(1);
});

// Parse command line arguments
program.parse(process.argv);

// If no command was provided, show help
if (!process.argv.slice(2).length) {
  program.outputHelp();
}
