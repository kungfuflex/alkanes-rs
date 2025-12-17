/**
 * BRC20-Prog command group
 * Programmable BRC-20 operations (EVM-compatible)
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error } from '../utils/formatting.js';
import ora from 'ora';

export function registerBrc20ProgCommands(program: Command): void {
  const brc20prog = program.command('brc20-prog').description('Programmable BRC-20 operations');

  // balance
  brc20prog
    .command('balance <address>')
    .description('Get balance for address')
    .option('--block <tag>', 'Block tag')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting balance...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.brc20prog_get_balance_js(
          address,
          options.block || null
        );
        const balance = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(balance, globalOpts));
      } catch (err: any) {
        error(`Failed to get balance: ${err.message}`);
        process.exit(1);
      }
    });

  // code
  brc20prog
    .command('code <address>')
    .description('Get contract code')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting code...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.brc20prog_get_code_js(address);
        const code = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(code, globalOpts));
      } catch (err: any) {
        error(`Failed to get code: ${err.message}`);
        process.exit(1);
      }
    });

  // block-number
  brc20prog
    .command('block-number')
    .description('Get current block number')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block number...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.brc20prog_block_number_js();
        const blockNumber = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(blockNumber, globalOpts));
      } catch (err: any) {
        error(`Failed to get block number: ${err.message}`);
        process.exit(1);
      }
    });

  // chain-id
  brc20prog
    .command('chain-id')
    .description('Get chain ID')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting chain ID...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.brc20prog_chain_id_js();
        const chainId = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(chainId, globalOpts));
      } catch (err: any) {
        error(`Failed to get chain ID: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-receipt
  brc20prog
    .command('tx-receipt <hash>')
    .description('Get transaction receipt')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction receipt...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.brc20prog_get_transaction_receipt_js(hash);
        const receipt = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(receipt, globalOpts));
      } catch (err: any) {
        error(`Failed to get transaction receipt: ${err.message}`);
        process.exit(1);
      }
    });

  // tx
  brc20prog
    .command('tx <hash>')
    .description('Get transaction by hash')
    .action(async (hash, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.brc20prog_get_transaction_by_hash_js(hash);
        const tx = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(tx, globalOpts));
      } catch (err: any) {
        error(`Failed to get transaction: ${err.message}`);
        process.exit(1);
      }
    });

  // block
  brc20prog
    .command('block <number>')
    .description('Get block by number')
    .option('--full', 'Include full transactions', false)
    .action(async (number, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.brc20prog_get_block_by_number_js(
          number,
          options.full
        );
        const block = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(block, globalOpts));
      } catch (err: any) {
        error(`Failed to get block: ${err.message}`);
        process.exit(1);
      }
    });

  // call
  brc20prog
    .command('call <to> <data>')
    .description('Call contract function')
    .option('--block <tag>', 'Block tag')
    .action(async (to, data, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Calling contract...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.brc20prog_call_js(
          to,
          data,
          options.block || null
        );
        const output = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(output, globalOpts));
      } catch (err: any) {
        error(`Failed to call contract: ${err.message}`);
        process.exit(1);
      }
    });

  // estimate-gas
  brc20prog
    .command('estimate-gas <to> <data>')
    .description('Estimate gas for transaction')
    .option('--block <tag>', 'Block tag')
    .action(async (to, data, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Estimating gas...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
        });

        const result = await provider.brc20prog_estimate_gas_js(
          to,
          data,
          options.block || null
        );
        const gas = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(gas, globalOpts));
      } catch (err: any) {
        error(`Failed to estimate gas: ${err.message}`);
        process.exit(1);
      }
    });
}
