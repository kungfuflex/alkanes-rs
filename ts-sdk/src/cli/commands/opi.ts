/**
 * OPI command group
 * Open Protocol Indexer (BRC-20, Runes, Bitmap, etc.)
 *
 * Note: Many OPI commands make direct HTTP requests to OPI endpoints
 * and may not have dedicated WASM bindings. This is a placeholder
 * implementation for the command structure.
 */

import { Command } from 'commander';
import { error } from '../utils/formatting.js';

export function registerOpiCommands(program: Command): void {
  const opi = program.command('opi').description('Open Protocol Indexer operations');

  // Placeholder message for unimplemented OPI commands
  const notImplemented = () => {
    error('OPI commands require direct HTTP endpoint access.');
    error('These will be implemented in a future version.');
    error('For now, please use the Rust alkanes-cli for OPI operations.');
    process.exit(1);
  };

  // BRC-20 commands
  opi.command('block-height').description('Get current indexed block height (BRC-20)').action(notImplemented);
  opi.command('extras-block-height').description('Get extras indexed block height (BRC-20)').action(notImplemented);
  opi.command('db-version').description('Get database version (BRC-20)').action(notImplemented);
  opi.command('current-balance <wallet>').description('Get current balance (BRC-20)').action(notImplemented);
  opi.command('holders <ticker>').description('Get holders of a BRC-20 ticker').action(notImplemented);

  // Subcommand groups
  const runes = opi.command('runes').description('Runes indexer subcommands');
  runes.command('placeholder').description('Runes commands placeholder').action(notImplemented);

  const bitmap = opi.command('bitmap').description('Bitmap indexer subcommands');
  bitmap.command('placeholder').description('Bitmap commands placeholder').action(notImplemented);

  const pow20 = opi.command('pow20').description('POW20 indexer subcommands');
  pow20.command('placeholder').description('POW20 commands placeholder').action(notImplemented);

  const sns = opi.command('sns').description('SNS (Sats Names Service) indexer subcommands');
  sns.command('placeholder').description('SNS commands placeholder').action(notImplemented);
}
