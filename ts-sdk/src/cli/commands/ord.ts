/**
 * Ord command group
 * Ordinals and Inscriptions operations
 *
 * The CLI uses the SDK's OrdClient via provider.ord for all operations.
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import {
  formatOutput,
  formatInscriptions,
  formatBlockInfo,
  success,
  error,
} from '../utils/formatting.js';
import ora from 'ora';

export function registerOrdCommands(program: Command): void {
  const ord = program.command('ord').description('Ordinals and Inscriptions operations');

  // inscription
  ord
    .command('inscription <id>')
    .description('Get inscription by ID')
    .option('--raw', 'Output raw JSON')
    .action(async (id, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting inscription...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getInscription(id);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
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
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting inscriptions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getInscriptions(parseInt(options.page));

        spinner.succeed();
        if (options.raw) {
          console.log(formatOutput(result, { raw: true }));
        } else {
          console.log(formatInscriptions(result));
        }
      } catch (err: any) {
        error(`Failed to get inscriptions: ${err.message}`);
        process.exit(1);
      }
    });

  // outputs
  ord
    .command('outputs <address>')
    .description('Get ordinal outputs for an address')
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting outputs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getOutputs(address);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get outputs: ${err.message}`);
        process.exit(1);
      }
    });

  // rune
  ord
    .command('rune <name>')
    .description('Get rune information')
    .option('--raw', 'Output raw JSON')
    .action(async (name, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting rune...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getRune(name);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get rune: ${err.message}`);
        process.exit(1);
      }
    });

  // list
  ord
    .command('list <outpoint>')
    .description('List ordinals in an output')
    .option('--raw', 'Output raw JSON')
    .action(async (outpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Listing ordinals...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.list(outpoint);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to list ordinals: ${err.message}`);
        process.exit(1);
      }
    });

  // find
  ord
    .command('find <sat>')
    .description('Find ordinal by satoshi number')
    .option('--raw', 'Output raw JSON')
    .action(async (sat, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Finding ordinal...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.find(parseInt(sat));

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to find ordinal: ${err.message}`);
        process.exit(1);
      }
    });

  // address-info
  ord
    .command('address-info <address>')
    .description('Get address information')
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getAddressInfo(address);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get address info: ${err.message}`);
        process.exit(1);
      }
    });

  // block-info
  ord
    .command('block-info <query>')
    .description('Get block information (height or hash)')
    .option('--raw', 'Output raw JSON')
    .action(async (query, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getBlockInfo(query);

        spinner.succeed();
        if (options.raw) {
          console.log(formatOutput(result, { raw: true }));
        } else {
          console.log(formatBlockInfo(result));
        }
      } catch (err: any) {
        error(`Failed to get block info: ${err.message}`);
        process.exit(1);
      }
    });

  // block-count
  ord
    .command('block-count')
    .description('Get latest block count')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block count...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getBlockCount();

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block count: ${err.message}`);
        process.exit(1);
      }
    });

  // blocks
  ord
    .command('blocks')
    .description('Get latest blocks')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting blocks...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getBlocks();

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get blocks: ${err.message}`);
        process.exit(1);
      }
    });

  // children
  ord
    .command('children <inscription-id>')
    .description('Get children of an inscription')
    .option('--page <number>', 'Page number', '0')
    .option('--raw', 'Output raw JSON')
    .action(async (inscriptionId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting children...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getChildren(inscriptionId, parseInt(options.page));

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get children: ${err.message}`);
        process.exit(1);
      }
    });

  // content
  ord
    .command('content <inscription-id>')
    .description('Get inscription content')
    .option('--raw', 'Output raw JSON')
    .action(async (inscriptionId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting content...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getContent(inscriptionId);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get content: ${err.message}`);
        process.exit(1);
      }
    });

  // parents
  ord
    .command('parents <inscription-id>')
    .description('Get parents of an inscription')
    .option('--page <number>', 'Page number', '0')
    .option('--raw', 'Output raw JSON')
    .action(async (inscriptionId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting parents...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getParents(inscriptionId, parseInt(options.page));

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get parents: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-info
  ord
    .command('tx-info <txid>')
    .description('Get transaction information')
    .option('--raw', 'Output raw JSON')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ord.getTxInfo(txid);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get transaction info: ${err.message}`);
        process.exit(1);
      }
    });
}
