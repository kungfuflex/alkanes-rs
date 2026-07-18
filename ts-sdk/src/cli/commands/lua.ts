/**
 * Lua command group
 * Lua script execution
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error } from '../utils/formatting.js';
import ora from 'ora';

export function registerLuaCommands(program: Command): void {
  const lua = program.command('lua').description('Lua script execution');

  // evalscript
  lua
    .command('evalscript <script>')
    .description('Evaluate a Lua script')
    .action(async (script, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Evaluating script...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const result = await provider.lua_eval_script_js(script);
        const output = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(output, globalOpts));
      } catch (err: any) {
        error(`Failed to evaluate script: ${err.message}`);
        process.exit(1);
      }
    });

  // eval
  lua
    .command('eval <script>')
    .description('Evaluate Lua with arguments')
    .option('--args <json>', 'Arguments as JSON', '{}')
    .action(async (script, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Evaluating script...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
        });

        const args = JSON.parse(options.args);
        const result = await provider.lua_eval_js(script, args);
        const output = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(output, globalOpts));
      } catch (err: any) {
        error(`Failed to evaluate script: ${err.message}`);
        process.exit(1);
      }
    });
}
