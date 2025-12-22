/**
 * Dataapi command group
 * Analytics and data API operations
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error, info } from '../utils/formatting.js';
import ora from 'ora';
import { resolveAddressWithProvider } from '../utils/address-resolver.js';

export function registerDataapiCommands(program: Command): void {
  const dataapi = program.command('dataapi').description('Analytics and data API operations');

  // pools
  dataapi
    .command('pools <factory-id>')
    .description('Get pools for factory')
    .action(async (factoryId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pools...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.data_api_get_pools_js(factoryId);
        const pools = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(pools, globalOpts));
      } catch (err: any) {
        error(`Failed to get pools: ${err.message}`);
        process.exit(1);
      }
    });

  // pool-history
  dataapi
    .command('pool-history <pool-id>')
    .description('Get pool history')
    .option('--category <category>', 'History category')
    .option('--limit <limit>', 'Limit results', '100')
    .option('--offset <offset>', 'Offset for pagination', '0')
    .action(async (poolId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pool history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.data_api_get_pool_history_js(
          poolId,
          options.category || null,
          options.limit ? parseInt(options.limit) : null,
          options.offset ? parseInt(options.offset) : null
        );
        const history = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(history, globalOpts));
      } catch (err: any) {
        error(`Failed to get pool history: ${err.message}`);
        process.exit(1);
      }
    });

  // trades
  dataapi
    .command('trades <pool>')
    .description('Get trade history for pool')
    .option('--start-time <timestamp>', 'Start time')
    .option('--end-time <timestamp>', 'End time')
    .option('--limit <limit>', 'Limit results', '100')
    .action(async (pool, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting trades...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.data_api_get_trades_js(
          pool,
          options.startTime ? parseFloat(options.startTime) : null,
          options.endTime ? parseFloat(options.endTime) : null,
          options.limit ? parseInt(options.limit) : null
        );
        const trades = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(trades, globalOpts));
      } catch (err: any) {
        error(`Failed to get trades: ${err.message}`);
        process.exit(1);
      }
    });

  // candles
  dataapi
    .command('candles <pool>')
    .description('Get candle data for pool')
    .requiredOption('--interval <interval>', 'Interval (1m, 5m, 1h, 1d)')
    .option('--start-time <timestamp>', 'Start time')
    .option('--end-time <timestamp>', 'End time')
    .option('--limit <limit>', 'Limit results', '100')
    .action(async (pool, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting candles...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.data_api_get_candles_js(
          pool,
          options.interval,
          options.startTime ? parseFloat(options.startTime) : null,
          options.endTime ? parseFloat(options.endTime) : null,
          options.limit ? parseInt(options.limit) : null
        );
        const candles = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(candles, globalOpts));
      } catch (err: any) {
        error(`Failed to get candles: ${err.message}`);
        process.exit(1);
      }
    });

  // reserves
  dataapi
    .command('reserves <pool>')
    .description('Get pool reserves')
    .action(async (pool, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting reserves...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.data_api_get_reserves_js(pool);
        const reserves = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(reserves, globalOpts));
      } catch (err: any) {
        error(`Failed to get reserves: ${err.message}`);
        process.exit(1);
      }
    });

  // holders
  dataapi
    .command('holders <alkane>')
    .description('Get alkane holders')
    .option('--page <page>', 'Page number', '0')
    .option('--limit <limit>', 'Limit results', '100')
    .action(async (alkane, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting holders...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.data_api_get_holders_js(
          alkane,
          parseInt(options.page),
          parseInt(options.limit)
        );
        const holders = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(holders, globalOpts));
      } catch (err: any) {
        error(`Failed to get holders: ${err.message}`);
        process.exit(1);
      }
    });

  // holders-count
  dataapi
    .command('holders-count <alkane>')
    .description('Get count of alkane holders')
    .action(async (alkane, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting holders count...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.data_api_get_holders_count_js(alkane);
        const count = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(count, globalOpts));
      } catch (err: any) {
        error(`Failed to get holders count: ${err.message}`);
        process.exit(1);
      }
    });

  // bitcoin-price
  dataapi
    .command('bitcoin-price')
    .description('Get current Bitcoin price')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Bitcoin price...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.data_api_get_bitcoin_price_js();
        const price = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(price, globalOpts));
      } catch (err: any) {
        error(`Failed to get Bitcoin price: ${err.message}`);
        process.exit(1);
      }
    });

  // bitcoin-market-chart
  dataapi
    .command('bitcoin-market-chart <days>')
    .description('Get Bitcoin market chart')
    .action(async (days, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting market chart...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.data_api_get_bitcoin_market_chart_js(days);
        const chart = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(chart, globalOpts));
      } catch (err: any) {
        error(`Failed to get market chart: ${err.message}`);
        process.exit(1);
      }
    });

  // address-balances
  dataapi
    .command('address-balances <address>')
    .description('Get alkanes balances for address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
    .option('--include-outpoints', 'Include outpoint details', false)
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting balances...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        // Resolve wallet address identifiers
        const resolvedAddress = await resolveAddressWithProvider(address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.data_api_get_address_balances_js(
          resolvedAddress,
          options.includeOutpoints
        );
        const balances = JSON.parse(result);

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
        console.log(formatOutput(balances, globalOpts));
      } catch (err: any) {
        error(`Failed to get balances: ${err.message}`);
        process.exit(1);
      }
    });

  // alkanes-by-address
  dataapi
    .command('alkanes-by-address <address>')
    .description('Get alkanes owned by address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkanes...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        // Resolve wallet address identifiers
        const resolvedAddress = await resolveAddressWithProvider(address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.data_api_get_alkanes_by_address_js(resolvedAddress);
        const alkanes = JSON.parse(result);

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
        console.log(formatOutput(alkanes, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkanes: ${err.message}`);
        process.exit(1);
      }
    });

  // health
  dataapi
    .command('health')
    .description('Check data API health')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Checking health...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        await provider.dataApiHealth();

        spinner.succeed('Data API is healthy');
      } catch (err: any) {
        error(`Health check failed: ${err.message}`);
        process.exit(1);
      }
    });

  // get-alkanes
  dataapi
    .command('get-alkanes')
    .description('Get all alkanes')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkanes...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : null;
        const limit = options.limit ? parseInt(options.limit) : null;
        const result = await provider.dataApiGetAlkanes(page, limit);
        const alkanes = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(alkanes, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkanes: ${err.message}`);
        process.exit(1);
      }
    });

  // get-alkane-details
  dataapi
    .command('get-alkane-details <alkane-id>')
    .description('Get alkane details by ID (format: block:tx)')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkane details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.dataApiGetAlkaneDetails(alkaneId);
        const details = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(details, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkane details: ${err.message}`);
        process.exit(1);
      }
    });

  // get-pool-by-id
  dataapi
    .command('get-pool-by-id <pool-id>')
    .description('Get pool details by ID (format: block:tx)')
    .action(async (poolId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pool details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.dataApiGetPoolById(poolId);
        const pool = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(pool, globalOpts));
      } catch (err: any) {
        error(`Failed to get pool: ${err.message}`);
        process.exit(1);
      }
    });

  // get-outpoint-balances
  dataapi
    .command('get-outpoint-balances <outpoint>')
    .description('Get balances for an outpoint (format: txid:vout)')
    .action(async (outpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting outpoint balances...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.dataApiGetOutpointBalances(outpoint);
        const balances = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(balances, globalOpts));
      } catch (err: any) {
        error(`Failed to get outpoint balances: ${err.message}`);
        process.exit(1);
      }
    });

  // get-block-height
  dataapi
    .command('get-block-height')
    .description('Get latest indexed block height')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.dataApiGetBlockHeight();
        const height = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(height, globalOpts));
      } catch (err: any) {
        error(`Failed to get block height: ${err.message}`);
        process.exit(1);
      }
    });

  // get-block-hash
  dataapi
    .command('get-block-hash')
    .description('Get latest indexed block hash')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.dataApiGetBlockHash();
        const hash = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(hash, globalOpts));
      } catch (err: any) {
        error(`Failed to get block hash: ${err.message}`);
        process.exit(1);
      }
    });

  // get-indexer-position
  dataapi
    .command('get-indexer-position')
    .description('Get indexer position (height and hash)')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting indexer position...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.dataApiGetIndexerPosition();
        const position = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(position, globalOpts));
      } catch (err: any) {
        error(`Failed to get indexer position: ${err.message}`);
        process.exit(1);
      }
    });
}
