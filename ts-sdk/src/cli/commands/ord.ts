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

  // address-info
  ord
    .command('address-info <address>')
    .description('Get address information')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ordAddressInfo(address);
        const addressInfo = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(addressInfo, globalOpts));
      } catch (err: any) {
        error(`Failed to get address info: ${err.message}`);
        process.exit(1);
      }
    });

  // block-info
  ord
    .command('block-info <query>')
    .description('Get block information (height or hash)')
    .action(async (query, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ordBlockInfo(query);
        const blockInfo = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(blockInfo, globalOpts));
      } catch (err: any) {
        error(`Failed to get block info: ${err.message}`);
        process.exit(1);
      }
    });

  // block-count
  ord
    .command('block-count')
    .description('Get latest block count')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block count...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ordBlockCount();
        const blockCount = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(blockCount, globalOpts));
      } catch (err: any) {
        error(`Failed to get block count: ${err.message}`);
        process.exit(1);
      }
    });

  // blocks
  ord
    .command('blocks')
    .description('Get latest blocks')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting blocks...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ordBlocks();
        const blocks = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(blocks, globalOpts));
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
    .action(async (inscriptionId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting children...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const page = options.page ? parseFloat(options.page) : null;
        const result = await provider.ordChildren(inscriptionId, page);
        const children = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(children, globalOpts));
      } catch (err: any) {
        error(`Failed to get children: ${err.message}`);
        process.exit(1);
      }
    });

  // content
  ord
    .command('content <inscription-id>')
    .description('Get inscription content')
    .action(async (inscriptionId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting content...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ordContent(inscriptionId);
        const content = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(content, globalOpts));
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
    .action(async (inscriptionId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting parents...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const page = options.page ? parseFloat(options.page) : null;
        const result = await provider.ordParents(inscriptionId, page);
        const parents = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(parents, globalOpts));
      } catch (err: any) {
        error(`Failed to get parents: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-info
  ord
    .command('tx-info <txid>')
    .description('Get transaction information')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.ordTxInfo(txid);
        const txInfo = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(txInfo, globalOpts));
      } catch (err: any) {
        error(`Failed to get transaction info: ${err.message}`);
        process.exit(1);
      }
    });
}
