/**
 * ESPO command group
 * ESPO balance indexer operations
 *
 * The CLI uses the SDK's EspoClient via provider.espo for all operations.
 * This ensures the CLI and SDK share the same typed interface.
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error } from '../utils/formatting.js';
import ora from 'ora';

export function registerEspoCommands(program: Command): void {
  const espo = program.command('espo').description('ESPO balance indexer operations');

  // height
  espo
    .command('height')
    .description('Get current ESPO height')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const height = await provider.espo.getHeight();

        spinner.succeed();
        console.log(formatOutput(height, globalOpts));
      } catch (err: any) {
        error(`Failed to get height: ${err.message}`);
        process.exit(1);
      }
    });

  // ping
  espo
    .command('ping')
    .description('Ping ESPO service')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Pinging...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const pong = await provider.espo.ping();

        spinner.succeed();
        console.log(formatOutput(pong, globalOpts));
      } catch (err: any) {
        error(`Failed to ping: ${err.message}`);
        process.exit(1);
      }
    });

  // address-balances
  espo
    .command('address-balances <address>')
    .description('Get balances for an address')
    .option('--include-outpoints', 'Include outpoint details', false)
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting balances...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const balances = await provider.espo.getAddressBalances(
          address,
          options.includeOutpoints
        );

        spinner.succeed();
        console.log(formatOutput(balances, globalOpts));
      } catch (err: any) {
        error(`Failed to get balances: ${err.message}`);
        process.exit(1);
      }
    });

  // address-outpoints
  espo
    .command('address-outpoints <address>')
    .description('Get outpoints for an address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting outpoints...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const outpoints = await provider.espo.getAddressOutpoints(address);

        spinner.succeed();
        console.log(formatOutput(outpoints, globalOpts));
      } catch (err: any) {
        error(`Failed to get outpoints: ${err.message}`);
        process.exit(1);
      }
    });

  // outpoint-balances
  espo
    .command('outpoint-balances <outpoint>')
    .description('Get balances for an outpoint')
    .action(async (outpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting balances...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const balances = await provider.espo.getOutpointBalances(outpoint);

        spinner.succeed();
        console.log(formatOutput(balances, globalOpts));
      } catch (err: any) {
        error(`Failed to get balances: ${err.message}`);
        process.exit(1);
      }
    });

  // holders
  espo
    .command('holders <alkane-id>')
    .description('Get holders for an alkane')
    .option('--page <page>', 'Page number', '0')
    .option('--limit <limit>', 'Limit results', '100')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting holders...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const holders = await provider.espo.getHolders(
          alkaneId,
          parseInt(options.page, 10),
          parseInt(options.limit, 10)
        );

        spinner.succeed();
        console.log(formatOutput(holders, globalOpts));
      } catch (err: any) {
        error(`Failed to get holders: ${err.message}`);
        process.exit(1);
      }
    });

  // holders-count
  espo
    .command('holders-count <alkane-id>')
    .description('Get holder count for an alkane')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting holder count...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const count = await provider.espo.getHoldersCount(alkaneId);

        spinner.succeed();
        console.log(formatOutput({ count }, globalOpts));
      } catch (err: any) {
        error(`Failed to get holder count: ${err.message}`);
        process.exit(1);
      }
    });

  // keys
  espo
    .command('keys <alkane-id>')
    .description('Get storage keys for an alkane')
    .option('--page <page>', 'Page number', '0')
    .option('--limit <limit>', 'Limit results', '100')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting keys...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const keys = await provider.espo.getKeys(
          alkaneId,
          parseInt(options.page, 10),
          parseInt(options.limit, 10)
        );

        spinner.succeed();
        console.log(formatOutput(keys, globalOpts));
      } catch (err: any) {
        error(`Failed to get keys: ${err.message}`);
        process.exit(1);
      }
    });

  // ammdata-ping
  espo
    .command('ammdata-ping')
    .description('Ping ESPO AMM data service')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Pinging AMM data service...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const pong = await provider.espo.ammdataPing();

        spinner.succeed();
        console.log(formatOutput(pong, globalOpts));
      } catch (err: any) {
        error(`Failed to ping: ${err.message}`);
        process.exit(1);
      }
    });

  // candles
  espo
    .command('candles <pool>')
    .description('Get OHLCV candlestick data for a pool')
    .option('--timeframe <timeframe>', 'Timeframe (e.g., "1m", "5m", "1h", "1d")')
    .option('--side <side>', 'Side ("buy" or "sell")')
    .option('--limit <limit>', 'Limit results')
    .option('--page <page>', 'Page number')
    .action(async (pool, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting candles...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const candles = await provider.espo.getCandles(
          pool,
          options.timeframe,
          options.side,
          options.limit ? parseInt(options.limit, 10) : undefined,
          options.page ? parseInt(options.page, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(candles, globalOpts));
      } catch (err: any) {
        error(`Failed to get candles: ${err.message}`);
        process.exit(1);
      }
    });

  // trades
  espo
    .command('trades <pool>')
    .description('Get trade history for a pool')
    .option('--limit <limit>', 'Limit results')
    .option('--page <page>', 'Page number')
    .option('--side <side>', 'Side filter')
    .option('--filter-side <side>', 'Filter by side')
    .option('--sort <field>', 'Sort field')
    .option('--dir <direction>', 'Sort direction')
    .action(async (pool, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting trades...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const trades = await provider.espo.getTrades(
          pool,
          options.limit ? parseInt(options.limit, 10) : undefined,
          options.page ? parseInt(options.page, 10) : undefined,
          options.side,
          options.filterSide,
          options.sort,
          options.dir
        );

        spinner.succeed();
        console.log(formatOutput(trades, globalOpts));
      } catch (err: any) {
        error(`Failed to get trades: ${err.message}`);
        process.exit(1);
      }
    });

  // pools
  espo
    .command('pools')
    .description('Get all pools with pagination')
    .option('--limit <limit>', 'Limit results')
    .option('--page <page>', 'Page number')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pools...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const pools = await provider.espo.getPools(
          options.limit ? parseInt(options.limit, 10) : undefined,
          options.page ? parseInt(options.page, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(pools, globalOpts));
      } catch (err: any) {
        error(`Failed to get pools: ${err.message}`);
        process.exit(1);
      }
    });

  // find-best-swap-path
  espo
    .command('find-best-swap-path <token-in> <token-out>')
    .description('Find the best swap path between two tokens')
    .option('--mode <mode>', 'Mode')
    .option('--amount-in <amount>', 'Amount in')
    .option('--amount-out <amount>', 'Amount out')
    .option('--amount-out-min <amount>', 'Minimum amount out')
    .option('--amount-in-max <amount>', 'Maximum amount in')
    .option('--available-in <amount>', 'Available amount in')
    .option('--fee-bps <bps>', 'Fee in basis points')
    .option('--max-hops <hops>', 'Maximum number of hops')
    .action(async (tokenIn, tokenOut, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Finding best swap path...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const path = await provider.espo.findBestSwapPath(
          tokenIn,
          tokenOut,
          options.mode,
          options.amountIn,
          options.amountOut,
          options.amountOutMin,
          options.amountInMax,
          options.availableIn,
          options.feeBps ? parseInt(options.feeBps, 10) : undefined,
          options.maxHops ? parseInt(options.maxHops, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(path, globalOpts));
      } catch (err: any) {
        error(`Failed to find swap path: ${err.message}`);
        process.exit(1);
      }
    });

  // get-best-mev-swap
  espo
    .command('get-best-mev-swap <token>')
    .description('Find the best MEV swap opportunity for a token')
    .option('--fee-bps <bps>', 'Fee in basis points')
    .option('--max-hops <hops>', 'Maximum number of hops')
    .action(async (token, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Finding best MEV swap...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const mevSwap = await provider.espo.getBestMevSwap(
          token,
          options.feeBps ? parseInt(options.feeBps, 10) : undefined,
          options.maxHops ? parseInt(options.maxHops, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(mevSwap, globalOpts));
      } catch (err: any) {
        error(`Failed to find MEV swap: ${err.message}`);
        process.exit(1);
      }
    });
}
