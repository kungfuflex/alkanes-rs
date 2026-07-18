#!/usr/bin/env node

/**
 * Alkanes CLI - Node.js CLI with feature parity to alkanes-cli (Rust)
 *
 * This CLI leverages the shared business logic from alkanes-cli-common
 * through WASM bindings in alkanes-web-sys.
 */

import { Command } from 'commander';
import chalk from 'chalk';
import { error as logError, setVerbosity, getVerbosity } from './utils/formatting.js';

// Store original console methods for verbose filtering
const originalConsoleLog = console.log;
const originalConsoleInfo = console.info;

// Filter console output based on verbosity level
// WASM logging uses console.log with [INFO] prefix
function setupLogging(verbose: number): void {
  setVerbosity(verbose);

  // Replace console.log to filter WASM debug output
  console.log = (...args: any[]) => {
    const message = args.map(a => String(a)).join(' ');

    // WASM RPC debug logs have specific patterns
    if (message.includes('[INFO]') ||
        message.includes('JsonRpcProvider::call') ||
        message.includes('Raw RPC response')) {
      // Only show if verbosity >= 3 (-vvv)
      if (verbose >= 3) {
        originalConsoleLog.apply(console, [chalk.dim(...args)]);
      }
      return;
    }

    // Pass through all other logs
    originalConsoleLog.apply(console, args);
  };

  console.info = (...args: any[]) => {
    const message = args.map(a => String(a)).join(' ');

    if (message.includes('[INFO]')) {
      if (verbose >= 2) {
        originalConsoleInfo.apply(console, [chalk.dim(...args)]);
      }
      return;
    }

    originalConsoleInfo.apply(console, args);
  };
}

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
  .option('-v, --verbose', 'Verbose output (-v, -vv, -vvv for increasing levels)', (_, prev) => prev + 1, 0)
  .option('-y, --auto-confirm', 'Skip confirmation prompts');

// Import and register all command groups
// We'll implement these incrementally

// Import the command modules we have implemented
import { registerWalletCommands } from './commands/wallet.js';
import { registerBitcoindCommands } from './commands/bitcoind.js';
import { registerAlkanesCommands } from './commands/alkanes.js';
import { registerEsploraCommands } from './commands/esplora.js';
import { registerOrdCommands } from './commands/ord.js';
import { registerRunestoneCommands } from './commands/runestone.js';
import { registerProtorunesCommands } from './commands/protorunes.js';
import { registerMetashrewCommands } from './commands/metashrew.js';
import { registerLuaCommands } from './commands/lua.js';
import { registerDataapiCommands } from './commands/dataapi.js';
import { registerEspoCommands } from './commands/espo.js';
import { registerBrc20ProgCommands } from './commands/brc20prog.js';
import { registerOpiCommands } from './commands/opi.js';
import { registerSubfrostCommands } from './commands/subfrost.js';

// Register implemented command groups
registerWalletCommands(program);
registerBitcoindCommands(program);
registerAlkanesCommands(program);
registerEsploraCommands(program);
registerOrdCommands(program);
registerRunestoneCommands(program);
registerProtorunesCommands(program);
registerMetashrewCommands(program);
registerLuaCommands(program);
registerDataapiCommands(program);
registerEspoCommands(program);
registerBrc20ProgCommands(program);
registerOpiCommands(program);
registerSubfrostCommands(program);

// Standalone command: decodepsbt
program
  .command('decodepsbt <psbt>')
  .description('Decode a PSBT (Partially Signed Bitcoin Transaction) without calling bitcoind')
  .action(async (psbt, options, command) => {
    try {
      const { decode_psbt } = await import('../wasm/alkanes_web_sys.js');
      const globalOpts = command.parent?.opts() || {};

      const result = decode_psbt(psbt);
      const decoded = JSON.parse(result);

      const { formatOutput } = await import('./utils/formatting.js');
      console.log(formatOutput(decoded, globalOpts));
    } catch (err: any) {
      const { error } = await import('./utils/formatting.js');
      error(`Failed to decode PSBT: ${err.message}`);
      process.exit(1);
    }
  });

// TODO: Additional commands that need WASM bindings:
// - Esplora: ~20 more commands (block operations, mempool, etc.)
// - Ord: ~8 more commands (address-info, block-info, children, content, etc.)
// - Dataapi: A few more specialized endpoints
//
// Note: Some commands within implemented groups may require additional WASM bindings:
// - Alkanes: execute, wrap-btc, init-pool, swap (transaction building commands)
// - OPI: Most commands require direct HTTP endpoint access (not available in WASM)

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

// Pre-parse to get verbosity level before command execution
// This is needed to setup logging before any WASM calls
const preParseOpts = program.opts();
const verboseCount = process.argv.filter(arg => arg === '-v' || arg === '--verbose').length +
  (process.argv.filter(arg => arg.match(/^-v+$/)).reduce((acc, arg) => acc + arg.length - 1, 0));
setupLogging(verboseCount);

// Parse command line arguments
program.parse(process.argv);

// If no command was provided, show help
if (!process.argv.slice(2).length) {
  program.outputHelp();
}
