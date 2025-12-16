/**
 * Bitcoind command group
 * Bitcoin Core RPC operations
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error } from '../utils/formatting.js';
import ora from 'ora';

export function registerBitcoindCommands(program: Command): void {
  const bitcoind = program.command('bitcoind').description('Bitcoin Core RPC commands');

  // getblockcount
  bitcoind
    .command('getblockcount')
    .description('Get current block count')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block count...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoin_getblockcount_js();
        const blockCount = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(blockCount, globalOpts));
      } catch (err: any) {
        error(`Failed to get block count: ${err.message}`);
        process.exit(1);
      }
    });

  // generatetoaddress
  bitcoind
    .command('generatetoaddress <nblocks> <address>')
    .description('Generate blocks to an address (regtest only)')
    .action(async (nblocks, address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora(`Generating ${nblocks} blocks...`).start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoin_generatetoaddress_js(parseInt(nblocks), address);
        const hashes = JSON.parse(result);

        spinner.succeed(`Generated ${nblocks} blocks`);
        console.log(formatOutput(hashes, globalOpts));
      } catch (err: any) {
        error(`Failed to generate blocks: ${err.message}`);
        process.exit(1);
      }
    });

  // getblockchaininfo
  bitcoind
    .command('getblockchaininfo')
    .description('Get blockchain information')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting blockchain info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoin_getblockchaininfo_js();
        const info = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(info, globalOpts));
      } catch (err: any) {
        error(`Failed to get blockchain info: ${err.message}`);
        process.exit(1);
      }
    });

  // getrawtransaction
  bitcoind
    .command('getrawtransaction <txid>')
    .description('Get raw transaction')
    .option('--verbose', 'Return decoded transaction')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoin_getrawtransaction_js(txid, options.verbose || false);
        const tx = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(tx, globalOpts));
      } catch (err: any) {
        error(`Failed to get transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // getblock
  bitcoind
    .command('getblock <hash>')
    .description('Get block by hash')
    .option('--verbosity <level>', 'Verbosity level (0-2)', '1')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoin_getblock_js(hash, parseInt(options.verbosity));
        const block = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(block, globalOpts));
      } catch (err: any) {
        error(`Failed to get block: ${err.message}`);
        process.exit(1);
      }
    });

  // getblockhash
  bitcoind
    .command('getblockhash <height>')
    .description('Get block hash by height')
    .action(async (height, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoin_getblockhash_js(parseInt(height));
        const hash = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(hash, globalOpts));
      } catch (err: any) {
        error(`Failed to get block hash: ${err.message}`);
        process.exit(1);
      }
    });

  // sendrawtransaction
  bitcoind
    .command('sendrawtransaction <hex>')
    .description('Broadcast a raw transaction')
    .action(async (hex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Broadcasting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoin_sendrawtransaction_js(hex);
        const txid = JSON.parse(result);

        spinner.succeed('Transaction broadcast');
        success(`TXID: ${txid}`);
      } catch (err: any) {
        error(`Failed to broadcast transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // getnetworkinfo
  bitcoind
    .command('getnetworkinfo')
    .description('Get network information')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting network info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoin_getnetworkinfo_js();
        const info = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(info, globalOpts));
      } catch (err: any) {
        error(`Failed to get network info: ${err.message}`);
        process.exit(1);
      }
    });

  // getmempoolinfo
  bitcoind
    .command('getmempoolinfo')
    .description('Get mempool information')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting mempool info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoin_getmempoolinfo_js();
        const info = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(info, globalOpts));
      } catch (err: any) {
        error(`Failed to get mempool info: ${err.message}`);
        process.exit(1);
      }
    });

  // generatefuture
  bitcoind
    .command('generatefuture <address>')
    .description('Generate a future block (regtest only)')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Generating future block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoind_generate_future_js(address);
        const hash = JSON.parse(result);

        spinner.succeed('Future block generated');
        console.log(formatOutput(hash, globalOpts));
      } catch (err: any) {
        error(`Failed to generate future block: ${err.message}`);
        process.exit(1);
      }
    });

  // getblockheader
  bitcoind
    .command('getblockheader <hash>')
    .description('Get block header by hash')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block header...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoind_get_block_header_js(hash);
        const header = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(header, globalOpts));
      } catch (err: any) {
        error(`Failed to get block header: ${err.message}`);
        process.exit(1);
      }
    });

  // getblockstats
  bitcoind
    .command('getblockstats <hash>')
    .description('Get block statistics by hash')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block stats...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoind_get_block_stats_js(hash);
        const stats = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(stats, globalOpts));
      } catch (err: any) {
        error(`Failed to get block stats: ${err.message}`);
        process.exit(1);
      }
    });

  // estimatesmartfee
  bitcoind
    .command('estimatesmartfee <blocks>')
    .description('Estimate smart fee for confirmation in N blocks')
    .action(async (blocks, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Estimating fee...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoind_estimate_smart_fee_js(parseInt(blocks));
        const estimate = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(estimate, globalOpts));
      } catch (err: any) {
        error(`Failed to estimate fee: ${err.message}`);
        process.exit(1);
      }
    });

  // getchaintips
  bitcoind
    .command('getchaintips')
    .description('Get chain tips information')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting chain tips...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.bitcoind_get_chain_tips_js();
        const tips = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(tips, globalOpts));
      } catch (err: any) {
        error(`Failed to get chain tips: ${err.message}`);
        process.exit(1);
      }
    });

  // Note: The following commands are in the Rust CLI but not yet in WASM bindings:
  // - decoderawtransaction
  // - decodepsbt
  // - getrawmempool
  // - gettxout
}
