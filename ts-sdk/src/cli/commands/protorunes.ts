/**
 * Protorunes command group
 * Protorunes protocol operations
 */

import { Command } from 'commander';
import { createProvider } from '../utils/provider.js';
import { formatOutput, success, error, info } from '../utils/formatting.js';
import ora from 'ora';
import { resolveAddressWithProvider } from '../utils/address-resolver.js';

export function registerProtorunesCommands(program: Command): void {
  const protorunes = program.command('protorunes').description('Protorunes protocol operations');

  // by-address
  protorunes
    .command('by-address <address>')
    .description('Get protorunes by address. Address can be p2tr:0, p2wpkh:0, or a raw Bitcoin address.')
    .option('--block-tag <tag>', 'Block tag (e.g., "latest" or height)')
    .action(async (address, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting protorunes...').start();

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

        // Protocol tag 1 = Protorunes
        const result = await provider.alkanesByAddress(
          resolvedAddress,
          options.blockTag || null,
          1
        );
        const protorunes = JSON.parse(result);

        spinner.succeed();
        if (address !== resolvedAddress) {
          info(`Address: ${resolvedAddress} (resolved from ${address})`);
        }
        console.log(formatOutput(protorunes, globalOpts));
      } catch (err: any) {
        error(`Failed to get protorunes: ${err.message}`);
        process.exit(1);
      }
    });

  // by-outpoint
  protorunes
    .command('by-outpoint <outpoint>')
    .description('Get protorunes by outpoint')
    .option('--block-tag <tag>', 'Block tag (e.g., "latest" or height)')
    .action(async (outpoint, options, command) => {
      try {
        const globalOpts = command.parent?.parent?.opts() || {};
        const spinner = ora('Getting protorunes...').start();

        const provider = await createProvider({
          network: globalOpts.provider,
          jsonrpcUrl: globalOpts.jsonrpcUrl,
          metashrewUrl: globalOpts.metashrewUrl,
        });

        // Protocol tag 1 = Protorunes
        const result = await provider.alkanesByOutpoint(
          outpoint,
          options.blockTag || null,
          1
        );
        const protorunes = JSON.parse(result);

        spinner.succeed();
        console.log(formatOutput(protorunes, globalOpts));
      } catch (err: any) {
        error(`Failed to get protorunes: ${err.message}`);
        process.exit(1);
      }
    });
}
