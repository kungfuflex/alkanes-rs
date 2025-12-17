/**
 * Ord command group
 * Ordinals and Inscriptions operations
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error } from '../utils/formatting.js';
import ora from 'ora';

export function registerOrdCommands(program: Command): void {
  const ord = program.command('ord').description('Ordinals and Inscriptions operations');

  // inscription
  ord
    .command('inscription <id>')
    .description('Get inscription by ID')
    .action(async (id, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting inscription...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord_inscription_js(id);
        const inscription = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(inscription, globalOpts));
      } catch (err: any) {
        error(`Failed to get inscription: ${err.message}`);
        process.exit(1);
      }
    });

  // inscriptions
  ord
    .command('inscriptions')
    .description('List inscriptions')
    .option('--page <number>', 'Page number', '0')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting inscriptions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const page = options.page ? parseFloat(options.page) : null;
        const result = await provider.ord_inscriptions_js(page);
        const inscriptions = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(inscriptions, globalOpts));
      } catch (err: any) {
        error(`Failed to get inscriptions: ${err.message}`);
        process.exit(1);
      }
    });

  // outputs
  ord
    .command('outputs <address>')
    .description('Get ordinal outputs for an address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting outputs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord_outputs_js(address);
        const outputs = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(outputs, globalOpts));
      } catch (err: any) {
        error(`Failed to get outputs: ${err.message}`);
        process.exit(1);
      }
    });

  // rune
  ord
    .command('rune <name>')
    .description('Get rune information')
    .action(async (name, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting rune...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord_rune_js(name);
        const rune = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(rune, globalOpts));
      } catch (err: any) {
        error(`Failed to get rune: ${err.message}`);
        process.exit(1);
      }
    });

  // list
  ord
    .command('list <outpoint>')
    .description('List ordinals in an output')
    .action(async (outpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Listing ordinals...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord_list_js(outpoint);
        const list = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(list, globalOpts));
      } catch (err: any) {
        error(`Failed to list ordinals: ${err.message}`);
        process.exit(1);
      }
    });

  // find
  ord
    .command('find <sat>')
    .description('Find ordinal by satoshi number')
    .action(async (sat, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Finding ordinal...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord_find_js(parseFloat(sat));
        const location = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(location, globalOpts));
      } catch (err: any) {
        error(`Failed to find ordinal: ${err.message}`);
        process.exit(1);
      }
    });
}
