/**
 * Alkanes command group
 * Smart contract operations for Alkanes protocol
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error } from '../utils/formatting.js';
import ora from 'ora';

export function registerAlkanesCommands(program: Command): void {
  const alkanes = program.command('alkanes').description('Alkanes smart contract operations');

  // getbytecode
  alkanes
    .command('getbytecode <alkane-id>')
    .description('Get bytecode for an alkanes contract')
    .option('--block-tag <tag>', 'Block tag (e.g., "latest" or height)')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting bytecode...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes_bytecode_js(alkaneId, options.blockTag || null);
        const bytecode = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(bytecode, globalOpts));
      } catch (err: any) {
        error(`Failed to get bytecode: ${err.message}`);
        process.exit(1);
      }
    });

  // balance
  alkanes
    .command('balance')
    .description('Get alkanes balance for an address')
    .option('--address <address>', 'Address to check (defaults to wallet)')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting balance...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes_balance_js(options.address || null);
        const balance = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(balance, globalOpts));
      } catch (err: any) {
        error(`Failed to get balance: ${err.message}`);
        process.exit(1);
      }
    });

  // trace
  alkanes
    .command('trace <outpoint>')
    .description('Trace an alkanes transaction')
    .action(async (outpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Tracing transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes_trace_js(outpoint);
        const trace = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(trace, globalOpts));
      } catch (err: any) {
        error(`Failed to trace: ${err.message}`);
        process.exit(1);
      }
    });

  // inspect
  alkanes
    .command('inspect <target>')
    .description('Inspect alkanes bytecode')
    .option('--disasm', 'Enable disassembly to WAT format', false)
    .option('--fuzz', 'Enable fuzzing analysis', false)
    .option('--fuzz-ranges <ranges>', 'Opcode ranges for fuzzing')
    .option('--meta', 'Extract and display metadata', false)
    .option('--codehash', 'Compute and display codehash', false)
    .action(async (target, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Inspecting bytecode...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const config = {
          disasm: options.disasm,
          fuzz: options.fuzz,
          fuzz_ranges: options.fuzzRanges || null,
          meta: options.meta,
          codehash: options.codehash,
        };

        const result = await provider.alkanes_inspect_js(target, config);
        const inspection = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(inspection, globalOpts));
      } catch (err: any) {
        error(`Failed to inspect: ${err.message}`);
        process.exit(1);
      }
    });

  // simulate
  alkanes
    .command('simulate <contract-id>')
    .description('Simulate alkanes execution')
    .option('--params <params>', 'Calldata params (format: [block,tx,inputs...]:[block:tx:value])')
    .option('--block-hex <hex>', 'Block hex')
    .option('--transaction-hex <hex>', 'Transaction hex')
    .option('--block-tag <tag>', 'Block tag')
    .action(async (contractId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Simulating execution...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        // Build context JSON
        const context = {
          params: options.params || null,
          block_hex: options.blockHex || null,
          transaction_hex: options.transactionHex || null,
        };

        const result = await provider.alkanes_simulate_js(
          contractId,
          JSON.stringify(context),
          options.blockTag || null
        );
        const simulation = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(simulation, globalOpts));
      } catch (err: any) {
        error(`Failed to simulate: ${err.message}`);
        process.exit(1);
      }
    });

  // unwrap (get pending unwraps)
  alkanes
    .command('unwrap')
    .description('Get pending unwraps')
    .option('--block-tag <tag>', 'Block tag')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pending unwraps...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes_pending_unwraps_js(options.blockTag || null);
        const unwraps = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(unwraps, globalOpts));
      } catch (err: any) {
        error(`Failed to get unwraps: ${err.message}`);
        process.exit(1);
      }
    });

  // get-all-pools
  alkanes
    .command('get-all-pools <factory-id>')
    .description('Get all pools from an AMM factory')
    .action(async (factoryId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting all pools...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes_get_all_pools_js(factoryId);
        const pools = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(pools, globalOpts));
      } catch (err: any) {
        error(`Failed to get pools: ${err.message}`);
        process.exit(1);
      }
    });

  // all-pools-details
  alkanes
    .command('all-pools-details <factory-id>')
    .description('Get all pools with detailed information')
    .action(async (factoryId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pool details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes_get_all_pools_with_details_js(
          factoryId,
          null // protocol_tag
        );
        const details = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(details, globalOpts));
      } catch (err: any) {
        error(`Failed to get pool details: ${err.message}`);
        process.exit(1);
      }
    });

  // reflect
  alkanes
    .command('reflect <alkane-id>')
    .description('Reflect alkane metadata')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Reflecting alkane...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes_reflect_js(alkaneId);
        const metadata = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(metadata, globalOpts));
      } catch (err: any) {
        error(`Failed to reflect alkane: ${err.message}`);
        process.exit(1);
      }
    });

  // by-address
  alkanes
    .command('by-address <address>')
    .description('Get alkanes by address')
    .option('--block-tag <tag>', 'Block tag')
    .option('--protocol-tag <tag>', 'Protocol tag', '0')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkanes by address...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const protocolTag = options.protocolTag ? parseFloat(options.protocolTag) : null;
        const result = await provider.alkanes_by_address_js(
          address,
          options.blockTag || null,
          protocolTag
        );
        const alkanes = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(alkanes, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkanes by address: ${err.message}`);
        process.exit(1);
      }
    });

  // by-outpoint
  alkanes
    .command('by-outpoint <outpoint>')
    .description('Get alkanes by outpoint')
    .option('--block-tag <tag>', 'Block tag')
    .option('--protocol-tag <tag>', 'Protocol tag', '0')
    .action(async (outpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkanes by outpoint...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const protocolTag = options.protocolTag ? parseFloat(options.protocolTag) : null;
        const result = await provider.alkanes_by_outpoint_js(
          outpoint,
          options.blockTag || null,
          protocolTag
        );
        const alkanes = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(alkanes, globalOpts));
      } catch (err: any) {
        error(`Failed to get alkanes by outpoint: ${err.message}`);
        process.exit(1);
      }
    });

  // traceblock
  alkanes
    .command('traceblock <height>')
    .description('Trace all alkanes transactions in a block')
    .action(async (height, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Tracing block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.traceBlock(parseFloat(height));
        const trace = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(trace, globalOpts));
      } catch (err: any) {
        error(`Failed to trace block: ${err.message}`);
        process.exit(1);
      }
    });

  // sequence
  alkanes
    .command('sequence')
    .description('Get sequence for the current block')
    .option('--block-tag <tag>', 'Block tag (e.g., "latest" or block height)')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting sequence...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanesSequence(options.blockTag || null);
        const sequence = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(sequence, globalOpts));
      } catch (err: any) {
        error(`Failed to get sequence: ${err.message}`);
        process.exit(1);
      }
    });

  // spendables
  alkanes
    .command('spendables <address>')
    .description('Get spendable outpoints for an address')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting spendables...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanesSpendables(address);
        const spendables = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(spendables, globalOpts));
      } catch (err: any) {
        error(`Failed to get spendables: ${err.message}`);
        process.exit(1);
      }
    });

  // Note: The following commands require complex transaction building
  // or are not available in alkanes-cli-common:
  // - execute: Requires EnhancedAlkanesExecutor and transaction construction
  // - wrap-btc: Requires BTC wrapping logic
  // - init-pool, swap: Require transaction construction
  // - backtest: Only available in alkanes-cli (not in alkanes-cli-common)
  // - pool-details: Use dataapi get-pool-by-id instead
  // - reflect-alkane-range: Not available as a provider method
  //
  // These will be implemented in future iterations if provider methods become available
}
