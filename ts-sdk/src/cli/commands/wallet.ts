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
import {
  resolveAddressWithProvider,
  resolveAddressesWithProvider,
  containsIdentifiers,
} from '../utils/address-resolver.js';

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
          // Use rawProvider to access low-level wallet methods
          const rawProvider = provider.rawProvider;

          // walletCreate takes (mnemonic?, passphrase?) and returns sync result
          // The passphrase is used for BIP39 passphrase (optional seed extension)
          const walletInfo = rawProvider.walletCreate(
            mnemonic || undefined,
            passphrase
          );

          // Save wallet data to file
          saveWalletFile(walletPath, {
            mnemonic: walletInfo.mnemonic,
            network: globalOpts.provider || 'mainnet',
            created_at: new Date().toISOString(),
          });

          spinner.succeed('Wallet created successfully!');

          // Display wallet info
          console.log();
          success(`Wallet saved to: ${walletPath}`);
          info(`Network: ${walletInfo.network || globalOpts.provider || 'mainnet'}`);
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

          const rawProvider = provider.rawProvider;

          // Load wallet with mnemonic from saved wallet file
          const walletData = loadWalletFile(walletPath);
          if (!walletData || !walletData.mnemonic) {
            spinner.fail();
            error('Failed to load wallet or wallet has no mnemonic');
            return;
          }

          // Load the mnemonic into the provider (passphrase is for BIP39 seed extension)
          rawProvider.walletLoadMnemonic(walletData.mnemonic, passphrase);
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

          // Get addresses using the Keystore.get_addresses method via WASM
          const startIndex = indices[0];
          const count = indices.length;
          const addresses = rawProvider.walletGetAddresses(addressType, startIndex, count);

          console.log();
          const table = createTable(['Index', 'Address Type', 'Derivation Path', 'Address']);
          for (const addr of addresses) {
            table.push([String(addr.index), addr.script_type, addr.derivation_path, addr.address]);
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

          const rawProvider = provider.rawProvider;

          // Load wallet with mnemonic from saved wallet file
          const walletData = loadWalletFile(walletPath);
          if (!walletData || !walletData.mnemonic) {
            spinner.fail();
            error('Failed to load wallet or wallet has no mnemonic');
            return;
          }

          // Load the mnemonic into the provider
          rawProvider.walletLoadMnemonic(walletData.mnemonic, passphrase);

          // Get UTXOs via walletGetUtxos
          const utxos = await rawProvider.walletGetUtxos();

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

          const rawProvider = provider.rawProvider;

          // Load wallet with mnemonic from saved wallet file
          const walletData = loadWalletFile(walletPath);
          if (!walletData || !walletData.mnemonic) {
            spinner.fail();
            error('Failed to load wallet or wallet has no mnemonic');
            return;
          }

          // Load the mnemonic into the provider
          rawProvider.walletLoadMnemonic(walletData.mnemonic, passphrase);

          // Get balance via WASM - walletGetBalance returns {confirmed, pending}
          const balance = await rawProvider.walletGetBalance();

          spinner.succeed('Balance calculated');

          console.log();
          const total = (balance.confirmed || 0) + (balance.pending || 0);
          success(`Total Balance: ${formatBTC(total)}`);
          info(`Confirmed: ${formatBTC(balance.confirmed || 0)}`);
          if (balance.pending && balance.pending > 0) {
            info(`Pending: ${formatBTC(balance.pending)}`);
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
    .description('Send BTC to an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
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

        const spinner = ora('Loading wallet...').start();

        try {
          // Create provider and load wallet
          const provider = await createProvider({
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          const rawProvider = provider.rawProvider;

          // Load wallet with mnemonic from saved wallet file
          const walletData = loadWalletFile(walletPath);
          if (!walletData || !walletData.mnemonic) {
            spinner.fail();
            error('Failed to load wallet or wallet has no mnemonic');
            return;
          }

          // Load the mnemonic into the provider
          rawProvider.walletLoadMnemonic(walletData.mnemonic, passphrase);

          // Resolve address identifiers
          const resolvedAddress = await resolveAddressWithProvider(address, provider, {
            walletFile: globalOpts.walletFile,
            passphrase,
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });

          // Resolve from addresses if specified
          const resolvedFrom = options.from
            ? await resolveAddressesWithProvider([options.from], provider, {
                walletFile: globalOpts.walletFile,
                passphrase,
                network: globalOpts.provider,
                jsonrpcUrl: globalOpts.jsonrpcUrl,
              })
            : undefined;

          spinner.stop();

          // Confirm transaction
          if (!globalOpts.autoConfirm) {
            console.log();
            info(`Sending ${amount} BTC to ${resolvedAddress}`);
            if (address !== resolvedAddress) {
              info(`  (resolved from ${address})`);
            }
            info(`Fee rate: ${options.feeRate} sats/vB`);
            const confirmed = await confirm('Proceed with transaction?', false);
            if (!confirmed) {
              info('Transaction cancelled');
              return;
            }
          }

          spinner.start('Creating and broadcasting transaction...');

          // Send transaction via WASM - walletSend takes JSON params
          const sendParams = {
            address: resolvedAddress,
            amount: Math.round(parseFloat(amount) * 100_000_000),  // Convert BTC to satoshis
            fee_rate: parseFloat(options.feeRate),
            from: resolvedFrom,
          };

          const txid = await rawProvider.walletSend(JSON.stringify(sendParams));

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

          const rawProvider = provider.rawProvider;

          // Load wallet with mnemonic from saved wallet file
          const walletData = loadWalletFile(walletPath);
          if (!walletData || !walletData.mnemonic) {
            spinner.fail();
            error('Failed to load wallet or wallet has no mnemonic');
            return;
          }

          // Load the mnemonic into the provider
          rawProvider.walletLoadMnemonic(walletData.mnemonic, passphrase);

          // Get history via walletGetHistory
          const history = await rawProvider.walletGetHistory(options.address);

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

          spinner.fail();
          error('PSBT signing is not yet available in the WASM CLI');
          info('Use a full node wallet for PSBT operations');
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

          spinner.fail();
          error('UTXO freezing is not yet available in the WASM CLI');
          info('Use a full node wallet for UTXO management');
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

          spinner.fail();
          error('UTXO unfreezing is not yet available in the WASM CLI');
          info('Use a full node wallet for UTXO management');
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

          spinner.fail();
          error('Transaction creation (PSBT) is not yet available in the WASM CLI');
          info('Use walletSend for direct transactions or a full node wallet for PSBT operations');
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

          spinner.fail();
          error('Transaction signing is not yet available in the WASM CLI');
          info('Use walletSend for direct transactions or a full node wallet for signing');
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

          const decoded = await provider.rawProvider.bitcoindDecodeRawTransaction(txHex);

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

          const txid = await provider.rawProvider.bitcoindSendRawTransaction(txHex);

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

        error('Fee estimation is not yet available in the WASM CLI');
        info('Use esplora fee-estimates for current network fee rates');
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

          const rates = await provider.rawProvider.esploraGetFeeEstimates();

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

        error('Wallet sync is not yet available in the WASM CLI');
        info('The WASM wallet syncs automatically when querying balance/UTXOs');
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

        // Simply load from file - the wallet file stores the mnemonic
        const walletData = loadWalletFile(walletPath);
        if (!walletData || !walletData.mnemonic) {
          error('Failed to load wallet or wallet has no mnemonic');
          return;
        }

        console.log();
        console.log(chalk.yellow.bold('⚠ WARNING: Keep this mnemonic safe and private!'));
        console.log();
        console.log(chalk.cyan(walletData.mnemonic));
        console.log();
      } catch (err: any) {
        error(`Failed to get mnemonic: ${err.message}`);
        process.exit(1);
      }
    });
}
