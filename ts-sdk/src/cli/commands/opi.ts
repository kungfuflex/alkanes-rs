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

  // ==================== RUNES COMMANDS ====================

  // runes-block-height
  opi
    .command('runes-block-height')
    .description('Get Runes indexed block height')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Runes block height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiRunesBlockHeight(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get Runes block height: ${err.message}`);
        process.exit(1);
      }
    });

  // runes-balance-on-block
  opi
    .command('runes-balance-on-block <block-height> <pkscript> <rune-id>')
    .description('Get Runes balance on a specific block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, pkscript, runeId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Runes balance on block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiRunesBalanceOnBlock(
          options.opiUrl,
          parseFloat(blockHeight),
          pkscript,
          runeId
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get Runes balance: ${err.message}`);
        process.exit(1);
      }
    });

  // runes-activity-on-block
  opi
    .command('runes-activity-on-block <block-height>')
    .description('Get Runes activity on a specific block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Runes activity on block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiRunesActivityOnBlock(
          options.opiUrl,
          parseFloat(blockHeight)
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get Runes activity: ${err.message}`);
        process.exit(1);
      }
    });

  // runes-current-balance
  opi
    .command('runes-current-balance')
    .description('Get current Runes balance for a wallet')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .option('--address <address>', 'Wallet address')
    .option('--pkscript <pkscript>', 'PK script')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting current Runes balance...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiRunesCurrentBalanceOfWallet(
          options.opiUrl,
          options.address || null,
          options.pkscript || null
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get Runes balance: ${err.message}`);
        process.exit(1);
      }
    });

  // runes-unspent-outpoints
  opi
    .command('runes-unspent-outpoints')
    .description('Get unspent Runes outpoints for a wallet')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .option('--address <address>', 'Wallet address')
    .option('--pkscript <pkscript>', 'PK script')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting unspent Runes outpoints...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiRunesUnspentOutpointsOfWallet(
          options.opiUrl,
          options.address || null,
          options.pkscript || null
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get unspent outpoints: ${err.message}`);
        process.exit(1);
      }
    });

  // runes-holders
  opi
    .command('runes-holders <rune-id>')
    .description('Get holders of a Rune')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (runeId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Runes holders...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiRunesHolders(options.opiUrl, runeId);

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get Runes holders: ${err.message}`);
        process.exit(1);
      }
    });

  // runes-hash-of-all-activity
  opi
    .command('runes-hash-of-all-activity <block-height>')
    .description('Get hash of all Runes activity on a block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Runes activity hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiRunesHashOfAllActivity(
          options.opiUrl,
          parseFloat(blockHeight)
        );

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get Runes activity hash: ${err.message}`);
        process.exit(1);
      }
    });

  // runes-event
  opi
    .command('runes-event <event-hash>')
    .description('Get Runes event details by hash')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (eventHash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Runes event details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiRunesEvent(options.opiUrl, eventHash);

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get Runes event: ${err.message}`);
        process.exit(1);
      }
    });

  // ==================== BITMAP COMMANDS ====================

  // bitmap-block-height
  opi
    .command('bitmap-block-height')
    .description('Get Bitmap indexed block height')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Bitmap block height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiBitmapBlockHeight(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get Bitmap block height: ${err.message}`);
        process.exit(1);
      }
    });

  // bitmap-hash-of-all-activity
  opi
    .command('bitmap-hash-of-all-activity <block-height>')
    .description('Get hash of all Bitmap activity on a block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Bitmap activity hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiBitmapHashOfAllActivity(
          options.opiUrl,
          parseFloat(blockHeight)
        );

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get Bitmap activity hash: ${err.message}`);
        process.exit(1);
      }
    });

  // bitmap-hash-of-all-bitmaps
  opi
    .command('bitmap-hash-of-all-bitmaps')
    .description('Get hash of all registered Bitmaps')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting hash of all Bitmaps...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiBitmapHashOfAllBitmaps(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get Bitmaps hash: ${err.message}`);
        process.exit(1);
      }
    });

  // bitmap-inscription-id
  opi
    .command('bitmap-inscription-id <bitmap-number>')
    .description('Get inscription ID for a Bitmap number')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (bitmapNumber, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting Bitmap inscription ID...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiBitmapInscriptionId(
          options.opiUrl,
          parseFloat(bitmapNumber)
        );

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get Bitmap inscription ID: ${err.message}`);
        process.exit(1);
      }
    });

  // ==================== POW20 COMMANDS ====================

  // pow20-block-height
  opi
    .command('pow20-block-height')
    .description('Get POW20 indexed block height')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting POW20 block height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiPow20BlockHeight(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get POW20 block height: ${err.message}`);
        process.exit(1);
      }
    });

  // pow20-balance-on-block
  opi
    .command('pow20-balance-on-block <block-height> <pkscript> <ticker>')
    .description('Get POW20 balance on a specific block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, pkscript, ticker, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting POW20 balance on block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiPow20BalanceOnBlock(
          options.opiUrl,
          parseFloat(blockHeight),
          pkscript,
          ticker
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get POW20 balance: ${err.message}`);
        process.exit(1);
      }
    });

  // pow20-activity-on-block
  opi
    .command('pow20-activity-on-block <block-height>')
    .description('Get POW20 activity on a specific block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting POW20 activity on block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiPow20ActivityOnBlock(
          options.opiUrl,
          parseFloat(blockHeight)
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get POW20 activity: ${err.message}`);
        process.exit(1);
      }
    });

  // pow20-current-balance
  opi
    .command('pow20-current-balance')
    .description('Get current POW20 balance for a wallet')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .option('--address <address>', 'Wallet address')
    .option('--pkscript <pkscript>', 'PK script')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting current POW20 balance...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiPow20CurrentBalanceOfWallet(
          options.opiUrl,
          options.address || null,
          options.pkscript || null
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get POW20 balance: ${err.message}`);
        process.exit(1);
      }
    });

  // pow20-valid-tx-notes-of-wallet
  opi
    .command('pow20-valid-tx-notes-of-wallet')
    .description('Get valid POW20 transaction notes for a wallet')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .option('--address <address>', 'Wallet address')
    .option('--pkscript <pkscript>', 'PK script')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting valid POW20 tx notes...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiPow20ValidTxNotesOfWallet(
          options.opiUrl,
          options.address || null,
          options.pkscript || null
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get POW20 tx notes: ${err.message}`);
        process.exit(1);
      }
    });

  // pow20-valid-tx-notes-of-ticker
  opi
    .command('pow20-valid-tx-notes-of-ticker <ticker>')
    .description('Get valid POW20 transaction notes for a ticker')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (ticker, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting valid POW20 tx notes for ticker...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiPow20ValidTxNotesOfTicker(
          options.opiUrl,
          ticker
        );

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get POW20 tx notes: ${err.message}`);
        process.exit(1);
      }
    });

  // pow20-holders
  opi
    .command('pow20-holders <ticker>')
    .description('Get holders of a POW20 ticker')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (ticker, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting POW20 holders...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiPow20Holders(options.opiUrl, ticker);

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get POW20 holders: ${err.message}`);
        process.exit(1);
      }
    });

  // pow20-hash-of-all-activity
  opi
    .command('pow20-hash-of-all-activity <block-height>')
    .description('Get hash of all POW20 activity on a block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting POW20 activity hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiPow20HashOfAllActivity(
          options.opiUrl,
          parseFloat(blockHeight)
        );

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get POW20 activity hash: ${err.message}`);
        process.exit(1);
      }
    });

  // pow20-hash-of-all-current-balances
  opi
    .command('pow20-hash-of-all-current-balances')
    .description('Get hash of all current POW20 balances')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting POW20 balances hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiPow20HashOfAllCurrentBalances(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get POW20 balances hash: ${err.message}`);
        process.exit(1);
      }
    });

  // pow20-event
  opi
    .command('pow20-event <event-hash>')
    .description('Get POW20 event details by hash')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (eventHash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting POW20 event details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiPow20Event(options.opiUrl, eventHash);

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get POW20 event: ${err.message}`);
        process.exit(1);
      }
    });

  // ==================== SNS COMMANDS ====================

  // sns-block-height
  opi
    .command('sns-block-height')
    .description('Get SNS indexed block height')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting SNS block height...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiSnsBlockHeight(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get SNS block height: ${err.message}`);
        process.exit(1);
      }
    });

  // sns-hash-of-all-activity
  opi
    .command('sns-hash-of-all-activity <block-height>')
    .description('Get hash of all SNS activity on a block')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (blockHeight, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting SNS activity hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiSnsHashOfAllActivity(
          options.opiUrl,
          parseFloat(blockHeight)
        );

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get SNS activity hash: ${err.message}`);
        process.exit(1);
      }
    });

  // sns-hash-of-all-registered-names
  opi
    .command('sns-hash-of-all-registered-names')
    .description('Get hash of all registered SNS names')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting hash of all SNS names...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiSnsHashOfAllRegisteredNames(options.opiUrl);

        spinner.succeed();
        console.log(result);
      } catch (err: any) {
        error(`Failed to get SNS names hash: ${err.message}`);
        process.exit(1);
      }
    });

  // sns-info
  opi
    .command('sns-info <domain>')
    .description('Get SNS domain information')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (domain, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting SNS domain info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiSnsInfo(options.opiUrl, domain);

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get SNS info: ${err.message}`);
        process.exit(1);
      }
    });

  // sns-inscriptions-of-domain
  opi
    .command('sns-inscriptions-of-domain <domain>')
    .description('Get inscriptions for an SNS domain')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (domain, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting SNS domain inscriptions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiSnsInscriptionsOfDomain(options.opiUrl, domain);

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get SNS inscriptions: ${err.message}`);
        process.exit(1);
      }
    });

  // sns-registered-namespaces
  opi
    .command('sns-registered-namespaces')
    .description('Get all registered SNS namespaces')
    .option('--opi-url <url>', 'OPI base URL', DEFAULT_OPI_URL)
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting SNS registered namespaces...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.opiSnsRegisteredNamespaces(options.opiUrl);

        spinner.succeed();
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, globalOpts));
      } catch (err: any) {
        error(`Failed to get SNS namespaces: ${err.message}`);
        process.exit(1);
      }
    });
}
