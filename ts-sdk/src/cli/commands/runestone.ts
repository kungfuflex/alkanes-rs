/**
 * Runestone command group
 * Runestone protocol operations
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error } from '../utils/formatting.js';
import ora from 'ora';

export function registerRunestoneCommands(program: Command): void {
  const runestone = program.command('runestone').description('Runestone protocol operations');

  // decode
  runestone
    .command('decode <txid>')
    .description('Decode runestone from transaction')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Decoding runestone...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.runestone_decode_tx_js(txid);
        const decoded = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(decoded, globalOpts));
      } catch (err: any) {
        error(`Failed to decode runestone: ${err.message}`);
        process.exit(1);
      }
    });

  // analyze
  runestone
    .command('analyze <txid>')
    .description('Analyze runestone transaction')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Analyzing runestone...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.runestone_analyze_tx_js(txid);
        const analysis = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(analysis, globalOpts));
      } catch (err: any) {
        error(`Failed to analyze runestone: ${err.message}`);
        process.exit(1);
      }
    });

  // trace
  runestone
    .command('trace <txid>')
    .description('Trace all protostones in a runestone transaction')
    .action(async (txid, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Tracing runestone...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        const result = await provider.traceProtostones(txid);
        const trace = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(trace, globalOpts));
      } catch (err: any) {
        error(`Failed to trace runestone: ${err.message}`);
        process.exit(1);
      }
    });
}
