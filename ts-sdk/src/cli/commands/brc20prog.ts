/**
 * BRC20-Prog command group
 * Programmable BRC-20 operations
 *
 * The CLI uses the SDK's Brc20ProgClient via provider.brc20prog for all operations.
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import {
  formatOutput,
  formatBlockInfo,
  success,
  error,
  info,
} from '../utils/formatting.js';
import ora from 'ora';
import {
  resolveAddressWithProvider,
  resolveAddressesWithProvider,
  containsIdentifiers,
} from '../utils/address-resolver.js';

export function registerBrc20ProgCommands(program: Command): void {
  const brc20Prog = program.command('brc20-prog').description('Programmable BRC-20 operations');

  // balance
  brc20Prog
    .command('balance <address>')
    .description('Get balance for address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
    .option('--block <tag>', 'Block tag')
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting balance...').start();

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

        const result = await provider.brc20prog.getBalance(resolvedAddress, options.block);

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get balance: ${err.message}`);
        process.exit(1);
      }
    });

  // code
  brc20Prog
    .command('code <address>')
    .description('Get contract code. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting code...').start();

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

        const result = await provider.brc20prog.getCode(resolvedAddress);

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get code: ${err.message}`);
        process.exit(1);
      }
    });

  // block-number
  brc20Prog
    .command('block-number')
    .description('Get current block number')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block number...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.brc20prog.getBlockNumber();

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get block number: ${err.message}`);
        process.exit(1);
      }
    });

  // chain-id
  brc20Prog
    .command('chain-id')
    .description('Get chain ID')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting chain ID...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.brc20prog.getChainId();

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get chain ID: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-receipt
  brc20Prog
    .command('tx-receipt <hash>')
    .description('Get transaction receipt')
    .option('--raw', 'Output raw JSON')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction receipt...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.brc20prog.getTxReceipt(hash);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get transaction receipt: ${err.message}`);
        process.exit(1);
      }
    });

  // tx
  brc20Prog
    .command('tx <hash>')
    .description('Get transaction by hash')
    .option('--raw', 'Output raw JSON')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.brc20prog.getTx(hash);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // block
  brc20Prog
    .command('block <number>')
    .description('Get block by number')
    .option('--include-txs', 'Include full transaction objects', false)
    .option('--raw', 'Output raw JSON')
    .action(async (number, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.brc20prog.getBlock(number, options.includeTxs);

        spinner.succeed();
        if (options.raw) {
          console.log(formatOutput(result, { raw: true }));
        } else {
          console.log(formatBlockInfo(result));
        }
      } catch (err: any) {
        error(`Failed to get block: ${err.message}`);
        process.exit(1);
      }
    });

  // call
  brc20Prog
    .command('call <to> <data>')
    .description('Call contract function. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
    .option('--from <address>', 'Caller address (can be p2tr:0, p2wpkh:0, or raw address)')
    .option('--block-tag <tag>', 'Block tag (latest, pending, or number)')
    .option('--raw', 'Output raw JSON')
    .action(async (to, data, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Calling contract...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        // Resolve wallet address identifiers for --from option
        let resolvedFrom = options.from;
        if (options.from) {
          resolvedFrom = await resolveAddressWithProvider(options.from, provider, {
            walletFile: globalOpts.walletFile,
            passphrase: globalOpts.passphrase,
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });
        }

        const result = await provider.brc20prog.call(to, data, resolvedFrom, options.blockTag);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to call contract: ${err.message}`);
        process.exit(1);
      }
    });

  // estimate-gas
  brc20Prog
    .command('estimate-gas <to> <data>')
    .description('Estimate gas for transaction. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
    .option('--from <address>', 'Caller address (can be p2tr:0, p2wpkh:0, or raw address)')
    .option('--raw', 'Output raw JSON')
    .action(async (to, data, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Estimating gas...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        // Resolve wallet address identifiers for --from option
        let resolvedFrom = options.from;
        if (options.from) {
          resolvedFrom = await resolveAddressWithProvider(options.from, provider, {
            walletFile: globalOpts.walletFile,
            passphrase: globalOpts.passphrase,
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });
        }

        const result = await provider.brc20prog.estimateGas(to, data, resolvedFrom);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to estimate gas: ${err.message}`);
        process.exit(1);
      }
    });

  // ============================================================================
  // FrBTC Operations (using WASM bindings)
  // ============================================================================

  // wrap-btc - Simple wrap BTC to frBTC
  brc20Prog
    .command('wrap-btc <amount>')
    .description('Wrap BTC to frBTC (simple wrap without execution)')
    .option('--from <addresses...>', 'Addresses to source UTXOs from (can be p2tr:0, p2wpkh:0, or raw addresses)')
    .option('--change <address>', 'Change address (can be p2tr:0, p2wpkh:0, or raw address)')
    .option('--fee-rate <rate>', 'Fee rate in sat/vB', parseFloat)
    .option('--use-slipstream', 'Use MARA Slipstream for broadcasting')
    .option('--use-rebar', 'Use Rebar Shield for private relay')
    .option('--rebar-tier <tier>', 'Rebar fee tier (1 or 2)', parseInt)
    .option('--resume <txid>', 'Resume from existing commit transaction')
    .option('--raw', 'Output raw JSON')
    .action(async (amount, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Wrapping BTC to frBTC...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        // Resolve wallet address identifiers
        const resolverOpts = {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        };

        const resolvedFrom = options.from
          ? await resolveAddressesWithProvider(options.from, provider, resolverOpts)
          : undefined;
        const resolvedChange = options.change
          ? await resolveAddressWithProvider(options.change, provider, resolverOpts)
          : undefined;

        const params = {
          from_addresses: resolvedFrom,
          change_address: resolvedChange,
          fee_rate: options.feeRate,
          use_slipstream: options.useSlipstream,
          use_rebar: options.useRebar,
          rebar_tier: options.rebarTier,
          resume_from_commit: options.resume,
          auto_confirm: true,
        };

        const rawProvider = provider.rawProvider;
        const result = await rawProvider.frbtcWrap(BigInt(amount), JSON.stringify(params));

        spinner.succeed('BTC wrapped to frBTC successfully!');
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to wrap BTC: ${err.message}`);
        process.exit(1);
      }
    });

  // unwrap-btc - Unwrap frBTC to BTC
  brc20Prog
    .command('unwrap-btc <amount>')
    .description('Unwrap frBTC to BTC (burns frBTC and queues BTC payment)')
    .requiredOption('--to <address>', 'Recipient address for the unwrapped BTC (can be p2tr:0, p2wpkh:0, or raw address)')
    .option('--vout <index>', 'Vout index for inscription output', parseInt, 0)
    .option('--from <addresses...>', 'Addresses to source UTXOs from (can be p2tr:0, p2wpkh:0, or raw addresses)')
    .option('--change <address>', 'Change address (can be p2tr:0, p2wpkh:0, or raw address)')
    .option('--fee-rate <rate>', 'Fee rate in sat/vB', parseFloat)
    .option('--use-slipstream', 'Use MARA Slipstream for broadcasting')
    .option('--use-rebar', 'Use Rebar Shield for private relay')
    .option('--rebar-tier <tier>', 'Rebar fee tier (1 or 2)', parseInt)
    .option('--resume <txid>', 'Resume from existing commit transaction')
    .option('--raw', 'Output raw JSON')
    .action(async (amount, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Unwrapping frBTC to BTC...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        // Resolve wallet address identifiers
        const resolverOpts = {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        };

        const resolvedTo = await resolveAddressWithProvider(options.to, provider, resolverOpts);
        const resolvedFrom = options.from
          ? await resolveAddressesWithProvider(options.from, provider, resolverOpts)
          : undefined;
        const resolvedChange = options.change
          ? await resolveAddressWithProvider(options.change, provider, resolverOpts)
          : undefined;

        const params = {
          from_addresses: resolvedFrom,
          change_address: resolvedChange,
          fee_rate: options.feeRate,
          use_slipstream: options.useSlipstream,
          use_rebar: options.useRebar,
          rebar_tier: options.rebarTier,
          resume_from_commit: options.resume,
          auto_confirm: true,
        };

        const rawProvider = provider.rawProvider;
        const result = await rawProvider.frbtcUnwrap(
          BigInt(amount),
          BigInt(options.vout || 0),
          resolvedTo,
          JSON.stringify(params)
        );

        spinner.succeed('frBTC unwrap queued successfully!');
        console.log(formatOutput(result, { raw: options.raw }));
        success(`BTC will be sent to ${resolvedTo} by the subfrost operator`);
      } catch (err: any) {
        error(`Failed to unwrap frBTC: ${err.message}`);
        process.exit(1);
      }
    });

  // wrap-and-execute - Wrap BTC and deploy+execute a script
  brc20Prog
    .command('wrap-and-execute <amount>')
    .description('Wrap BTC and deploy+execute a script (wrapAndExecute)')
    .requiredOption('--script <bytecode>', 'Script bytecode to deploy and execute (hex)')
    .option('--from <addresses...>', 'Addresses to source UTXOs from (can be p2tr:0, p2wpkh:0, or raw addresses)')
    .option('--change <address>', 'Change address (can be p2tr:0, p2wpkh:0, or raw address)')
    .option('--fee-rate <rate>', 'Fee rate in sat/vB', parseFloat)
    .option('--use-slipstream', 'Use MARA Slipstream for broadcasting')
    .option('--use-rebar', 'Use Rebar Shield for private relay')
    .option('--rebar-tier <tier>', 'Rebar fee tier (1 or 2)', parseInt)
    .option('--resume <txid>', 'Resume from existing commit transaction')
    .option('--raw', 'Output raw JSON')
    .action(async (amount, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Wrapping BTC and executing script...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        // Resolve wallet address identifiers
        const resolverOpts = {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        };

        const resolvedFrom = options.from
          ? await resolveAddressesWithProvider(options.from, provider, resolverOpts)
          : undefined;
        const resolvedChange = options.change
          ? await resolveAddressWithProvider(options.change, provider, resolverOpts)
          : undefined;

        const params = {
          from_addresses: resolvedFrom,
          change_address: resolvedChange,
          fee_rate: options.feeRate,
          use_slipstream: options.useSlipstream,
          use_rebar: options.useRebar,
          rebar_tier: options.rebarTier,
          resume_from_commit: options.resume,
          auto_confirm: true,
        };

        const rawProvider = provider.rawProvider;
        const result = await rawProvider.frbtcWrapAndExecute(
          BigInt(amount),
          options.script,
          JSON.stringify(params)
        );

        spinner.succeed('BTC wrapped and script executed successfully!');
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to wrap and execute: ${err.message}`);
        process.exit(1);
      }
    });

  // wrap-and-execute2 - Wrap BTC and call an existing contract
  brc20Prog
    .command('wrap-and-execute2 <amount>')
    .description('Wrap BTC and call an existing contract (wrapAndExecute2)')
    .requiredOption('--target <address>', 'Target contract address')
    .requiredOption('--signature <sig>', 'Function signature (e.g., "deposit()")')
    .option('--calldata <args>', 'Comma-separated calldata arguments', '')
    .option('--from <addresses...>', 'Addresses to source UTXOs from (can be p2tr:0, p2wpkh:0, or raw addresses)')
    .option('--change <address>', 'Change address (can be p2tr:0, p2wpkh:0, or raw address)')
    .option('--fee-rate <rate>', 'Fee rate in sat/vB', parseFloat)
    .option('--use-slipstream', 'Use MARA Slipstream for broadcasting')
    .option('--use-rebar', 'Use Rebar Shield for private relay')
    .option('--rebar-tier <tier>', 'Rebar fee tier (1 or 2)', parseInt)
    .option('--resume <txid>', 'Resume from existing commit transaction')
    .option('--raw', 'Output raw JSON')
    .action(async (amount, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Wrapping BTC and calling contract...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        // Resolve wallet address identifiers
        const resolverOpts = {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        };

        const resolvedFrom = options.from
          ? await resolveAddressesWithProvider(options.from, provider, resolverOpts)
          : undefined;
        const resolvedChange = options.change
          ? await resolveAddressWithProvider(options.change, provider, resolverOpts)
          : undefined;

        const params = {
          from_addresses: resolvedFrom,
          change_address: resolvedChange,
          fee_rate: options.feeRate,
          use_slipstream: options.useSlipstream,
          use_rebar: options.useRebar,
          rebar_tier: options.rebarTier,
          resume_from_commit: options.resume,
          auto_confirm: true,
        };

        const rawProvider = provider.rawProvider;
        const result = await rawProvider.frbtcWrapAndExecute2(
          BigInt(amount),
          options.target,
          options.signature,
          options.calldata || '',
          JSON.stringify(params)
        );

        spinner.succeed('BTC wrapped and contract called successfully!');
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to wrap and execute2: ${err.message}`);
        process.exit(1);
      }
    });

  // signer-address - Get the FrBTC signer address
  brc20Prog
    .command('signer-address')
    .description('Get the FrBTC signer address for the current network')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting FrBTC signer address...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const rawProvider = provider.rawProvider;
        const signerAddress = await rawProvider.frbtcGetSignerAddress();

        spinner.succeed('FrBTC signer address retrieved!');

        if (options.raw) {
          console.log(formatOutput({ signer_address: signerAddress }, { raw: true }));
        } else {
          console.log(`FrBTC Signer Address`);
          console.log(`   Network: ${globalOpts.provider || 'mainnet'}`);
          console.log(`   Signer Address: ${signerAddress}`);
        }
      } catch (err: any) {
        error(`Failed to get signer address: ${err.message}`);
        process.exit(1);
      }
    });
}
