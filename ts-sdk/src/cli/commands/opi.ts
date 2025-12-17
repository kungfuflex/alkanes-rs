/**
 * OPI command group
 * Open Protocol Indexer (BRC-20, Runes, Bitmap, etc.)
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error } from '../utils/formatting.js';
import ora from 'ora';

export function registerOpiCommands(program: Command): void {
  const opi = program.command('opi').description('Open Protocol Indexer operations');

  // Default OPI base URL
  const DEFAULT_OPI_URL = 'https://opi.alkanes.build';

  // block-height
  opi
    .command('block-height')
    .description('Get current indexed block height')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting OPI block height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiBlockHeight(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get OPI block height: ${err.message}`);
        process.exit(1);
      }
    });

  // extras-block-height
  opi
    .command('extras-block-height')
    .description('Get extras indexed block height')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting OPI extras block height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiExtrasBlockHeight(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get OPI extras block height: ${err.message}`);
        process.exit(1);
      }
    });

  // db-version
  opi
    .command('db-version')
    .description('Get database version')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting OPI database version...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiDbVersion(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get OPI database version: ${err.message}`);
        process.exit(1);
      }
    });

  // event-hash-version
  opi
    .command('event-hash-version')
    .description('Get event hash version')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting OPI event hash version...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiEventHashVersion(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get OPI event hash version: ${err.message}`);
        process.exit(1);
      }
    });

  // balance-on-block
  opi
    .command('balance-on-block <block-height> <pkscript> <ticker>')
    .description('Get balance on a specific block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, pkscript, ticker, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting balance on block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiBalanceOnBlock(
          options.opiUrl,
          parseFloat(blockHeight),
          pkscript,
          ticker
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get balance on block: ${err.message}`);
        process.exit(1);
      }
    });

  // activity-on-block
  opi
    .command('activity-on-block <block-height>')
    .description('Get activity on a specific block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting activity on block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiActivityOnBlock(
          options.opiUrl,
          parseFloat(blockHeight)
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get activity on block: ${err.message}`);
        process.exit(1);
      }
    });

  // bitcoin-rpc-results-on-block
  opi
    .command('bitcoin-rpc-results-on-block <block-height>')
    .description('Get Bitcoin RPC results on a specific block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Bitcoin RPC results...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiBitcoinRpcResultsOnBlock(
          options.opiUrl,
          parseFloat(blockHeight)
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get Bitcoin RPC results: ${err.message}`);
        process.exit(1);
      }
    });

  // current-balance
  opi
    .command('current-balance <ticker>')
    .description('Get current balance for a ticker')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .option('--address <address>', 'Wallet address')
    .option('--pkscript <pkscript>', 'PK script')
    .action(async (ticker, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting current balance...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiCurrentBalance(
          options.opiUrl,
          ticker,
          options.address || null,
          options.pkscript || null
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get current balance: ${err.message}`);
        process.exit(1);
      }
    });

  // valid-tx-notes-of-wallet
  opi
    .command('valid-tx-notes-of-wallet')
    .description('Get valid transaction notes for a wallet')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .option('--address <address>', 'Wallet address')
    .option('--pkscript <pkscript>', 'PK script')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting valid tx notes...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiValidTxNotesOfWallet(
          options.opiUrl,
          options.address || null,
          options.pkscript || null
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get valid tx notes: ${err.message}`);
        process.exit(1);
      }
    });

  // valid-tx-notes-of-ticker
  opi
    .command('valid-tx-notes-of-ticker <ticker>')
    .description('Get valid transaction notes for a ticker')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (ticker, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting valid tx notes for ticker...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiValidTxNotesOfTicker(
          options.opiUrl,
          ticker
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get valid tx notes: ${err.message}`);
        process.exit(1);
      }
    });

  // holders
  opi
    .command('holders <ticker>')
    .description('Get holders of a ticker')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (ticker, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting holders...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiHolders(options.opiUrl, ticker);

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get holders: ${err.message}`);
        process.exit(1);
      }
    });

  // hash-of-all-activity
  opi
    .command('hash-of-all-activity <block-height>')
    .description('Get hash of all activity on a block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting hash of all activity...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiHashOfAllActivity(
          options.opiUrl,
          parseFloat(blockHeight)
        );

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get hash of all activity: ${err.message}`);
        process.exit(1);
      }
    });

  // hash-of-all-current-balances
  opi
    .command('hash-of-all-current-balances')
    .description('Get hash of all current balances')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting hash of all current balances...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiHashOfAllCurrentBalances(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get hash of all current balances: ${err.message}`);
        process.exit(1);
      }
    });

  // event
  opi
    .command('event <event-hash>')
    .description('Get event details by hash')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (eventHash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting event details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiEvent(options.opiUrl, eventHash);

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get event: ${err.message}`);
        process.exit(1);
      }
    });

  // ip
  opi
    .command('ip')
    .description('Get OPI server IP address')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting OPI IP address...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiIp(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get OPI IP: ${err.message}`);
        process.exit(1);
      }
    });

  // raw
  opi
    .command('raw <endpoint>')
    .description('Make a raw request to an OPI endpoint')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (endpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Making raw OPI request...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiRaw(options.opiUrl, endpoint);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to make raw OPI request: ${err.message}`);
        process.exit(1);
      }
    });
}
