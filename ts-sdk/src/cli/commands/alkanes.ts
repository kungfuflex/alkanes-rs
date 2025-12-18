/**
 * Alkanes command group
 * Smart contract operations for Alkanes protocol
 *
 * The CLI uses the SDK's AlkanesRpcClient via provider.alkanes for all operations.
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import {
  formatOutput,
  formatAlkaneBalances,
  formatReflectMetadata,
  success,
  error,
} from '../utils/formatting.js';
import ora from 'ora';

export function registerAlkanesCommands(program: Command): void {
  const alkanes = program.command('alkanes').description('Alkanes smart contract operations');

  // getbytecode
  alkanes
    .command('getbytecode <alkane-id>')
    .description('Get bytecode for an alkanes contract')
    .option('--block-tag <tag>', 'Block tag (e.g., "latest" or height)')
    .option('--raw', 'Output raw JSON')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting bytecode...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const bytecode = await provider.alkanes.getBytecode(alkaneId, options.blockTag);

        spinner.succeed();
        console.log(formatOutput(bytecode, { raw: options.raw }));
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
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting balance...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const balance = await provider.alkanes.getBalance(options.address);

        spinner.succeed();
        if (options.raw) {
          console.log(formatOutput(balance, { raw: true }));
        } else if (Array.isArray(balance)) {
          console.log(formatAlkaneBalances(balance));
        } else {
          console.log(formatOutput(balance, { raw: false }));
        }
      } catch (err: any) {
        error(`Failed to get balance: ${err.message}`);
        process.exit(1);
      }
    });

  // trace
  alkanes
    .command('trace <outpoint>')
    .description('Trace an alkanes transaction')
    .option('--raw', 'Output raw JSON')
    .action(async (outpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Tracing transaction...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const trace = await provider.alkanes.trace(outpoint);

        spinner.succeed();
        console.log(formatOutput(trace, { raw: options.raw }));
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
    .option('--raw', 'Output raw JSON')
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
          fuzzRanges: options.fuzzRanges,
        };

        const result = await provider.alkanes.inspect(target, config);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to inspect: ${err.message}`);
        process.exit(1);
      }
    });

  // simulate
  alkanes
    .command('simulate <contract-id>')
    .description('Simulate alkanes execution')
    .option('--inputs <json>', 'Input parameters JSON')
    .option('--context <json>', 'Execution context JSON')
    .option('--block-tag <tag>', 'Block tag')
    .option('--raw', 'Output raw JSON')
    .action(async (contractId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Simulating execution...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const contextJson = options.context || JSON.stringify({
          inputs: options.inputs ? JSON.parse(options.inputs) : [],
        });

        const result = await provider.alkanes.simulate(contractId, contextJson, options.blockTag);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to simulate: ${err.message}`);
        process.exit(1);
      }
    });

  // unwrap
  alkanes
    .command('unwrap')
    .description('Get pending unwraps')
    .option('--block-tag <tag>', 'Block tag')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pending unwraps...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes.getPendingUnwraps(options.blockTag);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get pending unwraps: ${err.message}`);
        process.exit(1);
      }
    });

  // get-all-pools
  alkanes
    .command('get-all-pools <factory-id>')
    .description('Get all pools from an AMM factory')
    .option('--raw', 'Output raw JSON')
    .action(async (factoryId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pools...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes.getAllPools(factoryId);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get pools: ${err.message}`);
        process.exit(1);
      }
    });

  // all-pools-details
  alkanes
    .command('all-pools-details <factory-id>')
    .description('Get all pools with detailed information')
    .option('--raw', 'Output raw JSON')
    .action(async (factoryId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pool details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes.getAllPoolsWithDetails(factoryId);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get pool details: ${err.message}`);
        process.exit(1);
      }
    });

  // reflect
  alkanes
    .command('reflect <alkane-id>')
    .description('Reflect alkane metadata')
    .option('--raw', 'Output raw JSON')
    .action(async (alkaneId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Reflecting alkane...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes.reflect(alkaneId);

        spinner.succeed();
        if (options.raw) {
          console.log(formatOutput(result, { raw: true }));
        } else {
          console.log(formatReflectMetadata(result));
        }
      } catch (err: any) {
        error(`Failed to reflect: ${err.message}`);
        process.exit(1);
      }
    });

  // by-address
  alkanes
    .command('by-address <address>')
    .description('Get alkanes by address')
    .option('--block-tag <tag>', 'Block tag')
    .option('--protocol-tag <tag>', 'Protocol tag')
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkanes by address...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes.getByAddress(
          address,
          options.blockTag,
          options.protocolTag ? parseInt(options.protocolTag) : undefined
        );

        spinner.succeed();
        if (options.raw) {
          console.log(formatOutput(result, { raw: true }));
        } else if (Array.isArray(result)) {
          console.log(formatAlkaneBalances(result));
        } else {
          console.log(formatOutput(result, { raw: false }));
        }
      } catch (err: any) {
        error(`Failed to get alkanes: ${err.message}`);
        process.exit(1);
      }
    });

  // by-outpoint
  alkanes
    .command('by-outpoint <outpoint>')
    .description('Get alkanes by outpoint')
    .option('--block-tag <tag>', 'Block tag')
    .option('--protocol-tag <tag>', 'Protocol tag')
    .option('--raw', 'Output raw JSON')
    .action(async (outpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting alkanes by outpoint...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes.getByOutpoint(
          outpoint,
          options.blockTag,
          options.protocolTag ? parseInt(options.protocolTag) : undefined
        );

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get alkanes: ${err.message}`);
        process.exit(1);
      }
    });

  // traceblock
  alkanes
    .command('traceblock <height>')
    .description('Trace all alkanes transactions in a block')
    .option('--raw', 'Output raw JSON')
    .action(async (height, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Tracing block...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes.traceBlock(parseInt(height));

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to trace block: ${err.message}`);
        process.exit(1);
      }
    });

  // sequence
  alkanes
    .command('sequence')
    .description('Get sequence for the current block')
    .option('--block-tag <tag>', 'Block tag')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting sequence...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes.getSequence(options.blockTag);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get sequence: ${err.message}`);
        process.exit(1);
      }
    });

  // spendables
  alkanes
    .command('spendables <address>')
    .description('Get spendable outpoints for an address')
    .option('--raw', 'Output raw JSON')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting spendables...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes.getSpendables(address);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get spendables: ${err.message}`);
        process.exit(1);
      }
    });

  // pool-details
  alkanes
    .command('pool-details <pool-id>')
    .description('Get detailed information about a specific pool')
    .option('--raw', 'Output raw JSON')
    .action(async (poolId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pool details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes.getPoolDetails(poolId);

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to get pool details: ${err.message}`);
        process.exit(1);
      }
    });

  // reflect-alkane-range
  alkanes
    .command('reflect-alkane-range <block> <start-tx> <end-tx>')
    .description('Reflect metadata for a range of alkanes in a block')
    .option('--raw', 'Output raw JSON')
    .action(async (block, startTx, endTx, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Reflecting alkane range...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanes.reflectAlkaneRange(
          parseInt(block),
          parseInt(startTx),
          parseInt(endTx)
        );

        spinner.succeed();
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to reflect alkane range: ${err.message}`);
        process.exit(1);
      }
    });

  // === STATE-CHANGING COMMANDS (require wallet) ===

  // execute (state-changing)
  alkanes
    .command('execute')
    .description('Execute an alkanes smart contract')
    .option('--contract <id>', 'Contract ID')
    .option('--inputs <json>', 'Input parameters JSON')
    .option('--target <target>', 'Target address')
    .option('--pointer <pointer>', 'Pointer value')
    .option('--refund-pointer <pointer>', 'Refund pointer')
    .option('--feeRate <rate>', 'Fee rate in sat/vB')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Executing contract...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        // Access internal provider for execute operations
        const params = {
          contractId: options.contract,
          inputs: options.inputs ? JSON.parse(options.inputs) : [],
          target: options.target,
          pointer: options.pointer ? parseInt(options.pointer) : undefined,
          refundPointer: options.refundPointer ? parseInt(options.refundPointer) : undefined,
          feeRate: options.feeRate ? parseFloat(options.feeRate) : undefined,
        };

        const result = await (provider as any)._provider.alkanesExecuteWithStrings(
          JSON.stringify(params.inputs),
          params.contractId,
          params.pointer || 0,
          params.refundPointer || 0,
          params.target || '',
          params.feeRate || 1
        );

        spinner.succeed('Contract executed');
        console.log(formatOutput(JSON.parse(result), { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to execute: ${err.message}`);
        process.exit(1);
      }
    });

  // wrap-btc (state-changing)
  alkanes
    .command('wrap-btc <amount>')
    .description('Wrap BTC to frBTC')
    .option('--feeRate <rate>', 'Fee rate in sat/vB')
    .option('--raw', 'Output raw JSON')
    .action(async (amount, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Wrapping BTC...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const params = {
          amount: parseInt(amount),
          feeRate: options.feeRate ? parseFloat(options.feeRate) : 1,
        };

        const result = await (provider as any)._provider.alkanesWrapBtc(JSON.stringify(params));

        spinner.succeed('BTC wrapped');
        console.log(formatOutput(JSON.parse(result), { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to wrap BTC: ${err.message}`);
        process.exit(1);
      }
    });

  // init-pool (state-changing)
  alkanes
    .command('init-pool')
    .description('Initialize a new AMM liquidity pool')
    .option('--token0 <id>', 'First token ID')
    .option('--token1 <id>', 'Second token ID')
    .option('--amount0 <amount>', 'Amount of first token')
    .option('--amount1 <amount>', 'Amount of second token')
    .option('--feeRate <rate>', 'Fee rate in sat/vB')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Initializing pool...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const params = {
          token0: options.token0,
          token1: options.token1,
          amount0: options.amount0,
          amount1: options.amount1,
          feeRate: options.feeRate ? parseFloat(options.feeRate) : 1,
        };

        const txid = await (provider as any)._provider.alkanesInitPool(JSON.stringify(params));

        spinner.succeed('Pool initialized');
        if (options.raw) {
          console.log(formatOutput({ txid }, { raw: true }));
        } else {
          success(`TXID: ${txid}`);
        }
      } catch (err: any) {
        error(`Failed to init pool: ${err.message}`);
        process.exit(1);
      }
    });

  // swap (state-changing)
  alkanes
    .command('swap')
    .description('Execute an AMM token swap')
    .option('--token-in <id>', 'Token to swap from')
    .option('--token-out <id>', 'Token to swap to')
    .option('--amount-in <amount>', 'Amount to swap')
    .option('--min-amount-out <amount>', 'Minimum output amount')
    .option('--feeRate <rate>', 'Fee rate in sat/vB')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Executing swap...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const params = {
          tokenIn: options.tokenIn,
          tokenOut: options.tokenOut,
          amountIn: options.amountIn,
          minAmountOut: options.minAmountOut || '0',
          feeRate: options.feeRate ? parseFloat(options.feeRate) : 1,
        };

        const txid = await (provider as any)._provider.alkanesSwap(JSON.stringify(params));

        spinner.succeed('Swap executed');
        if (options.raw) {
          console.log(formatOutput({ txid }, { raw: true }));
        } else {
          success(`TXID: ${txid}`);
        }
      } catch (err: any) {
        error(`Failed to swap: ${err.message}`);
        process.exit(1);
      }
    });

  // tx-script (state-changing)
  alkanes
    .command('tx-script')
    .description('Execute a tx-script with WASM bytecode')
    .option('--bytecode <hex>', 'WASM bytecode hex')
    .option('--feeRate <rate>', 'Fee rate in sat/vB')
    .option('--raw', 'Output raw JSON')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Executing tx-script...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const params = {
          bytecode: options.bytecode,
          feeRate: options.feeRate ? parseFloat(options.feeRate) : 1,
        };

        const result = await (provider as any)._provider.alkanesTxScript(JSON.stringify(params));

        spinner.succeed('tx-script executed');
        console.log(formatOutput(JSON.parse(result), { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to execute tx-script: ${err.message}`);
        process.exit(1);
      }
    });
}
