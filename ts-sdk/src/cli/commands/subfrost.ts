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
    .option('--fee-rate <rate>', 'Fee rate override in sat/vB (otherwise fetches from network)')
    .option('--premium <percent>', 'Premium percentage (default: 0.1)', '0.1')
    .option('--expected-inputs <n>', 'Expected number of inputs', '10')
    .option('--expected-outputs <n>', 'Expected number of outputs', '10')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Calculating minimum unwrap amount...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          esploraUrl: globalOpts.esploraUrl,
        });

        const result = await provider.subfrostMinimumUnwrap(
          options.feeRate ? parseFloat(options.feeRate) : null,
          parseFloat(options.premium) / 100, // Convert percentage to decimal
          options.expectedInputs ? parseFloat(options.expectedInputs) : null,
          options.expectedOutputs ? parseFloat(options.expectedOutputs) : null,
          globalOpts.raw || false
        );

        spinner.succeed();

        if (globalOpts.raw) {
          const parsed = JSON.parse(result);
          console.log(formatOutput(parsed, globalOpts));
        } else {
          // Result is already formatted as a nice table/box
          console.log(result);
        }
      } catch (err: any) {
        error(`Failed to calculate minimum unwrap: ${err.message}`);
        process.exit(1);
      }
    });
}
