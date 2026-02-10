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

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAlkanes(page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
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

        const result = await provider.oylApi.getAlkaneDetails(alkaneId);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
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

        const result = await provider.oylApi.getPoolById(poolId);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
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

        const result = await provider.oylApi.getOutpointBalances(outpoint);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
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

        const result = await provider.oylApi.getBlockHeight();

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
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

        const result = await provider.oylApi.getBlockHash();

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
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

        const result = await provider.oylApi.getIndexerPosition();

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get indexer position: ${err.message}`);
        process.exit(1);
      }
    });

  // ============================================================================
  // NEW OYLAPI COMMANDS
  // ============================================================================

  // get-alkanes-utxo
  dataapi
    .command('get-alkanes-utxo <address>')
    .description('Get alkanes UTXOs for address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkanes UTXOs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getAlkanesUtxo(address);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkanes UTXOs: ${err.message}`);
        process.exit(1);
      }
    });

  // get-amm-utxos
  dataapi
    .command('get-amm-utxos <address>')
    .description('Get AMM UTXOs for address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting AMM UTXOs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getAmmUtxos(address);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get AMM UTXOs: ${err.message}`);
        process.exit(1);
      }
    });

  // global-search
  dataapi
    .command('global-search <query>')
    .description('Global search for alkanes')
    .option('--limit <number>', 'Limit results', '100')
    .option('--offset <number>', 'Offset for pagination', '0')
    .action(async (query, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Searching alkanes...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const limit = options.limit ? parseInt(options.limit) : undefined;
        const offset = options.offset ? parseInt(options.offset) : undefined;
        const result = await provider.oylApi.globalAlkanesSearch(query, limit, offset);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to search: ${err.message}`);
        process.exit(1);
      }
    });

  // get-address-outpoints
  dataapi
    .command('get-address-outpoints <address>')
    .description('Get outpoints for address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address outpoints...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getAddressOutpoints(address);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address outpoints: ${err.message}`);
        process.exit(1);
      }
    });

  // pathfind
  dataapi
    .command('pathfind <token-in> <token-out> <amount-in>')
    .description('Find swap path between tokens')
    .option('--max-hops <number>', 'Maximum swap hops', '3')
    .action(async (tokenIn, tokenOut, amountIn, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Finding swap path...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const maxHops = options.maxHops ? parseInt(options.maxHops) : undefined;
        const result = await provider.oylApi.pathfind(tokenIn, tokenOut, amountIn, maxHops);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to find path: ${err.message}`);
        process.exit(1);
      }
    });

  // get-pool-details
  dataapi
    .command('get-pool-details <pool-id>')
    .description('Get detailed pool information')
    .action(async (poolId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pool details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getPoolDetails(poolId);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get pool details: ${err.message}`);
        process.exit(1);
      }
    });

  // get-all-pools-details
  dataapi
    .command('get-all-pools-details <factory-id>')
    .description('Get all pools details for factory')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (factoryId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting all pools details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAllPoolsDetails(factoryId, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get pools details: ${err.message}`);
        process.exit(1);
      }
    });

  // get-address-positions
  dataapi
    .command('get-address-positions <address> <factory-id>')
    .description('Get LP positions for address')
    .action(async (address, factoryId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address positions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getAddressPositions(address, factoryId);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address positions: ${err.message}`);
        process.exit(1);
      }
    });

  // get-token-pairs
  dataapi
    .command('get-token-pairs <alkane-id> <factory-id>')
    .description('Get token pairs for specific token')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (alkaneId, factoryId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting token pairs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getTokenPairs(alkaneId, factoryId, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get token pairs: ${err.message}`);
        process.exit(1);
      }
    });

  // get-all-token-pairs
  dataapi
    .command('get-all-token-pairs <factory-id>')
    .description('Get all token pairs')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (factoryId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting all token pairs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAllTokenPairs(factoryId, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get all token pairs: ${err.message}`);
        process.exit(1);
      }
    });

  // get-alkane-swap-pair-details
  dataapi
    .command('get-alkane-swap-pair-details <alkane-id> <factory-id>')
    .description('Get swap pair details for alkane')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (alkaneId, factoryId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting swap pair details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAlkaneSwapPairDetails(alkaneId, factoryId, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get swap pair details: ${err.message}`);
        process.exit(1);
      }
    });

  // get-pool-creation-history
  dataapi
    .command('get-pool-creation-history')
    .description('Get pool creation history')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pool creation history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getPoolCreationHistory(page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get pool creation history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-pool-swap-history
  dataapi
    .command('get-pool-swap-history <pool-id>')
    .description('Get swap history for pool')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (poolId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pool swap history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getPoolSwapHistory(poolId, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get pool swap history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-token-swap-history
  dataapi
    .command('get-token-swap-history <alkane-id>')
    .description('Get swap history for token')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting token swap history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getTokenSwapHistory(alkaneId, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get token swap history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-pool-mint-history
  dataapi
    .command('get-pool-mint-history <pool-id>')
    .description('Get mint history for pool')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (poolId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pool mint history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getPoolMintHistory(poolId, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get pool mint history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-pool-burn-history
  dataapi
    .command('get-pool-burn-history <pool-id>')
    .description('Get burn history for pool')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (poolId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pool burn history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getPoolBurnHistory(poolId, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get pool burn history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-address-swap-history-for-pool
  dataapi
    .command('get-address-swap-history-for-pool <address> <pool-id>')
    .description('Get swap history for address in pool')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (address, poolId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address swap history for pool...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAddressSwapHistoryForPool(address, poolId, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address swap history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-address-swap-history-for-token
  dataapi
    .command('get-address-swap-history-for-token <address> <alkane-id>')
    .description('Get swap history for address with token')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (address, alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address swap history for token...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAddressSwapHistoryForToken(address, alkaneId, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address swap history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-address-wrap-history
  dataapi
    .command('get-address-wrap-history <address>')
    .description('Get wrap history for address')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address wrap history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAddressWrapHistory(address, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address wrap history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-address-unwrap-history
  dataapi
    .command('get-address-unwrap-history <address>')
    .description('Get unwrap history for address')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address unwrap history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAddressUnwrapHistory(address, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address unwrap history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-all-wrap-history
  dataapi
    .command('get-all-wrap-history')
    .description('Get all wrap history')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting all wrap history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAllWrapHistory(page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get all wrap history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-all-unwrap-history
  dataapi
    .command('get-all-unwrap-history')
    .description('Get all unwrap history')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting all unwrap history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAllUnwrapHistory(page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get all unwrap history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-total-unwrap-amount
  dataapi
    .command('get-total-unwrap-amount')
    .description('Get total unwrap amount')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting total unwrap amount...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getTotalUnwrapAmount();

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get total unwrap amount: ${err.message}`);
        process.exit(1);
      }
    });

  // get-address-pool-creation-history
  dataapi
    .command('get-address-pool-creation-history <address>')
    .description('Get pool creation history for address')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address pool creation history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAddressPoolCreationHistory(address, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address pool creation history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-address-pool-mint-history
  dataapi
    .command('get-address-pool-mint-history <address>')
    .description('Get pool mint history for address')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address pool mint history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAddressPoolMintHistory(address, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address pool mint history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-address-pool-burn-history
  dataapi
    .command('get-address-pool-burn-history <address>')
    .description('Get pool burn history for address')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address pool burn history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAddressPoolBurnHistory(address, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address pool burn history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-all-address-amm-tx-history
  dataapi
    .command('get-all-address-amm-tx-history <address>')
    .description('Get all AMM transaction history for address')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting all address AMM tx history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAllAddressAmmTxHistory(address, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get all address AMM tx history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-all-amm-tx-history
  dataapi
    .command('get-all-amm-tx-history')
    .description('Get all AMM transaction history')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting all AMM tx history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getAllAmmTxHistory(page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get all AMM tx history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-address-balance
  dataapi
    .command('get-address-balance <address>')
    .description('Get BTC balance for address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address balance...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getAddressBalance(address);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address balance: ${err.message}`);
        process.exit(1);
      }
    });

  // get-taproot-balance
  dataapi
    .command('get-taproot-balance <address>')
    .description('Get taproot address balance')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting taproot balance...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getTaprootBalance(address);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get taproot balance: ${err.message}`);
        process.exit(1);
      }
    });

  // get-address-utxos
  dataapi
    .command('get-address-utxos <address>')
    .description('Get UTXOs for address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address UTXOs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getAddressUtxos(address);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address UTXOs: ${err.message}`);
        process.exit(1);
      }
    });

  // get-account-utxos
  dataapi
    .command('get-account-utxos <account>')
    .description('Get UTXOs for account')
    .action(async (account, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting account UTXOs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getAccountUtxos(account);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get account UTXOs: ${err.message}`);
        process.exit(1);
      }
    });

  // get-account-balance
  dataapi
    .command('get-account-balance <account>')
    .description('Get balance for account')
    .action(async (account, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting account balance...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getAccountBalance(account);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get account balance: ${err.message}`);
        process.exit(1);
      }
    });

  // get-taproot-history
  dataapi
    .command('get-taproot-history <address> <total-txs>')
    .description('Get taproot address history')
    .action(async (address, totalTxs, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting taproot history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getTaprootHistory(address, parseInt(totalTxs));

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get taproot history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-intent-history
  dataapi
    .command('get-intent-history <address>')
    .description('Get intent history for address')
    .option('--page <number>', 'Page number', '0')
    .option('--limit <number>', 'Results per page', '100')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting intent history...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const page = options.page ? parseInt(options.page) : undefined;
        const limit = options.limit ? parseInt(options.limit) : undefined;
        const result = await provider.oylApi.getIntentHistory(address, page, limit);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get intent history: ${err.message}`);
        process.exit(1);
      }
    });

  // get-bitcoin-market-weekly
  dataapi
    .command('get-bitcoin-market-weekly')
    .description('Get Bitcoin weekly market data')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Bitcoin weekly market data...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getBitcoinMarketWeekly();

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get Bitcoin market weekly: ${err.message}`);
        process.exit(1);
      }
    });

  // get-bitcoin-markets
  dataapi
    .command('get-bitcoin-markets')
    .description('Get Bitcoin markets data')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Bitcoin markets...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.oylApi.getBitcoinMarkets();

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get Bitcoin markets: ${err.message}`);
        process.exit(1);
      }
    });
}
