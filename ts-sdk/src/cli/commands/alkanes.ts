/**
 * Alkanes command group
 * Smart contract operations for Alkanes protocol
 *
 * The CLI uses the SDK's AlkanesRpcClient via provider.alkanes for all operations.
 */

import { Command } from 'commander';
import chalk from 'chalk';
import { createProvider } from '../utils/provider.js';
import {
  formatOutput,
  formatAlkaneBalances,
  formatReflectMetadata,
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
    .description('Get alkanes balance for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
    .option('--address <address>', 'Address to check (e.g., p2tr:0 or bc1q...)')
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

        // Resolve wallet address identifiers if provided
        let resolvedAddress = options.address;
        if (options.address) {
          resolvedAddress = await resolveAddressWithProvider(options.address, provider, {
            walletFile: globalOpts.walletFile,
            passphrase: globalOpts.passphrase,
            network: globalOpts.provider,
            jsonrpcUrl: globalOpts.jsonrpcUrl,
          });
        }

        const balance = await provider.alkanes.getBalance(resolvedAddress);

        spinner.succeed();
        if (options.address && options.address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${options.address})`);
        }
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

  // inspect-bytecode - Inspect bytecode directly from file or hex string
  alkanes
    .command('inspect-bytecode <bytecode>')
    .description('Inspect alkanes bytecode directly from file or hex string (no RPC fetch)')
    .option('--alkane-id <id>', 'Alkane ID for context (format: block:tx)', '0:0')
    .option('--disasm', 'Enable disassembly to WAT format', false)
    .option('--fuzz', 'Enable fuzzing analysis', false)
    .option('--fuzz-ranges <ranges>', 'Opcode ranges for fuzzing (e.g., "0-100,200-300")')
    .option('--meta', 'Extract and display metadata', false)
    .option('--codehash', 'Compute and display codehash', false)
    .option('--raw', 'Output raw JSON')
    .action(async (bytecode, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Inspecting bytecode...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        // Determine if bytecode is a file path or hex string
        let bytecodeHex: string;
        const fs = await import('fs');
        if (fs.existsSync(bytecode)) {
          // Read from file
          const fileContent = fs.readFileSync(bytecode);
          bytecodeHex = fileContent.toString('hex');
        } else {
          // Assume it's a hex string
          bytecodeHex = bytecode;
        }

        const config = {
          disasm: options.disasm,
          fuzz: options.fuzz,
          fuzz_ranges: options.fuzzRanges,
          meta: options.meta,
          codehash: options.codehash,
          raw: options.raw,
        };

        const result = await provider.alkanes.inspectBytecode(bytecodeHex, options.alkaneId, config);

        spinner.succeed('Inspection complete');
        console.log(formatOutput(result, { raw: options.raw }));
      } catch (err: any) {
        error(`Failed to inspect bytecode: ${err.message}`);
        process.exit(1);
      }
    });

  // simulate
  alkanes
    .command('simulate <contract-id>')
    .description('Simulate alkanes execution (format: block:tx or block:tx:opcode)')
    .option('--inputs <alkanes>', 'Input alkanes as comma-separated triplets (e.g., 2:1:1000,2:2:500)')
    .option('--height <height>', 'Block height for simulation')
    .option('--txindex <index>', 'Transaction index (default: 1)', '1')
    .option('--pointer <ptr>', 'Pointer value (default: 0)', '0')
    .option('--refund <ptr>', 'Refund pointer (default: 0)', '0')
    .option('--block-tag <tag>', 'Block tag to query')
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

        // Parse contract-id: block:tx or block:tx:opcode
        const parts = contractId.split(':');
        if (parts.length < 2 || parts.length > 3) {
          throw new Error('Invalid contract-id format. Use block:tx or block:tx:opcode (e.g., 2:112 or 2:112:10)');
        }
        const targetBlock = parseInt(parts[0], 10);
        const targetTx = parseInt(parts[1], 10);
        const calldataOpcode = parts.length === 3 ? parseInt(parts[2], 10) : 0;

        // Parse input alkanes if provided (format: block:tx:amount,block:tx:amount)
        const alkanes: Array<{id: {block: {lo: number, hi: number}, tx: {lo: number, hi: number}}, value: {lo: number, hi: number}}> = [];
        if (options.inputs) {
          const inputParts = options.inputs.split(',');
          for (const input of inputParts) {
            const [block, tx, amount] = input.split(':').map((s: string) => parseInt(s, 10));
            if (isNaN(block) || isNaN(tx) || isNaN(amount)) {
              throw new Error(`Invalid input format: ${input}. Use block:tx:amount`);
            }
            alkanes.push({
              id: { block: { lo: block, hi: 0 }, tx: { lo: tx, hi: 0 } },
              value: { lo: amount, hi: 0 }
            });
          }
        }

        // Get simulation height
        let height = options.height ? parseInt(options.height, 10) : 0;
        if (!height) {
          // Get current metashrew height
          try {
            height = await provider.metashrew.height();
          } catch {
            height = 0;
          }
        }

        // Build calldata with LEB128 encoding
        const calldata: number[] = [];
        // LEB128 encode targetBlock
        let value = targetBlock;
        do {
          let byte = value & 0x7f;
          value >>>= 7;
          if (value !== 0) byte |= 0x80;
          calldata.push(byte);
        } while (value !== 0);
        // LEB128 encode targetTx
        value = targetTx;
        do {
          let byte = value & 0x7f;
          value >>>= 7;
          if (value !== 0) byte |= 0x80;
          calldata.push(byte);
        } while (value !== 0);
        // LEB128 encode calldataOpcode
        value = calldataOpcode;
        do {
          let byte = value & 0x7f;
          value >>>= 7;
          if (value !== 0) byte |= 0x80;
          calldata.push(byte);
        } while (value !== 0);

        // Build MessageContextParcel
        // Note: protobuf bytes fields expect base64 strings or arrays, not empty strings
        const context = {
          alkanes,
          transaction: [],  // Empty byte array
          block: [],        // Empty byte array
          height,
          txindex: parseInt(options.txindex, 10),
          calldata: Array.from(calldata),  // Pass as array of numbers
          vout: 0,
          pointer: parseInt(options.pointer, 10),
          refund_pointer: parseInt(options.refund, 10),
        };

        const contractIdStr = `${targetBlock}:${targetTx}`;
        const result = await provider.alkanes.simulate(contractIdStr, context, options.blockTag);

        spinner.succeed();

        // Try to decode the hex result as SimulateResponse protobuf
        // The result may be a string (hex) or an object with data field
        if (typeof result === 'string' && result.startsWith('0x') && !options.raw) {
          try {
            const hexData = result.slice(2);
            const bytes = Buffer.from(hexData, 'hex');

            // Simple protobuf decoding for SimulateResponse:
            // field 1 (execution): ExtendedCallResponse
            // field 2 (gas_used): uint64
            // field 3 (error): string
            let pos = 0;
            let gasUsed = 0;
            let errorMsg = '';
            let executionData = '';

            while (pos < bytes.length) {
              const tag = bytes[pos++];
              const fieldNum = tag >> 3;
              const wireType = tag & 0x7;

              if (wireType === 0) { // varint
                let value = 0;
                let shift = 0;
                while (pos < bytes.length) {
                  const b = bytes[pos++];
                  value |= (b & 0x7f) << shift;
                  if ((b & 0x80) === 0) break;
                  shift += 7;
                }
                if (fieldNum === 2) gasUsed = value;
              } else if (wireType === 2) { // length-delimited
                let len = 0;
                let shift = 0;
                while (pos < bytes.length) {
                  const b = bytes[pos++];
                  len |= (b & 0x7f) << shift;
                  if ((b & 0x80) === 0) break;
                  shift += 7;
                }
                const data = bytes.slice(pos, pos + len);
                pos += len;
                if (fieldNum === 1) executionData = '0x' + data.toString('hex');
                if (fieldNum === 3) errorMsg = data.toString('utf8');
              }
            }

            console.log();
            if (errorMsg) {
              console.log(chalk.red(`Error: ${errorMsg}`));
            } else {
              console.log(chalk.green('✓ Simulation successful'));
            }
            if (gasUsed) console.log(`Gas used: ${gasUsed}`);
            if (executionData && executionData !== '0x') console.log(`Execution: ${executionData}`);
            console.log();
            console.log(chalk.gray(`Raw: ${result}`));
          } catch {
            // Fall back to raw output if decoding fails
            console.log(formatOutput(result, { raw: true }));
          }
        } else {
          console.log(formatOutput(result, { raw: options.raw }));
        }
      } catch (err: any) {
        error(`Failed to simulate: ${err.message || err}`);
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
    .description('Get alkanes by address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
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

        // Resolve wallet address identifiers
        const resolvedAddress = await resolveAddressWithProvider(address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.alkanes.getByAddress(
          resolvedAddress,
          options.blockTag,
          options.protocolTag ? parseInt(options.protocolTag) : undefined
        );

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
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
    .description('Get spendable outpoints for an address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
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

        // Resolve wallet address identifiers
        const resolvedAddress = await resolveAddressWithProvider(address, provider, {
          walletFile: globalOpts.walletFile,
          passphrase: globalOpts.passphrase,
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.alkanes.getSpendables(resolvedAddress);

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
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
    .option('--ordinals-strategy <strategy>', 'Strategy for inscribed UTXOs: exclude (default), preserve, burn')
    .option('--mempool-indexer', 'Enable mempool tracing for pending UTXO inscriptions')
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
          ordinalsStrategy: options.ordinalsStrategy,
          mempoolIndexer: options.mempoolIndexer,
        };

        const result = await (provider as any)._provider.alkanesExecuteWithStrings(
          JSON.stringify(params.inputs),
          params.contractId,
          params.pointer || 0,
          params.refundPointer || 0,
          params.target || '',
          params.feeRate || 1,
          params.ordinalsStrategy,
          params.mempoolIndexer
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
    .option('--ordinals-strategy <strategy>', 'Strategy for inscribed UTXOs: exclude (default), preserve, burn')
    .option('--mempool-indexer', 'Enable mempool tracing for pending UTXO inscriptions')
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
          ordinals_strategy: options.ordinalsStrategy,
          mempool_indexer: options.mempoolIndexer,
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
    .option('--ordinals-strategy <strategy>', 'Strategy for inscribed UTXOs: exclude (default), preserve, burn')
    .option('--mempool-indexer', 'Enable mempool tracing for pending UTXO inscriptions')
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
          ordinals_strategy: options.ordinalsStrategy,
          mempool_indexer: options.mempoolIndexer,
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
    .option('--ordinals-strategy <strategy>', 'Strategy for inscribed UTXOs: exclude (default), preserve, burn')
    .option('--mempool-indexer', 'Enable mempool tracing for pending UTXO inscriptions')
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
          ordinals_strategy: options.ordinalsStrategy,
          mempool_indexer: options.mempoolIndexer,
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
    .option('--ordinals-strategy <strategy>', 'Strategy for inscribed UTXOs: exclude (default), preserve, burn')
    .option('--mempool-indexer', 'Enable mempool tracing for pending UTXO inscriptions')
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
          ordinals_strategy: options.ordinalsStrategy,
          mempool_indexer: options.mempoolIndexer,
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
