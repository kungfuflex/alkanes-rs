/**
 * Wallet command group
 * Implements wallet management operations
 */

import { Command } from 'commander';
import chalk from 'chalk';
import { createProvider } from '../utils/provider.js';
import { expandPath } from '../utils/config.js';
import { walletExists, saveWalletFile, loadWalletFile, isValidMnemonic } from '../utils/wallet.js';
import { success, error, info, formatOutput, createTable, formatAddress, formatBTC } from '../utils/formatting.js';
import { confirm, password as promptPassword, input } from '../utils/prompts.js';
import ora from 'ora';

export function registerWalletCommands(program: Command): void {
  const wallet = program.command('wallet').description('Wallet management operations');

  // wallet create
  wallet
    .command('create')
    .description('Create a new wallet')
    .option('--mnemonic <phrase>', 'Restore from mnemonic phrase (12-24 words)')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        // Check if wallet already exists
        if (walletExists(walletPath)) {
          error(`Wallet already exists at ${walletPath}`);
          const overwrite = await confirm('Do you want to overwrite it?', false);
          if (!overwrite) {
            info('Wallet creation cancelled');
            return;
          }
        }

        // Get passphrase
        const passphrase = globalOpts.passphrase || await promptPassword('Enter passphrase to encrypt wallet:');
        const passphraseConfirm = globalOpts.passphrase || await promptPassword('Confirm passphrase:');

        if (passphrase !== passphraseConfirm) {
          error('Passphrases do not match');
          return;
        }

        const spinner = ora('Creating wallet...').start();

        try {
          // Create provider
          const provider = await createProvider({
            network: globalOpts.provider || 'mainnet',
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          // Create wallet using WASM provider
          let mnemonic = options.mnemonic;

          if (mnemonic) {
            // Validate mnemonic
            if (!isValidMnemonic(mnemonic)) {
              spinner.fail();
              error('Invalid mnemonic phrase. Must be 12, 15, 18, 21, or 24 words');
              return;
            }
          }

          // Create wallet via WASM
          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          const result = provider.wallet_create_js(
            JSON.stringify(walletConfig),
            mnemonic || undefined,
            passphrase
          );

          // Wait for promise to resolve
          const walletInfo = await result;

          spinner.succeed('Wallet created successfully!');

          // Display wallet info
          console.log();
          success(`Wallet saved to: ${walletPath}`);
          info(`Network: ${walletInfo.network}`);
          info(`First address (p2tr:0): ${walletInfo.address}`);

          if (walletInfo.mnemonic && !options.mnemonic) {
            console.log();
            console.log(chalk.yellow.bold('⚠ IMPORTANT: Write down your recovery phrase!'));
            console.log();
            console.log(chalk.cyan(walletInfo.mnemonic));
            console.log();
            console.log(chalk.yellow('Keep this phrase safe. It\'s the only way to recover your wallet.'));
          }
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to create wallet: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet addresses
  wallet
    .command('addresses <spec>')
    .description('Get addresses from wallet (e.g., p2tr:0-10, p2wpkh:0)')
    .action(async (spec, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          info('Create a wallet first with: alkanes-cli wallet create');
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');

        const spinner = ora('Loading wallet...').start();

        try {
          // Create provider and load wallet
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
          spinner.succeed('Wallet loaded');

          // Parse the spec to get address type and range
          const [addressType, range] = spec.split(':');

          if (!addressType || !range) {
            error('Invalid address spec format. Use: <type>:<range> (e.g., p2tr:0-10 or p2wpkh:5)');
            return;
          }

          let indices: number[] = [];

          if (range.includes('-')) {
            const [start, end] = range.split('-').map(Number);
            for (let i = start; i <= end; i++) {
              indices.push(i);
            }
          } else {
            indices.push(Number(range));
          }

          // Get addresses
          console.log();
          const table = createTable(['Index', 'Address Type', 'Address']);

          for (const index of indices) {
            const addr = await provider.get_address(addressType, index);
            table.push([String(index), addressType, addr]);
          }

          console.log(table.toString());
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to get addresses: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet utxos
  wallet
    .command('utxos <spec>')
    .description('Get UTXOs for addresses (e.g., p2tr:0-5)')
    .action(async (spec, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');

        const spinner = ora('Loading wallet and fetching UTXOs...').start();

        try {
          // Create provider and load wallet
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
            esploraUrl: globalOpts.esploraUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);

          // Get UTXOs via the UtxoProvider trait we implemented
          const utxos_result = await provider.get_utxos_by_spec_js([spec]);
          const utxos = JSON.parse(utxos_result);

          spinner.succeed(`Found ${utxos.length} UTXOs`);

          if (utxos.length === 0) {
            info('No UTXOs found for the specified addresses');
            return;
          }

          // Display UTXOs
          console.log();
          const table = createTable(['Outpoint', 'Amount (BTC)', 'Address']);

          let totalAmount = 0;

          for (const utxo of utxos) {
            table.push([
              `${utxo.txid.slice(0, 8)}...${utxo.txid.slice(-8)}:${utxo.vout}`,
              formatBTC(utxo.amount),
              formatAddress(utxo.address, 30),
            ]);
            totalAmount += utxo.amount;
          }

          console.log(table.toString());
          console.log();
          success(`Total: ${formatBTC(totalAmount)}`);
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to get UTXOs: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet balance
  wallet
    .command('balance')
    .description('Get wallet balance')
    .option('--address <spec>', 'Get balance for specific addresses (e.g., p2tr:0-5)')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');

        const spinner = ora('Calculating balance...').start();

        try {
          // Create provider and load wallet
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
            esploraUrl: globalOpts.esploraUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);

          // Get balance via WASM
          const balance_result = await provider.wallet_get_balance_js(options.address);
          const balance = JSON.parse(balance_result);

          spinner.succeed('Balance calculated');

          console.log();
          success(`Total Balance: ${formatBTC(balance.total || 0)}`);

          if (balance.confirmed !== undefined) {
            info(`Confirmed: ${formatBTC(balance.confirmed)}`);
          }
          if (balance.unconfirmed !== undefined && balance.unconfirmed > 0) {
            info(`Unconfirmed: ${formatBTC(balance.unconfirmed)}`);
          }
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to get balance: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet send
  wallet
    .command('send <address> <amount>')
    .description('Send BTC to an address')
    .option('--fee-rate <sats/vB>', 'Fee rate in satoshis per virtual byte', '1')
    .option('--from <spec>', 'Source addresses (e.g., p2tr:0-5)')
    .action(async (address, amount, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');

        // Confirm transaction
        if (!globalOpts.autoConfirm) {
          console.log();
          info(`Sending ${amount} BTC to ${address}`);
          info(`Fee rate: ${options.feeRate} sats/vB`);
          const confirmed = await confirm('Proceed with transaction?', false);
          if (!confirmed) {
            info('Transaction cancelled');
            return;
          }
        }

        const spinner = ora('Creating and broadcasting transaction...').start();

        try {
          // Create provider and load wallet
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);

          // Send transaction via WASM
          const sendParams = {
            to_address: address,
            amount: parseFloat(amount) * 100_000_000,  // Convert BTC to satoshis
            fee_rate: parseFloat(options.feeRate),
            from: options.from,
          };

          const txid_result = await provider.wallet_send_js(JSON.stringify(sendParams));
          const txid = JSON.parse(txid_result);

          spinner.succeed('Transaction broadcast successfully!');

          console.log();
          success(`Transaction ID: ${txid}`);
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to send transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet history
  wallet
    .command('history')
    .description('Get transaction history')
    .option('--count <n>', 'Number of transactions to fetch', '10')
    .option('--address <spec>', 'Filter by address spec')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');
        const spinner = ora('Fetching transaction history...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);

          const history_result = await provider.wallet_get_history_js(
            options.address,
            parseInt(options.count)
          );
          const history = JSON.parse(history_result);

          spinner.succeed('Transaction history fetched');

          if (history.length === 0) {
            info('No transactions found');
            return;
          }

          console.log();
          const table = createTable(['TXID', 'Height', 'Confirmations', 'Amount']);

          for (const tx of history) {
            table.push([
              formatAddress(tx.txid, 20),
              tx.block_height || 'unconfirmed',
              tx.confirmations || 0,
              formatBTC(tx.amount || 0),
            ]);
          }

          console.log(table.toString());
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to get history: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet sign (PSBT)
  wallet
    .command('sign <psbt>')
    .description('Sign a PSBT')
    .action(async (psbt, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');
        const spinner = ora('Signing PSBT...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);

          const signed_result = await provider.wallet_sign_psbt_js(psbt);
          const signed = JSON.parse(signed_result);

          spinner.succeed('PSBT signed');

          console.log();
          success('Signed PSBT:');
          console.log(formatOutput(signed, globalOpts));
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to sign PSBT: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet freeze
  wallet
    .command('freeze <outpoint>')
    .description('Freeze a UTXO')
    .option('--reason <text>', 'Reason for freezing')
    .action(async (outpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');
        const spinner = ora('Freezing UTXO...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);

          await provider.wallet_freeze_utxo_js(outpoint, options.reason || '');

          spinner.succeed(`UTXO ${outpoint} frozen`);
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to freeze UTXO: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet unfreeze
  wallet
    .command('unfreeze <outpoint>')
    .description('Unfreeze a UTXO')
    .action(async (outpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');
        const spinner = ora('Unfreezing UTXO...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);

          await provider.wallet_unfreeze_utxo_js(outpoint);

          spinner.succeed(`UTXO ${outpoint} unfrozen`);
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to unfreeze UTXO: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet create-tx
  wallet
    .command('create-tx')
    .description('Create a transaction')
    .requiredOption('--to <address>', 'Recipient address')
    .requiredOption('--amount <satoshis>', 'Amount in satoshis')
    .option('--fee-rate <sats/vB>', 'Fee rate', '1')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');
        const spinner = ora('Creating transaction...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);

          const tx_result = await provider.wallet_create_tx_js(
            options.to,
            parseInt(options.amount),
            parseFloat(options.feeRate)
          );
          const tx = JSON.parse(tx_result);

          spinner.succeed('Transaction created');

          console.log();
          console.log(formatOutput(tx, globalOpts));
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to create transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet sign-tx
  wallet
    .command('sign-tx <tx-hex>')
    .description('Sign a transaction')
    .action(async (txHex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');
        const spinner = ora('Signing transaction...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);

          const signed_result = await provider.wallet_sign_tx_js(txHex);
          const signed = JSON.parse(signed_result);

          spinner.succeed('Transaction signed');

          console.log();
          success('Signed transaction:');
          console.log(formatOutput(signed, globalOpts));
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to sign transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet decode-tx
  wallet
    .command('decode-tx <tx-hex>')
    .description('Decode a transaction')
    .action(async (txHex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Decoding transaction...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const decoded_result = await provider.bitcoin_decoderawtransaction_js(txHex);
          const decoded = JSON.parse(decoded_result);

          spinner.succeed('Transaction decoded');

          console.log();
          console.log(formatOutput(decoded, globalOpts));
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to decode transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet broadcast-tx
  wallet
    .command('broadcast-tx <tx-hex>')
    .description('Broadcast a transaction')
    .action(async (txHex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Broadcasting transaction...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const txid_result = await provider.bitcoin_sendrawtransaction_js(txHex);
          const txid = JSON.parse(txid_result);

          spinner.succeed('Transaction broadcast');

          console.log();
          success(`TXID: ${txid}`);
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to broadcast transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet estimate-fee
  wallet
    .command('estimate-fee')
    .description('Estimate transaction fee')
    .requiredOption('--to <address>', 'Recipient address')
    .requiredOption('--amount <satoshis>', 'Amount in satoshis')
    .option('--fee-rate <sats/vB>', 'Fee rate', '1')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');
        const spinner = ora('Estimating fee...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);

          const fee_result = await provider.wallet_estimate_fee_js(
            options.to,
            parseInt(options.amount),
            parseFloat(options.feeRate)
          );
          const fee = JSON.parse(fee_result);

          spinner.succeed('Fee estimated');

          console.log();
          info(`Estimated fee: ${formatBTC(fee.fee || 0)}`);
          info(`Total: ${formatBTC((parseInt(options.amount) + (fee.fee || 0)))}`);
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to estimate fee: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet fee-rates
  wallet
    .command('fee-rates')
    .description('Get current fee rates')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting fee rates...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
            esploraUrl: globalOpts.esploraUrl,
          });

          const rates_result = await provider.esplora_get_fee_estimates_js();
          const rates = JSON.parse(rates_result);

          spinner.succeed('Fee rates fetched');

          console.log();
          console.log(formatOutput(rates, globalOpts));
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to get fee rates: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet sync
  wallet
    .command('sync')
    .description('Sync wallet with blockchain')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');
        const spinner = ora('Syncing wallet...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
          await provider.wallet_sync_js();

          spinner.succeed('Wallet synced');
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to sync wallet: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet backup
  wallet
    .command('backup <output-path>')
    .description('Backup wallet')
    .action(async (outputPath, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const spinner = ora('Backing up wallet...').start();

        try {
          const fs = await import('fs');
          const expandedOutput = expandPath(outputPath);

          fs.copyFileSync(walletPath, expandedOutput);

          spinner.succeed(`Wallet backed up to ${expandedOutput}`);
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to backup wallet: ${err.message}`);
        process.exit(1);
      }
    });

  // wallet mnemonic
  wallet
    .command('mnemonic')
    .description('Get wallet mnemonic')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');

        if (!walletExists(walletPath)) {
          error(`Wallet not found at ${walletPath}`);
          return;
        }

        const passphrase = globalOpts.passphrase || await promptPassword('Enter wallet passphrase:');
        const spinner = ora('Getting mnemonic...').start();

        try {
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const walletConfig = {
            wallet_path: walletPath,
            network: globalOpts.provider || 'mainnet',
          };

          await provider.wallet_load_js(JSON.stringify(walletConfig), passphrase);
          const mnemonic_result = await provider.wallet_get_mnemonic_js();
          const mnemonic = JSON.parse(mnemonic_result);

          spinner.succeed('Mnemonic retrieved');

          console.log();
          console.log(chalk.yellow.bold('⚠ WARNING: Keep this mnemonic safe and private!'));
          console.log();
          console.log(chalk.cyan(mnemonic));
          console.log();
        } catch (err: any) {
          spinner.fail();
          throw err;
        }
      } catch (err: any) {
        error(`Failed to get mnemonic: ${err.message}`);
        process.exit(1);
      }
    });
}
