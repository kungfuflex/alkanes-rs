/**
 * Subfrost command group
 * frBTC unwrap utilities
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error } from '../utils/formatting.js';
import ora from 'ora';

export function registerSubfrostCommands(program: Command): void {
  const subfrost = program.command('subfrost').description('Subfrost operations (frBTC unwrap utilities)');

  // minimum-unwrap
  subfrost
    .command('minimum-unwrap')
    .description('Calculate minimum unwrap amount based on current fee rates')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};

        error('Subfrost minimum-unwrap command is not yet implemented.');
        error('This requires WASM bindings for subfrost minimum unwrap calculation.');
        error('For now, please use the Rust alkanes-cli for Subfrost operations.');
        process.exit(1);
      } catch (err: any) {
        error(`Failed to calculate minimum unwrap: ${err.message}`);
        process.exit(1);
      }
    });
}
