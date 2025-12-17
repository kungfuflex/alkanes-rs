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

  // execute
  alkanes
    .command('execute')
    .description('Execute an alkanes smart contract')
    .option('--inputs <requirements>', 'Input requirements (e.g., "B:10000" or "2:0:1000")')
    .option('--to <addresses>', 'Recipient addresses (JSON array)')
    .option('--from <addresses>', 'Source addresses (JSON array)', '[]')
    .option('--change <address>', 'Change address')
    .option('--protostones <spec>', 'Protostone specification')
    .option('--envelope <hex>', 'Envelope data as hex')
    .option('--fee-rate <rate>', 'Fee rate in sat/vB')
    .option('--trace', 'Enable transaction tracing')
    .option('--mine', 'Mine a block after broadcasting (regtest only)')
    .option('-y, --auto-confirm', 'Automatically confirm transaction')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Executing contract...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        // Build params object
        const params = {
          input_requirements: options.inputs || '',
          to_addresses: options.to ? JSON.parse(options.to) : [],
          from_addresses: options.from ? JSON.parse(options.from) : [],
          change_address: options.change || null,
          protostones: options.protostones || '',
          envelope_hex: options.envelope || null,
          fee_rate: options.feeRate ? parseFloat(options.feeRate) : null,
          trace_enabled: options.trace || false,
          mine_enabled: options.mine || false,
          auto_confirm: options.autoConfirm || false,
          raw_output: globalOpts.raw || false,
        };

        const result = await provider.alkanesExecuteWithStrings(
          JSON.stringify(params.to_addresses),
          params.input_requirements,
          params.protostones,
          params.fee_rate,
          params.envelope_hex,
          JSON.stringify({
            trace_enabled: params.trace_enabled,
            mine_enabled: params.mine_enabled,
            auto_confirm: params.auto_confirm,
            raw_output: params.raw_output,
          })
        );

        spinner.succeed();
        console.log(formatOutput(JSON.parse(result), globalOpts));
      } catch (err: any) {
        error(`Failed to execute: ${err.message}`);
        process.exit(1);
      }
    });

  // wrap-btc
  alkanes
    .command('wrap-btc <amount>')
    .description('Wrap BTC to frBTC')
    .option('--to <address>', 'Address to receive frBTC', 'p2tr:0')
    .option('--from <addresses>', 'Source addresses (JSON array)', '[]')
    .option('--change <address>', 'Change address')
    .option('--fee-rate <rate>', 'Fee rate in sat/vB')
    .option('--trace', 'Enable transaction tracing')
    .option('--mine', 'Mine a block after broadcasting (regtest only)')
    .option('-y, --auto-confirm', 'Automatically confirm transaction')
    .action(async (amount, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Wrapping BTC to frBTC...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const params = {
          amount: parseInt(amount),
          to_address: options.to,
          from_addresses: options.from !== '[]' ? JSON.parse(options.from) : null,
          change_address: options.change || null,
          fee_rate: options.feeRate ? parseFloat(options.feeRate) : null,
          raw_output: globalOpts.raw || false,
          trace_enabled: options.trace || false,
          mine_enabled: options.mine || false,
          auto_confirm: options.autoConfirm || false,
        };

        const result = await provider.alkanesWrapBtc(JSON.stringify(params));

        spinner.succeed();
        console.log(formatOutput(JSON.parse(result), globalOpts));
      } catch (err: any) {
        error(`Failed to wrap BTC: ${err.message}`);
        process.exit(1);
      }
    });

  // init-pool
  alkanes
    .command('init-pool')
    .description('Initialize a new AMM liquidity pool')
    .option('--pair <tokens>', 'Token pair (format: BLOCK:TX,BLOCK:TX)', '2:0,32:0')
    .option('--liquidity <amounts>', 'Initial liquidity (format: AMOUNT0:AMOUNT1)', '300000000:50000')
    .option('--to <address>', 'Recipient address', 'p2tr:0')
    .option('--from <address>', 'Source address', 'p2tr:0')
    .option('--change <address>', 'Change address')
    .option('--minimum <lp>', 'Minimum LP tokens to receive')
    .option('--fee-rate <rate>', 'Fee rate in sat/vB')
    .option('--trace', 'Show trace after transaction confirms')
    .option('--factory <id>', 'Factory ID (format: BLOCK:TX)', '4:1')
    .option('--auto-confirm', 'Auto-confirm transaction')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Initializing pool...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        // Parse pair
        const [token0Str, token1Str] = options.pair.split(',');
        const [token0Block, token0Tx] = token0Str.split(':').map((n: string) => parseInt(n));
        const [token1Block, token1Tx] = token1Str.split(':').map((n: string) => parseInt(n));

        // Parse liquidity
        const [amount0, amount1] = options.liquidity.split(':').map((n: string) => parseInt(n));

        // Parse factory
        const [factoryBlock, factoryTx] = options.factory.split(':').map((n: string) => parseInt(n));

        const params = {
          factory_id: { block: factoryBlock, tx: factoryTx },
          token0: { block: token0Block, tx: token0Tx },
          token1: { block: token1Block, tx: token1Tx },
          amount0,
          amount1,
          minimum_lp: options.minimum ? parseInt(options.minimum) : null,
          to_address: options.to,
          from_address: options.from,
          change_address: options.change || null,
          fee_rate: options.feeRate ? parseFloat(options.feeRate) : null,
          trace: options.trace || false,
          auto_confirm: options.autoConfirm || false,
        };

        const txid = await provider.alkanesInitPool(JSON.stringify(params));

        spinner.succeed(`Pool initialized! Transaction: ${txid}`);
      } catch (err: any) {
        error(`Failed to initialize pool: ${err.message}`);
        process.exit(1);
      }
    });

  // swap
  alkanes
    .command('swap')
    .description('Execute an AMM token swap')
    .option('--path <tokens>', 'Swap path (comma-separated alkane IDs)', '2:0,32:0')
    .option('--input <amount>', 'Input token amount (required)', '1000000')
    .option('--minimum-output <amount>', 'Minimum output amount')
    .option('--slippage <percent>', 'Slippage percentage', '5.0')
    .option('--expires <height>', 'Expiry block height')
    .option('--to <address>', 'Recipient address', 'p2tr:0')
    .option('--from <address>', 'Source address', 'p2tr:0')
    .option('--change <address>', 'Change address')
    .option('--fee-rate <rate>', 'Fee rate in sat/vB')
    .option('--trace', 'Show trace after transaction confirms')
    .option('--mine', 'Mine a block after broadcasting (regtest only)')
    .option('--factory <id>', 'Factory ID', '4:65522')
    .option('--no-optimize', 'Skip path optimization')
    .option('--auto-confirm', 'Auto-confirm transaction')
    .action(async (options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Executing swap...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        // Parse path
        const pathTokens = options.path.split(',').map((token: string) => {
          const [block, tx] = token.split(':').map((n: string) => parseInt(n));
          return { block, tx };
        });

        // Parse factory
        const [factoryBlock, factoryTx] = options.factory.split(':').map((n: string) => parseInt(n));

        // Calculate minimum output if not provided
        const inputAmount = parseInt(options.input);
        const minimumOutput = options.minimumOutput
          ? parseInt(options.minimumOutput)
          : Math.floor(inputAmount * (1 - parseFloat(options.slippage) / 100));

        // Get current height for expiry if not provided
        let expires = options.expires ? parseInt(options.expires) : 0;
        if (!expires) {
          // Default to current height + 100
          const heightResult = await provider.get_metashrew_height_js();
          expires = parseInt(heightResult) + 100;
        }

        const params = {
          factory_id: { block: factoryBlock, tx: factoryTx },
          path: pathTokens,
          input_amount: inputAmount,
          minimum_output: minimumOutput,
          expires,
          to_address: options.to,
          from_address: options.from,
          change_address: options.change || null,
          fee_rate: options.feeRate ? parseFloat(options.feeRate) : null,
          trace: options.trace || false,
          auto_confirm: options.autoConfirm || false,
        };

        const txid = await provider.alkanesSwap(JSON.stringify(params));

        spinner.succeed(`Swap executed! Transaction: ${txid}`);
      } catch (err: any) {
        error(`Failed to execute swap: ${err.message}`);
        process.exit(1);
      }
    });

  // pool-details
  alkanes
    .command('pool-details <pool-id>')
    .description('Get detailed information about a specific pool')
    .action(async (poolId, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting pool details...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanesPoolDetails(poolId);
        const poolDetails = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(poolDetails, globalOpts));
      } catch (err: any) {
        error(`Failed to get pool details: ${err.message}`);
        process.exit(1);
      }
    });

  // reflect-alkane-range
  alkanes
    .command('reflect-alkane-range <block> <start-tx> <end-tx>')
    .description('Reflect metadata for a range of alkanes in a block')
    .option('--concurrency <n>', 'Number of concurrent requests', '30')
    .action(async (block, startTx, endTx, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Reflecting alkane range...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.alkanesReflectAlkaneRange(
          parseFloat(block),
          parseFloat(startTx),
          parseFloat(endTx),
          options.concurrency ? parseFloat(options.concurrency) : null
        );
        const reflections = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(reflections, globalOpts));
      } catch (err: any) {
        error(`Failed to reflect alkane range: ${err.message}`);
        process.exit(1);
      }
    });
}
