/**
 * ESPO command group
 * ESPO balance indexer operations
 *
 * The CLI uses the SDK's EspoClient via provider.espo for all operations.
 * This ensures the CLI and SDK share the same typed interface.
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error, info } from '../utils/formatting.js';
import ora from 'ora';
import { resolveAddressWithProvider } from '../utils/address-resolver.js';

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
    .description('Get balances for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
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

        const balances = await provider.espo.getAddressBalances(
          resolvedAddress,
          options.includeOutpoints
        );

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

  // address-outpoints
  espo
    .command('address-outpoints <address>')
    .description('Get outpoints for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting outpoints...').start();

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

        const outpoints = await provider.espo.getAddressOutpoints(resolvedAddress);

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
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

  // amm-factories
  espo
    .command('amm-factories')
    .description('Get AMM factories')
    .option('--page <page>', 'Page number')
    .option('--limit <limit>', 'Limit results')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting AMM factories...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getAmmFactories(
          options.page ? parseInt(options.page, 10) : undefined,
          options.limit ? parseInt(options.limit, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get AMM factories: ${err.message}`);
        process.exit(1);
      }
    });

  // all-alkanes
  espo
    .command('all-alkanes')
    .description('Get all alkanes with pagination')
    .option('--page <page>', 'Page number')
    .option('--limit <limit>', 'Limit results')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting all alkanes...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getAllAlkanes(
          options.page ? parseInt(options.page, 10) : undefined,
          options.limit ? parseInt(options.limit, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get all alkanes: ${err.message}`);
        process.exit(1);
      }
    });

  // alkane-info
  espo
    .command('alkane-info <alkane-id>')
    .description('Get info for a specific alkane')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkane info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getAlkaneInfo(alkaneId);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkane info: ${err.message}`);
        process.exit(1);
      }
    });

  // block-summary
  espo
    .command('block-summary <height>')
    .description('Get block summary')
    .action(async (height, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block summary...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getBlockSummary(parseInt(height, 10));

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get block summary: ${err.message}`);
        process.exit(1);
      }
    });

  // circulating-supply
  espo
    .command('circulating-supply <alkane-id>')
    .description('Get circulating supply of an alkane')
    .option('--height <height>', 'Block height for historical query')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting circulating supply...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getCirculatingSupply(
          alkaneId,
          options.height ? parseInt(options.height, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get circulating supply: ${err.message}`);
        process.exit(1);
      }
    });

  // transfer-volume
  espo
    .command('transfer-volume <alkane-id>')
    .description('Get transfer volume for an alkane')
    .option('--page <page>', 'Page number')
    .option('--limit <limit>', 'Limit results')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transfer volume...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getTransferVolume(
          alkaneId,
          options.page ? parseInt(options.page, 10) : undefined,
          options.limit ? parseInt(options.limit, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get transfer volume: ${err.message}`);
        process.exit(1);
      }
    });

  // total-received
  espo
    .command('total-received <alkane-id>')
    .description('Get total received for an alkane')
    .option('--page <page>', 'Page number')
    .option('--limit <limit>', 'Limit results')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting total received...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getTotalReceived(
          alkaneId,
          options.page ? parseInt(options.page, 10) : undefined,
          options.limit ? parseInt(options.limit, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get total received: ${err.message}`);
        process.exit(1);
      }
    });

  // address-activity
  espo
    .command('address-activity <address>')
    .description('Get activity for an address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address activity...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const resolvedAddress = await resolveAddressWithProvider(address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.espo.getAddressActivity(resolvedAddress);

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address activity: ${err.message}`);
        process.exit(1);
      }
    });

  // alkane-balances (all holders of an alkane)
  espo
    .command('alkane-balances <alkane-id>')
    .description('Get all balances for an alkane (all holders)')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkane balances...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getAlkaneBalances(alkaneId);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkane balances: ${err.message}`);
        process.exit(1);
      }
    });

  // alkane-balance-metashrew
  espo
    .command('alkane-balance-metashrew <owner> <target>')
    .description('Get alkane balance via metashrew (owner and target are AlkaneIds)')
    .option('--height <height>', 'Block height')
    .action(async (owner, target, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkane balance metashrew...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getAlkaneBalanceMetashrew(
          owner,
          target,
          options.height ? parseInt(options.height, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkane balance metashrew: ${err.message}`);
        process.exit(1);
      }
    });

  // alkane-balance-txs
  espo
    .command('alkane-balance-txs <alkane-id>')
    .description('Get alkane balance transactions')
    .option('--page <page>', 'Page number')
    .option('--limit <limit>', 'Limit results')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkane balance txs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getAlkaneBalanceTxs(
          alkaneId,
          options.page ? parseInt(options.page, 10) : undefined,
          options.limit ? parseInt(options.limit, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkane balance txs: ${err.message}`);
        process.exit(1);
      }
    });

  // alkane-balance-txs-by-token
  espo
    .command('alkane-balance-txs-by-token <owner> <token>')
    .description('Get alkane balance transactions by token')
    .option('--page <page>', 'Page number')
    .option('--limit <limit>', 'Limit results')
    .action(async (owner, token, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkane balance txs by token...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getAlkaneBalanceTxsByToken(
          owner,
          token,
          options.page ? parseInt(options.page, 10) : undefined,
          options.limit ? parseInt(options.limit, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkane balance txs by token: ${err.message}`);
        process.exit(1);
      }
    });

  // block-traces
  espo
    .command('block-traces <height>')
    .description('Get traces for a block')
    .action(async (height, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block traces...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getBlockTraces(parseInt(height, 10));

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get block traces: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-summary
  espo
    .command('tx-summary <txid>')
    .description('Get alkane transaction summary')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting tx summary...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getAlkaneTxSummary(txid);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get tx summary: ${err.message}`);
        process.exit(1);
      }
    });

  // block-txs
  espo
    .command('block-txs <height>')
    .description('Get alkane transactions in a block')
    .option('--page <page>', 'Page number')
    .option('--limit <limit>', 'Limit results')
    .action(async (height, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block txs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getAlkaneBlockTxs(
          parseInt(height, 10),
          options.page ? parseInt(options.page, 10) : undefined,
          options.limit ? parseInt(options.limit, 10) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get block txs: ${err.message}`);
        process.exit(1);
      }
    });

  // address-txs
  espo
    .command('address-txs <address>')
    .description('Get alkane transactions for an address')
    .option('--page <page>', 'Page number')
    .option('--limit <limit>', 'Limit results')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address txs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const resolvedAddress = await resolveAddressWithProvider(address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.espo.getAlkaneAddressTxs(
          resolvedAddress,
          options.page ? parseInt(options.page, 10) : undefined,
          options.limit ? parseInt(options.limit, 10) : undefined
        );

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address txs: ${err.message}`);
        process.exit(1);
      }
    });

  // address-transactions
  espo
    .command('address-transactions <address>')
    .description('Get all transactions for an address')
    .option('--page <page>', 'Page number')
    .option('--limit <limit>', 'Limit results')
    .option('--only-alkane-txs', 'Only return alkane transactions', false)
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting address transactions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const resolvedAddress = await resolveAddressWithProvider(address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.espo.getAddressTransactions(
          resolvedAddress,
          options.page ? parseInt(options.page, 10) : undefined,
          options.limit ? parseInt(options.limit, 10) : undefined,
          options.onlyAlkaneTxs || undefined
        );

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get address transactions: ${err.message}`);
        process.exit(1);
      }
    });

  // latest-traces
  espo
    .command('latest-traces')
    .description('Get latest alkane traces')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting latest traces...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getAlkaneLatestTraces();

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get latest traces: ${err.message}`);
        process.exit(1);
      }
    });

  // mempool-traces
  espo
    .command('mempool-traces')
    .description('Get mempool traces')
    .option('--page <page>', 'Page number')
    .option('--limit <limit>', 'Limit results')
    .option('--address <address>', 'Filter by address')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting mempool traces...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getMempoolTraces(
          options.page ? parseInt(options.page, 10) : undefined,
          options.limit ? parseInt(options.limit, 10) : undefined,
          options.address
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get mempool traces: ${err.message}`);
        process.exit(1);
      }
    });

  // wrap-events
  espo
    .command('wrap-events')
    .description('Get all wrap events')
    .option('--count <count>', 'Number of events')
    .option('--offset <offset>', 'Offset for pagination')
    .option('--successful', 'Filter by success status')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting wrap events...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getWrapEvents(
          options.count ? parseInt(options.count, 10) : undefined,
          options.offset ? parseInt(options.offset, 10) : undefined,
          options.successful !== undefined ? true : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get wrap events: ${err.message}`);
        process.exit(1);
      }
    });

  // wrap-events-by-address
  espo
    .command('wrap-events-by-address <address>')
    .description('Get wrap events for an address')
    .option('--count <count>', 'Number of events')
    .option('--offset <offset>', 'Offset for pagination')
    .option('--successful', 'Filter by success status')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting wrap events...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const resolvedAddress = await resolveAddressWithProvider(address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.espo.getWrapEventsByAddress(
          resolvedAddress,
          options.count ? parseInt(options.count, 10) : undefined,
          options.offset ? parseInt(options.offset, 10) : undefined,
          options.successful !== undefined ? true : undefined
        );

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get wrap events: ${err.message}`);
        process.exit(1);
      }
    });

  // unwrap-events
  espo
    .command('unwrap-events')
    .description('Get all unwrap events')
    .option('--count <count>', 'Number of events')
    .option('--offset <offset>', 'Offset for pagination')
    .option('--successful', 'Filter by success status')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting unwrap events...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getUnwrapEvents(
          options.count ? parseInt(options.count, 10) : undefined,
          options.offset ? parseInt(options.offset, 10) : undefined,
          options.successful !== undefined ? true : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get unwrap events: ${err.message}`);
        process.exit(1);
      }
    });

  // unwrap-events-by-address
  espo
    .command('unwrap-events-by-address <address>')
    .description('Get unwrap events for an address')
    .option('--count <count>', 'Number of events')
    .option('--offset <offset>', 'Offset for pagination')
    .option('--successful', 'Filter by success status')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting unwrap events...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const resolvedAddress = await resolveAddressWithProvider(address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.espo.getUnwrapEventsByAddress(
          resolvedAddress,
          options.count ? parseInt(options.count, 10) : undefined,
          options.offset ? parseInt(options.offset, 10) : undefined,
          options.successful !== undefined ? true : undefined
        );

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get unwrap events: ${err.message}`);
        process.exit(1);
      }
    });

  // series-id-from-alkane
  espo
    .command('series-id-from-alkane <alkane-id>')
    .description('Get series ID from alkane ID (pizzafun)')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting series ID...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getSeriesIdFromAlkaneId(alkaneId);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get series ID: ${err.message}`);
        process.exit(1);
      }
    });

  // series-ids-from-alkanes
  espo
    .command('series-ids-from-alkanes <alkane-ids>')
    .description('Get series IDs from comma-separated alkane IDs (pizzafun)')
    .action(async (alkaneIds, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting series IDs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const ids = alkaneIds.split(',').map((s: string) => s.trim());
        const result = await provider.espo.getSeriesIdsFromAlkaneIds(ids);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get series IDs: ${err.message}`);
        process.exit(1);
      }
    });

  // alkane-from-series-id
  espo
    .command('alkane-from-series-id <series-id>')
    .description('Get alkane ID from series ID (pizzafun)')
    .action(async (seriesId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkane ID...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.espo.getAlkaneIdFromSeriesId(seriesId);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkane ID: ${err.message}`);
        process.exit(1);
      }
    });

  // alkanes-from-series-ids
  espo
    .command('alkanes-from-series-ids <series-ids>')
    .description('Get alkane IDs from comma-separated series IDs (pizzafun)')
    .action(async (seriesIds, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkane IDs...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const ids = seriesIds.split(',').map((s: string) => s.trim());
        const result = await provider.espo.getAlkaneIdsFromSeriesIds(ids);

        spinner.succeed();
        console.log(formatOutput(result, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkane IDs: ${err.message}`);
        process.exit(1);
      }
    });
}
