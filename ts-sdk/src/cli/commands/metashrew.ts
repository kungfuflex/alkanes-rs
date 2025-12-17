/**
 * Metashrew command group
 * Metashrew RPC operations
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
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.metashrew_height_js();
        const height = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(height, globalOpts));
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
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting state root...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const height = options.height ? parseFloat(options.height) : null;
        const result = await provider.metashrew_state_root_js(height);
        const stateRoot = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(stateRoot, globalOpts));
      } catch (err: any) {
        error(`Failed to get state root: ${err.message}`);
        process.exit(1);
      }
    });

  // getblockhash
  metashrew
    .command('getblockhash <height>')
    .description('Get block hash at height')
    .action(async (height, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.metashrew_get_block_hash_js(parseFloat(height));
        const hash = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(hash, globalOpts));
      } catch (err: any) {
        error(`Failed to get block hash: ${err.message}`);
        process.exit(1);
      }
    });

  // view
  metashrew
    .command('view <function> <payload> <block-tag>')
    .description('Call metashrew view function')
    .action(async (fn, payload, blockTag, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Calling view function...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.metashrew_view_js(fn, payload, blockTag);
        const view = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(view, globalOpts));
      } catch (err: any) {
        error(`Failed to call view function: ${err.message}`);
        process.exit(1);
      }
    });
}
