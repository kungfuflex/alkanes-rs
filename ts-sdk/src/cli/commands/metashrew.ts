/**
 * Metashrew command group
 * Metashrew RPC operations
 *
 * The CLI uses the SDK's MetashrewClient via provider.metashrew for all operations.
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error } from '../utils/formatting.js';
import ora from 'ora';

export function registerMetashrewCommands(program: Command): void {
  const metashrew = program.command('metashrew').description('Metashrew RPC operations');

  // height
  metashrew
    .command('height')
    .description('Get current metashrew height')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const height = await provider.metashrew.getHeight();

        spinner.succeed();
        console.log(formatOutput(height, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get height: ${err.message}`);
        process.exit(1);
      }
    });

  // state-root
  metashrew
    .command('state-root')
    .description('Get state root at height')
    .option('--height <number>', 'Block height')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting state root...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const height = options.height ? parseInt(options.height) : undefined;
        const stateRoot = await provider.metashrew.getStateRoot(height);

        spinner.succeed();
        console.log(formatOutput(stateRoot, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get state root: ${err.message}`);
        process.exit(1);
      }
    });

  // getblockhash
  metashrew
    .command('getblockhash <height>')
    .description('Get block hash at height')
    .option('--raw', 'Output raw JSON')
    .action(async (height, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const hash = await provider.metashrew.getBlockHash(parseInt(height));

        spinner.succeed();
        console.log(formatOutput(hash, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block hash: ${err.message}`);
        process.exit(1);
      }
    });

  // view
  metashrew
    .command('view <function> <payload> <block-tag>')
    .description('Call metashrew view function')
    .option('--raw', 'Output raw JSON')
    .action(async (fn, payload, blockTag, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Calling view function...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.metashrew.view(fn, payload, blockTag);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to call view function: ${err.message}`);
        process.exit(1);
      }
    });
}
