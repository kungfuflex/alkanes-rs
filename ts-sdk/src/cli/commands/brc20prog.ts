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
} from '../utils/formatting.js';
import ora from 'ora';

export function registerBrc20ProgCommands(program: Command): void {
  const brc20Prog = program.command('brc20-prog').description('Programmable BRC-20 operations');

  // balance
  brc20Prog
    .command('balance <address>')
    .description('Get balance for address')
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

        const result = await provider.brc20prog.getBalance(address, options.block);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get balance: ${err.message}`);
        process.exit(1);
      }
    });

  // code
  brc20Prog
    .command('code <address>')
    .description('Get contract code')
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting code...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.brc20prog.getCode(address);

        spinner.succeed();
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
    .description('Call contract function')
    .option('--from <address>', 'Caller address')
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

        const result = await provider.brc20prog.call(to, data, options.from, options.blockTag);

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
    .description('Estimate gas for transaction')
    .option('--from <address>', 'Caller address')
    .option('--raw', 'Output raw JSON')
    .action(async (to, data, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Estimating gas...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.brc20prog.estimateGas(to, data, options.from);

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
    .option('--from <addresses...>', 'Addresses to source UTXOs from')
    .option('--change <address>', 'Change address')
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

        const { frbtc_wrap } = await import('../../wasm/alkanes_web_sys.js');

        const params = {
          from_addresses: options.from,
          change_address: options.change,
          fee_rate: options.feeRate,
          use_slipstream: options.useSlipstream,
          use_rebar: options.useRebar,
          rebar_tier: options.rebarTier,
          resume_from_commit: options.resume,
        };

        const result = await frbtc_wrap(
          globalOpts.provider || 'mainnet',
          BigInt(amount),
          JSON.stringify(params)
        );

        spinner.succeed('BTC wrapped to frBTC successfully!');
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to wrap BTC: ${err.message}`);
        process.exit(1);
      }
    });

  // unwrap-btc - Unwrap frBTC to BTC
  brc20Prog
    .command('unwrap-btc <amount>')
    .description('Unwrap frBTC to BTC (burns frBTC and queues BTC payment)')
    .requiredOption('--to <address>', 'Recipient address for the unwrapped BTC')
    .option('--vout <index>', 'Vout index for inscription output', parseInt, 0)
    .option('--from <addresses...>', 'Addresses to source UTXOs from')
    .option('--change <address>', 'Change address')
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

        const { frbtc_unwrap } = await import('../../wasm/alkanes_web_sys.js');

        const params = {
          from_addresses: options.from,
          change_address: options.change,
          fee_rate: options.feeRate,
          use_slipstream: options.useSlipstream,
          use_rebar: options.useRebar,
          rebar_tier: options.rebarTier,
          resume_from_commit: options.resume,
        };

        const result = await frbtc_unwrap(
          globalOpts.provider || 'mainnet',
          BigInt(amount),
          BigInt(options.vout || 0),
          options.to,
          JSON.stringify(params)
        );

        spinner.succeed('frBTC unwrap queued successfully!');
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, { raw: options.raw }));
        success(`BTC will be sent to ${options.to} by the subfrost operator`);
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
    .option('--from <addresses...>', 'Addresses to source UTXOs from')
    .option('--change <address>', 'Change address')
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

        const { frbtc_wrap_and_execute } = await import('../../wasm/alkanes_web_sys.js');

        const params = {
          from_addresses: options.from,
          change_address: options.change,
          fee_rate: options.feeRate,
          use_slipstream: options.useSlipstream,
          use_rebar: options.useRebar,
          rebar_tier: options.rebarTier,
          resume_from_commit: options.resume,
        };

        const result = await frbtc_wrap_and_execute(
          globalOpts.provider || 'mainnet',
          BigInt(amount),
          options.script,
          JSON.stringify(params)
        );

        spinner.succeed('BTC wrapped and script executed!');
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, { raw: options.raw }));
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
    .option('--from <addresses...>', 'Addresses to source UTXOs from')
    .option('--change <address>', 'Change address')
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

        const { frbtc_wrap_and_execute2 } = await import('../../wasm/alkanes_web_sys.js');

        const params = {
          from_addresses: options.from,
          change_address: options.change,
          fee_rate: options.feeRate,
          use_slipstream: options.useSlipstream,
          use_rebar: options.useRebar,
          rebar_tier: options.rebarTier,
          resume_from_commit: options.resume,
        };

        const result = await frbtc_wrap_and_execute2(
          globalOpts.provider || 'mainnet',
          BigInt(amount),
          options.target,
          options.signature,
          options.calldata || '',
          JSON.stringify(params)
        );

        spinner.succeed('BTC wrapped and contract called!');
        const parsed = JSON.parse(result);
        console.log(formatOutput(parsed, { raw: options.raw }));
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

        const { frbtc_get_signer_address } = await import('../../wasm/alkanes_web_sys.js');

        const result = await frbtc_get_signer_address(globalOpts.provider || 'mainnet');

        spinner.succeed('FrBTC signer address retrieved!');
        const parsed = JSON.parse(result);

        if (options.raw) {
          console.log(formatOutput(parsed, { raw: true }));
        } else {
          console.log(`🔑 FrBTC Signer Address`);
          console.log(`   Network: ${parsed.network}`);
          console.log(`   FrBTC Contract: ${parsed.frbtc_contract}`);
          console.log(`   Signer Address: ${parsed.signer_address}`);
        }
      } catch (err: any) {
        error(`Failed to get signer address: ${err.message}`);
        process.exit(1);
      }
    });
}
