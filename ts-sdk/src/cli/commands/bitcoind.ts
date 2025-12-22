/**
 * Bitcoind command group
 * Bitcoin Core RPC operations
 *
 * The CLI uses the SDK's BitcoinRpcClient via provider.bitcoin for all operations.
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import {
  formatOutput,
  formatBlockchainInfo,
  formatBlockInfo,
  success,
  error,
  info,
} from '../utils/formatting.js';
import ora from 'ora';
import {
  containsIdentifiers,
  createAddressResolver,
  resolveAddress,
} from '../utils/address-resolver.js';
import { expandPath } from '../utils/config.js';
import { walletExists, loadWalletFile } from '../utils/wallet.js';

export function registerBitcoindCommands(program: Command): void {
  const bitcoind = program.command('bitcoind').description('Bitcoin Core RPC commands');

  // getblockcount
  bitcoind
    .command('getblockcount')
    .description('Get current block count')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block count...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const blockCount = await provider.bitcoin.getBlockCount();

        spinner.succeed();
        console.log(formatOutput(blockCount, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block count: ${err.message}`);
        process.exit(1);
      }
    });

  // generatetoaddress
  bitcoind
    .command('generatetoaddress <nblocks> <address>')
    .description('Generate blocks to an address (regtest only). Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
    .option('--raw', 'Output raw JSON')
    .action(async (nblocks, address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora(`Generating ${nblocks} blocks...`).start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        // Resolve wallet address identifiers (e.g., p2tr:0) to actual addresses
        let resolvedAddress = address;
        if (containsIdentifiers(address)) {
          spinner.text = 'Loading wallet...';

          const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');
          if (!walletExists(walletPath)) {
            spinner.fail();
            error(`Wallet not found at ${walletPath}`);
            info('Create a wallet first with: alkanes-bindgen-cli wallet create');
            process.exit(1);
          }

          const walletData = loadWalletFile(walletPath);
          if (!walletData || !walletData.mnemonic) {
            spinner.fail();
            error('Failed to load wallet or wallet has no mnemonic');
            process.exit(1);
          }

          // Load mnemonic and resolve address
          const rawProvider = provider.rawProvider;
          rawProvider.walletLoadMnemonic(walletData.mnemonic, globalOpts.passphrase || '');

          const resolver = await createAddressResolver({
            walletFile: walletPath,
            passphrase: globalOpts.passphrase,
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          }, createProvider);

          resolvedAddress = await resolver.resolve(address);
          spinner.text = `Generating ${nblocks} blocks to ${resolvedAddress}...`;
        }

        const hashes = await provider.bitcoin.generateToAddress(parseInt(nblocks), resolvedAddress);

        spinner.succeed(`Generated ${nblocks} blocks to ${resolvedAddress}`);
        console.log(formatOutput(hashes, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to generate blocks: ${err.message}`);
        process.exit(1);
      }
    });

  // getblockchaininfo
  bitcoind
    .command('getblockchaininfo')
    .description('Get blockchain information')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting blockchain info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const info = await provider.bitcoin.getBlockchainInfo();

        spinner.succeed();
        if (options.raw) {
          console.log(formatOutput(info, { raw: true }));
        } else {
          console.log(formatBlockchainInfo(info));
        }
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
    .option('--raw', 'Output raw JSON')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const tx = await provider.bitcoin.getTransaction(txid);

        spinner.succeed();
        console.log(formatOutput(tx, { raw: options.raw }));
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
    .option('--raw', 'Output raw JSON')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const rawOutput = options.verbosity === '0';
        const block = await provider.bitcoin.getBlock(hash, rawOutput);

        spinner.succeed();
        if (options.raw) {
          console.log(formatOutput(block, { raw: true }));
        } else if (typeof block === 'object') {
          console.log(formatBlockInfo(block));
        } else {
          console.log(formatOutput(block, { raw: false }));
        }
      } catch (err: any) {
        error(`Failed to get block: ${err.message}`);
        process.exit(1);
      }
    });

  // getblockhash
  bitcoind
    .command('getblockhash <height>')
    .description('Get block hash by height')
    .option('--raw', 'Output raw JSON')
    .action(async (height, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block hash...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const hash = await provider.bitcoin.getBlockHash(parseInt(height));

        spinner.succeed();
        console.log(formatOutput(hash, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block hash: ${err.message}`);
        process.exit(1);
      }
    });

  // sendrawtransaction
  bitcoind
    .command('sendrawtransaction <hex>')
    .description('Broadcast a raw transaction')
    .option('--raw', 'Output raw JSON')
    .action(async (hex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Broadcasting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const txid = await provider.bitcoin.sendRawTransaction(hex);

        spinner.succeed('Transaction broadcast');
        if (options.raw) {
          console.log(formatOutput(txid, { raw: true }));
        } else {
          success(`TXID: ${txid}`);
        }
      } catch (err: any) {
        error(`Failed to broadcast transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // getnetworkinfo
  bitcoind
    .command('getnetworkinfo')
    .description('Get network information')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting network info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const info = await provider.bitcoin.getNetworkInfo();

        spinner.succeed();
        console.log(formatOutput(info, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get network info: ${err.message}`);
        process.exit(1);
      }
    });

  // getmempoolinfo
  bitcoind
    .command('getmempoolinfo')
    .description('Get mempool information')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting mempool info...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const info = await provider.bitcoin.getMempoolInfo();

        spinner.succeed();
        console.log(formatOutput(info, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get mempool info: ${err.message}`);
        process.exit(1);
      }
    });

  // generatefuture
  bitcoind
    .command('generatefuture <address>')
    .description('Generate a future block (regtest only). Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Generating future block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        // Resolve wallet address identifiers (e.g., p2tr:0) to actual addresses
        let resolvedAddress = address;
        if (containsIdentifiers(address)) {
          spinner.text = 'Loading wallet...';

          const walletPath = expandPath(globalOpts.walletFile || '~/.alkanes/wallet.json');
          if (!walletExists(walletPath)) {
            spinner.fail();
            error(`Wallet not found at ${walletPath}`);
            info('Create a wallet first with: alkanes-bindgen-cli wallet create');
            process.exit(1);
          }

          const walletData = loadWalletFile(walletPath);
          if (!walletData || !walletData.mnemonic) {
            spinner.fail();
            error('Failed to load wallet or wallet has no mnemonic');
            process.exit(1);
          }

          // Load mnemonic and resolve address
          const rawProvider = provider.rawProvider;
          rawProvider.walletLoadMnemonic(walletData.mnemonic, globalOpts.passphrase || '');

          const resolver = await createAddressResolver({
            walletFile: walletPath,
            passphrase: globalOpts.passphrase,
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          }, createProvider);

          resolvedAddress = await resolver.resolve(address);
          spinner.text = `Generating future block to ${resolvedAddress}...`;
        }

        const hash = await provider.bitcoin.generateFuture(resolvedAddress);

        spinner.succeed(`Future block generated to ${resolvedAddress}`);
        console.log(formatOutput(hash, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to generate future block: ${err.message}`);
        process.exit(1);
      }
    });

  // getblockheader
  bitcoind
    .command('getblockheader <hash>')
    .description('Get block header by hash')
    .option('--raw', 'Output raw JSON')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block header...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const header = await provider.bitcoin.getBlockHeader(hash);

        spinner.succeed();
        console.log(formatOutput(header, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block header: ${err.message}`);
        process.exit(1);
      }
    });

  // getblockstats
  bitcoind
    .command('getblockstats <hash>')
    .description('Get block statistics by hash')
    .option('--raw', 'Output raw JSON')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block stats...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const stats = await provider.bitcoin.getBlockStats(hash);

        spinner.succeed();
        console.log(formatOutput(stats, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block stats: ${err.message}`);
        process.exit(1);
      }
    });

  // estimatesmartfee
  bitcoind
    .command('estimatesmartfee <blocks>')
    .description('Estimate smart fee for confirmation in N blocks')
    .option('--raw', 'Output raw JSON')
    .action(async (blocks, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Estimating fee...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const estimate = await provider.bitcoin.estimateSmartFee(parseInt(blocks));

        spinner.succeed();
        console.log(formatOutput(estimate, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to estimate fee: ${err.message}`);
        process.exit(1);
      }
    });

  // getchaintips
  bitcoind
    .command('getchaintips')
    .description('Get chain tips information')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting chain tips...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const tips = await provider.bitcoin.getChainTips();

        spinner.succeed();
        console.log(formatOutput(tips, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get chain tips: ${err.message}`);
        process.exit(1);
      }
    });

  // decoderawtransaction
  bitcoind
    .command('decoderawtransaction <hex>')
    .description('Decode a raw transaction hex')
    .option('--raw', 'Output raw JSON')
    .action(async (hex, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Decoding transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const decoded = await provider.bitcoin.decodeRawTransaction(hex);

        spinner.succeed();
        console.log(formatOutput(decoded, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to decode transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // decodepsbt
  bitcoind
    .command('decodepsbt <psbt>')
    .description('Decode a PSBT (base64)')
    .option('--raw', 'Output raw JSON')
    .action(async (psbt, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Decoding PSBT...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const decoded = await provider.bitcoin.decodePsbt(psbt);

        spinner.succeed();
        console.log(formatOutput(decoded, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to decode PSBT: ${err.message}`);
        process.exit(1);
      }
    });

  // getrawmempool
  bitcoind
    .command('getrawmempool')
    .description('Get raw mempool transactions')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting mempool transactions...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const mempool = await provider.bitcoin.getRawMempool();

        spinner.succeed();
        console.log(formatOutput(mempool, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get mempool: ${err.message}`);
        process.exit(1);
      }
    });

  // gettxout
  bitcoind
    .command('gettxout <txid> <vout>')
    .description('Get transaction output details')
    .option('--include-mempool', 'Include mempool transactions', false)
    .option('--raw', 'Output raw JSON')
    .action(async (txid, vout, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction output...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const txout = await provider.bitcoin.getTxOut(txid, parseInt(vout), options.includeMempool);

        spinner.succeed();
        console.log(formatOutput(txout, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get tx out: ${err.message}`);
        process.exit(1);
      }
    });
}
